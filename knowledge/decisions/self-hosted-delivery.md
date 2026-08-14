---
type: Decision Record
title: ncpages serves the site itself
description: Adding a small static web server to the stack removed every remote publish backend, deploy secret and atomicity workaround.
tags: [decision, delivery, simplification, architecture]
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

Until late in the design, the assumption was that a publish *target* exists outside
the stack — git-pages, or some web server. That assumption produced a menu of
publish backends: rsync, SSH keys, upload-and-swap, and a long discussion about how
to make any of them atomic.

The objection that broke it: if everything already goes through WebDAV, the machine
should not matter.

# Decision

ncpages ships its own static file server rooted at `current/`. The operator proxies
to it like to any other container.

> **Refined later:** the server is a task inside the ncpages binary rather than a
> separate image. See [One binary with roles](single-binary-roles.md).

# Rationale

The objection is **completely right for the input side** and **wrong for the
output side**:

* Input: WebDAV fetches the files, exactly as the desktop client does. Generic and
  machine-independent.
* Output: atomicity requires `rename(2)` within one filesystem. No network protocol
  offers "swap two directories atomically". Without it the gate is useless (the
  site is mixed during upload) and webmentions are mistimed (there is no defined
  moment of "live"). That is exactly the behaviour of `git-pages-cli --upload-dir`
  that motivated the move.

Rather than weaken the guarantee to fit remote publishing, the guarantee was kept
and the target moved into the stack.

# Consequences

* Every remote publish backend, SSH key and deploy secret disappeared from the
  design. The user contract is **two volumes and one port**.
* TLS, certificates, DNS and routing become the operator's reverse proxy's job —
  explicitly out of scope.
* Caching headers become ncpages' responsibility; git-pages used to set them. See
  [Delivery](../architecture/delivery.md).
* The web server is the only real single point of failure, and it has no
  dependencies — deliberately no `depends_on`, so it keeps serving while watcher
  and builder are down.
* This was the largest simplification of the session, and it came from the
  objection, not from the analysis.
