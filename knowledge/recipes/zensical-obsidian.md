---
type: Recipe
title: Zensical + Obsidian (reference recipe)
description: The reference deployment — an Obsidian vault in Nextcloud built by Zensical with a custom Markdown extension, webmentions and frontmatter-driven navigation.
tags: [recipe, zensical, obsidian, webmentions, reference]
status: draft
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

This is the recipe ncpages was designed against: a personal blog written in
Obsidian, synced to Nextcloud, built with Zensical, previously published through
GitHub Actions and git-pages.

# What the vault contains

Content only:

```
<vault>/
└── docs/
    ├── *.md
    ├── <assets>
    └── stylesheets/extra.css
```

Everything else — generator config, `pyproject.toml`, `uv.lock`, `overrides/`,
`src/obsidian_md/`, hooks — lives in the config directory. See
[Code outside the vault](../decisions/code-outside-vault.md).

# What the config directory contains

| File | Role |
|---|---|
| `zensical.toml` | generator config, **without** a `nav` tree — the hook appends it |
| `pyproject.toml`, `uv.lock` | dependency manifest, installed at image build time |
| `overrides/` | Jinja templates (`custom_dir`) — executable, therefore code-side |
| `src/obsidian_md/` | local Python-Markdown extension, loaded by module name |
| `hooks/nav_from_frontmatter.py` | `pre_build` — frontmatter → `nav` |
| `hooks/fetch_comments.py` | phase not yet decided, see below |
| `hooks/send_webmentions.sh` | `post_publish` — irreversible, runs after the swap |

# Notable properties

**No Obsidian preprocessing step.** The pleasant surprise of the audit: the legacy
workflow only ran `zensical build --clean`. Wikilinks and embeds are handled by
`obsidian_md` *inside* the build, as a Markdown extension. The migration is
therefore much smaller than expected.

**`obsidian_md` is local code, not a third-party package.** Which is why it is made
importable via `PYTHONPATH` into the assembled tree rather than by an editable
install. See [Baked dependencies](../decisions/baked-dependencies.md).

**Navigation comes from frontmatter.** See
[Navigation convention](nav-frontmatter-convention.md). Migration and aggregator
are written and verified: round-trip over all 46 pages byte-identical, including
under shuffled input order.

**Webmentions are the irreversible step.** `static-webmentions` diffs the previous
release against the new one (`NCPAGES_PREV_DIR` vs. `NCPAGES_RELEASE_DIR`) and
sends. It runs in `post_publish`, never earlier.

**The timer is required here.** `fetch_comments.py` pulls incoming comments and
annotations from outside; without a timer they would only appear when the author
next edits something. See [Trigger composition](../decisions/trigger-composition.md).

# Hook assignment (verified against the repository)

| Script | Phase | Why |
|---|---|---|
| `update_overview.py` | `pre_build` | generates the nav from front matter; must write into the build tree, never the vault |
| `fetch_comments.py` | `post_build` | walks `site/**/*.html`, injects webmention and Hypothesis timelines in place |
| `static-webmentions find` + `send` | `post_publish` | irreversible; must run after the swap |

Details and the four corrections this audit produced are in
[Audit of the current setup](../history/current-setup-audit.md).

# Things that must not become hooks

`update_images.py` generates images through the Gemini API and `pip install`s
`google-genai` at runtime if it is missing. It is an authoring-time tool: it needs
network and a paid API, and it writes into the content directory. Keep it local.

# Open items specific to this recipe

* **`category` or `nav` as the front-matter key**, and whether ordering stays in
  the script's `CATEGORY_ORDER` or moves to `nav_order` per note.
* **The generated index pages** (`docs/index.md`, `docs/de/index.md`,
  `docs/concept-maps.md`) currently live in the content directory but would be
  produced into the build tree instead.
* **Seeding the first webmention run** — nothing has ever been sent, so the first
  working `post_publish` would flush the entire backlog.

Tracked in [Open questions](../open-questions.md).
