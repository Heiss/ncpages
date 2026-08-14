---
type: Product Overview
title: ncpages
description: A service that watches a Nextcloud folder over WebDAV, runs a configurable build when it changes, and serves the result atomically from its own web server.
tags: [ncpages, nextcloud, webdav, static-site, homelab]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-15T00:45:00Z }
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
---

ncpages watches a folder in Nextcloud, and when its contents change it assembles a
build tree, runs a site generator in an isolated sandbox, checks the output against
quality rules, and swaps it live in a single atomic operation. It then serves the
result itself over plain HTTP.

The motivating case: an Obsidian vault syncs to Nextcloud, and the blog goes live
without touching git. The service replaces the chain *git → GitHub Actions →
git-pages* for a personal site,[^concept] but nothing in the core is specific to
that stack — the generator, the navigation logic and every external side effect
live in user-supplied scripts.

# Scope

**In scope.** Change detection, scheduling, build isolation, quality gating,
atomic publish, ordered side effects, status reporting, serving static files.

**Out of scope.** TLS, certificates, DNS, domain routing. The operator's reverse
proxy handles those, exactly as it does for any other container.[^concept]

**Explicitly not a goal for v1.** Sources for S3, Dropbox or SFTP. The `Source`
abstraction exists in code, but only `webdav` and `fs` ship. Generalising storage
backends before the Nextcloud path is solid is a known project-killer; the one
abstraction v1 needs is *core versus recipe*.[^session]

# When you do not need it

Anyone running Nextcloud on local storage without server-side encryption, building
on the same machine, with no irreversible post-publish steps, can do this with
`watchexec` and a shell script. That comparison belongs in the README, honestly
stated.[^session]

ncpages earns its complexity in five places:

* **WebDAV instead of the filesystem** — works with S3 primary storage and
  server-side encryption, where inotify cannot work in principle.
  See [WebDAV over inotify](decisions/webdav-over-inotify.md).
* **Push instead of polling** — roughly one second of latency via notify_push,
  with polling retained as a safety net.
  See [notify_push over webhook_listeners](decisions/notify-push-over-webhook-listeners.md).
* **A gate against half-synced state** — a sync error must not be able to replace
  a blog with a three-page site. See [Quality gate](interfaces/quality-gate.md).
* **Atomic publish** — `rename(2)` on a symlink; no request ever sees half a site.
  See [Symlink swap as the only publish backend](decisions/symlink-swap-publish.md).
* **Guaranteed phase ordering for irreversible steps** — webmentions, cache purges
  and search-engine pings fire only after a verified state is actually live.
  See [Hook contract](interfaces/hook-contract.md).

# Shape of the system

One Rust binary with several roles, and one privilege boundary that is never
optional. The *watcher* role holds the Nextcloud credentials and all network
egress; the *serve* role answers HTTP from the current release and runs in the same
process by default; the *builder* runs in its own container with the build tools
and neither credentials nor egress.

Deployments that want the site to survive a watcher crash split `serve` into its
own container — same binary, different role. See
[One binary with roles](decisions/single-binary-roles.md).

Read next: [Pipeline](architecture/pipeline.md) for the ten-step flow,
[Topology](architecture/topology.md) for containers, networks and volumes, and
[Security model](architecture/security-model.md) for why the split exists.

[^session]: Design session, part 2.1 and 3.4.
[^concept]: Concept note, sections 1 and 2.
