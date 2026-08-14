---
type: Open Questions
title: Open questions
description: Decisions still open after the repository audit, what each blocks, and what would settle them.
tags: [open-questions, decisions, planning]
status: draft
stale_after: 2026-11-15
generated: { by: claude-code/opus-5, at: 2026-08-15T01:20:00Z }
sources:
  - id: audit
    resource: history/current-setup-audit.md
    title: Audit of the current setup
    author: claude-code/opus-5
    last_modified: 2026-08-15
  - id: session
    resource: history/design-session-transcript.md
    title: ncpages design session, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
---

Three questions from the design session were settled by the
[repository audit](history/current-setup-audit.md); they are listed at the bottom
for the record.

# 1 · `category` or `nav` — and where does ordering live?

The existing setup already derives navigation from front matter, using
`category: Architecture & Strategy/Domain-Driven Design`. The session designed the
same thing under the key `nav`, plus `nav_order` per note.

Sub-questions:

* **Key name.** Keeping `category` means zero migration and no re-touching of 95
  notes. Renaming to `nav` is more accurate (it is a menu path, not a taxonomy) and
  frees `category` for actual tagging.
* **Ordering.** Today it is a hard-coded `CATEGORY_ORDER` list inside
  `update_overview.py` — which lives on the code side, so it survives the security
  model unchanged. `nav_order` in front matter moves ordering into the vault, where
  it is editable from a phone, at the cost of spreading one global decision across
  95 files.
* **Both?** `CATEGORY_ORDER` for top-level sections, `nav_order` for pages within
  a section, is a coherent split.

**Blocks:** the `pre_build` hook, and whether `migrate_nav.py` runs at all.

# 2 · What happens to `update_overview.py`'s other outputs?

It does three things: generate the nav, regenerate the category tables in
`docs/index.md` and `docs/de/index.md`, and regenerate `docs/concept-maps.md`. The
last two write into the *content* directory.

As a hook it must write into the assembled build tree, never into the vault working
copy — otherwise the output is lost on the next sync, or syncs back and creates a
trigger loop. That is a straightforward change, but it means the generated index
pages stop being visible in Obsidian.

**Options:** accept that (they are generated artifacts), or keep a manual
authoring-time run that writes into the vault and treat the hook as the
authoritative one for builds.

**Blocks:** the recipe's `pre_build` configuration.

# 3 · Seeding the first webmention run

Webmentions have never actually been sent — the legacy workflow only ever ran
`static-webmentions find`. The first ncpages build with a working `post_publish`
hook would therefore diff against an empty previous release and send the entire
backlog accumulated since the site began.

**Options:** seed `NCPAGES_PREV_DIR` with a crawl of the current live site, run the
first build with sending disabled and adopt its output as the baseline, or
deliberately let the backlog go out once.

**Blocks:** cutover, phase 6. Irreversible if done wrong.

# 4 · German pages and the menu

`docs/de/` holds 53 files including its own index, and notes carry `translation_de`
links, so the language switcher works. But the committed `nav` tree contains only
English pages, while `update_overview.py` carries EN↔DE category label maps.

**Settles it:** re-run the generator and diff against the committed
`zensical.toml`. Either the committed tree is stale, or German pages are
deliberately menu-less.

**Blocks:** nothing structural; it changes what "orphan" means in the status report.

# 5 · git as an invisible internal layer

Proposed, not decided. Before each build, `git add -A && git commit` into a local
bare repository — roughly fifteen lines.

Gain: an exact diff answering "what triggered this build?", reproducible rebuilds,
`git bisect` for layout regressions, and a backup independent of Nextcloud. Without
it, Nextcloud versioning replaces only per-file history, with no commit boundaries.

Tension: the point of the project is to stop touching git. The counter is that the
user never would — it is an implementation detail with no interface.

**Blocks:** nothing. Can be added later without changing any contract.

# 6 · Draft preview

`draft: true` excludes a page but shows nothing, so the preview that pull requests
*could* have provided is gone. (The legacy workflow builds only on push to `main`
and on cron, so there was no PR preview in practice either.) A second vault folder
with its own publish target behind basic auth would be the replacement: two
sources, one core.

**Blocks:** nothing in v1, but it decides whether the core must support multiple
sources and publish roots in one process.

# 7 · The name

Nextcloud GmbH holds the word mark. A community repo named after the platform is
common practice; a *product name* with logo, domain and Docker image can look like
official affiliation, and "Pages" amplifies that, because GitHub, GitLab and
Codeberg Pages are first-party features. Safe route: a distinct name plus a
descriptive subtitle.

**Blocks:** repository name, image tags, documentation URLs, the environment
variable prefix. Renaming after the first release costs all of them.
**Deadline:** before the first public release.

# Settled by the audit

* **Which phase does `fetch_comments.py` belong to?** → `post_build`. It rewrites
  `site/**/*.html` in place and never touches `docs/`. No comment delay exists.
* **Is there an LLM call in the build path?** → No. `google-genai` belongs to
  `update_images.py`, an authoring-time tool that must not become a hook.
* **Does `/de/` point nowhere?** → No. `docs/de/index.md` exists and the
  translations are linked from front matter.
