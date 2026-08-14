---
type: Interface Contract
title: Builder HTTP API
description: The internal, token-authenticated endpoint the watcher uses to run a build in the isolated container.
tags: [builder, api, interface, isolation]
status: draft
generated: { by: claude-code/opus-5, at: 2026-08-15T00:45:00Z }
sources:
  - id: session
    resource: ../history/design-session-transcript.md
    title: ncpages design session, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
  - id: concept
    resource: ../history/original-concept-note.md
    title: ncpages concept note, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
---

The watcher cannot run the build itself — it holds the credentials, and the build
must not. It cannot start a container either, because that would require the Docker
socket, which is equivalent to root on the host. What is left is a small HTTP agent
inside the builder.[^session]

# Endpoints

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/build` | run the configured build command in the assembled tree |
| `GET` | `/healthz` | readiness, image version |

Authentication is a shared token, passed as a header, sourced from a file in both
containers. The endpoint is only reachable on the internal compose network; the
token is defence in depth, not the primary control.

# Semantics

* **Synchronous.** `POST /build` returns when the build finishes or the timeout
  (`build.timeout`) expires. The watcher already serialises builds, so there is no
  queue on this side.
* **Stateless.** The builder keeps nothing between runs. All state lives on the
  shared volumes and in the watcher.
* **One command.** The agent does not accept a command from the request. What runs
  is fixed in the image; the request carries no code and no arguments that could
  become code.
* **Response** carries exit code, duration, and captured stdout/stderr (truncated),
  which the watcher folds into the status report.

# Constraints this interface must preserve

* no egress from the builder (`internal: true`),
* no secrets in the builder's environment,
* same UID as the watcher, so files written to `releases/` stay writable,
* writable `/tmp` via tmpfs, since the root filesystem is read-only.

See [Watcher/builder split](../decisions/watcher-builder-split.md) for why this
boundary exists at all.

[^session]: Design session, parts 1.5 and 2.3.
