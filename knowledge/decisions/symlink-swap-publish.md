---
type: Decision Record
title: Symlink swap is the only publish backend
description: Publishing is rename(2) on a symlink within one filesystem — the only mechanism that is genuinely atomic.
tags: [decision, publish, atomicity, filesystem]
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

Two guarantees of the pipeline depend on a well-defined instant at which a build
becomes live: the [gate](../interfaces/quality-gate.md) (nothing half-published)
and `post_publish` (irreversible effects announce a state that actually exists).

# Decision

One publish backend: `kind = "symlink"`. Move the finished output into
`releases/<id>/`, then repoint `current` with `rename(2)` on the same filesystem.

# Rationale

`rename(2)` within one filesystem is atomic — there is no instant at which a
request sees half a site. Nothing over a network protocol provides this. Offering
an rsync or WebDAV publish backend "for flexibility" would mean shipping a mode in
which the two guarantees above silently do not hold.

# Consequences

* The build tree must live on the **same volume** as `releases/`, so step 6 is a
  move, not a copy.
* The web server must mount the **parent directory** and resolve the symlink
  itself. Mounting the symlink makes Docker bind its current target at container
  start, and every later swap becomes invisible: the site never updates, with no
  error and no log line.
* Open-file caching in the web server must be off, for the same class of reason.
* `releases/` replaces the build cache: `NCPAGES_PREV_DIR` is `readlink current`.
  Retention of five gives rollback as a side effect.
* Retention must be enforced actively — a full root filesystem takes Nextcloud
  down with it.
* Rollback is `ln -sfn releases/<id> current.tmp && mv -T current.tmp current`,
  which is the same atomic operation. No special tooling needed.
