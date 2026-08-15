---
type: Interface Contract
title: Command line
description: The roles one binary can take, and which of them a deployment needs.
tags: [cli, interface, deployment]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-15T13:30:00Z }
sources:
  - id: implementation
    resource: https://github.com/Heiss/ncpages/blob/main/src/main.rs
    title: src/main.rs
    author: human:heiss
    last_modified: 2026-08-15
---

One binary, several roles. Rationale in
[One binary with roles](../decisions/single-binary-roles.md).

```
ncpages [--config /etc/ncpages/ncpages.toml] <command>
```

| Command | Runs | Typical use |
|---|---|---|
| `run` | watcher, scheduler and HTTP server in one process | the homelab default: one container |
| `watch` | watcher and scheduler, plus `/healthz` | split deployment, alongside `serve` |
| `serve` | HTTP server only | split deployment, survives a watcher crash |
| `build-agent --listen ADDR` | the build endpoint | inside the isolated builder container |
| `build` | one build now, regardless of changes | first run, manual republish, debugging |
| `doctor` | diagnostics against this deployment | before opening an issue |
| `check` | parse and validate the config, then exit | CI, and before a restart |

# Exit codes

| Command | Non-zero when |
|---|---|
| `build` | the build failed, or the gate refused it |
| `doctor` | at least one check failed (warnings do not fail) |
| `check` | the config is invalid |
| `run`, `watch`, `serve`, `build-agent` | startup failed; otherwise they run until terminated |

`build` exits `0` when there was nothing to do, which makes it safe in a cron job.

# Logging

`NCPAGES_LOG` takes a level name — `trace`, `debug`, `info` (default), `warn` or
`error`. `NCPAGES_LOG=debug` adds per-file and per-request detail.

Directive strings like `ncpages::source=debug` are deliberately not supported:
the regex-based filter that parses them costs about a megabyte of binary, which
is a poor trade for a service with nine modules. See
[A small static binary](../decisions/small-static-binary.md).

# Signals

`SIGTERM` and `SIGINT` stop the scheduler after the current step, persist state,
and exit — so `docker stop` does not look like a crash on the next start.

# What none of these commands do

There is no `ncpages init`, no interactive setup and no config generation. The
configuration lives in a directory the operator controls, and a tool that writes
its own config into that directory would blur the line the security model draws.
