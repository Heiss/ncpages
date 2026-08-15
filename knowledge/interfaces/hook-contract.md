---
type: Interface Contract
title: Hook contract
description: The four hook phases, what each may touch, the environment variables every hook receives, and the meaning of exit codes.
tags: [hooks, interface, contract, extensibility]
status: draft
generated: { by: claude-code/opus-5, at: 2026-08-15T00:45:00Z }
sources:
  - id: concept
    resource: ../history/original-concept-note.md
    title: ncpages concept note, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
  - id: session
    resource: ../history/design-session-transcript.md
    title: ncpages design session, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
---

This is the extension interface of ncpages. There is no plugin system and there are
no dynamically loaded modules — scripts plus environment variables, an interface
that still works in five years. See
[Hooks, not plugins](../decisions/hooks-not-plugins.md).

# Phases

| Phase | Network | Secrets | Runs in | Purpose |
|---|---|---|---|---|
| `pre_build` | yes | yes | watcher | generate navigation, fetch external data |
| `build` | **no** | **no** | builder | run the generator |
| `post_build` | yes | yes | watcher | post-process HTML, before the gate |
| `post_publish` | yes | yes | watcher | irreversible effects: webmentions, cache purge |

The ordering is the point of the whole structure. `post_publish` is the only phase
allowed to have irreversible outward effect, and it runs only after the gate passed
and the swap succeeded. See [Pipeline](../architecture/pipeline.md).

`post_build` runs *before* the gate deliberately: whatever it produces is subject
to the same quality checks as the generator output.

# Environment

Every hook receives:

```
NCPAGES_SRC_DIR      vault working copy
NCPAGES_BUILD_DIR    assembled build tree
NCPAGES_OUT_DIR      build/site
NCPAGES_RELEASE_DIR  releases/<id>          (from post_build onwards)
NCPAGES_PREV_DIR     previous release       (empty on the first build)
NCPAGES_TRIGGER      push | poll | timer | manual
```

`NCPAGES_PREV_DIR` is what makes diff-based side effects work without a build
cache: a webmention sender compares old and new output directly.

Additional variables reach a hook only through explicit `env_passthrough` in its
config entry.

# Exit codes

| Code | Meaning |
|---|---|
| `0` | success |
| `1` | warning — the build continues, the warning appears in the status report |
| `2` | abort — the pipeline stops, `current` stays where it is |

Any other non-zero exit is treated as `2`.

Exit code `1` exists because some failures should be visible without being fatal:
an external comment API being down should not take a blog post offline.

# Rules for hook authors

* Hooks live in the config directory, outside the vault. A hook directory inside
  `source.path` prevents startup.
* **Hooks run in the watcher image**, so their interpreter and libraries have to
  exist there. The stock image is Alpine with a shell; anything heavier means
  deriving an image on a base of your choice and copying the binary in. See
  [A small static binary](../decisions/small-static-binary.md).
* Hooks must be idempotent. `queue_latest` plus timer triggers means a hook may run
  many times on unchanged content.
* Anything in `post_publish` should be safe to re-run, or must maintain its own
  record of what it already sent.
* Long-running hooks count against the build timeout; there is no separate budget.
