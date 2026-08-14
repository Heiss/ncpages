---
type: Decision Record
title: Bake dependencies into the builder image
description: Dependencies are installed at image build time with a frozen lockfile; nothing is installed at runtime, which is what makes a network-less builder possible.
tags: [decision, build, docker, supply-chain, reproducibility]
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

The original design had two hooks: `install_script` at container start and
`run_script` on change. Installing dependencies at startup is the familiar pattern
from CI.

# Decision

Install dependencies in the `Dockerfile` from a frozen lockfile, without installing
the project itself. At runtime the package manager does not run at all. The
`install_script` hook was dropped entirely.

Locally developed extension code is made importable with `PYTHONPATH` pointing into
the assembled build tree, not with an editable install.

# Rationale

* **A runtime installer means runtime egress.** With dependencies baked in, the
  builder can genuinely run with no network — which is the precondition for the
  whole sandbox argument.
* **Editable installs break on volume changes.** They write absolute paths into
  `.pth` files; the build tree is assembled fresh under a path that need not match.
* **Silent drift becomes a deliberate rebuild.** With a `0.0.x` generator that can
  ship breaking changes, "the same image builds the same site" is worth more than
  automatic updates.

# Consequences

* Updating a dependency requires rebuilding and redeploying the builder image. This
  is the intended friction.
* Slim base image rather than Alpine: the reference generator is maturin-based and
  without `musllinux` wheels pip compiles the Rust part from source.
* Every additional binary in the image is pinned by version **and** sha256 — the
  opposite of the legacy workflow, which fetched two tools as `latest` and ran them
  as root next to the deploy credentials.
* The recipe owns the image. The core does not know what a build needs.
