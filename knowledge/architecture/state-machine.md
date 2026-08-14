---
type: State Machine
title: Scheduler state machine
description: How trigger events become at most one running build plus one waiting slot, including debounce, hard deadline, persistence and startup reconcile.
tags: [scheduler, state-machine, debounce, architecture]
status: stable
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

Watching a directory is a solved problem — `watchexec` and the `notify` crate do it
in a few lines. What does not exist off the shelf, and what carries the value of
ncpages, is the state machine behind it: debounce, coalescing queue, build sandbox,
quality check, atomic publish, status report.[^session]

# States

```
Idle → Dirty{deadline, hard_deadline} → Fetch → Assemble → Build → Gate → Publish → Idle
```

All trigger sources feed one event channel and are individually switchable. A
trigger event moves `Idle → Dirty` and arms two timers:

* **debounce** (default 10 s) — reset on every further event,
* **hard deadline** (default 120 s) — never reset.

A vault that is being edited continuously therefore still gets built. Obsidian
autosaves every few seconds and a rename with link updates rewrites dozens of
files, so the debounce is not optional comfort — without it every keystroke burst
becomes a build.[^session]

# Busy policy

`on_busy = queue_latest`: at most one running build plus one waiting slot. New
events overwrite the waiting slot; they never cancel the running build. The reason
is the irreversibility of `post_publish`, not efficiency. See
[queue_latest over cancel](../decisions/queue-latest-over-cancel.md).

# Persistence

The last ETag, the last content hash and the build history are persisted to the
`state` volume. Without persistence every `compose up` causes a full rebuild, and —
worse — the reconcile logic stays untested until the day it is actually needed.[^session]

# Startup reconcile

On start the service compares persisted state against reality:

* no `current` symlink → create a holding-page release, so the web server does not
  answer 404 to everything during the first sync,
* stored ETag differs from the live ETag → schedule a build,
* source unreachable and `source.required = false` → start anyway, report
  `degraded`, keep serving.

# Error handling in the trigger loop

* **HTTP 503** (Nextcloud maintenance mode) → exponential backoff, not a hot loop.
* **HTTP 401** → stop immediately. Retrying against Nextcloud's brute-force
  protection makes recovery harder, not easier.
* **WebSocket drop** → reconnect with backoff; the poll keeps running throughout,
  which is exactly why it stays enabled when push is active.

[^session]: Design session, parts 1.2, 2.2 and 3.2.
