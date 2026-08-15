---
type: Decision Record
title: A service beside Nextcloud, not a Nextcloud app
description: The core stays a standalone binary; the companion app is a UI sink and never owns change detection, building or publishing.
tags: [decision, architecture, nextcloud-app, scope, longevity]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-15T21:30:00Z }
sources:
  - id: operator
    resource: ../history/design-session-transcript.md
    title: Operator review, 2026-08-15 — "do we need a Rust service at all?"
    author: human:heiss
    last_modified: 2026-08-15
---

# Context

A companion Nextcloud app is already planned as rung 3 of the setup ladder, and
[RFC 6578 collection sync](../roadmap.md) is on the roadmap as a possible
replacement for descending the tree by ETag. Both raise the same objection:

> If an app ships anyway, why is there a separate service? Let the app do the work
> and put a plain web server beside it, so the site stays up when Nextcloud is
> down. The architecture barely changes — and if the RFC already delivers every
> change including deletions, the app adds nothing to the data path either.

The second half of that is right, and it is why this record exists.

# Decision

**The core stays a standalone binary that runs beside Nextcloud and treats it as
a soft dependency.** The companion app is a *reporting and UI sink* — it receives
the [report payload](../interfaces/status-reporting.md), and it never owns change
detection, scheduling, building or publishing.

# What the objection gets right

* **Serving must survive Nextcloud, and does.** That property is already bought by
  the symlink root and the dependency-free web role, not by where the logic lives.
  See [One binary with roles](single-binary-roles.md).
* **Build availability is a non-argument.** If Nextcloud is down, the content is
  not changing either, so there is nothing to build.
* **An app would add nothing to the data path.** Its only real advantage over
  polling would be a file-event hook delivering *what* changed. notify_push already
  gives the *that* in about a second, and RFC 6578 gives the *what*, including
  deletions, from a frozen protocol with no server-side install. Whatever remains
  is UI.

# Why the app-only variant fails anyway

**It does not remove a component.** Something must run permanently outside
Nextcloud to serve the site — that is settled. The app-only variant replaces one
container with one container, and *adds* a mount of the release volume into the
Nextcloud container. There is no deployment simplification to buy.

**The build cannot live in PHP.** A generator needs Python or Node in a sandbox
with no egress and no credentials — the boundary carrying the actual security
argument, see [Watcher/builder split](watcher-builder-split.md). An app runs inside
the Nextcloud container, which has neither the toolchain nor any business getting
it. Reaching a sibling builder container from there means a Docker socket inside
Nextcloud, which is host root for *every* installed app. So a builder container and
something to drive it are needed regardless; that is
[the builder API](../interfaces/builder-api.md).

**Nextcloud has no execution model for long work.** Everything long-running goes
through background jobs at a five-minute default — the exact reason
[webhook_listeners was rejected](notify-push-over-webhook-listeners.md). Debounce,
`queue_latest`, timeouts and guaranteed phase ordering for irreversible steps would
have to be rebuilt in the least suitable host available. The problem is the
request/cron lifecycle, not PHP's speed; comparing an ETag is fast in any language.

**Blast radius grows.** Today the builder holds no credentials and the watcher runs
no PHP. In the app-only variant the Nextcloud container writes into the publicly
served directory, so any vulnerability in any installed Nextcloud app becomes
arbitrary content on the operator's website.

**It costs reach and it costs maintenance.** An app requires admin rights on the
server, which forfeits the [public share link](../interfaces/configuration.md)
credential — today ncpages can publish from a Nextcloud the operator does not
administer — and the `fs` source with it. A PHP app then chases major versions,
deprecated APIs and app store review every year. A WebDAV client does not; the
protocol is finished.

# Consequences

* The [status reporting](../interfaces/status-reporting.md) wire contract stays the
  entire interface to the app. If a future feature needs the app to push data *into*
  the pipeline, this record is what it has to argue against.
* The app remains optional at every rung, and absent by default: one `OPTIONS`
  probe per fifteen minutes.
* What the app is genuinely good for is unchanged and worth building later: build
  history, a diff between releases, surfacing conflict copies, a rollback button,
  and configuration from the Files UI. All of it needs a Nextcloud session and a
  front end, and none of it needs to be in the core.
* **RFC 6578 stays where it is** — an optimisation waiting on a measurement, not a
  replacement for anything. Ruling out the app on the data path does not promote
  it. For a flat vault the ETag descent already costs two requests, the token
  expiry path stays untested until it runs, and whether the `sync-token` `REPORT`
  works over `public.php/dav` at all is unverified. See the roadmap.
* Rejecting the app-only shape is not the same as rejecting a Nextcloud-side
  trigger. If one is ever added it enters through the existing
  [trigger composition](trigger-composition.md) as a third accelerator, and polling
  still never stops.
