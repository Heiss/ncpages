---
type: Decision Record
title: The watcher never writes to the source
description: Nextcloud is read-only to ncpages; status reporting moves to a companion app instead of a file in the vault.
tags: [decision, security, reporting, nextcloud, obsidian]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-15T18:10:00Z }
sources:
  - id: operator
    resource: ../history/design-session-transcript.md
    title: Operator decision, 2026-08-15 (supersedes the session on this point)
    author: human:heiss
    last_modified: 2026-08-15
---

# Context

The design session specified a status file written back over WebDAV, so build
results would be visible from the same place the content is edited — including
from a phone. It came with a known hazard: writing anything below the watched
root changes its ETag, which triggers a build, which writes the status again. The
mitigations were a sibling folder plus a fingerprint of the service's own last
write.

# Decision

Drop it. **The source is read-only to ncpages.** It syncs down; nothing goes back
up. Reports go to a [companion Nextcloud app](../interfaces/status-reporting.md)
over HTTP, or to ntfy, or nowhere.

# Rationale

The loop was only the visible symptom. The deeper problem: the vault is the
author's workspace and the single source of truth, and it is mirrored into
Obsidian on every device they own. A status file there means a machine writing
into a space that should only ever be touched by its owner or their editor. The
sync side effects of that are not fully predictable — conflict copies, versions,
selective-sync surprises — and none of them are ncpages' business.

The mitigations also proved the point rather than solving it. Needing a
fingerprint of your own writes to avoid reacting to yourself is a sign the
direction of data flow is wrong.

A separate app is strictly better as a delivery mechanism: it has a real UI, it
can present history and diagrams, and it cannot corrupt anyone's notes.

# Consequences

* One less write path, one less credential scope, one less failure mode. The
  source password could in principle be a read-only share.
* Reporting becomes an optional rung rather than a built-in: log and `/healthz`
  always, ntfy if you want to be told, the app if you want a UI.
* The app is a separate project with its own release cadence and its own place in
  the Nextcloud app store. ncpages must therefore treat it as absent by default,
  which the `OPTIONS` probe does.
* `report.webdav_status_path` is gone from the configuration surface, along with
  the validation that kept it outside `source.path`.
* The self-trigger hazard disappears entirely rather than being managed. That is
  the kind of fix worth preferring: no code, no check, no failure mode.
