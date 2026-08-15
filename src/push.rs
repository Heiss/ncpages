//! notify_push client.
//!
//! notify_push turns Nextcloud's Redis pub/sub into a WebSocket stream with about
//! a second of latency. It says only *that* something changed, never *what* —
//! which is exactly the right shape here: the socket wakes the watcher, and the
//! ETag check decides whether the change was real.
//!
//! It is an accelerator, never a dependency. The poll keeps running, so a dropped
//! socket costs latency rather than correctness.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, info, warn};

/// Messages notify_push sends that mean "files changed".
const FILE_EVENTS: [&str; 2] = ["notify_file", "notify_storage_update"];

/// Connect, authenticate and forward file events until the connection drops,
/// then reconnect with backoff. Never returns while the runtime is alive.
pub async fn run(url: String, user: String, password: String, events: mpsc::Sender<()>) {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(60);

    loop {
        match session(&url, &user, &password, &events).await {
            Ok(()) => {
                info!("notify_push connection closed; reconnecting");
                backoff = Duration::from_secs(1);
            }
            Err(e) => {
                warn!(error = %e, retry_in = ?backoff, "notify_push connection failed");
            }
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(max_backoff);
    }
}

async fn session(url: &str, user: &str, password: &str, events: &mpsc::Sender<()>) -> Result<()> {
    let (mut socket, _) = tokio_tungstenite::connect_async(url)
        .await
        .with_context(|| format!("connecting to {url}"))?;

    // notify_push authenticates by taking the username and password as two
    // separate messages, and validates them against Nextcloud itself. That is
    // why the watcher can talk to it directly on the internal network and does
    // not need the reverse-proxied /push path, which exists for desktop and
    // mobile clients.
    socket.send(Message::Text(user.into())).await?;
    socket.send(Message::Text(password.into())).await?;

    let mut authenticated = false;
    while let Some(message) = socket.next().await {
        let message = message.context("reading from notify_push")?;
        let text = match message {
            Message::Text(text) => text.to_string(),
            Message::Ping(_) | Message::Pong(_) => continue,
            Message::Close(_) => return Ok(()),
            _ => continue,
        };
        let text = text.trim();

        if !authenticated {
            if text == "authenticated" {
                authenticated = true;
                info!("notify_push connected");
                continue;
            }
            bail!("notify_push refused the credentials: {text}");
        }

        if FILE_EVENTS.contains(&text) {
            debug!(event = text, "notify_push event");
            // A full channel already means "a build is pending"; dropping the
            // event loses nothing, because the event carries no information
            // beyond its own existence.
            let _ = events.try_send(());
        }
    }

    Ok(())
}

/// notify_push exposes an HTTP self-test that reports the problems most
/// deployments actually hit — wrong `trusted_proxies` above all.
pub async fn self_test(base_url: &str) -> Result<String> {
    let url = format!("{}/test/cookie", base_url.trim_end_matches('/'));
    let body = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .text()
        .await?;
    Ok(body.trim().to_string())
}

/// `ws://host:port/ws` → `http://host:port`, for the self-test endpoint.
pub fn http_base(ws_url: &str) -> String {
    let base = ws_url
        .trim_end_matches("/ws")
        .replacen("wss://", "https://", 1)
        .replacen("ws://", "http://", 1);
    base.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_urls_map_to_their_http_origin() {
        assert_eq!(
            http_base("ws://notify-push:7867/ws"),
            "http://notify-push:7867"
        );
        assert_eq!(
            http_base("wss://cloud.example.org/push/ws"),
            "https://cloud.example.org/push"
        );
    }

    #[test]
    fn only_file_events_are_treated_as_changes() {
        assert!(FILE_EVENTS.contains(&"notify_file"));
        assert!(!FILE_EVENTS.contains(&"notify_notification"));
        assert!(!FILE_EVENTS.contains(&"authenticated"));
    }
}
