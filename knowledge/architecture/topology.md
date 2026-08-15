---
type: Deployment Topology
title: Container, network and volume topology
description: Three containers with different privileges, an externally created bridge network, three volumes, and the build tree layout.
tags: [docker, compose, networking, volumes, architecture]
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

```
┌─ Stack: nextcloud ────────────┐   ┌─ Stack: ncpages ─────────────────┐
│  db, redis                    │   │                                  │
│  nextcloud (fpm)              │   │  watcher   [nc-bridge, build]    │
│  nginx        ──┐             │   │  builder   [build]  internal     │
│  notify-push  ──┤             │   │  web       [edge]   :8080        │
└─────────────────┼─────────────┘   └───────────┬──────────────────────┘
                  │                             │
              cloud-bridge (external, 172.28.0.0/16)
                                                │
                                 operator's reverse proxy → :8080
```

# Roles

| Container | Credentials | Egress | Build tools | Fails how |
|---|---|---|---|---|
| watcher | yes | yes | no | site stays live, stops updating |
| builder | no | no (`internal: true`) | yes | site stays live, stops updating |
| web | no | inbound only | no | site is down |

The split between watcher and builder is a security decision, documented in
[Watcher/builder split](../decisions/watcher-builder-split.md).

> **The default is one container, not three.** `ncpages run` hosts the watcher,
> the scheduler and the HTTP server in one process, and `build.kind = "local"`
> runs the generator as a subprocess of it. The layout above is what you get by
> opting into both splits:
>
> * `ncpages build-agent` in its own container — the build without credentials or
>   egress; see [Watcher/builder split](../decisions/watcher-builder-split.md),
> * `ncpages serve` in its own container — the site survives a watcher crash,
>   because that role has **no `depends_on`** and depends on nothing.
>
> Same binary, same image, different roles. See
> [One binary with roles](../decisions/single-binary-roles.md).

# Network

`cloud-bridge` is created manually (`docker network create --driver bridge --subnet
172.28.0.0/16 cloud-bridge`) and referenced as `external` by both stacks. Attaching
to the Nextcloud stack's *default* network instead is a trap: a `docker compose
down` there deletes the network and recreates it with a new ID, leaving the ncpages
containers attached to something that no longer exists. A manually created network
survives that, has a fixed subnet for `trusted_proxies`, and makes the direction of
the dependency explicit.[^session] See
[External bridge network](../decisions/external-bridge-network.md).

Only `nginx` and `notify-push` join the bridge. The blog reaches the Nextcloud
*API*, never its database.

**FPM does not speak HTTP.** With `nextcloud:*-fpm-*` as the base image, both
`NEXTCLOUD_URL` (for notify_push) and the WebDAV base URL of the watcher must point
at the nginx container. Pointing them at the FPM container produces errors that
look like authentication failures.[^session] Because the request then arrives
internally as `http://nginx`, the watcher sends an explicit `Host:` header carrying
the real domain, so `server_name` matching and `trusted_domains` still apply.

# Volumes

| Volume | Contents | Writer | Reader |
|---|---|---|---|
| `src` | vault working copy (`docs/`) | watcher | builder (ro) |
| `releases` | `build/`, `releases/<id>/`, `current` | watcher, builder | web (ro) |
| `state` | last ETag, content hash, build history | watcher | — |

Watcher and builder must run under the **same fixed UID**. Different UIDs produce
`EACCES` on `releases/` on the second build, not the first — which makes it look
like a transient failure.[^concept]

# Build tree

Lives on the same volume as `releases/`, so `mv` stays atomic:

```
/work/build/
├── <generator config>   ← /etc/ncpages/   (nav appended by a pre_build hook)
├── <dependency lock>    ← /etc/ncpages/
├── overrides/           ← /etc/ncpages/   (Jinja templates are code)
├── src/<extension>/     ← /etc/ncpages/
├── docs/                ← Nextcloud vault
└── site/                → mv to releases/<id>/
```

# Mounting the symlink

The web server mounts the **parent directory** (`releases:/site:ro`) and takes
`--root /site/current`. Never mount the symlink itself: Docker resolves mount
sources at container start, binds whatever the symlink pointed at *then*, and every
later swap becomes invisible. The site silently never updates — no error, no log
entry.[^session] Open-file caching in the web server is disabled for the same
reason.

[^session]: Design session, parts 1.5 and 1.8.
[^concept]: Concept note, sections 3.5, 3.6 and 4.
