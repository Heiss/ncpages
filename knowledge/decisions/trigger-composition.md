---
type: Decision Record
title: Three trigger sources into one channel
description: Push, poll and timer are independent, individually switchable sources feeding a single event channel; the timer is functionally required whenever a build pulls external data.
tags: [decision, triggers, timer, scheduling]
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

A change-triggered builder seems complete: content changes, site rebuilds. The
legacy workflow also ran on a 12-hour cron, which looked like redundancy.

# Decision

Three sources, one event channel, each individually switchable:

| Source | Interval | Role |
|---|---|---|
| notify_push | ~1 s | accelerator |
| WebDAV ETag poll | 30 s | safety net, always on |
| timer | 6 h, optional | pulls external state |

# Why the timer is not redundant

The legacy cron was **functionally required**, not belt-and-braces. A hook that
fetches incoming comments or webmentions only runs when a build runs. Purely
change-triggered means: someone comments, and their comment appears when the author
next publishes an article. Possibly weeks later.

Anyone whose build does not reach outward can switch the timer off.

# Why the poll stays on when push is active

A WebSocket that dies quietly would otherwise mean a site that stops updating
quietly. The poll interval may be widened when push is healthy, but it does not
stop.

# Consequences

* **Jitter (0–10 %)** on the timer, so installations do not all hit the same
  external APIs in the same second.
* With a timer configured, "no build in 2 × the interval" becomes a liveness
  signal for `/healthz` — a free deadlock detector that the other two sources
  cannot provide.
* Every hook must be idempotent: timer builds re-run the whole pipeline against
  unchanged content.
* `NCPAGES_TRIGGER` tells hooks which source fired, so an expensive external fetch
  can be limited to `timer` and `manual` runs.
