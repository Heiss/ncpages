---
type: Audit
title: Audit of the current setup (digital-garden-next)
description: Verified facts from the live repository, including four corrections to assumptions made during the design session.
resource: https://github.com/Heiss/digital-garden-next
tags: [audit, legacy, verification, zensical, workflow]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-15T01:20:00Z }
verified:
  - { by: claude-code/opus-5, at: 2026-08-15T01:20:00Z }
sources:
  - id: repo
    resource: https://github.com/Heiss/digital-garden-next
    title: digital-garden-next (private repository, branch main)
    author: human:heiss
    last_modified: 2026-05-07
  - id: session
    resource: design-session-transcript.md
    title: ncpages design session, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
---

Read directly from the repository (private, `main`, last push 2026-05-07). Where
this file and the [design session transcript](design-session-transcript.md)
disagree, **this file wins** — the transcript predates the audit.

# Confirmed

* `zensical build --clean` is the whole build. No Obsidian preprocessing step;
  `[project.markdown_extensions.obsidian_md]` loads the local extension from
  `src/obsidian_md/` by module name.[^repo]
* `custom_dir = "overrides"` — Jinja templates, correctly classified as code.
* `extra_css = ["stylesheets/extra.css"]` — inert, stays in the vault.
* `requires-python = ">=3.13"`, `zensical>=0.0.38`; dev group is
  `beautifulsoup4`, `google-genai`, `httpx`, `zensical`. No runtime dependencies.
* Workflow triggers: push to `main`/`master` **and** `cron: '0 */12 * * *'`.
* `concurrency: { group: deploy, cancel-in-progress: true }`.
* `cp -r site site-previous` after an `actions/cache` restore — the nesting bug.
* `static-webmentions` and `git-pages-cli` fetched as `latest` without checksums.
* Deploy is `git-pages-cli https://www.netzmuffel.de --upload-dir site`.

# Corrections to the design session

## 1 · `fetch_comments.py` is a `post_build` hook — settled

It walks `site/**/*.html`, fetches webmention.io and Hypothesis data per page URL,
appends a rendered timeline into `article.h-entry`, and writes the HTML back in
place. It never touches `docs/`.[^repo]

So: `post_build`, needs network and `WEBMENTION_IO_TOKEN`, runs in the watcher,
before the gate. The feared 12-hour comment delay does not exist — comments are
injected into every build, and the 12-hour cron exists precisely to make that
happen without an author edit.

## 2 · No LLM call in the build path — settled

`google-genai` is used only by `scripts/update_images.py`, an authoring-time tool
that generates images and copies them into `docs/assets/images/`. It is not part of
the workflow.[^repo]

Two notes for the migration: it must **not** become a hook (it writes into the
content directory and calls a paid API), and it `pip install`s `google-genai` at
runtime if missing, which would fail in a network-less builder by design.

## 3 · Navigation is already generated from frontmatter

The most consequential correction. The `nav` tree in `zensical.toml` is **generated
output**, not hand-curated. `scripts/update_overview.py` scans the flat `docs/` for
files carrying a `category` front-matter key and writes the tree into
`zensical.toml`, then regenerates the category tables in `docs/index.md`,
`docs/de/index.md` and `docs/concept-maps.md`.[^repo]

The existing convention already looks like the one the session designed:

```yaml
---
title: Bounded Context
category: Architecture & Strategy/Domain-Driven Design
translation_de: bounded-context
translation_de_title: Bounded Context
---
```

Same hierarchical path, same `/` separator. What differs:

| | Existing (`update_overview.py`) | Session proposal |
|---|---|---|
| key | `category` | `nav` |
| ordering | hard-coded `CATEGORY_ORDER` list in the script, then alphabetical | `nav_order` per note, steps of ten |
| translations | `translation_de`, `translation_de_title` + EN↔DE label maps | not considered |
| drafts | none | `draft: true` |
| output | writes `zensical.toml` **and** back into `docs/` | writes nav only |

The premise "a new note does not appear until someone edits `zensical.toml` over
SSH" was therefore never quite right — the real workflow is "run
`update_overview.py` locally, commit". The conclusion still holds, though, and
gets stronger: that script must become a `pre_build` hook, because after the
migration there is no local checkout to run it in.

**New constraint this exposes:** `update_overview.py` writes into `docs/`. As a
hook it must write into the assembled build tree (`NCPAGES_BUILD_DIR`), never into
the vault working copy. A hook that modified the working copy would either lose its
output on the next sync or, if written back, create a trigger loop.

Open decision: keep `category` or rename to `nav`, and where ordering lives. See
[Open questions](../open-questions.md).

## 4 · `/de/` does not point nowhere

`docs/de/` exists with 53 files including `docs/de/index.md`, and the pages carry
`translation_de` links. The alternate-language switcher has a target.

What is true: the committed `nav` tree contains only the English pages, so German
pages build but sit outside the menu, and `update_overview.py` carries EN↔DE
category label maps that the committed tree does not reflect. Worth re-running the
generator to see whether the committed `zensical.toml` is simply stale.

# New findings

## Webmentions have never been sent

The workflow step named *Send webmentions* runs:

```
static-webmentions find --newDir site --oldDir site-previous --output mentions.json
```

`find` only discovers pending webmentions and writes them to a file; `send` is the
subcommand that transmits them, and it is never invoked. `mentions.json` is
uploaded as an artifact with 90-day retention and otherwise unused.[^tool]

Two independent reasons the feature does not work, then: the diff runs against a
[broken `site-previous` tree](legacy-workflow-findings.md), and nothing sends the
result. The design's careful `post_publish` ordering is building a capability that
has not actually been in production.

**Consequence for the cutover:** the first ncpages run with a working
`post_publish` hook will send the backlog of every mention discovered since the
site began. Seed `NCPAGES_PREV_DIR` from the current live site for the first
build, or run once with sending disabled.

## `.env` is committed

`.env` containing `GEMINI_API_KEY` is tracked in the repository and absent from
`.gitignore`. The repository is private, so this is not a public exposure, but the
key is in git history. If this repository is ever made public — plausible, given
the plan to publish ncpages from adjacent work — the key must be rotated first, not
merely deleted.

## Page counts

42 English notes at the top level of `docs/` plus section index pages (the ~46 the
session refers to), and 53 files under `docs/de/`. The `migrate_nav.py` round-trip
was verified against the English tree.

[^repo]: Read from `zensical.toml`, `pyproject.toml`, `.github/workflows/docs.yml`, `scripts/fetch_comments.py`, `scripts/update_overview.py`, `scripts/update_images.py` and the git tree of `main`.
[^tool]: `static-webmentions` documents `find` as "find out which webmentions are to be sent and save them to a temporary file" and `send` as "read the pending webmentions from the temporary file and send them out".
