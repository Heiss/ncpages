---
type: Findings
title: Bugs in the legacy GitHub Actions workflow
description: Three defects in the publishing chain being replaced — a broken cache copy, webmentions that are discovered but never sent, and two unpinned binaries next to the deploy credentials.
resource: https://github.com/Heiss/digital-garden-next/blob/main/.github/workflows/docs.yml
tags: [history, findings, security, ci, legacy]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-15T01:20:00Z }
verified:
  - { by: claude-code/opus-5, at: 2026-08-15T01:20:00Z }
sources:
  - id: repo
    resource: https://github.com/Heiss/digital-garden-next/blob/main/.github/workflows/docs.yml
    title: docs.yml workflow, branch main
    author: human:heiss
    last_modified: 2026-05-07
  - id: session
    resource: design-session-transcript.md
    title: ncpages design session, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
---

All three are **live until the cutover**, and all three disappear with the
migration. Verified against the workflow file; see
[Audit of the current setup](current-setup-audit.md).

# 1 — `cp -r site site-previous` copies into, not over

`actions/cache` restores `site-previous` via `restore-keys`, so the directory
already exists when the line runs. `cp -r src dst` with an existing `dst` copies
*into* it: `site-previous/site/`, then on the next run `site-previous/site/site/`,
while the copy from the first run stays alongside.

Consequence: since the second run ever, the webmention diff has compared against a
partly stale, partly nested tree. `|| true` and `continue-on-error: true` made sure
none of it was ever visible.

Immediate fix, if the old workflow keeps running for a while:

```sh
rm -rf site-previous && cp -r site site-previous
```

Under ncpages the problem does not exist: the previous build is a release on disk,
and `NCPAGES_PREV_DIR` is `readlink current`. See
[Delivery](../architecture/delivery.md).

# 2 — Webmentions are discovered but never sent

The step named *Send webmentions* runs `static-webmentions find … --output
mentions.json`. `find` writes pending mentions to a file; `send` transmits them,
and `send` is never called. The file is uploaded as an artifact and otherwise
unused.

So the feature has never worked, for two independent reasons — the diff input is
broken *and* nothing sends the result.

It also runs **before** the deploy step, which is the ordering problem the ncpages
pipeline is built to prevent: mentions would announce a state that is not live yet.
See [Pipeline](../architecture/pipeline.md).

Migration consequence: the first ncpages build with a working `post_publish` hook
would send the entire backlog accumulated since the site began. Seed
`NCPAGES_PREV_DIR` from the current live site, or make the first run a dry run.

# 3 — Two unpinned binaries next to the deploy credentials

`static-webmentions` (GitHub releases, `latest`) and `git-pages-cli` (Codeberg,
`latest`) are downloaded without checksums into `/usr/local/bin` and executed with
the job's full privileges — in a job whose environment holds
`GIT_PAGES_PASSWORD` and `WEBMENTION_IO_TOKEN`. `astral-sh/setup-uv` in the same
workflow is cleanly pinned to a commit SHA; these two are not.

This is a supply-chain path straight to the site's publishing credentials: whoever
controls those release artifacts controls the deploy.

Under ncpages: binaries are pinned by version and sha256 at image build time, the
builder runs non-root without egress, and credentials live in a different container
entirely. See [Security model](../architecture/security-model.md).

# Why they stayed invisible

`continue-on-error: true` and `|| true` turn a broken step into a green run, which
is worse than a failure: a red build gets fixed, a green one that does nothing does
not. Same failure mode the [quality gate](../interfaces/quality-gate.md) exists to
prevent — exit code `0` is not evidence that the right thing happened.

# Not a bug, but worth knowing

`cancel-in-progress: true` on the `deploy` concurrency group is safe *here* only
because nothing irreversible ever ran. Once webmention sending works, it stops
being safe. See [queue_latest over cancel](../decisions/queue-latest-over-cancel.md).
