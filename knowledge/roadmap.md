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
