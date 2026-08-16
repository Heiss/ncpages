//! A mock Nextcloud WebDAV endpoint.
//!
//! It reproduces the one behaviour the whole change-detection design rests on:
//! **ETag propagation**. A directory's ETag is derived from every descendant, so
//! a single `PROPFIND Depth: 0` on the root really does answer "did anything
//! below this change" — and a test can prove that ncpages issues exactly one
//! request when nothing did.
//!
//! It also reproduces the failure modes worth asserting on: 401 (which must stop
//! rather than retry into brute-force protection) and 503 (maintenance mode).

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Router;
use ncpages::config::Config;
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use sha2::{Digest, Sha256};

const PATH_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'?');

pub const USER: &str = "publisher";
pub const ROOT: &str = "Notes/blog";
/// Token of the public share the mock also answers on.
pub const SHARE_TOKEN: &str = "shareXYZ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Ok,
    Unauthorized,
    Maintenance,
    /// Accepts only the literal user `anonymous`, the way newer Nextcloud
    /// versions document the public endpoint. Anything else gets a 401.
    AnonymousOnly,
}

#[derive(Debug, Clone)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub depth: Option<String>,
    pub host: Option<String>,
    pub authorization: bool,
    /// Username from the Basic header. For a share link this must be the share
    /// id — the part that is easy to get wrong and impossible to guess from a
    /// 401.
    pub basic_user: Option<String>,
}

#[derive(Default)]
struct Inner {
    files: BTreeMap<String, String>,
    requests: Vec<RecordedRequest>,
    mode: Option<Mode>,
    /// A raw href injected into every Depth:1 listing, to play the part of a
    /// hostile or compromised server.
    poison: Option<String>,
}

#[derive(Clone)]
struct AppState(Arc<Mutex<Inner>>);

pub struct MockNextcloud {
    pub url: String,
    inner: Arc<Mutex<Inner>>,
}

impl MockNextcloud {
    /// Start on an ephemeral port with an initial set of files, keyed by path
    /// relative to the watched root.
    pub async fn start(files: &[(&str, &str)]) -> Self {
        let inner = Arc::new(Mutex::new(Inner {
            files: files
                .iter()
                .map(|(path, body)| (path.to_string(), body.to_string()))
                .collect(),
            requests: Vec::new(),
            mode: Some(Mode::Ok),
            poison: None,
        }));

        let app = Router::new()
            .fallback(handle)
            .with_state(AppState(inner.clone()));

        let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .expect("binding mock nextcloud");
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        Self {
            url: format!("http://{addr}"),
            inner,
        }
    }

    pub fn write(&self, path: &str, body: &str) {
        self.inner
            .lock()
            .unwrap()
            .files
            .insert(path.into(), body.into());
    }

    pub fn delete(&self, path: &str) {
        self.inner.lock().unwrap().files.remove(path);
    }

    /// Make the server answer with a path that escapes the working copy.
    pub fn poison_listing_with(&self, href: &str) {
        self.inner.lock().unwrap().poison = Some(href.to_string());
    }

    pub fn set_mode(&self, mode: Mode) {
        self.inner.lock().unwrap().mode = Some(mode);
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.inner.lock().unwrap().requests.clone()
    }

    pub fn reset_requests(&self) {
        self.inner.lock().unwrap().requests.clear();
    }

    pub fn request_count(&self, method: &str) -> usize {
        self.requests()
            .iter()
            .filter(|r| r.method == method)
            .count()
    }

    /// A config wired to this mock, with the password in a file as production
    /// does it.
    pub fn config(&self, workdir: &Path) -> Config {
        let secret = workdir.join("password");
        std::fs::write(&secret, "app-password\n").unwrap();
        let toml = format!(
            r#"
            schema_version = 1

            [source]
            kind = "webdav"
            url = "{url}"
            host_header = "cloud.example.org"
            path = "{root}"
            user = "{user}"
            password_file = "{secret}"

            [paths]
            src = "{work}/src"
            build = "{work}/build"
            state = "{work}/state"
            config_dir = "{work}/etc"

            [build]
            kind = "local"
            command = ["true"]

            [publish]
            root = "{work}/publish"
            "#,
            url = self.url,
            root = ROOT,
            user = USER,
            secret = secret.display(),
            work = workdir.display(),
        );
        toml::from_str(&toml).expect("mock config parses")
    }
}

impl MockNextcloud {
    /// The same content, reached through a public share link instead of an
    /// account: no user, no password, just the token.
    pub fn share_config(&self, workdir: &Path) -> Config {
        let toml = format!(
            r#"
            schema_version = 1

            [source]
            kind = "webdav"
            url = "{url}"
            path = ""
            share_token = "{token}"

            [paths]
            src = "{work}/src"
            build = "{work}/build"
            state = "{work}/state"
            config_dir = "{work}/etc"

            [build]
            kind = "local"
            command = ["true"]

            [publish]
            root = "{work}/publish"
            "#,
            url = self.url,
            token = SHARE_TOKEN,
            work = workdir.display(),
        );
        toml::from_str(&toml).expect("share config parses")
    }
}

/// Decode the username out of a Basic authorization header.
fn basic_user(headers: &HeaderMap) -> Option<String> {
    use base64::Engine;
    let raw = headers.get("authorization")?.to_str().ok()?;
    let encoded = raw.strip_prefix("Basic ")?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    let text = String::from_utf8(decoded).ok()?;
    Some(
        text.split_once(':')
            .map(|(user, _)| user)
            .unwrap_or(&text)
            .to_string(),
    )
}

async fn handle(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let path = percent_decode_str(uri.path())
        .decode_utf8_lossy()
        .to_string();

    {
        let mut inner = state.0.lock().unwrap();
        inner.requests.push(RecordedRequest {
            method: method.to_string(),
            path: path.clone(),
            depth: headers
                .get("depth")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string),
            host: headers
                .get("host")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string),
            authorization: headers.contains_key("authorization"),
            basic_user: basic_user(&headers),
        });

        if inner.mode == Some(Mode::AnonymousOnly)
            && basic_user(&headers).as_deref() != Some("anonymous")
        {
            return (StatusCode::UNAUTHORIZED, "use anonymous").into_response();
        }

        match inner.mode {
            Some(Mode::Unauthorized) => {
                return (StatusCode::UNAUTHORIZED, "invalid credentials").into_response()
            }
            Some(Mode::Maintenance) => {
                return (StatusCode::SERVICE_UNAVAILABLE, "maintenance").into_response()
            }
            _ => {}
        }
    }

    // The mock answers on both endpoints: the account one and the public share.
    let account = format!("/remote.php/dav/files/{USER}/{ROOT}");
    let share = format!("/public.php/dav/files/{SHARE_TOKEN}");
    let (prefix, rel) = if let Some(rel) = path.strip_prefix(&account) {
        (account.clone(), rel)
    } else if let Some(rel) = path.strip_prefix(&share) {
        (share.clone(), rel)
    } else {
        return (StatusCode::NOT_FOUND, "outside the watched root").into_response();
    };
    let rel = rel.trim_matches('/').to_string();

    let files = state.0.lock().unwrap().files.clone();

    if method.as_str() == "PROPFIND" {
        let depth = headers
            .get("depth")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("0")
            .to_string();
        let poison = state.0.lock().unwrap().poison.clone();
        return multistatus(&files, &rel, &depth, poison.as_deref(), &prefix);
    }

    if method == Method::GET {
        // Serve the poisoned path too, so a missing traversal guard results in a
        // file actually landing outside the destination rather than a 404.
        if state.0.lock().unwrap().poison.as_deref() == Some(rel.as_str()) {
            return (StatusCode::OK, "escaped!").into_response();
        }
        return match files.get(&rel) {
            Some(body) => (StatusCode::OK, body.clone()).into_response(),
            None => (StatusCode::NOT_FOUND, "no such file").into_response(),
        };
    }

    (StatusCode::METHOD_NOT_ALLOWED, "unsupported").into_response()
}

fn multistatus(
    files: &BTreeMap<String, String>,
    target: &str,
    depth: &str,
    poison: Option<&str>,
    prefix: &str,
) -> Response {
    let is_dir = target.is_empty() || files.keys().any(|p| p.starts_with(&format!("{target}/")));
    if !is_dir && !files.contains_key(target) {
        return (StatusCode::NOT_FOUND, "no such collection").into_response();
    }

    let mut body = String::from(r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:">"#);
    body.push_str(&entry(prefix, target, &etag_for(files, target), is_dir));

    if depth == "1" && is_dir {
        for (name, dir) in children(files, target) {
            let path = if target.is_empty() {
                name.clone()
            } else {
                format!("{target}/{name}")
            };
            body.push_str(&entry(prefix, &path, &etag_for(files, &path), dir));
        }
    }
    if depth == "1" {
        if let Some(href) = poison {
            body.push_str(&format!(
                "<d:response><d:href>{prefix}/{href}</d:href>\
                 <d:propstat><d:prop><d:getetag>\"poison\"</d:getetag>\
                 <d:resourcetype/></d:prop></d:propstat></d:response>"
            ));
        }
    }
    body.push_str("</d:multistatus>");

    Response::builder()
        .status(207)
        .header("content-type", "application/xml")
        .body(Body::from(body))
        .unwrap()
}

/// Immediate children of `dir`: `(name, is_dir)`.
fn children(files: &BTreeMap<String, String>, dir: &str) -> Vec<(String, bool)> {
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    let mut seen: BTreeMap<String, bool> = BTreeMap::new();
    for path in files.keys() {
        let Some(rest) = path.strip_prefix(&prefix) else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        match rest.split_once('/') {
            Some((child, _)) => {
                seen.insert(child.to_string(), true);
            }
            None => {
                seen.insert(rest.to_string(), false);
            }
        }
    }
    seen.into_iter().collect()
}

/// The behaviour that makes a single root request sufficient: a collection's
/// ETag changes whenever anything beneath it changes.
fn etag_for(files: &BTreeMap<String, String>, target: &str) -> String {
    if let Some(body) = files.get(target) {
        return short_hash(body.as_bytes());
    }
    let prefix = if target.is_empty() {
        String::new()
    } else {
        format!("{target}/")
    };
    let mut hasher = Sha256::new();
    for (path, body) in files {
        if path.starts_with(&prefix) {
            hasher.update(path.as_bytes());
            hasher.update(body.as_bytes());
        }
    }
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn short_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn entry(prefix: &str, rel: &str, etag: &str, is_dir: bool) -> String {
    let encoded: String = rel
        .split('/')
        .map(|segment| utf8_percent_encode(segment, PATH_SET).to_string())
        .collect::<Vec<_>>()
        .join("/");
    let href = if rel.is_empty() {
        format!("{prefix}/")
    } else if is_dir {
        format!("{prefix}/{encoded}/")
    } else {
        format!("{prefix}/{encoded}")
    };
    let resourcetype = if is_dir { "<d:collection/>" } else { "" };
    format!(
        "<d:response><d:href>{href}</d:href><d:propstat><d:prop>\
         <d:getetag>\"{etag}\"</d:getetag>\
         <d:resourcetype>{resourcetype}</d:resourcetype>\
         </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>"
    )
}
