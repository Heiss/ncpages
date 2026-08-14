---
type: Interface Contract
title: Quality gate
description: The checks between build and publish that stop a broken or half-synced build from replacing a working site.
tags: [gate, quality, safety, interface]
status: draft
generated: { by: claude-code/opus-5, at: 2026-08-15T00:45:00Z }
sources:
  - id: concept
    resource: ../history/original-concept-note.md
    title: ncpages concept note, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
  - id: session
    resource: ../history/design-session-transcript.md
    title: ncpages design session, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
---

The gate runs after the build and before the swap. On violation nothing is
published, `current` stays where it is, and the failure is reported loudly.

# Checks

| Check | Config | Catches |
|---|---|---|
| required files present | `require_files` | generator wrote nothing usable |
| minimum page count | `min_pages` | empty or near-empty output |
| page-count drop vs. previous release | `max_page_drop` | half-synced or partly deleted vault |
| navigation churn | `max_nav_churn` | mass frontmatter damage |
| duplicate basenames | — | breaks wikilink resolution |
| conflict copies | — | filtered from output **and** reported |

# Why the third check matters most

An exit code of `0` is not sufficient evidence that a build is good. The realistic
scenario: a sync error or an accidental delete on a phone leaves the vault
half-empty on the server, the generator builds it happily, and a three-page website
replaces the blog. Nothing in the toolchain notices, because nothing failed.[^concept]

The page-count drop check is the one that turns "the build succeeded" into "the
build is plausible".

# Conflict copies

Files named `… (conflicted copy 2026-08-14 120000).md` are excluded from the build
*and* surfaced in the report. Filtering alone would be wrong: a conflict copy means
a version of someone's work is at risk of being lost, which is worth an alert on
its own.[^concept]

# Duplicate basenames

Wikilink resolution in Obsidian-style content matches by basename. Two files with
the same basename in different folders make link targets ambiguous, so the resolver
picks one arbitrarily and some links silently point at the wrong page. Cheap to
detect, invisible otherwise.

# Design notes

* All checks are local. The gate needs no network and no credentials.
* Thresholds are ratios or counts, not percentages of a moving average — a gate
  that is hard to reason about will be disabled by its user.
* A failing gate is a *report*, not a retry. Repeating the same build produces the
  same output; the fix is upstream, in the vault or in a hook.

[^concept]: Concept note, sections 5.3 and 2.6 of the transcript.
