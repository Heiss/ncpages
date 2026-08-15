---
type: Pipeline
title: Build pipeline
description: The ten steps from trigger to report, which of them touch the network, and why the order is fixed.
tags: [pipeline, architecture, hooks, publish]
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
┌── Triggers ─────────────────────────────────────────────┐
│  notify_push (WebSocket)  ~1 s                          │
│  WebDAV ETag poll         30 s   (safety net)           │
│  Timer                     6 h   (optional)             │
└───────────────────┬─────────────────────────────────────┘
                    │  debounce 10 s / hard deadline 120 s
                    │  on_busy = queue_latest
                    ▼
  1. SYNC          WebDAV delta → src/                  [network, credentials]
  2. ASSEMBLE      src/ + /etc/ncpages/ → build/        [local]
  3. pre_build     nav from frontmatter, fetch data     [network, credentials]
  4. build         generator run → build/site/          [ISOLATED]
  5. post_build    post-processing on the HTML          [network, credentials]
  6. MOVE          build/site/ → releases/<id>/         [local, mv]
  7. GATE          required files, page count, nav diff [local]
  8. PUBLISH       current → releases/<id>  rename(2)   [ATOMIC]
  9. post_publish  send webmentions, purge caches       [IRREVERSIBLE]
 10. REPORT        retention, then report out (never into the source)
```

If any step fails, `current` keeps pointing where it pointed before. The site is
never in an intermediate state. Step 9 runs if and only if step 8 succeeded.

# Why the order is fixed

A webmention is an HTTP request to someone else's server. Once sent, it cannot be
recalled. It must therefore fire only after the state it announces is genuinely
live — after the gate, after the swap. The same reasoning covers search-engine
pings, CDN cache purges and social posts.[^session]

That single observation produced the four-phase hook structure that is now the core
of ncpages: it is the generalisation of one concrete constraint, not a plugin
system invented up front. See [Hook contract](../interfaces/hook-contract.md).

The same reasoning forbids cancelling a running build. An abort between step 8 and
step 9 leaves a state with no clean way back. See
[queue_latest over cancel](../decisions/queue-latest-over-cancel.md).

# Step notes

**1 · SYNC.** WebDAV delta into the persistent working copy. Descend by ETag:
`Depth: 0` on the root answers *whether* anything changed at all; only then descend
with `Depth: 1`. Change detection uses ETags and content hashes exclusively —
never mtime, which is unreliable across sync boundaries.[^session]

**2 · ASSEMBLE.** Overlay from `/etc/ncpages/` plus the vault content into the
build tree. The build tree lives on the same volume as `releases/`, so step 6 is a
rename rather than a copy. See [Topology](topology.md).

**3 · pre_build.** Runs in the watcher, with network and secrets. This is where the
reference recipe aggregates navigation from frontmatter and fetches external data.

**4 · build.** Runs in the builder: no egress, no secrets, read-only root
filesystem. Triggered over an internal HTTP endpoint with a shared token — never a
Docker socket, which would be equivalent to root on the host. See
[Builder API](../interfaces/builder-api.md).

**5 · post_build.** HTML post-processing, before the gate, so anything it produces
is also subject to the quality checks.

**6 · MOVE.** `mv build/site → releases/<id>` within one filesystem.

**7 · GATE.** See [Quality gate](../interfaces/quality-gate.md). On violation:
do not publish, keep `current`, report loudly.

**8 · PUBLISH.** Atomic symlink swap.

**9 · post_publish.** The only phase permitted to have irreversible outward effect.

**10 · REPORT.** Retention, then the result goes out through channels that do not
touch the source: the companion app if it is installed, ntfy if something needs a
human. See [Status reporting](../interfaces/status-reporting.md).

[^session]: Design session, parts 1.6, 2.2 and 3.2.
