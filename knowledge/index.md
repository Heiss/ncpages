---
type: Knowledge Bundle
title: ncpages
description: Knowledge bundle for ncpages — a service that watches a Nextcloud folder over WebDAV, builds a static site when it changes, and publishes it atomically.
okf_version: "0.2"
tags: [ncpages, nextcloud, webdav, static-site, homelab, okf]
status: draft
generated: { by: claude-code/opus-5, at: 2026-08-15T01:20:00Z }
sources:
  - id: session
    resource: history/design-session-transcript.md
    title: ncpages design session, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
  - id: concept
    resource: history/original-concept-note.md
    title: ncpages concept note, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
  - id: audit
    resource: history/current-setup-audit.md
    title: Audit of the current setup
    author: claude-code/opus-5
    last_modified: 2026-08-15
---

An [OKF v0.2](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
bundle: the design knowledge behind ncpages, written for humans and agents to read
before touching the code. Start with the overview, then the pipeline.

# Start here

* [Overview](overview.md) — what ncpages is, what it deliberately is not, and when
  you do not need it
* [Glossary](glossary.md) — terms with the meaning they carry here specifically
* [Open questions](open-questions.md) — what is still undecided and what it blocks
* [Roadmap](roadmap.md) — v1 scope, and what publishing would require

# Architecture

* [Pipeline](architecture/pipeline.md) — the ten steps from trigger to report
* [Scheduler state machine](architecture/state-machine.md) — debounce, queueing,
  persistence, reconcile
* [Topology](architecture/topology.md) — containers, networks, volumes, build tree
* [Security model](architecture/security-model.md) — why a build is code execution
* [Delivery](architecture/delivery.md) — atomic publish, releases, caching headers
* [Build environment](architecture/build-environment.md) — the network-less builder

# Interfaces

* [Command line](interfaces/cli.md) — the roles one binary can take
* [Configuration](interfaces/configuration.md) — `ncpages.toml`, field by field
* [Hook contract](interfaces/hook-contract.md) — four phases, environment, exit codes
* [Quality gate](interfaces/quality-gate.md) — the checks between build and publish
* [Observability](interfaces/observability.md) — `/healthz`, status file, ntfy
* [Builder API](interfaces/builder-api.md) — the internal build trigger

# Decisions

* [Decision records](decisions/) — thirteen records covering why the system has
  this shape, including every alternative that was rejected

# Recipes

* [Zensical + Obsidian](recipes/zensical-obsidian.md) — the reference deployment
* [Navigation convention](recipes/nav-frontmatter-convention.md) — front matter
  to menu

# Operations

* [Cutover runbook](operations/cutover-runbook.md) — the migration, phase by phase
* [Failure modes](operations/failure-modes.md) — everything found in the red-team pass
* [Doctor checks](operations/doctor-checks.md) — the diagnostics that keep support
  tractable

# History and provenance

* [Design narrative](history/design-narrative.md) — how the design got here
* [Audit of the current setup](history/current-setup-audit.md) — verified facts,
  and four corrections to the session
* [Legacy workflow findings](history/legacy-workflow-findings.md) — three live bugs
* [Design session transcript](history/design-session-transcript.md) — primary
  source, German
* [Original concept note](history/original-concept-note.md) — primary source, German

# Reading this bundle

Every concept declares its `type` and its `sources`. The two German primary sources
are authoritative for the design session; where an English concept disagrees with
them, they win — except where the
[audit](history/current-setup-audit.md) supersedes both, which it does for anything
concerning the current repository.

The core pipeline is implemented and exercised end to end — sync, assemble, hooks,
build, gate, atomic publish, serving, `doctor`. Not yet built: the notify_push
trigger and the status file written back to Nextcloud; polling covers the first,
`/healthz` and ntfy cover the second. Per-section status is in
[Configuration](interfaces/configuration.md); concepts describing
designed-but-unbuilt interfaces carry `status: draft`.
