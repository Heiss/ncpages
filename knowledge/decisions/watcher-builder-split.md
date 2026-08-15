---
type: Decision Record
title: Split watcher and builder into separate containers
description: Credentials and network live in one container, build tools in another; neither has both.
tags: [decision, security, docker, isolation]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-15T00:45:00Z }
sources:
  - id: session
    resource: ../history/design-session-transcript.md
    title: ncpages design session, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
---

> **Amended 2026-08-16: the split is now optional, and off by default.** The
> reasoning below still holds for what it protects against — a generator
> compromised through vault content — but that turned out to be narrower than it
> first appeared, and the cost was paid by every operator. Two things changed the
> balance:
>
> * **Hooks already run in the privileged container.** `pre_build` and
>   `post_build` process the same untrusted vault content with network and
>   secrets available. The split never covered them, so it hardened one step of
>   four.
> * **A share link is a read-only credential.** With
>   [`share_token`](../interfaces/configuration.md), the worst a compromised
>   generator gets is read access to one folder — which is most of what the
>   isolation was buying, obtained by configuration rather than topology.
>
> So the default is one container: the generator runs as a subprocess, a crash in
> it is an exit code rather than a dead service, and there is no shared volume,
> no matching UID and no second image to keep in step.
> `build.kind = "agent"` still does everything below, for anyone whose vault is
> shared with people they do not fully trust.

# Context

The simplest implementation is one container that polls, builds and publishes. It
would hold the Nextcloud credentials *and* execute a generator over untrusted
content in the same process space.

# Decision

Two containers with disjoint privileges:

* **watcher** — Nextcloud credentials, network egress, hooks for `pre_build`,
  `post_build`, `post_publish`. No build tools.
* **builder** — build tools, `internal: true` (no egress), `read_only: true`,
  `cap_drop: ALL`, `no-new-privileges`, non-root, memory and time limits, tmpfs on
  `/tmp`. No secrets.

The watcher triggers the builder over an internal HTTP endpoint with a shared
token. See [Builder API](../interfaces/builder-api.md).

# Rejected alternatives

* **Docker socket** — the watcher starting a build container itself. Mounting the
  socket is equivalent to root on the host; it would undo the entire model.
* **`network_mode: none`** — cannot be combined with a compose network, and the
  builder needs one to receive its trigger. `internal: true` is the honest limit of
  what Compose provides.
* **One container, dropped privileges** — no boundary that survives a generator
  extension executing code.

# Consequences

* Even if a crafted page makes a generator extension execute code, it has no
  credentials and no route out.
* Stricter than the GitHub job it replaces, where the build and the deploy tokens
  shared one environment.
* Both containers must run under the **same fixed UID**, or the second build fails
  with `EACCES` on `releases/` — a failure that looks transient and is not.
* `read_only: true` requires a writable `/tmp` for Python; verify it explicitly
  rather than discovering it in the first real build.
* The web server is a third container, deliberately without `depends_on`, so the
  site survives the failure of both others.
