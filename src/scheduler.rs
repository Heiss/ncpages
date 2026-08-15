//! Trigger sources, debouncing, and the busy policy.
//!
//! All sources feed one decision point. At most one build runs, plus one waiting
//! slot that newer events overwrite — never a cancellation, because an abort
//! between the swap and `post_publish` leaves a state with no clean way back.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::time::MissedTickBehavior;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::pipeline;
use crate::publish;
use crate::serve::SharedHealth;
use crate::source::Source;
use crate::state::State;

pub async fn run(
    config: Arc<Config>,
    health: SharedHealth,
    shutdown: ShutdownSignal,
) -> Result<()> {
    let source = Source::from_config(&config)?;
    let mut state = State::load(&config.paths.state)?;

    {
        let mut health = health.write().await;
        health.source = source.describe();
        health.source_status = "starting".into();
        health.last_release = state.last_release.clone();
    }

    // Without a current release the site answers 404 to everything during the
    // first sync, which reads as a broken deployment rather than a starting one.
    if publish::ensure_bootstrap(&config.publish.root)? {
        info!("published a holding page; no build has completed yet");
    }

    // Reconcile: persisted state against reality.
    let mut dirty = state.last_release.is_none();
    let mut trigger = "manual";
    match source.probe().await {
        Ok(token) => {
            if state.root_etag.as_deref() != Some(token.as_str()) {
                info!("source changed while the service was down");
                dirty = true;
                trigger = "poll";
            }
            set_source_status(&health, "ok").await;
        }
        Err(e) => {
            if config.source.required {
                return Err(e);
            }
            warn!(error = %e, "source unreachable at startup; continuing to serve the current release");
            set_source_status(&health, "unreachable").await;
        }
    }

    let mut poll = tokio::time::interval(config.triggers.poll);
    poll.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let timer_interval = config
        .triggers
        .timer
        .map(|base| with_jitter(base, config.triggers.jitter));
    let mut timer = timer_interval.map(|interval| {
        let mut t = tokio::time::interval(interval);
        t.set_missed_tick_behavior(MissedTickBehavior::Delay);
        t
    });

    let mut first_seen: Option<Instant> = dirty.then(Instant::now);
    let mut last_event: Option<Instant> = first_seen;
    let mut shutdown = shutdown;

    loop {
        let wake = next_wake(&config, first_seen, last_event);

        tokio::select! {
            biased;

            _ = shutdown.recv() => {
                info!("shutting down");
                state.save(&config.paths.state)?;
                return Ok(());
            }

            _ = poll.tick() => {
                match source.probe().await {
                    Ok(token) => {
                        set_source_status(&health, "ok").await;
                        touch_check(&health).await;
                        if state.root_etag.as_deref() != Some(token.as_str()) {
                            if first_seen.is_none() {
                                first_seen = Some(Instant::now());
                                trigger = "poll";
                            }
                            last_event = Some(Instant::now());
                        }
                    }
                    Err(e) => {
                        // 401 and 503 are distinguished in the source; here the
                        // response is the same: degrade, keep serving, retry.
                        warn!(error = %e, "source check failed");
                        set_source_status(&health, "degraded").await;
                    }
                }
            }

            _ = async { timer.as_mut().unwrap().tick().await }, if timer.is_some() => {
                info!("timer trigger");
                if first_seen.is_none() {
                    first_seen = Some(Instant::now());
                }
                trigger = "timer";
                last_event = Some(Instant::now());
            }

            _ = tokio::time::sleep_until(wake.into()), if first_seen.is_some() => {
                first_seen = None;
                last_event = None;
                let current_trigger = std::mem::replace(&mut trigger, "poll");

                {
                    let mut health = health.write().await;
                    health.building = true;
                }

                let result = pipeline::run_once(config.clone(), &source, &mut state, current_trigger).await;
                crate::report::notify(&config, &result).await;

                match &result {
                    Ok(outcome) => {
                        info!(trigger = current_trigger, summary = %outcome.summary(), "build finished");
                        state.last_result = Some(outcome.summary());
                    }
                    Err(e) => {
                        error!(trigger = current_trigger, error = %e, "build failed; the live site is unchanged");
                        state.last_result = Some(format!("failed: {e}"));
                    }
                }
                state.save(&config.paths.state)?;

                let mut health = health.write().await;
                health.building = false;
                health.last_release = state.last_release.clone();
                health.last_result = state.last_result.clone();
                health.last_build_finished = Some(publish::new_release_id(std::time::SystemTime::now()));
            }
        }
    }
}

/// Debounce, bounded by the hard deadline: a vault that is being edited
/// continuously still gets built.
fn next_wake(config: &Config, first_seen: Option<Instant>, last_event: Option<Instant>) -> Instant {
    let now = Instant::now();
    match (first_seen, last_event) {
        (Some(first), Some(last)) => {
            let debounced = last + config.schedule.debounce;
            let hard = first + config.schedule.max_delay;
            debounced.min(hard).max(now)
        }
        _ => now + Duration::from_secs(3600),
    }
}

/// Spread timer builds across installations so they do not all hit the same
/// external APIs in the same second.
fn with_jitter(base: Duration, jitter: f64) -> Duration {
    let jitter = jitter.clamp(0.0, 1.0);
    if jitter == 0.0 {
        return base;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let fraction = (nanos % 1_000_000) as f64 / 1_000_000.0;
    let factor = 1.0 + jitter * fraction;
    Duration::from_secs_f64(base.as_secs_f64() * factor)
}

async fn set_source_status(health: &SharedHealth, status: &str) {
    health.write().await.source_status = status.to_string();
}

async fn touch_check(health: &SharedHealth) {
    health.write().await.last_check_seconds_ago = Some(0);
}

/// Ctrl-C and SIGTERM, so a `docker stop` does not look like a crash.
pub struct ShutdownSignal {
    receiver: tokio::sync::mpsc::Receiver<()>,
}

impl ShutdownSignal {
    pub fn install() -> Self {
        let (tx, receiver) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            #[cfg(unix)]
            {
                let mut term =
                    match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    {
                        Ok(term) => term,
                        Err(_) => return,
                    };
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
            let _ = tx.send(()).await;
        });
        Self { receiver }
    }

    pub async fn recv(&mut self) {
        self.receiver.recv().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        toml::from_str(
            r#"
            schema_version = 1
            [source]
            kind = "fs"
            path = "/tmp/vault"
            [schedule]
            debounce = "10s"
            max_delay = "120s"
            [build]
            kind = "local"
            command = ["true"]
            "#,
        )
        .unwrap()
    }

    #[test]
    fn a_quiet_vault_waits_for_the_debounce() {
        let config = config();
        let now = Instant::now();
        let wake = next_wake(&config, Some(now), Some(now));
        assert!(wake.duration_since(now) >= Duration::from_secs(9));
        assert!(wake.duration_since(now) <= Duration::from_secs(11));
    }

    #[test]
    fn continuous_editing_still_builds_at_the_hard_deadline() {
        let config = config();
        let first = Instant::now() - Duration::from_secs(119);
        let last = Instant::now(); // still typing
        let wake = next_wake(&config, Some(first), Some(last));
        assert!(
            wake.duration_since(Instant::now()) <= Duration::from_secs(2),
            "the hard deadline did not cap the debounce"
        );
    }

    #[test]
    fn jitter_only_ever_extends_and_stays_within_bounds() {
        let base = Duration::from_secs(6 * 3600);
        for _ in 0..50 {
            let jittered = with_jitter(base, 0.1);
            assert!(jittered >= base);
            assert!(jittered <= base.mul_f64(1.1));
        }
        assert_eq!(with_jitter(base, 0.0), base);
    }
}
