---
type: Playbook
title: Cutover runbook — from GitHub Actions to ncpages
description: The seven-phase migration from a git-based publishing chain to ncpages, including parallel operation and secret revocation.
tags: [operations, migration, cutover, runbook]
status: draft
stale_after: 2026-12-31
generated: { by: claude-code/opus-5, at: 2026-08-15T00:45:00Z }
sources:
  - id: concept
    resource: ../history/original-concept-note.md
    title: ncpages concept note, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
---

Written for the reference deployment (Nextcloud on a home server, Obsidian vault,
Zensical, git-pages). The goal state: the git-pages app is uninstalled, the vault
syncs through Nextcloud, and publishing needs no git and no GitHub.

# Phase 0 — Prerequisites (blocks everything else)

- [x] ~~Decide the phase of `fetch_comments.py`~~ → `post_build`, confirmed by
      [audit](../history/current-setup-audit.md)
- [x] ~~Clarify `google-genai`~~ → not in the build path; `update_images.py` stays
      an authoring-time tool
- [ ] Decide `category` vs. `nav` and where ordering lives
      ([open question 1](../open-questions.md))
- [ ] Decide how the first webmention run is seeded — nothing has ever been sent,
      so an unseeded first run flushes the whole backlog
- [ ] Rotate `GEMINI_API_KEY`; it is committed in `.env` in the source repository
- [ ] Create the vault layout in Nextcloud: `docs/` only (plus `docs/stylesheets/`)
- [ ] Create a dedicated Nextcloud **app password** for the watcher
- [ ] Verify `trusted_proxies` contains the nginx container's subnet

# Phase 1 — Push path

- [ ] Add the notify_push sidecar to the Nextcloud stack, `config.php` mounted
      read-only, `NEXTCLOUD_URL` pointing at the HTTP frontend
- [ ] `occ app:install notify_push`, then `occ notify_push:setup https://…/push`
- [ ] nginx: `map $http_upgrade $connection_upgrade` in `http{}`, plus
      `location ^~ /push/` with upgrade headers and a long `proxy_read_timeout`
- [ ] Verify with the bundled test client — including the internal path
      `ws://notify-push:7867/ws` that the watcher will use
- [ ] Check `fastcgi_param HTTPS on;` in the PHP location block

# Phase 2 — Network and skeleton

- [ ] `docker network create --driver bridge --subnet 172.28.0.0/16 cloud-bridge`
- [ ] Attach nginx and notify-push to the bridge network
- [ ] Idempotent `net` target in the Makefile — compose cannot express cross-stack
      dependencies
- [ ] Create the root-owned config directory with generator config (without `nav`),
      dependency manifests, `overrides/`, extension source

# Phase 3 — Navigation migration

- [ ] Dry-run `migrate_nav.py` against a **copy** of the vault
- [ ] Review the result in Obsidian's properties view
- [ ] Apply, then remove the `nav` tree from the generator config
- [ ] Keep the old config with its `nav` tree until the first build succeeds
- [ ] Install `nav_from_frontmatter.py` as a `pre_build` hook

# Phase 4 — Builder image

- [ ] Dockerfile on a slim base, package manager pinned to a concrete tag,
      dependencies installed frozen and without the project itself
- [ ] `PYTHONPATH` into the build tree, no bytecode writing, tmpfs on `/tmp`
- [ ] Pin every fetched binary by version **and** sha256
- [ ] HTTP agent with `/build` and `/healthz`, token auth
- [ ] Fixed UID identical to the watcher
- [ ] Verify `read_only: true` actually works

# Phase 5 — Core

Implementation order for the service itself:

- [ ] `Source` trait: `webdav` (ETag `Depth: 0` → descend `Depth: 1`) and `fs`,
      both into one event channel
- [ ] notify_push client with reconnect and backoff; poll keeps running
- [ ] Timer source with jitter
- [ ] State machine with `queue_latest`, max one running plus one waiting
- [ ] Persistence of ETag, content hash and build history
- [ ] Startup reconcile
- [ ] Assemble: overlay + vault content
- [ ] Hook runner with four phases and the environment contract
- [ ] Gate
- [ ] Symlink swap and retention
- [ ] Bootstrap holding page
- [ ] Status report with self-write protection
- [ ] `/healthz`
- [ ] Backoff on 503, immediate stop on 401
- [ ] **Fail-closed overlap check**

# Phase 6 — Cutover

- [ ] Caching headers: `immutable` + long `max-age` for hashed assets, `no-cache`
      for HTML
- [ ] Reverse proxy block pointing at the ncpages web server
- [ ] **Parallel operation**: new stack on a test domain, old workflow still live
- [ ] Compare output: page count, sitemap, spot checks
- [ ] Switch DNS, leave the old target in place until the TTL has passed
- [ ] Disable the GitHub workflow
- [ ] Remove the deploy and API secrets from GitHub
- [ ] Uninstall the git-pages app from the server

The last two steps are the point of the exercise; do them only after the site has
run on ncpages long enough to have survived a failed build, a restart, and a timer
run.

# Phase 7 — Publication (optional, afterwards)

See [Roadmap](../roadmap.md).
