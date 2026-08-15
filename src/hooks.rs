//! Hook runner.
//!
//! Programs plus environment variables — no plugin API, no dynamically loaded
//! modules. Exit codes carry meaning: `0` success, `1` warning (the build
//! continues and the warning is reported), `2` abort.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tracing::{info, warn};

use crate::config::Hook;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    PreBuild,
    PostBuild,
    PostPublish,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::PreBuild => "pre_build",
            Phase::PostBuild => "post_build",
            Phase::PostPublish => "post_publish",
        }
    }
}

#[derive(Debug, Default)]
pub struct HookOutcome {
    pub warnings: Vec<String>,
}

/// Run every hook of one phase in order.
///
/// A hook that aborts stops the pipeline. In `post_publish` that is reported but
/// cannot undo anything — which is exactly why nothing irreversible is allowed
/// to run in an earlier phase.
pub async fn run_phase(
    phase: Phase,
    hooks: &[Hook],
    hooks_dir: &Path,
    workdir: &Path,
    env: &BTreeMap<String, String>,
    timeout: Duration,
) -> Result<HookOutcome> {
    let mut outcome = HookOutcome::default();

    for hook in hooks {
        let program = hooks_dir.join(&hook.run);
        if !program.exists() {
            bail!(
                "{}: hook {} not found (hooks live in {}, never in the vault)",
                phase.as_str(),
                hook.run,
                hooks_dir.display()
            );
        }

        let mut command = tokio::process::Command::new(&program);
        command
            .args(&hook.args)
            .current_dir(workdir)
            .env_clear()
            .env(
                "PATH",
                std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()),
            )
            .envs(env);

        // Secrets are opt-in per hook, never ambient.
        for key in &hook.env_passthrough {
            if let Ok(value) = std::env::var(key) {
                command.env(key, value);
            }
        }

        info!(phase = phase.as_str(), hook = %hook.run, "running hook");
        let started = std::time::Instant::now();
        let output = tokio::time::timeout(timeout, command.output())
            .await
            .with_context(|| format!("{}: hook {} timed out", phase.as_str(), hook.run))?
            .with_context(|| format!("{}: running hook {}", phase.as_str(), hook.run))?;

        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        match code {
            0 => info!(
                phase = phase.as_str(),
                hook = %hook.run,
                elapsed_ms = started.elapsed().as_millis() as u64,
                "hook finished"
            ),
            1 => {
                let message = format!(
                    "{}: {} warned: {}",
                    phase.as_str(),
                    hook.run,
                    first_line(&stderr)
                );
                warn!("{message}");
                outcome.warnings.push(message);
            }
            other => bail!(
                "{}: hook {} aborted with exit code {other}: {}",
                phase.as_str(),
                hook.run,
                first_line(&stderr)
            ),
        }
    }

    Ok(outcome)
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or("(no output)").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(dir: &Path, name: &str, body: &str) -> Hook {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        Hook {
            run: name.into(),
            args: vec![],
            env_passthrough: vec![],
        }
    }

    fn env() -> BTreeMap<String, String> {
        let mut env = BTreeMap::new();
        env.insert("NCPAGES_TRIGGER".to_string(), "manual".to_string());
        env
    }

    #[tokio::test]
    async fn exit_zero_succeeds_and_receives_the_environment() {
        let dir = tempfile::tempdir().unwrap();
        let hook = script(
            dir.path(),
            "ok.sh",
            "#!/bin/sh\ntest \"$NCPAGES_TRIGGER\" = manual || exit 2\n",
        );
        let outcome = run_phase(
            Phase::PreBuild,
            std::slice::from_ref(&hook),
            dir.path(),
            dir.path(),
            &env(),
            Duration::from_secs(10),
        )
        .await
        .unwrap();
        assert!(outcome.warnings.is_empty());
    }

    #[tokio::test]
    async fn exit_one_warns_without_stopping_the_build() {
        let dir = tempfile::tempdir().unwrap();
        let hook = script(
            dir.path(),
            "warn.sh",
            "#!/bin/sh\necho 'api unreachable' >&2\nexit 1\n",
        );
        let outcome = run_phase(
            Phase::PostBuild,
            std::slice::from_ref(&hook),
            dir.path(),
            dir.path(),
            &env(),
            Duration::from_secs(10),
        )
        .await
        .unwrap();
        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.warnings[0].contains("api unreachable"));
    }

    #[tokio::test]
    async fn exit_two_aborts_the_pipeline() {
        let dir = tempfile::tempdir().unwrap();
        let hook = script(dir.path(), "abort.sh", "#!/bin/sh\necho boom >&2\nexit 2\n");
        let result = run_phase(
            Phase::PreBuild,
            std::slice::from_ref(&hook),
            dir.path(),
            dir.path(),
            &env(),
            Duration::from_secs(10),
        )
        .await;
        assert!(result.unwrap_err().to_string().contains("aborted"));
    }

    #[tokio::test]
    async fn ambient_environment_does_not_leak_into_hooks() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("NCPAGES_TEST_SECRET", "leaked");
        let hook = script(
            dir.path(),
            "leak.sh",
            "#!/bin/sh\ntest -z \"$NCPAGES_TEST_SECRET\" || exit 2\n",
        );
        let result = run_phase(
            Phase::PreBuild,
            std::slice::from_ref(&hook),
            dir.path(),
            dir.path(),
            &env(),
            Duration::from_secs(10),
        )
        .await;
        std::env::remove_var("NCPAGES_TEST_SECRET");
        assert!(result.is_ok(), "{result:?}");
    }

    #[tokio::test]
    async fn a_missing_hook_is_an_error_not_a_skip() {
        let dir = tempfile::tempdir().unwrap();
        let hook = Hook {
            run: "absent.sh".into(),
            args: vec![],
            env_passthrough: vec![],
        };
        let result = run_phase(
            Phase::PreBuild,
            std::slice::from_ref(&hook),
            dir.path(),
            dir.path(),
            &env(),
            Duration::from_secs(10),
        )
        .await;
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
}
