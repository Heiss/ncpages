# ncpages

Watch a folder in Nextcloud, build a static site when it changes, publish it
atomically, and serve it.

The motivating case: an Obsidian vault syncs to Nextcloud, and the blog goes live
without touching git. Nothing in the core is specific to that stack — the site
generator, the navigation logic and every external side effect live in scripts you
supply.

> **Status: design complete, implementation not started.** Nothing here works yet.
> The full design — architecture, interfaces, decision records with the rejected
> alternatives, failure catalogue — is in [`knowledge/`](knowledge/index.md), as an
> [OKF v0.2](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
> bundle. Start at [`knowledge/overview.md`](knowledge/overview.md).

**Documentation: <https://heiss.github.io/ncpages/>** — built with Zensical
directly from `knowledge/`, so the published documentation and the bundle in this
repository are the same files. `tools/check_bundle.py` enforces OKF conformance and
link integrity in CI.

## Bring your own generator

ncpages does not know what a site generator is. It syncs, assembles, gates,
publishes and serves; *what* runs in between is a script you configure:

```toml
[[hooks.pre_build]]
run = "nav_from_frontmatter.py"

[[hooks.post_publish]]
run = "send_webmentions.sh"
```

Four phases — `pre_build`, `build`, `post_build`, `post_publish` — each with a
fixed environment contract and three meaningful exit codes. No plugin API, no
dynamic modules: programs and environment variables, an interface that still works
in five years. Zensical, Quartz, Hugo and mkdocs-material are recipes, not
features.

## What it does

```
notify_push (~1s) ┐
WebDAV ETag poll  ├→ debounce → sync → assemble → pre_build → BUILD (sandboxed)
timer (optional)  ┘                  → post_build → gate → PUBLISH (atomic)
                                     → post_publish (irreversible) → report
```

If any step fails, the live site does not change. The irreversible phase — sending
webmentions, purging caches, pinging search engines — runs only after a verified
build is genuinely live.

## When you do not need it

If you run Nextcloud on local storage without server-side encryption, build on the
same machine, and have no irreversible post-publish steps, then `watchexec` plus a
shell script does this. That is a real answer, not false modesty.

ncpages earns its complexity in five places:

- **WebDAV instead of the filesystem** — works with S3 primary storage and
  server-side encryption, where inotify cannot work in principle
- **Push instead of polling** — about one second of latency, with polling kept as a
  safety net
- **A gate against half-synced state** — a sync error must not be able to replace
  your blog with a three-page site
- **Atomic publish** — `rename(2)` on a symlink; no request ever sees half a site
- **Guaranteed phase ordering** — irreversible outward effects run last, or not at
  all

## Security

A build is code execution, triggered by a change in a cloud folder. Scripts and
build configuration therefore live outside the vault, read-only; the vault holds
content only. If the hook directory is inside the watched path, ncpages refuses to
start. The build runs in a container with no egress, no secrets, a read-only root
filesystem and dropped capabilities.

Read [`knowledge/architecture/security-model.md`](knowledge/architecture/security-model.md)
before pointing this at a shared folder.

## Maintenance expectations

This is a personal project, currently with a bus factor of one, sitting in the
publication path of a website. If that is a problem for your use case, it should
be. Issues and pull requests are welcome; response times are not guaranteed.

## License

Apache-2.0. See [LICENSE](LICENSE).

`ncpages` is an independent project and is **not affiliated with, endorsed by, or
sponsored by Nextcloud GmbH**. "Nextcloud" is a trademark of Nextcloud GmbH.
