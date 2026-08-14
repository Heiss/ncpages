---
type: Convention
title: Navigation frontmatter convention
description: How notes declare their place in the site navigation, how groups sort, and what the aggregator and migration tools guarantee.
tags: [navigation, frontmatter, obsidian, convention, recipe]
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

Notes declare their own place in the menu. A `pre_build` hook aggregates the
declarations into the generator's navigation tree. Rationale in
[Navigation from frontmatter](../decisions/navigation-from-frontmatter.md).

> **Status.** The reference deployment already uses this pattern under a different
> key — `category:`, with the same `/`-separated hierarchy, plus `translation_de`
> for the German edition — and orders top-level sections from a hard-coded list in
> `update_overview.py` rather than from `nav_order`. Which key and which ordering
> source win is [open question 1](../open-questions.md). The rules below describe
> the target convention; see
> [Audit of the current setup](../history/current-setup-audit.md) for what exists
> today.

```yaml
---
title: Bounded Context
nav: Architecture & Strategy/Domain-Driven Design
nav_order: 130
---
```

# Rules

* **Separator is `/`.** A list may be used instead, for titles that contain a
  slash.
* **Groups without a file** (a path segment that no note occupies directly) sort by
  the minimum `nav_order` of their descendants; ties break alphabetically. No
  sidecar file, fully deterministic.
* **Section index:** if `nav` equals a group path exactly instead of sitting below
  it, that note becomes the group's index page. Matches generators with
  `navigation.indexes` enabled.
* **No `nav:`** → the page is built and reachable by link, but appears in no menu.
  Legitimate for a digital garden. Orphans are listed in the status report so it
  stays a decision rather than an accident.
* **`draft: true`** → excluded entirely. This replaces the preview function lost
  with pull requests.
* **`nav_order` in steps of ten**, so inserting does not require renumbering.
  Collisions sort alphabetically by title.

Determinism matters more than it looks: filesystem traversal has no guaranteed
order, so any rule that depends on "the order files were seen" produces a menu that
changes between builds.

# Tools

| Tool | Role |
|---|---|
| `nav_lib.py` | frontmatter parsing and tree conversion in both directions |
| `migrate_nav.py` | one-off: existing `nav` tree → frontmatter in the notes; dry-run by default; reports missing files and orphans |
| `nav_from_frontmatter.py` | the `pre_build` hook; warns on single-page sections (a typo indicator) and duplicate basenames |

`nav_lib.py` deliberately avoids a full YAML parser: it reads flat key-value pairs
only, skips indented lines, and leaves list-valued Obsidian properties untouched.
The goal is to modify frontmatter without reformatting anything the author wrote.

# Verification

Round-trip test: existing tree → frontmatter across 46 files → remove the tree from
config → re-aggregate → compare. Byte-identical, including with randomly shuffled
input order.

# Guard rails in the build

* **Duplicate basenames fail the gate.** Wikilink resolution matches by basename;
  duplicates make link targets ambiguous and some links silently wrong.
* **Single-page sections produce a warning**, because they are usually a typo in a
  group name rather than an intentional section.
