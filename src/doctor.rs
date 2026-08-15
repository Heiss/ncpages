//! `ncpages doctor` — the failure catalogue as executable checks.
//!
//! Most issues in a published tool are other people's deployments: broken
//! notify_push, wrong `trusted_proxies`, S3 storage, encryption. A check that
//! only prints FAIL moves the work back to the maintainer, so every check says
//! what it verified, what it found, and what to do about it.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;

use crate::config::{BuildKind, Config, SourceKind};
use crate::publish;
use crate::source::Source;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Pass,
    Warn,
    Fail,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Pass => "ok  ",
            Level::Warn => "warn",
            Level::Fail => "FAIL",
        }
    }
}

#[derive(Debug)]
pub struct Check {
    pub name: &'static str,
    pub level: Level,
    pub detail: String,
}

impl Check {
    fn pass(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            level: Level::Pass,
            detail: detail.into(),
        }
    }
    fn warn(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            level: Level::Warn,
            detail: detail.into(),
        }
    }
    fn fail(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            level: Level::Fail,
            detail: detail.into(),
        }
    }
}

pub async fn run(config: Arc<Config>) -> Result<Vec<Check>> {
    let mut checks = Vec::new();

    // Config-level invariants already refused startup if violated; saying so
    // explicitly is worth a line, because their absence is invisible.
    checks.push(Check::pass(
        "config/overlap",
        format!(
            "hook directory {} is outside the working copy {}",
            config.paths.hooks_dir().display(),
            config.paths.src.display()
        ),
    ));

    checks.push(Check::pass(
        "source/read-only",
        "ncpages never writes to the source; the vault is yours alone",
    ));

    match config.report.endpoint(config.source.url.as_deref()) {
        Some(endpoint) => checks.push(Check::pass(
            "report/app",
            format!("reports go to {endpoint} when the companion app is installed"),
        )),
        None => checks.push(Check::warn(
            "report/app",
            "companion app reporting is off; results are visible in the log and /healthz",
        )),
    }

    if config.report.ntfy_topic.is_none() {
        checks.push(Check::warn(
            "report/ntfy",
            "no ntfy topic; a failed build will not reach you unless you look",
        ));
    }

    // Directories
    for (name, path) in [
        ("paths/src", &config.paths.src),
        ("paths/state", &config.paths.state),
        ("paths/config_dir", &config.paths.config_dir),
    ] {
        checks.push(if path.exists() {
            Check::pass(name, format!("{} exists", path.display()))
        } else {
            Check::warn(name, format!("{} does not exist yet", path.display()))
        });
    }

    // The move from build tree to release must be a rename, not a copy.
    checks.push(
        match same_filesystem(&config.paths.build, &config.publish.root) {
            Some(true) => Check::pass(
                "publish/filesystem",
                "build tree and publish root share a filesystem, so releases are moved atomically",
            ),
            Some(false) => Check::fail(
                "publish/filesystem",
                format!(
                    "{} and {} are on different filesystems; the release move degrades to a copy",
                    config.paths.build.display(),
                    config.publish.root.display()
                ),
            ),
            None => Check::warn(
                "publish/filesystem",
                "cannot compare filesystems yet; both paths need to exist",
            ),
        },
    );

    // Publish root writability decides whether the second build fails with
    // EACCES, which looks transient and is not.
    checks.push(match writable(&config.publish.root) {
        Ok(()) => Check::pass(
            "publish/writable",
            format!(
                "{} is writable by this process",
                config.publish.root.display()
            ),
        ),
        Err(e) => Check::fail(
            "publish/writable",
            format!(
                "{} is not writable: {e}. If the build runs in its own container, \
                 it must use the same fixed UID as this one.",
                config.publish.root.display()
            ),
        ),
    });

    checks.push(match publish::current_release(&config.publish.root) {
        Some(release) => Check::pass(
            "publish/current",
            format!("current → {}", release.display()),
        ),
        None => Check::warn(
            "publish/current",
            "no current release; a holding page is created at startup",
        ),
    });

    // Hooks
    for (phase, hooks) in [
        ("pre_build", &config.hooks.pre_build),
        ("post_build", &config.hooks.post_build),
        ("post_publish", &config.hooks.post_publish),
    ] {
        for hook in hooks {
            let path = config.paths.hooks_dir().join(&hook.run);
            checks.push(if !path.exists() {
                Check::fail("hooks", format!("{phase}: {} not found", path.display()))
            } else if !is_executable(&path) {
                Check::fail(
                    "hooks",
                    format!("{phase}: {} is not executable", path.display()),
                )
            } else {
                Check::pass("hooks", format!("{phase}: {}", hook.run))
            });
        }
    }

    // Overlay entries
    for entry in &config.assemble.overlay {
        let path = config.paths.config_dir.join(entry);
        checks.push(if path.exists() {
            Check::pass("assemble/overlay", format!("{entry} present"))
        } else {
            Check::fail(
                "assemble/overlay",
                format!(
                    "{} is missing; the build tree cannot be assembled",
                    path.display()
                ),
            )
        });
    }

    // Build isolation
    checks.push(match config.build.kind {
        BuildKind::Agent => Check::pass(
            "build/isolation",
            format!(
                "builds run in the isolated builder at {}",
                config.build.url.as_deref().unwrap_or("?")
            ),
        ),
        BuildKind::Local => Check::pass(
            "build/isolation",
            "builds run here as a subprocess. A generator bug reaches this container's \
             credentials and network; a read-only share token keeps that blast radius small, \
             and build.kind = \"agent\" removes it entirely.",
        ),
    });

    if config.build.kind == BuildKind::Agent && config.build.token_file.is_none() {
        checks.push(Check::warn(
            "build/token",
            "no build.token_file; the builder endpoint is unauthenticated",
        ));
    }

    // Source — the only check that touches the network.
    match Source::from_config(&config) {
        Ok(source) => match source.probe().await {
            Ok(token) => checks.push(Check::pass(
                "source/reachable",
                format!(
                    "{} responded, change token {}",
                    source.describe(),
                    &token[..token.len().min(12)]
                ),
            )),
            Err(e) => checks.push(Check::fail("source/reachable", format!("{e}"))),
        },
        Err(e) => checks.push(Check::fail("source/config", format!("{e}"))),
    }

    if config.source.kind == SourceKind::Webdav && config.source.host_header.is_none() {
        checks.push(Check::warn(
            "source/host_header",
            "no source.host_header; if source.url is an internal name, server_name matching and \
             trusted_domains will not apply",
        ));
    }

    if config.triggers.push.is_none() {
        checks.push(Check::warn(
            "triggers/push",
            format!(
                "no notify_push configured; changes are detected by polling every {:?}",
                config.triggers.poll
            ),
        ));
    }

    Ok(checks)
}

pub fn print(checks: &[Check]) -> Level {
    let mut worst = Level::Pass;
    for check in checks {
        println!(
            "[{}] {:<22} {}",
            check.level.label(),
            check.name,
            check.detail
        );
        if check.level == Level::Fail {
            worst = Level::Fail;
        } else if check.level == Level::Warn && worst == Level::Pass {
            worst = Level::Warn;
        }
    }
    let fails = checks.iter().filter(|c| c.level == Level::Fail).count();
    let warns = checks.iter().filter(|c| c.level == Level::Warn).count();
    println!(
        "\n{} checks, {fails} failed, {warns} warnings",
        checks.len()
    );
    worst
}

fn same_filesystem(a: &Path, b: &Path) -> Option<bool> {
    use std::os::unix::fs::MetadataExt;
    let a = std::fs::metadata(a).ok()?;
    let b = std::fs::metadata(b).ok()?;
    Some(a.dev() == b.dev())
}

fn writable(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    let probe = path.join(".ncpages-write-probe");
    std::fs::write(&probe, b"")?;
    std::fs::remove_file(&probe)?;
    Ok(())
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_filesystem_detects_a_shared_volume() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("build");
        let b = dir.path().join("publish");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        assert_eq!(same_filesystem(&a, &b), Some(true));
    }

    #[test]
    fn missing_paths_are_inconclusive_rather_than_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(same_filesystem(&dir.path().join("nope"), dir.path()), None);
    }

    #[test]
    fn executability_is_checked_not_assumed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hook.sh");
        std::fs::write(&path, "#!/bin/sh\n").unwrap();
        assert!(!is_executable(&path));

        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(is_executable(&path));
    }
}
