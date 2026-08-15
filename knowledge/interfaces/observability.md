---
type: Interface Contract
title: Health, status reporting and notifications
description: What replaces the GitHub Actions dashboard — /healthz, a report to the companion app, and push notifications.
tags: [observability, health, reporting, interface]
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

Moving off GitHub Actions costs a dashboard. Actions wrote failures in your face; a
dead systemd service says nothing at all.[^session] Three mechanisms replace it,
deliberately, because none alone is sufficient.

# `/healthz`

Served by the watcher. Reports:

* timestamp and result of the last build,
* seconds since the last source check,
* source status (`ok` / `degraded` / `unreachable`),
* whether a build is currently running or queued.

`degraded` rather than a hard failure when the source is unreachable: the site is
still live and timer builds still run, which is a different situation from "the
service is down".

If `triggers.timer` is configured, "no build for longer than 2 × the timer
interval" is a liveness signal — it means the trigger loop itself is stuck, which
no other check would notice.[^session]

# Reporting

Everything beyond the log and `/healthz` leaves through
[status reporting](status-reporting.md): a companion Nextcloud app when it is
installed, and ntfy for anything that needs a human.

Nothing is written back into the watched folder. An earlier design put a status
file there; it was dropped, because the vault belongs to its author and is
mirrored to every device they own. See
[The watcher never writes to the source](../decisions/no-writes-to-the-source.md).

# What is deliberately absent

No metrics endpoint, no log shipping, no dashboard of its own in v1. A homelab
service that requires an observability stack to be trusted has the wrong shape.
