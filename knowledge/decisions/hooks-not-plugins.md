---
type: Decision Record
title: Scripts and environment variables instead of a plugin system
description: The extension interface is four hook phases running executables with a fixed environment — no dynamic modules, no ABI, no in-process extensions.
tags: [decision, extensibility, interface, longevity]
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

Everything generator-specific has to live somewhere: navigation aggregation, data
fetching, HTML post-processing, webmention sending. The tempting shape is a plugin
API.

# Decision

Four hook phases that execute programs, with a fixed set of environment variables
and three meaningful exit codes. No plugin system, no dynamically loaded modules,
no in-process extension points.

# Rationale

* **Longevity.** "Run this executable with these environment variables" still works
  in five years. A plugin ABI does not survive its host language's churn.
* **Language independence.** A hook can be Python, shell, or a compiled binary.
* **Isolation for free.** A separate process has its own memory, its own crash
  behaviour, its own exit code. The core never has to defend itself against a
  misbehaving extension in its own address space.
* **The core stays generator-agnostic.** All generator knowledge sits in the
  recipe, which is a directory of scripts and config, not code linked into ncpages.

# Consequences

* The environment contract is the API and must be treated as one: adding variables
  is compatible, renaming them is a breaking change.
* Exit code `1` (warning, continue and report) exists so a flaky external API does
  not take a site offline.
* Hooks must be idempotent, because timer triggers and `queue_latest` mean repeated
  runs over unchanged content.
* No hook ordering beyond the list order within a phase. Anything needing richer
  orchestration should be one script that does it.
* Debugging is ordinary: run the script by hand with the same environment.
