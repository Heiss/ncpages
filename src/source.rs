//! Change detection and synchronisation.
//!
//! WebDAV rather than the filesystem, because Nextcloud propagates ETag changes
//! up the tree: one `PROPFIND Depth: 0` on the root answers "did anything below
//! this change". That works with S3 primary storage and server-side encryption,
//! where inotify cannot work in principle.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use reqwest::{Method, StatusCode};
use tracing::{debug, warn};

use crate::config::{Config, SourceKind};
use crate::fsutil;
use crate::state::State;

/// Everything except unreserved characters and the path separator.
const PATH_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'%')
    .add(b'[')
    .add(b']')
    .add(b'^')
    .add(b'|');

#[derive(Debug, Default)]
pub struct SyncReport {
    pub changed: bool,
    pub downloaded: usize,
    pub deleted: usize,
    /// Reported rather than silently dropped: a conflict copy means a version of
    /// someone's work is at risk.
    pub conflict_copies: Vec<String>,
}

pub enum Source {
    Webdav(Webdav),
    Fs(Fs),
}

impl Source {
    pub fn from_config(config: &Config) -> Result<Self> {
        match config.source.kind {
            SourceKind::Webdav => Ok(Source::Webdav(Webdav::new(config)?)),
            SourceKind::Fs => Ok(Source::Fs(Fs {
                root: PathBuf::from(&config.source.path),
            })),
        }
    }

    /// Cheap change token. For WebDAV this is one request; for `fs` it is a
    /// content hash, because timestamps are unreliable across sync boundaries.
    pub async fn probe(&self) -> Result<String> {
        match self {
            Source::Webdav(s) => s.root_etag().await,
            Source::Fs(s) => fsutil::tree_hash(&s.root),
        }
    }

    pub async fn sync(&self, dest: &Path, state: &mut State) -> Result<SyncReport> {
        match self {
            Source::Webdav(s) => s.sync(dest, state).await,
            Source::Fs(s) => s.sync(dest, state),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Source::Webdav(s) => s.base.clone(),
            Source::Fs(s) => s.root.display().to_string(),
        }
    }
}

pub struct Fs {
    pub root: PathBuf,
}

impl Fs {
    fn sync(&self, dest: &Path, state: &mut State) -> Result<SyncReport> {
        let token = fsutil::tree_hash(&self.root)?;
        let mut report = SyncReport::default();
        if state.root_etag.as_deref() == Some(token.as_str()) && dest.exists() {
            return Ok(report);
        }
        fsutil::remove_dir_if_exists(dest)?;
        std::fs::create_dir_all(dest)?;
        for entry in walkdir::WalkDir::new(&self.root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if fsutil::is_conflict_copy(&name) {
                report.conflict_copies.push(name);
                continue;
            }
            let rel = entry.path().strip_prefix(&self.root).unwrap();
            let target = dest.join(rel);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
            report.downloaded += 1;
        }
        state.root_etag = Some(token);
        report.changed = true;
        Ok(report)
    }
}

pub struct Webdav {
    client: reqwest::Client,
    /// `{url}/remote.php/dav/files/{user}` for an account, or
    /// `{url}/public.php/dav/files/{token}` for a public share.
    base: String,
    /// Remote path below that root.
    path: String,
    host_header: Option<String>,
    auth: Auth,
}

/// Two ways in. A share link is the smaller one: no account credential leaves
/// Nextcloud, the share is read-only by nature, and revoking it is one click.
pub enum Auth {
    Account {
        user: String,
        password: String,
    },
    /// A share link. Nextcloud's public WebDAV takes the **share id as the
    /// username** and the share password, empty when the share has none. Newer
    /// documentation describes the literal user `anonymous` instead, depending on
    /// version and endpoint, so a 401 on a password-protected share is retried
    /// that way once.
    Share {
        token: String,
        password: Option<String>,
    },
}

#[derive(Debug)]
struct Entry {
    rel: String,
    etag: String,
    is_dir: bool,
}

impl Webdav {
    pub fn new(config: &Config) -> Result<Self> {
        let url = config
            .source
            .url
            .as_deref()
            .ok_or_else(|| anyhow!("source.url is required"))?
            .trim_end_matches('/')
            .to_string();
        let (base, auth) = match &config.source.share_token {
            Some(token) => (
                format!("{url}/public.php/dav/files/{token}"),
                Auth::Share {
                    token: token.clone(),
                    password: config.share_password()?,
                },
            ),
            None => {
                let user = config
                    .source
                    .user
                    .clone()
                    .ok_or_else(|| anyhow!("source.user is required"))?;
                let password = config
                    .source_password()?
                    .ok_or_else(|| anyhow!("source.password_file is required"))?;
                (
                    format!("{url}/remote.php/dav/files/{user}"),
                    Auth::Account { user, password },
                )
            }
        };

        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .context("building HTTP client")?,
            base,
            path: config.source.path.trim_matches('/').to_string(),
            host_header: config.source.host_header.clone(),
            auth,
        })
    }

    /// Join base, watched path and a relative entry, percent-encoding each
    /// segment but not the separators. Empty segments are dropped, so a share
    /// whose watched path is the share root does not produce a double slash.
    fn url_for(&self, rel: &str) -> String {
        let encoded: Vec<String> = self
            .path
            .split('/')
            .chain(rel.split('/'))
            .filter(|segment| !segment.is_empty())
            .map(|segment| utf8_percent_encode(segment, PATH_SET).to_string())
            .collect();

        if encoded.is_empty() {
            self.base.clone()
        } else {
            format!("{}/{}", self.base, encoded.join("/"))
        }
    }

    /// `anonymous` selects the alternative credential form for share links.
    fn request_as(&self, method: Method, url: &str, anonymous: bool) -> reqwest::RequestBuilder {
        let mut builder = self.client.request(method, url);
        builder = match &self.auth {
            Auth::Account { user, password } => builder.basic_auth(user, Some(password)),
            // The share id is the username. The password is empty when the share
            // has none, which the public endpoint accepts.
            Auth::Share { token, password } => {
                let user = if anonymous {
                    "anonymous"
                } else {
                    token.as_str()
                };
                builder.basic_auth(user, Some(password.clone().unwrap_or_default()))
            }
        };
        // Nextcloud answers 401 to non-GET requests against /public.php/dav
        // without this, unless server-to-server sharing happens to be enabled.
        // Harmless on the account endpoint, so it is sent unconditionally.
        builder = builder.header("X-Requested-With", "XMLHttpRequest");
        // Internally we speak to the HTTP frontend by service name; the real
        // domain still has to appear in Host: for server_name matching and
        // trusted_domains to work.
        if let Some(host) = &self.host_header {
            builder = builder.header(reqwest::header::HOST, host);
        }
        builder
    }

    /// Send, and on a 401 for a password-protected share try the alternative
    /// credential form once. Nextcloud's public endpoint has documented both the
    /// share id and the literal `anonymous` as the username, depending on
    /// version and endpoint, and guessing wrong looks exactly like a wrong
    /// password.
    async fn send<F>(&self, make: F) -> Result<reqwest::Response>
    where
        F: Fn(bool) -> reqwest::RequestBuilder,
    {
        let response = make(false).send().await?;
        if response.status() == StatusCode::UNAUTHORIZED && self.share_with_password() {
            debug!("share credentials rejected; retrying as anonymous");
            return Ok(make(true).send().await?);
        }
        Ok(response)
    }

    fn share_with_password(&self) -> bool {
        matches!(
            &self.auth,
            Auth::Share {
                password: Some(_),
                ..
            }
        )
    }

    async fn propfind(&self, rel: &str, depth: u8) -> Result<Vec<Entry>> {
        let url = self.url_for(rel);
        let body = r#"<?xml version="1.0"?>
<d:propfind xmlns:d="DAV:"><d:prop><d:getetag/><d:resourcetype/></d:prop></d:propfind>"#;

        let response = self
            .send(|anonymous| {
                self.request_as(Method::from_bytes(b"PROPFIND").unwrap(), &url, anonymous)
                    .header("Depth", depth.to_string())
                    .header(reqwest::header::CONTENT_TYPE, "application/xml")
                    .body(body)
            })
            .await
            .with_context(|| format!("PROPFIND {url}"))?;

        check_status(response.status(), &url)?;
        let text = response.text().await?;
        parse_multistatus(&text, &self.base, &self.path)
    }

    pub async fn root_etag(&self) -> Result<String> {
        let entries = self.propfind("", 0).await?;
        entries
            .into_iter()
            .next()
            .map(|e| e.etag)
            .ok_or_else(|| anyhow!("PROPFIND returned no response element for the watched root"))
    }

    /// Descend by ETag: only directories whose ETag changed are listed, and only
    /// files whose ETag changed are downloaded.
    async fn sync(&self, dest: &Path, state: &mut State) -> Result<SyncReport> {
        let mut report = SyncReport::default();
        let root_etag = self.root_etag().await?;
        if state.root_etag.as_deref() == Some(root_etag.as_str()) && dest.exists() {
            return Ok(report);
        }

        let mut remote: BTreeMap<String, String> = BTreeMap::new();
        let mut queue = vec![String::new()];
        while let Some(dir) = queue.pop() {
            for entry in self.propfind(&dir, 1).await? {
                if entry.rel == dir {
                    continue; // the collection itself
                }
                // The path comes from a server-controlled href. A hostile or
                // compromised server could return one that decodes to `../..`
                // and write outside the working copy.
                if !is_safe_relative(&entry.rel) {
                    warn!(path = %entry.rel, "ignoring a remote path that escapes the working copy");
                    continue;
                }
                if entry.is_dir {
                    queue.push(entry.rel);
                } else {
                    remote.insert(entry.rel, entry.etag);
                }
            }
        }

        std::fs::create_dir_all(dest)?;
        for (rel, etag) in &remote {
            let name = rel.rsplit('/').next().unwrap_or(rel);
            if fsutil::is_conflict_copy(name) {
                report.conflict_copies.push(rel.clone());
                continue;
            }
            let target = dest.join(rel);
            if state.file_etags.get(rel) == Some(etag) && target.exists() {
                continue;
            }
            let url = self.url_for(rel);
            let response = self
                .send(|anonymous| self.request_as(Method::GET, &url, anonymous))
                .await
                .with_context(|| format!("GET {url}"))?;
            check_status(response.status(), &url)?;
            let bytes = response.bytes().await?;
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&target, &bytes)
                .with_context(|| format!("writing {}", target.display()))?;
            report.downloaded += 1;
        }

        // Remove local files that are gone remotely. The gate is what stops a
        // mass deletion from reaching the live site.
        let keep: HashSet<&String> = remote.keys().collect();
        for entry in walkdir::WalkDir::new(dest)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(dest)
                .unwrap()
                .to_string_lossy()
                .to_string();
            if !keep.contains(&rel) {
                std::fs::remove_file(entry.path())?;
                report.deleted += 1;
            }
        }

        state.file_etags = remote;
        state.root_etag = Some(root_etag);
        report.changed = report.downloaded > 0 || report.deleted > 0;
        Ok(report)
    }
}

/// Whether a server-supplied path may be joined onto a local directory.
///
/// A legitimate entry never contains a `..` segment, a leading slash, a
/// backslash or a drive letter. Without this check, one crafted `href` in a
/// PROPFIND response writes anywhere the process can reach.
fn is_safe_relative(rel: &str) -> bool {
    if rel.is_empty() || rel.starts_with('/') || rel.contains('\\') {
        return false;
    }
    if rel.len() >= 2 && rel.as_bytes()[1] == b':' {
        return false;
    }
    !rel.split('/').any(|segment| segment == "..")
}

fn check_status(status: StatusCode, url: &str) -> Result<()> {
    match status {
        s if s.is_success() || s.as_u16() == 207 => Ok(()),
        // Retrying against brute-force protection makes recovery harder.
        StatusCode::UNAUTHORIZED => {
            bail!("{url}: 401 Unauthorized — stopping rather than retrying into brute-force protection")
        }
        StatusCode::SERVICE_UNAVAILABLE => {
            bail!("{url}: 503 Service Unavailable — Nextcloud is likely in maintenance mode")
        }
        other => bail!("{url}: unexpected status {other}"),
    }
}

/// Extract `(path, etag, is_dir)` triples from a WebDAV multistatus response,
/// with paths made relative to the watched root.
fn parse_multistatus(xml: &str, base: &str, root_path: &str) -> Result<Vec<Entry>> {
    let doc = roxmltree::Document::parse(xml).context("parsing PROPFIND response")?;
    let base_path = base
        .split_once("://")
        .map(|(_, rest)| {
            rest.split_once('/')
                .map(|(_, p)| format!("/{p}"))
                .unwrap_or_default()
        })
        .unwrap_or_else(|| base.to_string());
    let prefix = if root_path.is_empty() {
        base_path.clone()
    } else {
        format!("{base_path}/{root_path}")
    };

    let mut entries = Vec::new();
    for response in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "response")
    {
        let href = response
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "href")
            .and_then(|n| n.text())
            .unwrap_or_default();
        let href = percent_decode_str(href).decode_utf8_lossy().to_string();
        let href = href.trim_end_matches('/');

        let rel = match href.strip_prefix(prefix.trim_end_matches('/')) {
            Some(rest) => rest.trim_start_matches('/').to_string(),
            None => continue,
        };

        let etag = response
            .descendants()
            .find(|n| n.is_element() && n.tag_name().name() == "getetag")
            .and_then(|n| n.text())
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();

        let is_dir = response
            .descendants()
            .any(|n| n.is_element() && n.tag_name().name() == "collection");

        entries.push(Entry { rel, etag, is_dir });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESPONSE: &str = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/remote.php/dav/files/publisher/Notes/blog/</d:href>
    <d:propstat><d:prop>
      <d:getetag>"rootetag"</d:getetag>
      <d:resourcetype><d:collection/></d:resourcetype>
    </d:prop></d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/publisher/Notes/blog/bounded%20context.md</d:href>
    <d:propstat><d:prop>
      <d:getetag>"abc123"</d:getetag>
      <d:resourcetype/>
    </d:prop></d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/publisher/Notes/blog/assets/</d:href>
    <d:propstat><d:prop>
      <d:getetag>"dir2"</d:getetag>
      <d:resourcetype><d:collection/></d:resourcetype>
    </d:prop></d:propstat>
  </d:response>
</d:multistatus>"#;

    #[test]
    fn multistatus_yields_relative_paths_and_etags() {
        let entries = parse_multistatus(
            RESPONSE,
            "http://nginx/remote.php/dav/files/publisher",
            "Notes/blog",
        )
        .unwrap();

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].rel, "");
        assert_eq!(entries[0].etag, "rootetag");
        assert!(entries[0].is_dir);

        assert_eq!(
            entries[1].rel, "bounded context.md",
            "href must be percent-decoded"
        );
        assert_eq!(entries[1].etag, "abc123");
        assert!(!entries[1].is_dir);

        assert_eq!(entries[2].rel, "assets");
        assert!(entries[2].is_dir);
    }

    #[test]
    fn spaces_and_umlauts_survive_url_building() {
        let config: Config = toml::from_str(
            r#"
            schema_version = 1
            [source]
            kind = "fs"
            path = "Notizen/Mein Blog"
            "#,
        )
        .unwrap();
        let dav = Webdav {
            client: reqwest::Client::new(),
            base: "http://nginx/remote.php/dav/files/publisher".into(),
            path: config.source.path.clone(),
            host_header: None,
            auth: Auth::Account {
                user: "publisher".into(),
                password: "x".into(),
            },
        };
        assert_eq!(
            dav.url_for("Über Bäume.md"),
            "http://nginx/remote.php/dav/files/publisher/Notizen/Mein%20Blog/%C3%9Cber%20B%C3%A4ume.md",
            "path segments must be percent-encoded, separators must not"
        );
    }

    #[test]
    fn a_share_token_selects_the_public_endpoint() {
        let dir = tempfile::tempdir().unwrap();
        let config: Config = toml::from_str(&format!(
            r#"
            schema_version = 1
            [source]
            kind = "webdav"
            url = "https://cloud.example.org"
            path = ""
            share_token = "abc123XYZ"
            [build]
            kind = "local"
            command = ["true"]
            [paths]
            src = "{d}/src"
            build = "{d}/build"
            state = "{d}/state"
            config_dir = "{d}/etc"
            "#,
            d = dir.path().display()
        ))
        .unwrap();
        config
            .validate()
            .expect("a share token is a complete credential");

        let dav = Webdav::new(&config).unwrap();
        assert_eq!(
            dav.url_for("note.md"),
            "https://cloud.example.org/public.php/dav/files/abc123XYZ/note.md"
        );
        // The share id is the credential: it is both the path segment and the
        // username, which is the part that is easy to get wrong.
        assert!(matches!(
            &dav.auth,
            Auth::Share { token, password: None } if token == "abc123XYZ"
        ));
    }

    #[test]
    fn an_account_and_a_share_token_together_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let secret = dir.path().join("pw");
        std::fs::write(&secret, "x").unwrap();
        let config: Config = toml::from_str(&format!(
            r#"
            schema_version = 1
            [source]
            kind = "webdav"
            url = "https://cloud.example.org"
            path = "Notes"
            user = "publisher"
            password_file = "{secret}"
            share_token = "abc123"
            "#,
            secret = secret.display()
        ))
        .unwrap();
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("pick one"), "{err}");
    }

    #[test]
    fn server_supplied_paths_that_escape_the_working_copy_are_rejected() {
        assert!(is_safe_relative("notes/a.md"));
        assert!(is_safe_relative("Über Bäume.md"));

        assert!(!is_safe_relative("../outside.md"));
        assert!(!is_safe_relative("a/../../etc/cron.d/evil"));
        assert!(!is_safe_relative("/etc/passwd"));
        assert!(!is_safe_relative("C:\\Windows\\system32"));
        assert!(!is_safe_relative(""));
    }

    #[test]
    fn unauthorized_stops_instead_of_retrying() {
        let err = check_status(StatusCode::UNAUTHORIZED, "http://x").unwrap_err();
        assert!(err.to_string().contains("brute-force"));
    }

    #[test]
    fn fs_source_skips_conflict_copies_but_reports_them() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("a.md"), "one").unwrap();
        std::fs::write(
            src.path().join("a (conflicted copy 2026-08-14 120000).md"),
            "two",
        )
        .unwrap();

        let dest = tempfile::tempdir().unwrap();
        let source = Fs {
            root: src.path().to_path_buf(),
        };
        let mut state = State::default();
        let report = source.sync(&dest.path().join("src"), &mut state).unwrap();

        assert_eq!(report.downloaded, 1);
        assert_eq!(report.conflict_copies.len(), 1);
        assert!(!dest
            .path()
            .join("src")
            .join("a (conflicted copy 2026-08-14 120000).md")
            .exists());
    }

    #[test]
    fn fs_source_is_a_no_op_when_nothing_changed() {
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("a.md"), "one").unwrap();
        let dest = tempfile::tempdir().unwrap();
        let target = dest.path().join("src");

        let source = Fs {
            root: src.path().to_path_buf(),
        };
        let mut state = State::default();
        assert!(source.sync(&target, &mut state).unwrap().changed);
        assert!(!source.sync(&target, &mut state).unwrap().changed);
    }
}
