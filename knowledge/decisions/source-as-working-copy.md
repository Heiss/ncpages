---
type: Decision Record
title: The source directory is a persistent working copy, not a cache
description: Reframing the local copy as durable state makes Nextcloud a soft dependency — the site keeps building and serving when the cloud is down.
tags: [decision, resilience, state, architecture]
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

The blog should have its own stack but depend on Nextcloud. "Depend" turned out to
mean two different things: independence at startup, and substitutability of the
source.

# Decision

Treat the local copy as a **persistent working copy**, not a cache. Nextcloud is
merely *one* mechanism for updating it. `source.required` defaults to `false`, so
an unreachable source degrades the service instead of stopping it.

# Consequences

* If Nextcloud is down, the site stays live and timer builds keep running — so
  external data (comments, webmentions) still flows in. Only vault sync pauses.
* State that must be persisted: last ETag, last content hash, build history.
  Without persistence, every `compose up` triggers a full rebuild — and the
  reconcile logic stays untested until the day it is actually needed.
* `/healthz` reports `degraded` rather than failing, distinguishing "the source is
  unreachable" from "the service is broken".
* An `fs` source kind becomes conceptually easy: it updates the same working copy
  by a different mechanism.
* Startup order between the two stacks stops mattering. There is no `depends_on`
  across stacks to get right, because there is no hard dependency to express.
