# Working on ncpages

ncpages watches a folder in Nextcloud over WebDAV, runs a configurable build when
it changes, gates the output, publishes it with one atomic `rename(2)`, and serves
it. One Rust binary with several roles; the generator and every side effect live in
user-supplied hook scripts, never in the core.

**The design is not in the code comments — it is in [`knowledge/`](knowledge/index.md)**,
an OKF v0.2 bundle. Every non-obvious constraint has a concept page that states the
rejected alternatives. Read the page that governs what you are about to change; do
not re-derive it from the source, and do not re-litigate a decision record without
saying so.

Start at [Overview](knowledge/overview.md) · [Pipeline](knowledge/architecture/pipeline.md) ·
[Decision records](knowledge/decisions/index.md) · [Glossary](knowledge/glossary.md)
(terms here have specific meanings: *release*, *recipe*, *gate*, *conflict copy*).

## Invariants — breaking one of these is a design change, not a bugfix

* **Nothing ever writes into the watched folder.** [no-writes-to-the-source](knowledge/decisions/no-writes-to-the-source.md)
* **Executable config and scripts never live in the vault**, enforced fail-closed. [code-outside-vault](knowledge/decisions/code-outside-vault.md)
* **Publishing is `rename(2)` on a symlink**, in one filesystem. No other backend. [symlink-swap-publish](knowledge/decisions/symlink-swap-publish.md)
* **The builder has no credentials and no egress; the watcher has no build tools.** [watcher-builder-split](knowledge/decisions/watcher-builder-split.md)
* **The core knows nothing about any generator.** Zensical, webmentions and nav logic are *recipe*, not core. [hooks-not-plugins](knowledge/decisions/hooks-not-plugins.md) · [recipes](knowledge/recipes/index.md)
* **A running build is never cancelled**, only superseded. [queue-latest-over-cancel](knowledge/decisions/queue-latest-over-cancel.md)
* **Push is an accelerator, never a dependency** — polling never stops. [trigger-composition](knowledge/decisions/trigger-composition.md)
* **The work stays outside Nextcloud**; the companion app is a UI sink. [service-not-nextcloud-app](knowledge/decisions/service-not-nextcloud-app.md)

## Code → the page that governs it

| Code | Read first |
|---|---|
| `src/source.rs` | [webdav-over-inotify](knowledge/decisions/webdav-over-inotify.md), [source-as-working-copy](knowledge/decisions/source-as-working-copy.md), [security model § trusting the source](knowledge/architecture/security-model.md) |
| `src/push.rs` | [notify_push over webhook_listeners](knowledge/decisions/notify-push-over-webhook-listeners.md) |
| `src/scheduler.rs`, `src/state.rs` | [state machine](knowledge/architecture/state-machine.md), [trigger composition](knowledge/decisions/trigger-composition.md) |
| `src/pipeline.rs` | [pipeline](knowledge/architecture/pipeline.md) — the step order is fixed and the reasons are there |
| `src/hooks.rs` | [hook contract](knowledge/interfaces/hook-contract.md) — env vars and exit codes are a public API |
| `src/gate.rs` | [quality gate](knowledge/interfaces/quality-gate.md) |
| `src/publish.rs`, `src/serve.rs` | [delivery](knowledge/architecture/delivery.md), [single-binary-roles](knowledge/decisions/single-binary-roles.md) |
| `src/agent.rs` | [builder API](knowledge/interfaces/builder-api.md), [build environment](knowledge/architecture/build-environment.md) |
| `src/report.rs` | [status reporting](knowledge/interfaces/status-reporting.md) — the JSON payload is a wire contract |
| `src/config.rs` | [configuration](knowledge/interfaces/configuration.md) — field by field, with implementation status |
| `src/main.rs` | [command line](knowledge/interfaces/cli.md) |
| `src/doctor.rs` | [doctor checks](knowledge/operations/doctor-checks.md), [failure modes](knowledge/operations/failure-modes.md) |
| `Dockerfile`, compose | [topology](knowledge/architecture/topology.md), [external bridge network](knowledge/decisions/external-bridge-network.md), [small static binary](knowledge/decisions/small-static-binary.md) |
| `tests/` | [test strategy](knowledge/operations/test-strategy.md) — four layers, and what each can prove |

## Before you plan anything larger

* [Roadmap](knowledge/roadmap.md) — v1 scope, what is deliberately excluded, and what
  needs a measurement before it is worth building (currently RFC 6578 sync tokens).
* [Open questions](knowledge/open-questions.md) — undecided, with what each one blocks.
  If your task touches one, say so instead of picking silently.
* [Security model](knowledge/architecture/security-model.md) — a build is code execution;
  this is the page that explains why the containers are split the way they are.
* [History](knowledge/history/index.md) — primary sources are German; the
  [audit](knowledge/history/current-setup-audit.md) supersedes them for anything about
  the current repository.

## Keeping the bundle true

The bundle is documentation *of record*, so a behaviour change lands with its page.

* Every concept file needs OKF frontmatter (`type`, `title`, `description`, `status`,
  `sources`). Copy the shape from a neighbour.
* Append to [`knowledge/log.md`](knowledge/log.md) — creations, updates, corrections.
* A new page also needs an entry in its section index **and** in `zensical.toml`'s nav.
* A decision that reverses an earlier one supersedes that record explicitly; records
  are not deleted.

```sh
uv run python tools/check_bundle.py knowledge   # OKF conformance + link resolution
uv run zensical build --clean                   # the docs site
cargo test                                      # unit + integration
./examples/local-dev/smoke-test.sh ./target/release/ncpages   # end to end, throwaway vault
```
