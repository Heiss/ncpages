//! The executor.
//!
//! One way to run a program, used by all four phases: `pre_build`, `build`,
//! `post_build`, `post_publish`. Programs plus environment variables — no plugin
//! API, no dynamically loaded modules.
//!
//! Two things are deliberately *not* shared, because the phases differ in kind:
//!
//! * **Exit codes.** For hooks, `1` is a warning the build survives and `2` is an
//!   abort. For a build, any non-zero is a failure — generators follow the usual
//!   Unix convention and know nothing about ours.
//! * **The environment.** Hooks get a cleared environment plus the documented
//!   contract, because they sit closest to the secrets. A build inherits the
//!   container's environment, because the image *is* the generator's
//!   configuration: `PATH` into a virtualenv, `PYTHONPATH` into the assembled
//!   tree, and whatever else the recipe baked in.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tracing::{info, warn};

use crate::config::Hook;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    PreBuild,
    Build,
    PostBuild,
    PostPublish,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::PreBuild => "pre_build",
            Phase::Build => "build",
            Phase::PostBuild => "post_build",
            Phase::PostPublish => "post_publish",
        }
    }
}

#[derive(Debug, Default)]
pub struct HookOutcome {
    pub warnings: Vec<String>,
}

/// Whether a program starts from a cleared environment or the container's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Env {
    /// Only `PATH`, the `NCPAGES_*` contract and explicit passthrough. Secrets
    /// are opt-in, never ambient.
    Cleared,
    /// Everything this process has. For the build, whose image was built to
    /// carry exactly what the generator needs.
    Inherited,
}

/// What a program did. Interpreting the code is the caller's job.
#[derive(Debug)]
pub struct Execution {
    pub code: i32,
    pub stderr: String,
    pub elapsed: Duration,
}

/// One program to run, and the policy it runs under.
pub struct Run<'a> {
    pub phase: Phase,
    pub program: &'a Path,
    pub args: &'a [String],
    pub workdir: &'a Path,
    /// The `NCPAGES_*` contract.
    pub env: &'a BTreeMap<String, String>,
    /// Variables copied from this process, named explicitly. Secrets are opt-in.
    pub passthrough: &'a [String],
    /// Variables removed after the policy is applied. Used to keep secrets out
    /// of an inherited environment: a generator has no business seeing a
    /// Nextcloud password.
    pub deny: &'a [String],
    pub env_policy: Env,
    pub timeout: Duration,
}

/// Run one program under the pipeline's contract.
pub async fn execute(run: Run<'_>) -> Result<Execution> {
    let Run {
        phase,
        program,
        args,
        workdir,
        env,
        passthrough,
        deny,
        env_policy,
        timeout,
    } = run;

    let mut command = tokio::process::Command::new(program);
    command.args(args).current_dir(workdir);

    if env_policy == Env::Cleared {
        command.env_clear().env(
            "PATH",
            std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()),
        );
    }
    command.envs(env);

    for key in passthrough {
        if let Ok(value) = std::env::var(key) {
            command.env(key, value);
        }
    }

    for key in deny {
        command.env_remove(key);
    }

    let started = std::time::Instant::now();
    let output = tokio::time::timeout(timeout, command.output())
        .await
        .with_context(|| {
            format!(
                "{}: {} timed out after {timeout:?}",
                phase.as_str(),
                program.display()
            )
        })?
        .with_context(|| format!("{}: running {}", phase.as_str(), program.display()))?;

    Ok(Execution {
        code: output.status.code().unwrap_or(-1),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        elapsed: started.elapsed(),
    })
}

/// The build phase: one program, and any non-zero exit is a failure.
pub async fn run_build(
    command: &[String],
    workdir: &Path,
    env: &BTreeMap<String, String>,
    deny_env: &[String],
    timeout: Duration,
) -> Result<()> {
    let (program, args) = command
        .split_first()
        .ok_or_else(|| anyhow::anyhow!("build.command is empty"))?;

    info!(program, "running the build");
    let execution = execute(Run {
        phase: Phase::Build,
        program: Path::new(program),
        args,
        workdir,
        env,
        passthrough: &[],
        deny: deny_env,
        env_policy: Env::Inherited,
        timeout,
    })
    .await?;

    if execution.code != 0 {
        bail!(
            "build failed with exit code {}: {}",
            execution.code,
            first_line(&execution.stderr)
        );
    }
    info!(
        elapsed_ms = execution.elapsed.as_millis() as u64,
        "build finished"
    );
    Ok(())
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

        info!(phase = phase.as_str(), hook = %hook.run, "running hook");
        let execution = execute(Run {
            phase,
            program: &program,
            args: &hook.args,
            workdir,
            env,
            passthrough: &hook.env_passthrough,
            deny: &[],
            env_policy: Env::Cleared,
            timeout,
        })
        .await?;

        let code = execution.code;
        let stderr = execution.stderr.clone();

        match code {
            0 => info!(
                phase = phase.as_str(),
                hook = %hook.run,
                elapsed_ms = execution.elapsed.as_millis() as u64,
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
    async fn the_build_inherits_the_environment_but_never_the_secrets() {
        // The build needs the container's environment — PATH into a virtualenv,
        // PYTHONPATH into the assembled tree — but a generator has no business
        // seeing a Nextcloud password.
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("NCPAGES_TEST_GENERATOR_VAR", "needed");
        std::env::set_var("NCPAGES_TEST_SECRET_VAR", "secret");

        let script = dir.path().join("build.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\n\
             test \"$NCPAGES_TEST_GENERATOR_VAR\" = needed || exit 3\n\
             test -z \"$NCPAGES_TEST_SECRET_VAR\" || exit 4\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let result = run_build(
            &[script.display().to_string()],
            dir.path(),
            &env(),
            &["NCPAGES_TEST_SECRET_VAR".to_string()],
            Duration::from_secs(10),
        )
        .await;

        std::env::remove_var("NCPAGES_TEST_GENERATOR_VAR");
        std::env::remove_var("NCPAGES_TEST_SECRET_VAR");
        assert!(result.is_ok(), "{result:?}");
    }

    #[tokio::test]
    async fn a_build_treats_exit_one_as_a_failure_not_a_warning() {
        // Generators follow the Unix convention and know nothing about ours.
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fail.sh");
        std::fs::write(&script, "#!/bin/sh\necho 'template error' >&2\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let error = run_build(
            &[script.display().to_string()],
            dir.path(),
            &env(),
            &[],
            Duration::from_secs(10),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(error.contains("template error"), "{error}");
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
