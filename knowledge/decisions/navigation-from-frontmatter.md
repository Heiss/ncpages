---
type: Decision Record
title: Derive navigation from note frontmatter
description: Navigation is aggregated by a pre_build hook from frontmatter in ordinary notes, keeping generator config out of the vault and URLs stable.
tags: [decision, navigation, obsidian, recipe, security]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-15T00:45:00Z }
sources:
  - id: session
    resource: ../history/design-session-transcript.md
    title: ncpages design session, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
---

# Context

The reference deployment's generator config contained a fully spelled-out
navigation tree — seven sections with curated titles. The generator derives
navigation from the directory structure only when no explicit tree is configured.

> **Audit correction.** That tree is *generated*, not hand-written:
> `update_overview.py` builds it from a `category:` front-matter key that already
> uses the same `/`-separated hierarchy. The real workflow was "run the script
> locally, commit" rather than "edit the config over SSH". The decision below still
> holds — after the migration there is no local checkout to run the script in, so
> the aggregation has to become a `pre_build` hook either way — but option (d) is
> less a new design than the adoption of an existing convention. See
> [Audit of the current setup](../history/current-setup-audit.md).

That breaks the entire premise: write a note in Obsidian, it syncs, the watcher
fires, the site builds — and the note is not in the menu. Making it visible means
editing the config, which after
[Code outside the vault](code-outside-vault.md) lives on the server. So: SSH,
editor, restart. That is *worse* than the git workflow being replaced.

# Options considered

| | Approach | Verdict |
|---|---|---|
| (a) | Drop the explicit tree, use implicit navigation | The content directory is mostly flat → an alphabetical list of 46 pages. Restructuring it would change URLs and point existing webmentions at nothing. Rejected. |
| (b) | Put the generator config in the vault | It references `custom_dir` and extension module names, i.e. import paths. Vault-editable means code execution. Rejected. |
| (c) | A validated navigation fragment in the vault | Works, but requires a schema-validating merge primitive in the core. |
| (d) | Aggregate navigation from note frontmatter | **Chosen.** |

An initial assessment held that (d) needed *more* core logic than (c). The opposite
is true: under (c) a configuration-shaped file arrives from the vault and the core
must validate it. Under (d) only frontmatter in ordinary notes arrives, aggregation
is a hook in the recipe, the core stays free of generator knowledge, and the attack
surface is zero.

# Consequences

* The core knows nothing about navigation. The aggregator is a `pre_build` hook,
  exactly like fetching external data.
* **URLs stay stable.** Navigation is decoupled from file paths, so reordering the
  menu breaks no links — which matters because webmentions point at URLs.
* Sorting rules must be deterministic, since filesystem traversal order is not
  guaranteed. See [Navigation convention](../recipes/nav-frontmatter-convention.md).
* A migration is needed once, from the existing tree into frontmatter. Verified by
  round-trip: tree → frontmatter → tree, byte-identical across all 46 pages, also
  under randomly shuffled input order.
