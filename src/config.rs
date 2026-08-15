//! Configuration surface: `ncpages.toml`.
//!
//! Documented in `knowledge/interfaces/configuration.md`. Two rules here are
//! load-bearing and enforced in [`Config::validate`] rather than documented and
//! hoped for:
//!
//! * the hook directory must not live inside the source working copy, because a
//!   build is code execution and the vault is writable by anyone the folder is
//!   shared with,
//! * the status report path must not live inside the watched remote path, or the
//!   service triggers itself forever.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    pub source: Source,
    #[serde(default)]
    pub paths: Paths,
    #[serde(default)]
    pub triggers: Triggers,
    #[serde(default)]
    pub schedule: Schedule,
    #[serde(default)]
    pub assemble: Assemble,
    #[serde(default)]
    pub build: Build,
    #[serde(default)]
    pub hooks: Hooks,
    #[serde(default)]
    pub gate: Gate,
    #[serde(default)]
    pub publish: Publish,
    #[serde(default)]
    pub serve: Serve,
    #[serde(default)]
    pub health: Health,
    #[serde(default)]
    pub report: Report,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// `webdav` or `fs`.
    pub kind: SourceKind,
    /// Base URL of the *HTTP* frontend. With an FPM image this is nginx, never
    /// the FPM container, which does not speak HTTP.
    #[serde(default)]
    pub url: Option<String>,
    /// Real public domain, sent as `Host:` so `trusted_domains` still matches.
    #[serde(default)]
    pub host_header: Option<String>,
    /// Remote path below the user's files root, or a local path for `fs`.
    pub path: String,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub password_file: Option<PathBuf>,
    /// Refuse to start when the source is unreachable. Off by default: the
    /// working copy is persistent, so an unreachable source degrades rather
    /// than stops the service.
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Webdav,
    Fs,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paths {
    /// Working copy of the vault.
    pub src: PathBuf,
    /// Assembled build tree. Must share a filesystem with `publish.root`.
    pub build: PathBuf,
    /// Persisted ETags, hashes and build history.
    pub state: PathBuf,
    /// Hook scripts and overlay files. Read-only, outside the vault.
    pub config_dir: PathBuf,
}

impl Default for Paths {
    fn default() -> Self {
        Self {
            src: PathBuf::from("/work/src"),
            build: PathBuf::from("/work/build"),
            state: PathBuf::from("/work/state"),
            config_dir: PathBuf::from("/etc/ncpages"),
        }
    }
}

impl Paths {
    pub fn hooks_dir(&self) -> PathBuf {
        self.config_dir.join("hooks")
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Triggers {
    /// notify_push WebSocket URL. Absent disables push.
    #[serde(default)]
    pub push: Option<String>,
    /// Safety net. Keeps running even when push is healthy.
    #[serde(default = "d_poll", with = "humantime_serde")]
    pub poll: Duration,
    /// Needed by anyone whose build pulls external data. Absent disables it.
    #[serde(default, with = "humantime_serde")]
    pub timer: Option<Duration>,
    /// Spread timer builds so installations do not hit external APIs in lockstep.
    #[serde(default = "d_jitter")]
    pub jitter: f64,
}

impl Default for Triggers {
    fn default() -> Self {
        Self {
            push: None,
            poll: d_poll(),
            timer: None,
            jitter: d_jitter(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    /// Quiet period after the last change. Obsidian autosaves constantly.
    #[serde(default = "d_debounce", with = "humantime_serde")]
    pub debounce: Duration,
    /// Upper bound on debouncing, so a continuously edited vault still builds.
    #[serde(default = "d_max_delay", with = "humantime_serde")]
    pub max_delay: Duration,
    /// Only `queue_latest` is supported; cancelling a running build is unsafe.
    #[serde(default = "d_on_busy")]
    pub on_busy: String,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            debounce: d_debounce(),
            max_delay: d_max_delay(),
            on_busy: d_on_busy(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assemble {
    /// Entries copied from `paths.config_dir` into the build tree. Anything
    /// executable or configuring belongs here, never in the vault.
    #[serde(default)]
    pub overlay: Vec<String>,
    /// Where the vault content lands inside the build tree.
    #[serde(default = "d_source_subdir")]
    pub source_subdir: String,
}

impl Default for Assemble {
    fn default() -> Self {
        Self {
            overlay: Vec::new(),
            source_subdir: d_source_subdir(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Build {
    /// `agent` runs the build in the isolated builder container. `local` runs it
    /// in this process and exists for development only.
    #[serde(default = "d_build_kind")]
    pub kind: BuildKind,
    /// Builder endpoint for `kind = "agent"`.
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub token_file: Option<PathBuf>,
    /// Command for `kind = "local"`.
    #[serde(default)]
    pub command: Option<Vec<String>>,
    #[serde(default = "d_build_timeout", with = "humantime_serde")]
    pub timeout: Duration,
    /// Generator output directory, relative to the build tree.
    #[serde(default = "d_output")]
    pub output: String,
}

impl Default for Build {
    fn default() -> Self {
        Self {
            kind: d_build_kind(),
            url: None,
            token_file: None,
            command: None,
            timeout: d_build_timeout(),
            output: d_output(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildKind {
    Agent,
    Local,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hooks {
    #[serde(default)]
    pub pre_build: Vec<Hook>,
    #[serde(default)]
    pub post_build: Vec<Hook>,
    #[serde(default)]
    pub post_publish: Vec<Hook>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hook {
    /// Program to run, resolved against the hook directory.
    pub run: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Variables forwarded from the service environment. Nothing else reaches a
    /// hook; secrets are opt-in per hook.
    #[serde(default)]
    pub env_passthrough: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    #[serde(default)]
    pub require_files: Vec<String>,
    #[serde(default)]
    pub min_pages: usize,
    /// Maximum tolerated drop in page count against the previous release, as a
    /// ratio. Catches the half-synced vault that builds with exit code 0.
    #[serde(default = "d_max_page_drop")]
    pub max_page_drop: f64,
    /// Fail instead of warn when two content files share a basename, which makes
    /// wikilink resolution ambiguous.
    #[serde(default = "d_true")]
    pub forbid_duplicate_basenames: bool,
}

impl Default for Gate {
    fn default() -> Self {
        Self {
            require_files: Vec::new(),
            min_pages: 0,
            max_page_drop: d_max_page_drop(),
            forbid_duplicate_basenames: true,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publish {
    #[serde(default = "d_publish_root")]
    pub root: PathBuf,
    #[serde(default = "d_keep")]
    pub keep_releases: usize,
}

impl Default for Publish {
    fn default() -> Self {
        Self {
            root: d_publish_root(),
            keep_releases: d_keep(),
        }
    }
}

impl Publish {
    pub fn current(&self) -> PathBuf {
        self.root.join("current")
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Serve {
    #[serde(default = "d_true")]
    pub enabled: bool,
    #[serde(default = "d_listen")]
    pub listen: String,
    /// `Cache-Control` for hashed asset paths.
    #[serde(default = "d_cache_assets")]
    pub cache_control_assets: String,
    /// `Cache-Control` for everything else. Wrong values here mean stale HTML
    /// pointing at asset names that no longer exist.
    #[serde(default = "d_cache_html")]
    pub cache_control_html: String,
}

impl Default for Serve {
    fn default() -> Self {
        Self {
            enabled: true,
            listen: d_listen(),
            cache_control_assets: d_cache_assets(),
            cache_control_html: d_cache_html(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Health {
    #[serde(default = "d_health_listen")]
    pub listen: String,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            listen: d_health_listen(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// Status file written back over WebDAV. Must be outside `source.path`.
    #[serde(default)]
    pub webdav_status_path: Option<String>,
    #[serde(default)]
    pub ntfy_topic: Option<String>,
}

fn d_poll() -> Duration {
    Duration::from_secs(30)
}
fn d_jitter() -> f64 {
    0.1
}
fn d_debounce() -> Duration {
    Duration::from_secs(10)
}
fn d_max_delay() -> Duration {
    Duration::from_secs(120)
}
fn d_on_busy() -> String {
    "queue_latest".into()
}
fn d_source_subdir() -> String {
    "docs".into()
}
fn d_build_kind() -> BuildKind {
    BuildKind::Agent
}
fn d_build_timeout() -> Duration {
    Duration::from_secs(600)
}
fn d_output() -> String {
    "site".into()
}
fn d_max_page_drop() -> f64 {
    0.4
}
fn d_publish_root() -> PathBuf {
    PathBuf::from("/work/publish")
}
fn d_keep() -> usize {
    5
}
fn d_listen() -> String {
    "0.0.0.0:8080".into()
}
fn d_health_listen() -> String {
    "0.0.0.0:9090".into()
}
fn d_cache_assets() -> String {
    "public, max-age=31536000, immutable".into()
}
fn d_cache_html() -> String {
    "no-cache".into()
}
fn d_true() -> bool {
    true
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config {}", path.display()))?;
        let config: Config =
            toml::from_str(&text).with_context(|| format!("parsing config {}", path.display()))?;
        config.validate()?;
        Ok(config)
    }

    /// Startup checks that are fail-closed on purpose. A warning would be taken
    /// as advice and ignored exactly once.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != SCHEMA_VERSION {
            bail!(
                "schema_version {} is not supported by this build (expected {})",
                self.schema_version,
                SCHEMA_VERSION
            );
        }

        if self.schedule.on_busy != "queue_latest" {
            bail!(
                "schedule.on_busy = {:?} is not supported; only \"queue_latest\" is. \
                 Cancelling a running build can leave a published state whose \
                 irreversible post_publish effects half-fired.",
                self.schedule.on_busy
            );
        }

        // A hook directory inside the working copy means anyone with write
        // access to the shared folder has a shell on this host.
        let hooks = self.paths.hooks_dir();
        if starts_with(&hooks, &self.paths.src) {
            bail!(
                "refusing to start: hook directory {} is inside the source working copy {}. \
                 A build is code execution; scripts must live outside the vault.",
                hooks.display(),
                self.paths.src.display()
            );
        }
        if starts_with(&self.paths.config_dir, &self.paths.src) {
            bail!(
                "refusing to start: config directory {} is inside the source working copy {}",
                self.paths.config_dir.display(),
                self.paths.src.display()
            );
        }

        // Writing the status report inside the watched folder changes the root
        // ETag, which triggers a build, which writes the status again.
        if let Some(status) = &self.report.webdav_status_path {
            if remote_contains(&self.source.path, status) {
                bail!(
                    "refusing to start: report.webdav_status_path {:?} is inside source.path {:?}. \
                     Writing it would change the root ETag and trigger an endless build loop; \
                     the root ETag is path-blind, so excludes do not help.",
                    status,
                    self.source.path
                );
            }
        }

        match self.source.kind {
            SourceKind::Webdav => {
                if self.source.url.is_none() {
                    bail!("source.url is required for kind = \"webdav\"");
                }
                if self.source.user.is_none() || self.source.password_file.is_none() {
                    bail!(
                        "source.user and source.password_file are required for kind = \"webdav\""
                    );
                }
            }
            SourceKind::Fs => {}
        }

        match self.build.kind {
            BuildKind::Agent => {
                if self.build.url.is_none() {
                    bail!("build.url is required for kind = \"agent\"");
                }
            }
            BuildKind::Local => {
                if self.build.command.as_ref().is_none_or(|c| c.is_empty()) {
                    bail!("build.command is required for kind = \"local\"");
                }
            }
        }

        if !(0.0..=1.0).contains(&self.gate.max_page_drop) {
            bail!("gate.max_page_drop must be between 0.0 and 1.0");
        }

        Ok(())
    }

    /// Secrets are read from files, never taken from inline config values.
    pub fn source_password(&self) -> Result<Option<String>> {
        read_secret(self.source.password_file.as_deref())
    }

    pub fn build_token(&self) -> Result<Option<String>> {
        read_secret(self.build.token_file.as_deref())
    }

    pub fn out_dir(&self) -> PathBuf {
        self.paths.build.join(&self.build.output)
    }

    pub fn content_dir(&self) -> PathBuf {
        self.paths.build.join(&self.assemble.source_subdir)
    }
}

fn read_secret(path: Option<&Path>) -> Result<Option<String>> {
    match path {
        None => Ok(None),
        Some(path) => {
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("reading secret {}", path.display()))?;
            Ok(Some(raw.trim().to_string()))
        }
    }
}

/// Path containment without requiring either path to exist yet.
fn starts_with(inner: &Path, outer: &Path) -> bool {
    let inner = normalize(inner);
    let outer = normalize(outer);
    inner.starts_with(&outer)
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// Containment for remote (WebDAV) paths, which are plain strings.
fn remote_contains(outer: &str, inner: &str) -> bool {
    let outer = outer.trim_matches('/');
    let inner = inner.trim_matches('/');
    if outer.is_empty() {
        return true;
    }
    inner == outer || inner.starts_with(&format!("{outer}/"))
}

/// Environment handed to every hook, per the documented contract.
pub fn hook_env(
    config: &Config,
    release_dir: Option<&Path>,
    prev_dir: Option<&Path>,
    trigger: &str,
) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    env.insert(
        "NCPAGES_SRC_DIR".into(),
        config.paths.src.display().to_string(),
    );
    env.insert(
        "NCPAGES_BUILD_DIR".into(),
        config.paths.build.display().to_string(),
    );
    env.insert(
        "NCPAGES_OUT_DIR".into(),
        config.out_dir().display().to_string(),
    );
    env.insert(
        "NCPAGES_RELEASE_DIR".into(),
        release_dir
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    );
    env.insert(
        "NCPAGES_PREV_DIR".into(),
        prev_dir
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    );
    env.insert("NCPAGES_TRIGGER".into(), trigger.to_string());
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Config {
        toml::from_str(
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
        .unwrap()
    }

    #[test]
    fn defaults_are_the_documented_ones() {
        let c = base();
        assert_eq!(c.triggers.poll, Duration::from_secs(30));
        assert_eq!(c.schedule.debounce, Duration::from_secs(10));
        assert_eq!(c.schedule.max_delay, Duration::from_secs(120));
        assert_eq!(c.publish.keep_releases, 5);
        assert_eq!(c.assemble.source_subdir, "docs");
        assert!(c.validate().is_ok());
    }

    #[test]
    fn hook_directory_inside_the_vault_is_refused() {
        let mut c = base();
        c.paths.src = PathBuf::from("/work/src");
        c.paths.config_dir = PathBuf::from("/work/src/.ncpages");
        let err = c.validate().unwrap_err().to_string();
        assert!(err.contains("refusing to start"), "{err}");
    }

    #[test]
    fn status_path_inside_the_watched_folder_is_refused() {
        let mut c = base();
        c.source.path = "Notes/blog".into();
        c.report.webdav_status_path = Some("Notes/blog/_status/build.md".into());
        let err = c.validate().unwrap_err().to_string();
        assert!(err.contains("endless build loop"), "{err}");
    }

    #[test]
    fn sibling_status_path_is_fine() {
        let mut c = base();
        c.source.path = "Notes/blog".into();
        c.report.webdav_status_path = Some("Notes/_blog-status/build.md".into());
        assert!(c.validate().is_ok());
    }

    #[test]
    fn cancelling_builds_is_not_configurable() {
        let mut c = base();
        c.schedule.on_busy = "restart".into();
        assert!(c.validate().is_err());
    }

    #[test]
    fn unknown_keys_are_rejected_rather_than_silently_ignored() {
        let err = toml::from_str::<Config>(
            r#"
            schema_version = 1
            [source]
            kind = "fs"
            path = "/tmp/vault"
            typo_here = true
            "#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("typo_here"));
    }
}
