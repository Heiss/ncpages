---
type: Playbook
title: ncpages doctor — diagnostic checks
description: The red-team list turned into executable checks, so a broken deployment diagnoses itself instead of becoming an issue.
tags: [operations, diagnostics, doctor, support]
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

`ncpages doctor` runs every check below and prints a report. The issue template
requires its output. This is the single highest-leverage feature if the project is
published: most issues will be foreign deployments, not bugs in the code.

Each check states what it verified, what it found, and what to do — a check that
only prints `FAIL` moves the work back to the maintainer.

# Source

| Check | Detects |
|---|---|
| WebDAV reachable at `source.url` with `host_header` | wrong URL, FPM container instead of HTTP frontend |
| Credentials valid, app password not a login password | 401 loops, brute-force lockout |
| `source.path` exists and is readable | typo, wrong user's namespace |
| **ETag propagation works** | external storage and some group folder setups do not propagate — the watcher would never trigger |
| Content hash of the working copy matches the remote | half-synced state, silent divergence |

The ETag propagation check is the important one: write a file deep in the tree,
watch the root ETag change, delete the file. If it does not propagate, WebDAV
polling cannot work in this deployment, and the user needs to know that before
anything else.

# Push path

| Check | Detects |
|---|---|
| `ws://notify-push:7867/ws` reachable from the watcher | missing bridge network, wrong service name |
| notify_push self-test passes | `trusted_proxies` missing the nginx subnet |
| Proxy `location ^~ /push/` present with upgrade headers | broken desktop/mobile clients (not ncpages itself) |
| Redis reachable from notify_push | push silently degraded to poll-only |

# Isolation and layout

| Check | Detects |
|---|---|
| **Config overlap**: hook directory inside `source.path` | the fail-closed condition — refuses startup |
| Builder has no egress | `internal: true` missing |
| Builder UID equals watcher UID | `EACCES` on the second build |
| Builder `/tmp` writable under `read_only: true` | build fails at import time |
| Build tree and `releases/` on the same volume | non-atomic move |

# Publish and serve

| Check | Detects |
|---|---|
| `current` exists and resolves | bootstrap state, missing holding page |
| Web server mounts the **parent** of the symlink | site that never updates |
| Open-file caching disabled | stale content after a swap |
| Caching headers: `immutable` for hashed assets, `no-cache` for HTML | old HTML with new CSS |
| `base_url` matches the incoming `Host` header | broken absolute links, broken webmentions |
| Free space on the `releases` volume vs. retention | filling the root filesystem |

# Reporting

| Check | Detects |
|---|---|
| `report.webdav_status_path` outside `source.path` | infinite build loop |
| ntfy topic reachable | silent failures |
| `/healthz` reachable and current | dead trigger loop |
