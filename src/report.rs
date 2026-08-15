//! Reporting build results.
//!
//! Nothing here writes to the source. ncpages treats the watched folder as
//! read-only: a status file there would sync straight back into the author's
//! vault, and writing anything below the watched root changes its ETag, which
//! triggers the next build.
//!
//! Two channels instead, both optional:
//!
//! * the **Nextcloud companion app**, probed once with `OPTIONS`. If it is not
//!   installed, that probe is the entire cost and nothing else happens. If it
//!   is, the report is posted as JSON and the app owns the presentation.
//! * **ntfy**, for failures. Deliberately independent of Nextcloud, because it
//!   has to work when Nextcloud is the thing that broke.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::Serialize;
use tracing::{debug, warn};

use crate::config::Config;
use crate::pipeline::Outcome;

/// How long a negative probe is trusted. Long enough to cost nothing, short
/// enough that installing the app takes effect without a restart.
const PROBE_TTL: Duration = Duration::from_secs(900);

static PROBE: OnceLock<Mutex<Option<(bool, Instant)>>> = OnceLock::new();

/// What the companion app receives. This is the wire contract: the app is a
/// separate project, so adding fields is fine and renaming them is not.
#[derive(Debug, Serialize)]
pub struct BuildReport<'a> {
    /// Schema version of this payload, not of ncpages.
    pub version: u32,
    /// `published`, `refused`, `skipped` or `failed`.
    pub result: &'a str,
    /// `push`, `poll`, `timer` or `manual`.
    pub trigger: &'a str,
    pub release: Option<&'a str>,
    pub pages: usize,
    pub warnings: &'a [String],
    /// Gate violations, when the result is `refused`.
    pub violations: &'a [String],
    /// Conflict copies excluded from the build. Their presence means someone's
    /// work is at risk, which is worth surfacing in a UI.
    pub conflict_copies: &'a [String],
    pub error: Option<String>,
}

pub async fn deliver(config: &Config, result: &Result<Outcome>) {
    let report = match result {
        Ok(outcome) => BuildReport {
            version: 1,
            result: if outcome.skipped {
                "skipped"
            } else if outcome.published {
                "published"
            } else {
                "refused"
            },
            trigger: &outcome.trigger,
            release: outcome.release.as_deref(),
            pages: outcome.pages,
            warnings: &outcome.warnings,
            violations: &outcome.violations,
            conflict_copies: &outcome.conflict_copies,
            error: None,
        },
        Err(e) => BuildReport {
            version: 1,
            result: "failed",
            trigger: "unknown",
            release: None,
            pages: 0,
            warnings: &[],
            violations: &[],
            conflict_copies: &[],
            error: Some(format!("{e:#}")),
        },
    };

    post_to_app(config, &report).await;
    notify(config, result).await;
}

async fn post_to_app(config: &Config, report: &BuildReport<'_>) {
    let Some(endpoint) = config.report.endpoint(config.source.url.as_deref()) else {
        return;
    };
    let Ok(client) = http_client() else { return };

    let credentials = match (config.source.user.clone(), config.source_password()) {
        (Some(user), Ok(Some(password))) => Some((user, password)),
        _ => None,
    };

    if !app_available(&client, &endpoint, credentials.as_ref()).await {
        return;
    }

    // Serialised here rather than through reqwest's `json` feature: serde_json is
    // already a dependency, and the feature is not.
    let body = match serde_json::to_vec(report) {
        Ok(body) => body,
        Err(e) => {
            warn!(error = %e, "could not serialise the report");
            return;
        }
    };

    let mut request = client
        .post(&endpoint)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body);
    if let Some((user, password)) = &credentials {
        request = request.basic_auth(user, Some(password));
    }
    // Nextcloud rejects API requests without this header.
    request = request.header("OCS-APIRequest", "true");

    match request.send().await {
        Ok(response) if response.status().is_success() => {
            debug!(endpoint, "report delivered to the companion app")
        }
        Ok(response) => {
            warn!(endpoint, status = %response.status(), "companion app rejected the report")
        }
        Err(e) => warn!(endpoint, error = %e, "could not reach the companion app"),
    }
}

/// One `OPTIONS` call, cached. An operator who never installs the app pays a
/// request every fifteen minutes and nothing else; one who installs it later
/// does not have to restart anything.
async fn app_available(
    client: &reqwest::Client,
    endpoint: &str,
    credentials: Option<&(String, String)>,
) -> bool {
    let cache = PROBE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some((available, checked)) = *guard {
            if checked.elapsed() < PROBE_TTL {
                return available;
            }
        }
    }

    let mut request = client.request(reqwest::Method::OPTIONS, endpoint);
    if let Some((user, password)) = credentials {
        request = request.basic_auth(user, Some(password));
    }

    let available = match request.send().await {
        Ok(response) => response.status().is_success(),
        Err(e) => {
            debug!(endpoint, error = %e, "companion app not reachable");
            false
        }
    };

    if !available {
        debug!(
            endpoint,
            "companion app not installed; reports stay in the log"
        );
    }
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((available, Instant::now()));
    }
    available
}

/// Notify on anything that needs a human: a failed build, or a build the gate
/// refused. Successful publishes stay quiet on purpose — a notification per blog
/// post trains you to ignore the channel.
pub async fn notify(config: &Config, result: &Result<Outcome>) {
    let Some(topic) = config.report.ntfy_topic.as_deref() else {
        return;
    };

    let (title, priority, body) = match result {
        Err(e) => (
            "ncpages build failed",
            "high",
            format!("{e:#}\n\nThe live site is unchanged."),
        ),
        Ok(outcome) if !outcome.violations.is_empty() => (
            "ncpages gate refused a build",
            "high",
            format!(
                "trigger: {}\n{}\n\nThe live site is unchanged.",
                outcome.trigger,
                outcome.violations.join("\n")
            ),
        ),
        Ok(outcome) if !outcome.conflict_copies.is_empty() => (
            "ncpages found conflict copies",
            "default",
            format!(
                "trigger: {}\nExcluded from the build, and a sign that work is about to be lost:\n{}",
                outcome.trigger,
                outcome.conflict_copies.join("\n")
            ),
        ),
        Ok(_) => return,
    };

    if let Err(e) = post_ntfy(topic, title, priority, &body).await {
        // A failed notification must never fail a build that already succeeded.
        warn!(error = %e, "could not send notification");
    }
}

async fn post_ntfy(topic: &str, title: &str, priority: &str, body: &str) -> Result<()> {
    http_client()?
        .post(topic)
        .header("Title", title)
        .header("Priority", priority)
        .body(body.to_string())
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

fn http_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(topic: Option<&str>) -> Config {
        let mut config: Config = toml::from_str(
            r#"
            schema_version = 1
            [source]
            kind = "fs"
            path = "/tmp/vault"
            [build]
            kind = "local"
            command = ["true"]
            "#,
        )
        .unwrap();
        config.report.ntfy_topic = topic.map(str::to_string);
        config.report.app = false;
        config
    }

    fn outcome() -> Outcome {
        Outcome {
            trigger: "poll".into(),
            release: Some("20260815T101500Z".into()),
            pages: 46,
            published: true,
            skipped: false,
            warnings: vec![],
            violations: vec![],
            conflict_copies: vec![],
        }
    }

    #[test]
    fn the_payload_names_the_outcome_a_ui_would_show() {
        let outcome = outcome();
        let report = BuildReport {
            version: 1,
            result: "published",
            trigger: &outcome.trigger,
            release: outcome.release.as_deref(),
            pages: outcome.pages,
            warnings: &outcome.warnings,
            violations: &outcome.violations,
            conflict_copies: &outcome.conflict_copies,
            error: None,
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains(r#""result":"published""#), "{json}");
        assert!(json.contains(r#""pages":46"#), "{json}");
        assert!(json.contains(r#""release":"20260815T101500Z""#), "{json}");
    }

    #[tokio::test]
    async fn without_a_topic_nothing_is_attempted() {
        notify(&config(None), &Ok(outcome())).await;
    }

    #[tokio::test]
    async fn a_successful_publish_is_not_announced() {
        // An unreachable topic would log a warning if a request were made; a
        // successful publish must return before that.
        notify(&config(Some("http://127.0.0.1:1/never")), &Ok(outcome())).await;
    }

    #[tokio::test]
    async fn reporting_is_skipped_entirely_when_the_app_is_disabled() {
        // No endpoint means no probe, no request, and no error.
        deliver(&config(None), &Ok(outcome())).await;
    }
}
