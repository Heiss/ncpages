---
type: Delivery Model
title: Delivery and atomic publish
description: Why ncpages serves the site itself, how the symlink swap works, retention, and who owns caching headers now.
tags: [delivery, publish, atomicity, caching, architecture]
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

ncpages serves the finished site itself, from an HTTP server inside its own binary
rooted at `current/`. The operator proxies to it like to any other container. A
separate `serve` role exists for deployments that want the web server isolated in
its own container; see [One binary with roles](../decisions/single-binary-roles.md).

This removed an entire branch of the design: remote publish backends, rsync, SSH
keys, deploy secrets and the whole atomicity discussion disappeared with it. The
user contract shrank to *two volumes and one port*.[^session] See
[Self-hosted delivery](../decisions/self-hosted-delivery.md).

# Why input and output are not symmetric

For the **input** side, "the machine does not matter" is entirely true — WebDAV
fetches files, exactly as the Nextcloud desktop client does.

For the **output** side it is false. `rename(2)` on a symlink within one filesystem
is atomic: there is no instant at which a request can see half a site. No network
protocol offers that. Neither WebDAV nor rsync nor S3 has "swap two directories
atomically". Without atomicity the gate becomes pointless (the site is mixed during
upload) and webmentions are mistimed (there is no defined moment of "live"). That
is precisely the behaviour of `git-pages-cli --upload-dir` that motivated the
move.[^session]

# Layout

```
/work/releases/
├── build/            work tree, same volume so mv is atomic
├── 2026-08-15T09-31-04Z/
├── 2026-08-14T21-02-77Z/
├── …                 five retained
└── current →         symlink, swapped by rename(2)
```

`releases/` also replaces the build cache. The `actions/cache` dance in the old
workflow existed only because GitHub Actions is stateless; here the previous build
is simply on disk, and `oldDir` for a webmention diff is `readlink current`.
Retention of five additionally gives rollback.[^concept]

Retention must be enforced actively. An unbounded `releases/` fills the root
filesystem, and a full root filesystem takes Nextcloud down with it.[^session]

# Caching headers are now your problem

git-pages used to set them. Now the stack does:

* `assets/` → long `max-age`, `immutable` — the reference generator emits hashed
  asset names,
* HTML → `no-cache`.

Getting this wrong means old HTML with new CSS after a swap: the browser keeps the
cached HTML, which references asset names that no longer exist.[^concept]

# Bootstrap

If `current` does not exist at startup, the watcher creates a holding-page release
first. Without it the web server answers 404 to everything during the first sync,
which looks like a broken deployment.

[^session]: Design session, parts 1.9, 2.9 and 3.2.
[^concept]: Concept note, sections 3.5 and 6 (phase 6).
