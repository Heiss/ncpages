---
type: Interface Contract
title: Health, status reporting and notifications
description: What replaces the GitHub Actions dashboard — /healthz, a status file written back to Nextcloud, and push notifications.
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

# Status file in Nextcloud

A markdown file written back over WebDAV after every run, so the state of the site
is visible from the same place the content is edited — including from a phone.

Contents: trigger, duration, result per phase, gate outcome, hook warnings
(exit code `1`), orphan pages, conflict copies found, current release id.

**Its path must be outside `source.path`.** See
[Security model](../architecture/security-model.md) for the loop this prevents, and
the fingerprint that guards it a second time.

# Push notification

An ntfy topic for failures and gate violations. This is the only channel that
reaches the operator when they are not looking, which is the normal case for a
personal blog that publishes fine for months.

# What is deliberately absent

No metrics endpoint, no log shipping, no dashboard of its own in v1. A homelab
service that requires an observability stack to be trusted has the wrong shape.
