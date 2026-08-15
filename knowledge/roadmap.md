---
type: Roadmap
title: Roadmap
description: What ships in v1, what deliberately does not, and what publishing the project would require beyond the code.
tags: [roadmap, planning, publication]
status: draft
stale_after: 2026-11-15
generated: { by: claude-code/opus-5, at: 2026-08-15T00:45:00Z }
sources:
  - id: concept
    resource: history/original-concept-note.md
    title: ncpages concept note, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
  - id: session
    resource: history/design-session-transcript.md
    title: ncpages design session, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
---

# v1 — the reference deployment runs on it

Scope is defined by [the cutover runbook](operations/cutover-runbook.md), phases
0–6. Done means: the site is served by ncpages, the GitHub workflow is disabled,
the deploy secrets are revoked, and the git-pages app is uninstalled.

Shipped source kinds: `webdav` and `fs`. Shipped publish backend: `symlink`.

**Implementation choices, settled 2026-08-15:**

* **Rust.** One static binary, no runtime in the container, multi-arch without
  effort, and the same language as notify_push, whose WebSocket protocol the
  watcher speaks.
* **One binary, several roles** — `run`, `watch`, `serve`, `build-agent`,
  `doctor`. See [One binary with roles](decisions/single-binary-roles.md).
* **Repository:** `github.com/Heiss/ncpages`, public, Apache-2.0.

# Deliberately not in v1

* Sources for S3, Dropbox, SFTP. Abstracting storage before the Nextcloud path is
  solid is the failure mode that kills this class of project. The `Source` trait
  stays in the code; only two implementations ship.
* Remote publish backends. See
  [Symlink swap](decisions/symlink-swap-publish.md).
* A plugin system. See [Hooks, not plugins](decisions/hooks-not-plugins.md).
* Metrics, dashboards, log shipping.

The only abstraction v1 needs is **core versus recipe**.

# Needs a measurement first

**RFC 6578 collection sync (`sync-token` REPORT).** Nextcloud supports it, and
`obsidian-nextcloudsync` uses it: one `REPORT` returns every change since a
token, including deletions, instead of descending the tree by ETag.

The intuition is that one call beats a descent. It is probably right, but not
where it first appears, so this should be measured rather than assumed:

* **Idle** — the overwhelmingly common case, once every poll interval. ETag
  descent already costs exactly *one* request here, and so would a `REPORT`. No
  win.
* **After a change** — descent costs one request per directory along the changed
  paths. For the reference deployment, a flat `docs/` with about 40 files, that
  is two requests. For a deep vault it grows with depth, and a `REPORT` stays at
  one. The win is real but scales with tree shape, not with vault size.
* **Deletions** — the current implementation finds them by listing the whole tree
  and diffing. A `REPORT` reports them directly. This is likely the largest
  practical win, and it grows with the number of directories.

What it costs: a second code path that only works against Nextcloud, so the ETag
descent has to stay for `fs` sources, plain WebDAV servers and any deployment
where propagation behaves oddly. Sync tokens also expire, and the expiry path —
fall back to a full listing — is exactly the kind of code that stays untested
until the day it runs.

**How to measure it.** The mock Nextcloud already counts requests, and the
integration tests already assert exact counts. A benchmark over three vault
shapes — flat (50 files), deep (5×5×5), wide (500 files in one directory) — for
five scenarios — idle, one file edited, one added, one deleted, a rename that
touches many files — gives request counts and wall time for both strategies side
by side. If the flat and idle cases do not improve, this is an optimisation for
other people's vaults, which is still a reason to do it, just a different one.

**The shape it should take: progressive enhancement.** One binary that detects
the capability at runtime — `sync-collection` in the collection's
`supported-report-set` — and uses it when the server offers it. Nextcloud gets
the fast path, plain WebDAV servers and the `fs` source get the descent, and
nobody has to choose a build. This is what `obsidian-nextcloudsync` does:
`NextcloudClient.connect()` reports detected features and falls back to a
standard WebDAV client otherwise.

## Rejected for now: a compile flag and a second image

The idea: put the descent behind a cargo feature and ship two containers — one
with both algorithms, one minimal image that speaks only RFC 6578 to Nextcloud.
Rejected, with the reasons written down because they may not hold forever:

* **The saving is not where the size is.** The descent is roughly a hundred lines
  of glue over machinery the `REPORT` path needs anyway — the HTTP client, TLS,
  the multistatus parser, percent-encoding. Removing it would save a fraction of
  a percent of a 2.42 MB binary. The dependencies are the size, not the
  algorithm.
* **The fallback is not optional.** RFC 6578 lets a server invalidate a sync
  token (`DAV:valid-sync-token`), after which the client *must* do a full
  synchronisation. That happens on server restarts and routine database
  maintenance. A REPORT-only build would still need the full-listing path, or it
  would fail hard on a routine event.
* **It doubles the test matrix at the worst point.** Every feature combination
  needs its own build, lint, test and e2e run — and the path most likely to break
  is the fallback, which is exactly what the minimal build would compile out.

If a flag is ever wanted anyway, the removable half must be the `REPORT`, not the
descent: a build without `REPORT` is correct everywhere and merely slower, while
a build without the descent is broken against half the possible setups.

**Revisit when** the binary stops being small enough to ignore, or when the two
strategies have diverged enough that carrying both is a maintenance cost rather
than a hundred lines.

# Publication

Not automatic — a tool in someone else's publishing path carries obligations. What
it would require:

**Decide the name first.** See [Open questions](open-questions.md#7-the-name).
Renaming after release costs image tags, documentation links and stars.

**License Apache-2.0.** No Nextcloud code is linked; communication is plain HTTP,
so AGPL is not triggered. The patent clause matters for deployments inside
companies.

**Separate core from recipe** in the repository layout: the service in one place,
`examples/zensical-obsidian` beside it. Without that separation the project reads
as one person's blog setup.

**The companion Nextcloud app**, as its own repository and its own entry in the
Nextcloud app store. It receives the [report payload](interfaces/status-reporting.md)
and presents it: build history, what changed between releases, conflict copies
that need attention, possibly request counts from the serving side. Keeping it
separate is what lets ncpages treat it as absent by default — one `OPTIONS` probe
and nothing else. Requires a second project, so it comes after v1.

**More recipes.** `quartz` is the highest-value one — it has the largest Obsidian
publishing community, and "Quartz without git" is exactly the missing piece there.
Then `hugo` and `mkdocs-material`.

**`ncpages doctor`.** The entire red-team list as executable checks, plus an issue
template that requires its output. See [Doctor checks](operations/doctor-checks.md).

**`THREAT_MODEL.md` before the installation instructions**, and a `SECURITY.md`
with a contact route. The tool executes code when a cloud folder changes; someone
will point it at a shared team folder.

**Multi-arch images**, `amd64` and `arm64`. Homelab audiences run both; amd64-only
halves the reachable user base.

**Integration tests against real Nextcloud versions**, as a compose matrix. The
failure modes are deployment-shaped, so unit tests cannot find them.

**A documentation site on GitHub Pages**, built by GitHub Actions with Zensical
straight from this bundle (`docs_dir = "knowledge"`), so there is no second copy of
the documentation to keep in sync. Explicitly *not* built by ncpages: the
documentation lives in git, not in a Nextcloud folder, so ncpages is the wrong tool
for it. Dogfooding here would mean inventing a requirement to satisfy a slogan.

That Zensical is also the reference recipe's generator is a convenience, not a
coupling — the ncpages core has no knowledge of it. Which generator runs is decided
entirely by the recipe's hooks and builder image; see
[Hooks, not plugins](decisions/hooks-not-plugins.md) and
[Recipes](recipes/index.md).

**An honest README.** The comparison with `watchexec` plus a shell script, stated
plainly, and one sentence about maintenance expectations and bus factor. A tool in
the publication path of other people's websites that goes unmaintained for three
years is worse than no tool.
