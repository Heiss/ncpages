---
type: Decision Record
title: Use notify_push, not the webhook_listeners app
description: Push notification comes from Nextcloud's Redis-backed WebSocket service; the webhook app is slower than plain polling.
tags: [decision, notify-push, trigger, nextcloud, latency]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-15T00:45:00Z }
sources:
  - id: session
    resource: ../history/design-session-transcript.md
    title: ncpages design session, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
---

# Context

Polling at 30 s is fine but not satisfying. The wish is to be told by Nextcloud
rather than to ask it.

# Decision

Use **notify_push**: a Rust service that turns Redis pub/sub into a WebSocket
stream, with roughly one second of latency. It says only *that* something changed,
never *what* — which fits perfectly: the WebSocket wakes the watcher, and the ETag
check decides whether the change was real.

If the connection drops, the service falls back to polling, which never stopped
running anyway.

# Why not webhook_listeners

It sounds like the right tool and is not. It fires through Nextcloud's background
jobs, whose default cron interval is five minutes. Getting it faster requires
several dedicated `occ` worker processes. That is more moving parts and *worse*
latency than plain polling.

# Consequences

* **No proxy path needed for ncpages itself.** notify_push validates client
  credentials against Nextcloud, so the watcher can connect directly on the docker
  network (`ws://notify-push:7867/ws`). The `/push` location in nginx is only
  required for real desktop and mobile clients.
* Redis becomes a requirement on the Nextcloud side — already present in the
  reference deployment.
* `trusted_proxies` must contain the nginx container's subnet, or the notify_push
  self-test fails. This is the single most common reason for a non-working setup.
* nginx needs `map $http_upgrade $connection_upgrade` in `http{}` and a
  `location ^~ /push/` block with upgrade headers and a long `proxy_read_timeout`.
  `^~` matters so the regex locations of the stock Nextcloud config do not
  intercept it; the `map` matters because notify_push also serves ordinary HTTP
  endpoints under `/test/*` and `/metrics`.
* Push is an accelerator, never a dependency. See
  [Trigger composition](trigger-composition.md).
