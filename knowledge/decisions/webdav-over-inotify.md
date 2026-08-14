---
type: Decision Record
title: Detect changes over WebDAV, not with inotify
description: Change detection uses a single PROPFIND against the root folder's ETag instead of filesystem events.
tags: [decision, webdav, trigger, nextcloud]
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

The obvious starting point for "watch a folder" is inotify on the Nextcloud data
directory. It is also the reflex that leads past the actual problem.

# Decision

Detect changes over WebDAV. Nextcloud propagates ETag changes up the directory
tree — which is why the desktop client can decide with a handful of requests where
it needs to descend. For a watcher that means:

```
PROPFIND /remote.php/dav/files/<user>/<path>
Depth: 0
<d:prop><d:getetag/></d:prop>
```

One HTTP request, one string comparison, and you know whether anything below that
folder changed. Then descend with `Depth: 1` only where needed.

# Why not inotify

It breaks three ways, each of them silently:

* **server-side encryption** — the bytes on disk are encrypted blobs; you cannot
  build from them,
* **S3 primary storage** — there are no files in the filesystem at all,
* **group folders and external storage** — different ETag and event behaviour.

Plus the ordinary problems: not recursive, `max_user_watches` limits, events lost
across restarts, and a sync client that writes through `.part` files and `MOVE`
operations, so the event stream does not resemble the logical change.

# Consequences

* Works regardless of how and where Nextcloud stores the data. This is what makes
  ncpages meaningfully different from `watchexec` plus a shell script.
* Polling is not the compromise it appeared to be — it is one cheap request per
  interval, and it is the *better* mechanism here.
* `fs` remains a supported source kind for purely local setups, using the `notify`
  crate with a debouncer, feeding the same event channel.
* ETag propagation is a Nextcloud behaviour, not a WebDAV guarantee. External
  storage and some group folder configurations do not propagate reliably; this is
  a check that belongs in [`ncpages doctor`](../operations/doctor-checks.md).
* Timestamps are never used for change detection — they are unreliable across sync
  boundaries. ETag and content hash only.
