---
type: Build Environment
title: Builder image and runtime
description: How the build container is constructed so that it needs no network at runtime, and the constraints that shaped it.
tags: [docker, build, python, supply-chain, architecture]
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

The builder image is generator-specific and therefore part of a
[recipe](../recipes/index.md), not of the core. The reference recipe builds a
Python-based generator; the rules below are what made that image work, and most of
them generalise.

# Rules

**Dependencies are baked into the image.** Installed at image build time with a
frozen lockfile, never at runtime. That is what makes "no egress at runtime"
achievable at all, and it turns a dependency change into a deliberate image rebuild
rather than silent drift. See
[Baked dependencies](../decisions/baked-dependencies.md).

**No runtime installer.** The originally planned `install_script` hook was dropped
entirely once dependencies moved into the image.

**Local extension code via `PYTHONPATH`, not an editable install.** The reference
recipe's Obsidian handling is a Python-Markdown extension loaded by module name.
Editable installs write absolute paths into `.pth` files and break when the volume
layout changes.[^session]

**Slim, not Alpine.** The generator is maturin-based; without `musllinux` wheels,
pip compiles the Rust part from source on Alpine. Image size is irrelevant here
compared to build time and reproducibility.[^concept]

**Every fetched binary pinned by version and sha256.** Downloading tools at runtime
would reintroduce both egress and an unverified supply chain.

**Fixed UID, identical to the watcher.** Otherwise the second build fails with
`EACCES` on `releases/`.

**`read_only: true` needs a writable `/tmp`.** Python will not run without one;
mount a tmpfs.

# Runtime contract

The builder exposes an HTTP endpoint and nothing else. It receives a trigger, runs
one command in the assembled tree, and reports the exit code. It never reaches the
network, never sees a secret, and holds no state between runs. See
[Builder API](../interfaces/builder-api.md).

# Multi-arch

`amd64` and `arm64` both, if this is published. The audience is homelab and
selfhosting, a large part of which runs on arm64; amd64-only halves the usable
audience.[^session]

[^session]: Design session, parts 1.7, 2.8 and 3.4.
[^concept]: Concept note, section 3.3.
