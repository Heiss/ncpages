---
type: Decision Record
title: queue_latest instead of cancelling running builds
description: A running build is never aborted, because an abort between publish and post_publish leaves a state with no clean way back.
tags: [decision, scheduling, concurrency, safety]
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

The legacy GitHub workflow used `cancel-in-progress: true`: a new push kills the
running job. For a stateless CI job that is the obvious optimisation.

# Decision

`on_busy = queue_latest`. At most one running build plus one waiting slot; new
events overwrite the waiting slot. Running builds are never cancelled.

# Rationale

The pipeline's last phase sends irreversible outward effects — webmentions, cache
purges, search pings. An abort between the symlink swap and `post_publish` leaves a
state that cannot be cleanly resumed or rolled back: the site announces a version
whose side effects half-fired, and nothing on disk records which half.

Cancelling was safe in Actions only because Actions had no publish step of its own
worth protecting — and, as it turns out, the workflow's own state handling was
broken anyway. See [Legacy workflow findings](../history/legacy-workflow-findings.md).

# Consequences

* Worst case latency is one full build cycle plus debounce. Acceptable for a blog;
  documented so nobody treats it as a bug.
* Coalescing happens in the waiting slot: a burst of twenty saves produces at most
  one queued build, not twenty.
* The waiting slot stores only "something changed", not a diff. The build always
  works from current state, so a stale queued event cannot resurrect old content.
* This constraint propagates into the hook contract: `post_publish` hooks should
  be re-runnable, because a crashed process is still possible even if a cancel is
  not.
