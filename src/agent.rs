//! The builder-side HTTP agent.
//!
//! The watcher holds the credentials and must not run the build; it also must
//! not be able to start containers, because mounting the Docker socket is
//! equivalent to handing out root on the host. What is left is a small endpoint
//! inside the builder, reachable only on the internal network.

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Serialize;
use tracing::{info, warn};

use crate::config::Config;

#[derive(Clone)]
struct AgentState {
    config: Arc<Config>,
    token: Option<String>,
}

#[derive(Serialize)]
struct BuildResult {
    exit_code: i32,
    duration_ms: u64,
    stdout: String,
    stderr: String,
}

pub async fn run(config: Arc<Config>, listen: String) -> Result<()> {
    let token = config.build_token()?;
    if token.is_none() {
        warn!("no build.token_file configured; the build endpoint is unauthenticated");
    }

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/build", post(build))
        .with_state(AgentState { config, token });

    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .with_context(|| format!("binding {listen}"))?;
    info!(listen = %listen, "build agent listening");
    axum::serve(listener, app)
        .await
        .context("serving build agent")?;
    Ok(())
}

async fn build(State(state): State<AgentState>, headers: HeaderMap) -> Response {
    if let Some(expected) = &state.token {
        let presented = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));
        if presented != Some(expected.as_str()) {
            return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
        }
    }

    // The command is fixed in the image. The request carries no command and no
    // arguments, so nothing in it can become code.
    let Some(command) = state.config.build.command.clone() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "build.command is not configured in the builder",
        )
            .into_response();
    };
    let Some((program, args)) = command.split_first() else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "build.command is empty").into_response();
    };

    let started = std::time::Instant::now();
    let output = tokio::time::timeout(
        state.config.build.timeout,
        tokio::process::Command::new(program)
            .args(args)
            .current_dir(&state.config.paths.build)
            .output(),
    )
    .await;

    let output = match output {
        Err(_) => return (StatusCode::GATEWAY_TIMEOUT, "build timed out").into_response(),
        Ok(Err(e)) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("spawning build: {e}"),
            )
                .into_response()
        }
        Ok(Ok(output)) => output,
    };

    let result = BuildResult {
        exit_code: output.status.code().unwrap_or(-1),
        duration_ms: started.elapsed().as_millis() as u64,
        stdout: tail(&String::from_utf8_lossy(&output.stdout)),
        stderr: tail(&String::from_utf8_lossy(&output.stderr)),
    };

    let code = if output.status.success() {
        StatusCode::OK
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };
    (code, axum::Json(result)).into_response()
}

/// Build logs can be large; the watcher only needs the end of them for its
/// status report.
fn tail(text: &str) -> String {
    const LIMIT: usize = 8 * 1024;
    if text.len() <= LIMIT {
        return text.to_string();
    }
    let start = text.len() - LIMIT;
    let start = text
        .char_indices()
        .map(|(i, _)| i)
        .find(|i| *i >= start)
        .unwrap_or(text.len());
    format!("…{}", &text[start..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_output_is_truncated_from_the_front() {
        let text = "x".repeat(20_000);
        let out = tail(&text);
        assert!(out.len() < text.len());
        assert!(out.starts_with('…'));
    }

    #[test]
    fn short_output_is_untouched() {
        assert_eq!(tail("boom"), "boom");
    }

    #[test]
    fn truncation_does_not_split_a_character() {
        let text = "ä".repeat(20_000);
        let out = tail(&text);
        assert!(out.chars().filter(|c| *c == 'ä').count() > 0);
    }
}
