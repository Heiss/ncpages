# End-to-end environment

A real Nextcloud with notify_push, and ncpages in the production topology:
watcher and builder in separate containers, the builder on a network with no
egress.

```sh
tests/e2e/run.sh          # run and tear down
tests/e2e/run.sh --keep   # leave the stack up for poking at
```

Takes a few minutes on the first run, mostly pulling images and installing
Nextcloud. Requires Docker with the compose plugin.

## What it asserts

| Assertion | Why it needs a real Nextcloud |
|---|---|
| a seeded vault becomes a published site | ETag propagation is Nextcloud behaviour, not a WebDAV guarantee |
| a new note goes live within 60s while `poll = "300s"` | only notify_push can explain the latency |
| a deleted note disappears | remote deletions must reach the working copy |
| a collapsed vault does not replace the site | the gate, against real sync behaviour |
| `post_publish` ran after the swap | phase ordering in the real topology |
| the builder cannot reach the internet | `internal: true` actually holds |

## Ports

| Port | Service |
|---|---|
| 8081 | Nextcloud (admin / admin-password-123) |
| 8099 | the published site |
| 9099 | `/healthz` |

## Credentials

`etc/password` and `etc/build-token` are throwaway values for this stack, which
is why they are committed. They match `docker-compose.yml` and are useless
anywhere else. Nothing in this directory belongs in a real deployment.

## Why not Nextcloud AIO

AIO's mastercontainer manages its own containers through the Docker socket and
expects a domain with TLS and a web-based installer. That cannot be scripted in
CI, and it would test AIO rather than ncpages. The interfaces under test —
WebDAV, ETag propagation, notify_push — are the same on the plain image.

A manual AIO run is still worth doing before a release, since AIO is what a large
share of self-hosters actually run. That belongs in a release checklist, not in
CI.
