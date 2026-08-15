//! Serving the current release, and the health endpoint.
//!
//! The server opens files through the `current` symlink on every request, so a
//! swap takes effect immediately. This is the reason a container deployment must
//! mount the *parent* directory: Docker resolves a bind-mounted symlink once, at
//! container start, and every later swap would be invisible.

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Serialize;
use tokio::sync::RwLock;
use tower_http::services::ServeDir;
use tracing::info;

use crate::config::Config;

#[derive(Debug, Default, Clone, Serialize)]
pub struct Health {
    pub source: String,
    /// `ok`, `degraded` (source unreachable, site still served) or `starting`.
    pub source_status: String,
    pub last_check_seconds_ago: Option<u64>,
    pub last_build_finished: Option<String>,
    pub last_result: Option<String>,
    pub last_release: Option<String>,
    pub building: bool,
    pub queued: bool,
    /// Set when a timer is configured and no build has happened for twice its
    /// interval — the trigger loop itself is then suspect.
    pub liveness_overdue: bool,
}

pub type SharedHealth = Arc<RwLock<Health>>;

pub async fn serve_site(config: Arc<Config>) -> Result<()> {
    let root = config.publish.current();
    let assets = config.serve.cache_control_assets.clone();
    let html = config.serve.cache_control_html.clone();

    let service = ServeDir::new(&root)
        .append_index_html_on_directories(true)
        .call_fallback_on_method_not_allowed(true);

    let app = Router::new()
        .fallback_service(service)
        .layer(axum::middleware::from_fn(
            move |req: Request, next: Next| {
                let assets = assets.clone();
                let html = html.clone();
                async move {
                    let path = req.uri().path().to_string();
                    let mut response = next.run(req).await;
                    let value = if is_immutable_asset(&path) {
                        &assets
                    } else {
                        &html
                    };
                    if let Ok(value) = HeaderValue::from_str(value) {
                        response.headers_mut().insert(header::CACHE_CONTROL, value);
                    }
                    response
                }
            },
        ));

    let listener = tokio::net::TcpListener::bind(&config.serve.listen)
        .await
        .with_context(|| format!("binding {}", config.serve.listen))?;
    info!(listen = %config.serve.listen, root = %root.display(), "serving");
    axum::serve(listener, app).await.context("serving site")?;
    Ok(())
}

pub async fn serve_health(config: Arc<Config>, health: SharedHealth) -> Result<()> {
    let app = Router::new()
        .route("/healthz", get(healthz))
        .with_state(health);

    let listener = tokio::net::TcpListener::bind(&config.health.listen)
        .await
        .with_context(|| format!("binding {}", config.health.listen))?;
    info!(listen = %config.health.listen, "health endpoint");
    axum::serve(listener, app).await.context("serving health")?;
    Ok(())
}

async fn healthz(State(health): State<SharedHealth>) -> Response {
    let health = health.read().await.clone();
    // Degraded is not unhealthy: the source being unreachable still leaves a
    // correct site being served, which is a different situation from a crash.
    let code = if health.source_status == "unreachable" && health.last_release.is_none() {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };
    (code, axum::Json(health)).into_response()
}

/// Generators emit content-hashed asset names, which are safe to cache forever.
/// Everything else must not be cached, or a swap leaves old HTML referencing
/// asset names that no longer exist.
fn is_immutable_asset(path: &str) -> bool {
    path.contains("/assets/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_asset_paths_are_immutable() {
        assert!(is_immutable_asset("/assets/stylesheets/main.a1b2c3.css"));
        assert!(!is_immutable_asset("/index.html"));
        assert!(!is_immutable_asset("/"));
        assert!(!is_immutable_asset("/some-post/"));
    }
}
