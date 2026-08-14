---
type: Glossary
title: ncpages glossary
description: Terms used across this bundle, with the meaning they carry inside ncpages specifically.
tags: [glossary, reference]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-15T00:45:00Z }
sources:
  - id: session
    resource: history/design-session-transcript.md
    title: ncpages design session, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
---

**Vault** — the watched folder in Nextcloud. Holds content only: markdown, images,
`stylesheets/extra.css`. Never build configuration, never executable code.

**Working copy** (`src/`) — the local, persistent copy of the vault. Persistent by
design, not a cache: if Nextcloud is unreachable, timer builds keep running against
the last known content. See
[Source as working copy](decisions/source-as-working-copy.md).

**Overlay** — the files copied from the read-only config directory (`/etc/ncpages/`)
into the build tree: generator config, dependency manifests, templates, extension
source. The code half of the assembled tree.

**Assemble** — producing the build tree by combining overlay and working copy.

**Recipe** — everything specific to one generator: hooks, overlay files, config
snippets. Ships as an example, not as core. `zensical + obsidian` is the reference
recipe; `quartz`, `hugo` and `mkdocs-material` are candidates.

**Core** — the parts that know nothing about any generator: change detection,
scheduling, assembly, hook execution, gate, publish, reporting.

**Gate** — the quality checks between build and publish. A failed gate leaves the
live site untouched. See [Quality gate](interfaces/quality-gate.md).

**Release** — one completed, gated build under `releases/<id>/`. Five are retained,
which gives both rollback and the `oldDir` a webmention diff needs.

**Publish** — the atomic step: point the `current` symlink at a release via
`rename(2)`.

**Swap** — synonym for the symlink replacement inside publish.

**Trigger** — the reason a build started: `push`, `poll`, `timer` or `manual`.
Passed to every hook as `NCPAGES_TRIGGER`.

**Debounce** — the quiet period after the last observed change before a build
starts. Obsidian autosaves constantly; a rename with link updates rewrites dozens
of files.

**Hard deadline** — the upper bound on debouncing. A continuously changing vault
still gets built.

**`queue_latest`** — the busy policy: at most one running build plus one waiting
slot, where new events overwrite the waiting slot. Running builds are never
cancelled. See [queue_latest over cancel](decisions/queue-latest-over-cancel.md).

**Fingerprint** — the record of the state ncpages itself last wrote back to
Nextcloud, used to recognise its own status report and not trigger on it.

**ETag propagation** — Nextcloud's behaviour of bubbling ETag changes up the
directory tree, which lets a single `PROPFIND Depth: 0` on the root detect any
change beneath it.

**notify_push** — Nextcloud's Rust push service (Redis pub/sub → WebSocket). Says
*that* something changed, never *what*.

**Holding page** — the minimal release created at first start when no `current`
exists, so the web server does not answer 404 to everything during the first sync.

**Orphan** — a page that builds and is reachable by link but appears in no
navigation entry. Legitimate in a digital garden; listed in the status report so
it stays a decision rather than an accident.

**Conflict copy** — a file Nextcloud names `… (conflicted copy …)`. Filtered from
the build *and* reported, because its existence means someone's work is about to
be lost.
