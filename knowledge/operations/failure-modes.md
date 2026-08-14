---
type: Failure Mode Catalog
title: Failure modes
description: Every break identified in the red-team pass, what it looks like from outside, and how the design handles it.
tags: [operations, failure-modes, red-team, troubleshooting]
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

The unifying property of this list: almost every entry fails *silently*. That is
why the mitigations are structural rather than "check the logs".

# Failures that shaped the architecture

| Failure | Looks like | Handled by |
|---|---|---|
| Vault content executes as build code | nothing, until it does | [Code outside the vault](../decisions/code-outside-vault.md), sandbox, fail-closed overlap check |
| inotify on the data dir under encryption / S3 / group folders | no triggers, or triggers on unreadable blobs | [WebDAV over inotify](../decisions/webdav-over-inotify.md) |
| Status report inside the watched folder | infinite build loop | sibling path + self-write fingerprint |
| Publishing an empty or broken build | a three-page site replacing a blog | [Quality gate](../interfaces/quality-gate.md) |
| Irreversible post-publish steps firing on an unpublished state | webmentions announcing content nobody can see | four-phase ordering, `queue_latest` |
| Symlink bind-mounted into the web server | site never updates, no error, no log | mount the parent directory |

# Failures that bite in operation

* **Intermediate states during sync.** A rename with link updates writes many
  files, and WebDAV sync is not transactional. There is no boundary to wait for.
  Debounce makes it unlikely, the gate catches the rest, the next build repairs it.
* **Conflict copies** would otherwise become public pages. Filtered *and* reported.
* **Nextcloud maintenance mode** → 503 on every poll. Exponential backoff, not a
  hot loop. On 401, stop immediately rather than fighting brute-force protection.
* **`trusted_proxies` missing the nginx subnet** → notify_push self-test fails.
  The single most common cause of a broken setup.
* **State loss on restart** → full rebuild after every `compose up`, and the
  reconcile path stays untested until it is needed. Persist ETag and content hash.
* **Bootstrap with no `current`** → 404 on everything. Create a holding-page
  release at first start.
* **Volume growth.** Retention must be actively enforced; a full root filesystem
  takes Nextcloud down with it.
* **Timestamps across sync boundaries** are unreliable. Never use mtime for change
  detection.
* **UID mismatch between watcher and builder** → `EACCES` on `releases/`, on the
  *second* build, which makes it look transient.
* **No dashboard any more.** GitHub Actions wrote failures in your face; a dead
  service says nothing. Hence status report, ntfy and `/healthz`. See
  [Observability](../interfaces/observability.md).

# What the move costs

* **History and rollback granularity.** Git offered diff, blame, atomic multi-file
  commits, revert, and — through pull requests — an implicit preview. Nextcloud
  versioning replaces only per-file history, without commit boundaries. See the
  git-as-internal-layer proposal in [Open questions](../open-questions.md).
* **Draft preview.** `draft: true` excludes but shows nothing. A second vault
  folder with its own publish target behind basic auth would be the replacement:
  two sources, one core.
* **Certificates and DNS** move from the pages provider to the operator.

# If this is published

* **Support load is asymmetric.** Most issues will be other people's deployments —
  broken notify_push, wrong `trusted_proxies`, S3 storage, encryption. The only
  effective defence is [`ncpages doctor`](doctor-checks.md) plus an issue template
  that demands its output.
* **Security responsibility.** The tool executes code when a cloud folder changes,
  and someone will point it at a shared team folder. `THREAT_MODEL.md` belongs in
  front of the installation instructions.
* **Premature generalisation kills it.** Abstracting sources for S3, Dropbox and
  SFTP before the Nextcloud path is solid. Ship `webdav` and `fs`.
* **Multi-arch is mandatory.** A large part of the homelab audience runs arm64.
* **Abandonment risk.** A tool in the publication path of other people's websites,
  unmaintained for three years, is worse than no tool. One honest sentence in the
  README beats silent disappointment.
