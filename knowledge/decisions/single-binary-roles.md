---
type: Decision Record
title: One binary with roles; serving runs in-process by default
description: The web server is a task inside the ncpages binary rather than a separate image, with the split into separate containers preserved as a deployment option.
tags: [decision, architecture, rust, serving, deployment]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-15T01:45:00Z }
sources:
  - id: operator
    resource: ../history/design-session-transcript.md
    title: Operator decision, 2026-08-15 (supersedes the session on this point)
    author: human:heiss
    last_modified: 2026-08-15
---

# Context

The design session put serving in a separate container (`static-web-server`),
explicitly without `depends_on`, so the site stays live when watcher and builder
fail. That is a real availability property — and it costs a second image, a second
mount of the same volume, and a second thing to configure.

Rust serves static files perfectly well on its own (`tower-http::ServeDir` over
`axum`/`hyper`: range requests, conditional requests, sendfile-style I/O). The
serving path is a handful of syscalls against `current/`.

# Decision

**One binary, several roles.**

| Command | Runs |
|---|---|
| `ncpages run` | watcher, scheduler and HTTP server in one process — the default |
| `ncpages watch` | watcher and scheduler only |
| `ncpages serve` | HTTP server only |
| `ncpages build-agent` | the builder-side HTTP agent |
| `ncpages doctor` | diagnostics |

`run` is the homelab default: one container, one config file, one port. The split
roles exist for anyone who wants the original isolation, and are what the reference
deployment uses if it later wants it back.

# Concurrency

The server is a Tokio task, not an OS thread, and nothing in the build path blocks
it:

* generator runs are subprocesses (the builder is a separate container anyway),
* local filesystem work runs on the blocking pool,
* the only shared state between serving and publishing is the `current` symlink,
  and swapping it is a single atomic `rename(2)`.

An in-flight request reading through the old symlink keeps its already-opened file
handles; the swap does not disturb it. This is the same property that makes the
separate-container variant safe.

# Consequences accepted

* **Availability couples.** A panic in the watcher takes the site down, which the
  separate web container was specifically designed to prevent. Mitigations:
  `restart: always`, no `unwrap` in the trigger loop, and the build itself living
  in another container. Anyone who finds this unacceptable runs `serve` and `watch`
  separately — the reason the roles exist.
* **Credentials share a process with a public-facing handler.** The static file
  handler is the only public surface; it is a well-audited library path, and it
  serves one directory. Still a real narrowing of the original blast radius, and it
  is why the split remains a first-class option rather than dead code.
* **The builder stays separate regardless.** That boundary carries the actual
  security argument — no egress, no secrets, read-only root. See
  [Watcher/builder split](watcher-builder-split.md).

# Consequences gained

* One artifact to build, sign, and publish multi-arch; one version number.
* `doctor` can check the serving configuration directly, in-process, instead of
  inferring it from another container's config.
* The bootstrap holding page needs no special case — the server is already there
  when the first sync starts.
