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

**One executor, four phases.** The build is not a mechanism of its own: it is a
program run the same way the hooks are, with the same environment contract and
the same timeout. What differs between the phases is policy, not machinery.

# Phases

| Phase | Purpose | Non-zero exit | Environment |
|---|---|---|---|
| `pre_build` | generate navigation, fetch external data | `1` warns, else aborts | cleared |
| `build` | run the generator | always aborts | inherited |
| `post_build` | post-process HTML, before the gate | `1` warns, else aborts | cleared |
| `post_publish` | irreversible effects: webmentions, cache purge | `1` warns, else aborts | cleared |

Two deliberate asymmetries:

**Exit codes.** A generator follows the ordinary Unix convention and knows
nothing about ours, so for `build` any non-zero exit is a failure. Reading `1` as
"warning, carry on" there would publish broken output.

**Environment.** Hooks start from a cleared environment — only `PATH`, the
`NCPAGES_*` contract, and whatever `env_passthrough` names — because they sit
closest to the secrets. The build inherits the container's environment instead,
because the image *is* the generator's configuration: `PATH` into a virtualenv,
`PYTHONPATH` into the assembled tree, and whatever else the recipe baked in.

By default all four run in the same container. `build.kind = "agent"` moves the
build into an isolated container with no credentials and no egress; that changes
where it runs, not what it is. See
[Watcher/builder split](../decisions/watcher-builder-split.md).

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
