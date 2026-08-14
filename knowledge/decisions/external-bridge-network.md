---
type: Decision Record
title: Connect the stacks through a manually created bridge network
description: A third, externally created network instead of joining the Nextcloud stack's default network, which is destroyed and recreated on every compose down.
tags: [decision, docker, networking, compose]
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

ncpages runs as its own compose stack so the blog can be redeployed without
touching Nextcloud. It still needs to reach the Nextcloud HTTP frontend and the
notify_push service.

# Decision

Create a third network manually and reference it as `external` from both stacks:

```
docker network create --driver bridge --subnet 172.28.0.0/16 cloud-bridge
```

Only `nginx` and `notify-push` from the Nextcloud stack join it.

# Why not the Nextcloud stack's default network

Referencing it as `external` is a trap. A `docker compose down` on the Nextcloud
stack deletes that network and recreates it with a new ID; the ncpages containers
are then attached to something that no longer exists, and the failure appears later
and looks like DNS.

# Consequences

* A manually created network survives restarts of either stack.
* It has a **fixed subnet**, which is what `trusted_proxies` needs. A wrong entry
  there is the most common cause of a broken notify_push setup.
* The direction of the dependency becomes explicit and reviewable: the blog reaches
  the Nextcloud *API*, never its database.
* Compose cannot express cross-stack dependencies, so network creation needs an
  idempotent step outside compose — a `make net` target or an equivalent script.
* Documentation must state this as a prerequisite, before `compose up`. It is the
  first thing a new deployment gets wrong.
