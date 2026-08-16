//! The ten steps from trigger to report.
//!
//! If any step fails, `current` keeps pointing where it pointed before: the site
//! is never in an intermediate state. Step 9 (`post_publish`) runs if and only if
//! step 8 (the atomic swap) succeeded, because everything in it is irreversible.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use tracing::{info, warn};

use crate::config::{BuildKind, Config};
use crate::fsutil;
use crate::gate;
use crate::hooks::{self, Phase};
use crate::publish;
use crate::source::Source;
use crate::state::State;

#[derive(Debug)]
pub struct Outcome {
    pub trigger: String,
    pub release: Option<String>,
    pub pages: usize,
    pub published: bool,
    pub skipped: bool,
    pub warnings: Vec<String>,
    pub violations: Vec<String>,
    pub conflict_copies: Vec<String>,
}

impl Outcome {
    pub fn summary(&self) -> String {
        if self.skipped {
            return "no change".into();
        }
        if self.published {
            format!(
                "published {} ({} pages)",
                self.release.as_deref().unwrap_or("?"),
                self.pages
            )
        } else {
            format!("gate refused: {}", self.violations.join("; "))
        }
    }
}

pub async fn run_once(
    config: Arc<Config>,
    source: &Source,
    state: &mut State,
    trigger: &str,
) -> Result<Outcome> {
    let mut outcome = Outcome {
        trigger: trigger.to_string(),
        release: None,
        pages: 0,
        published: false,
        skipped: false,
        warnings: Vec::new(),
        violations: Vec::new(),
        conflict_copies: Vec::new(),
    };

    // 1 — SYNC
    let sync = source
        .sync(&config.paths.src, state)
        .await
        .context("sync from source")?;
    outcome.conflict_copies = sync.conflict_copies.clone();
    for conflict in &sync.conflict_copies {
        warn!(file = %conflict, "conflict copy excluded from the build — work may be about to be lost");
    }

    // A timer run rebuilds even without content changes: that is the whole point
    // of having one, since hooks pull data from outside.
    if !sync.changed && trigger != "timer" && trigger != "manual" && state.last_release.is_some() {
        outcome.skipped = true;
        return Ok(outcome);
    }

    // 2 — ASSEMBLE
    assemble(&config).context("assembling the build tree")?;

    let prev_dir = publish::current_release(&config.publish.root);
    let prev_pages = prev_dir
        .as_deref()
        .map(|dir| fsutil::count_files_with_extension(dir, "html"));

    let env = crate::config::hook_env(&config, None, prev_dir.as_deref(), trigger);

    // 3 — pre_build
    let pre = hooks::run_phase(
        Phase::PreBuild,
        &config.hooks.pre_build,
        &config.paths.hooks_dir(),
        &config.paths.build,
        &env,
        config.build.timeout,
    )
    .await?;
    outcome.warnings.extend(pre.warnings);

    // 4 — BUILD, here as a subprocess or over there in the builder container
    build(&config, &env).await.context("running the build")?;

    let out_dir = config.out_dir();
    if !out_dir.is_dir() {
        bail!(
            "the build produced no output directory at {} — check build.output",
            out_dir.display()
        );
    }

    // 5 — post_build, before the gate, so its output is gated too
    let post = hooks::run_phase(
        Phase::PostBuild,
        &config.hooks.post_build,
        &config.paths.hooks_dir(),
        &config.paths.build,
        &env,
        config.build.timeout,
    )
    .await?;
    outcome.warnings.extend(post.warnings);

    // 6 — MOVE (same filesystem, so this is a rename)
    let release_id = publish::new_release_id(std::time::SystemTime::now());
    let release_dir = publish::releases_dir(&config.publish.root).join(&release_id);
    std::fs::create_dir_all(publish::releases_dir(&config.publish.root))?;
    fsutil::remove_dir_if_exists(&release_dir)?;
    move_dir(&out_dir, &release_dir)
        .with_context(|| format!("moving {} to {}", out_dir.display(), release_dir.display()))?;
    outcome.release = Some(release_id.clone());

    // 7 — GATE
    let verdict = gate::evaluate(
        &config.gate,
        &release_dir,
        &config.content_dir(),
        prev_pages,
    );
    outcome.pages = verdict.pages;
    outcome.warnings.extend(verdict.warnings.clone());
    if !verdict.passed() {
        outcome.violations = verdict.violations.clone();
        warn!(
            release = %release_id,
            violations = ?verdict.violations,
            "gate refused the build; the live site is unchanged"
        );
        // The release stays on disk for inspection; retention will clear it.
        publish::retain(&config.publish.root, config.publish.keep_releases)?;
        return Ok(outcome);
    }

    // 8 — PUBLISH (atomic)
    publish::swap(&config.publish.root, &release_dir)?;
    outcome.published = true;
    state.last_release = Some(release_id.clone());
    state.content_hash = Some(fsutil::tree_hash(&config.content_dir())?);
    info!(release = %release_id, pages = verdict.pages, "published");

    // 9 — post_publish (irreversible; only ever reached after a successful swap)
    let env = crate::config::hook_env(&config, Some(&release_dir), prev_dir.as_deref(), trigger);
    let published_hooks = hooks::run_phase(
        Phase::PostPublish,
        &config.hooks.post_publish,
        &config.paths.hooks_dir(),
        &config.paths.build,
        &env,
        config.build.timeout,
    )
    .await;
    match published_hooks {
        Ok(result) => outcome.warnings.extend(result.warnings),
        Err(e) => {
            // The site is live and correct; only the announcement failed.
            warn!(error = %e, "post_publish failed after a successful publish");
            outcome.warnings.push(format!("post_publish failed: {e}"));
        }
    }

    // 10 — REPORT (retention; status reporting is handled by the caller)
    let removed = publish::retain(&config.publish.root, config.publish.keep_releases)?;
    if !removed.is_empty() {
        info!(removed = ?removed, "retention removed old releases");
    }

    Ok(outcome)
}

/// Overlay from the read-only config directory plus content from the vault.
///
/// The build tree is rebuilt from scratch every time: a leftover file from a
/// previous build that no longer exists in either source would otherwise be
/// published forever.
fn assemble(config: &Config) -> Result<()> {
    fsutil::remove_dir_if_exists(&config.paths.build)?;
    std::fs::create_dir_all(&config.paths.build)?;

    for entry in &config.assemble.overlay {
        let from = config.paths.config_dir.join(entry);
        let to = config.paths.build.join(entry);
        if !from.exists() {
            bail!(
                "overlay entry {} is missing from {}",
                entry,
                config.paths.config_dir.display()
            );
        }
        if from.is_dir() {
            fsutil::copy_dir(&from, &to)?;
        } else {
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&from, &to)?;
        }
    }

    let content = config.content_dir();
    std::fs::create_dir_all(&content)?;
    fsutil::copy_dir(&config.paths.src, &content)?;
    Ok(())
}

async fn build(config: &Config, env: &BTreeMap<String, String>) -> Result<()> {
    match config.build.kind {
        // The build is the fourth phase of the same executor, not a mechanism of
        // its own. `local` and `agent` differ only in *where* it runs.
        BuildKind::Local => {
            let command = config
                .build
                .command
                .as_ref()
                .expect("validated: build.command is set for kind = local");
            hooks::run_build(
                command,
                &config.paths.build,
                env,
                &config.secret_env_names(),
                config.build.timeout,
            )
            .await
        }
        BuildKind::Agent => {
            let url = format!(
                "{}/build",
                config
                    .build
                    .url
                    .as_deref()
                    .expect("validated")
                    .trim_end_matches('/')
            );
            let client = reqwest::Client::builder()
                .timeout(config.build.timeout)
                .build()?;
            let mut request = client.post(&url);
            if let Some(token) = config.build_token()? {
                request = request.bearer_auth(token);
            }
            let response = request
                .send()
                .await
                .with_context(|| format!("POST {url}"))?;
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if !status.is_success() {
                bail!("builder returned {status}: {}", body.trim());
            }
            Ok(())
        }
    }
}

/// Rename within one filesystem, with a copy fallback for the case where the
/// build tree and the publish root are on different volumes — a misconfiguration
/// that costs atomicity of the move, though not of the swap.
fn move_dir(from: &PathBuf, to: &PathBuf) -> Result<()> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(18) => {
            warn!(
                from = %from.display(),
                to = %to.display(),
                "build tree and publish root are on different filesystems; copying instead of renaming"
            );
            fsutil::copy_dir(from, to)?;
            fsutil::remove_dir_if_exists(from)?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_for(root: &std::path::Path) -> Config {
        let text = format!(
            r#"
            schema_version = 1
            [source]
            kind = "fs"
            path = "{src}"
            [paths]
            src = "{work}/src"
            build = "{work}/build"
            state = "{work}/state"
            config_dir = "{work}/etc"
            [assemble]
            overlay = ["build.sh"]
            source_subdir = "docs"
            [build]
            kind = "local"
            command = ["./build.sh"]
            [gate]
            min_pages = 1
            require_files = ["index.html"]
            [publish]
            root = "{work}/publish"
            keep_releases = 2
            "#,
            src = root.join("vault").display(),
            work = root.display(),
        );
        toml::from_str(&text).unwrap()
    }

    /// A stand-in generator: copies every markdown file to an HTML file.
    fn write_generator(dir: &std::path::Path) {
        let script = dir.join("build.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\nset -e\nmkdir -p site\nfor f in docs/*.md; do\n  [ -e \"$f\" ] || continue\n  n=$(basename \"$f\" .md)\n  cp \"$f\" \"site/$n.html\"\ndone\ntouch site/index.html\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[tokio::test]
    async fn end_to_end_publishes_and_then_refuses_a_collapsed_vault() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("vault")).unwrap();
        std::fs::create_dir_all(root.path().join("etc")).unwrap();
        write_generator(&root.path().join("etc"));
        for i in 0..10 {
            std::fs::write(root.path().join("vault").join(format!("p{i}.md")), "x").unwrap();
        }

        let config = Arc::new(config_for(root.path()));
        let source = Source::from_config(&config).unwrap();
        let mut state = State::default();

        let outcome = run_once(config.clone(), &source, &mut state, "manual")
            .await
            .unwrap();
        assert!(outcome.published, "{outcome:?}");
        assert_eq!(outcome.pages, 11); // ten pages plus index.html
        let first = publish::current_release(&config.publish.root).unwrap();

        // A sync accident empties the vault: the build still succeeds, and the
        // gate is the only thing standing between it and the live site.
        for i in 0..10 {
            std::fs::remove_file(root.path().join("vault").join(format!("p{i}.md"))).unwrap();
        }
        let outcome = run_once(config.clone(), &source, &mut state, "manual")
            .await
            .unwrap();
        assert!(!outcome.published, "a one-page site replaced the blog");
        assert!(
            outcome.violations[0].contains("dropped"),
            "{:?}",
            outcome.violations
        );
        assert_eq!(
            publish::current_release(&config.publish.root).unwrap(),
            first,
            "the live site changed despite a failed gate"
        );
    }

    #[tokio::test]
    async fn an_unchanged_vault_does_not_rebuild_but_a_timer_does() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("vault")).unwrap();
        std::fs::create_dir_all(root.path().join("etc")).unwrap();
        write_generator(&root.path().join("etc"));
        std::fs::write(root.path().join("vault/a.md"), "x").unwrap();

        let config = Arc::new(config_for(root.path()));
        let source = Source::from_config(&config).unwrap();
        let mut state = State::default();

        assert!(
            run_once(config.clone(), &source, &mut state, "poll")
                .await
                .unwrap()
                .published
        );
        assert!(
            run_once(config.clone(), &source, &mut state, "poll")
                .await
                .unwrap()
                .skipped
        );
        assert!(
            run_once(config.clone(), &source, &mut state, "timer")
                .await
                .unwrap()
                .published
        );
    }
}
