---
type: Decision Record
title: A small static binary and two images
description: Size choices that survived measurement — musl static linking, a size-oriented release profile, trimmed features, and a shell only where hooks need one.
tags: [decision, build, docker, size, distribution]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-15T18:10:00Z }
sources:
  - id: implementation
    resource: https://github.com/Heiss/ncpages/blob/main/Dockerfile
    title: Dockerfile and Cargo profile
    author: human:heiss
    last_modified: 2026-08-15
---

# Context

The target is a homelab: machines that run a dozen other containers, often on
arm64, often on a boot SSD someone would rather not fill. Pull time and image
size are user-visible costs, and a fat image invites the question of what all of
it is for.

# Decisions, with what each one bought

Measured on the same source, `x86_64`, stripped:

| Change | Size |
|---|---|
| baseline (`lto = "thin"`, strip) | 6.7 MB |
| `opt-level = "s"`, fat LTO, `codegen-units = 1`, `panic = "abort"` | 3.17 MB |
| `opt-level = "z"` | 2.71 MB |
| trimmed `clap` and `tracing-subscriber` features | 2.42 MB |

**`opt-level = "z"` over `"3"`.** The workload is I/O-bound: waiting on WebDAV,
waiting on a generator subprocess, copying files. There is no hot loop where
inlining pays, so size is the better thing to optimise.

**`panic = "abort"`.** Removes unwinding tables, and fixes a real problem at the
same time. With unwinding, a panic inside the serving task kills that task while
the process lives on — the site stops answering and nothing says so. Aborting
turns that into a crash, and a crash into a restart.

**Trimmed features.** `clap` without colours and suggestions, and
`tracing-subscriber` without the regex-based `EnvFilter`. `NCPAGES_LOG` therefore
takes a level name rather than a directive string: for a service with nine
modules, per-target filtering was not worth a megabyte.

**Static musl.** No libc in the runtime image, and the class of bug where the
build image ships a newer glibc than the runtime disappears — a failure this
project hit once, caught by the end-to-end layer.

# Two images

| Target | Base | Contains | For |
|---|---|---|---|
| `runtime` (default) | `alpine:3.22` | binary, `ca-certificates`, a shell | `run`, `watch` |
| `serve` | `scratch` | the binary | `serve` |

The default keeps a shell on purpose: **hook scripts are the extension
interface**, and a script needs something to run in. Shipping a shell-less image
as the default would mean the documented way to extend ncpages does not work in
the image everyone pulls.

The `serve` role runs no hooks and needs no certificates, so it gets `scratch` —
a few megabytes, and nothing to attack that is not the binary itself.

# Rejected

* **Distroless as the default.** Same shell problem, plus another registry to
  depend on.
* **UPX compression.** Saves a megabyte or two, costs startup time, decompresses
  into RAM anyway, and reliably alarms security scanners.
* **`build-std` with a nightly toolchain.** Another megabyte at the price of
  pinning nightly for everyone who builds from source.
* **Dropping `reqwest` for raw hyper.** Would save a little; would cost TLS
  handling written by hand, in the component that holds the credentials.

# Also worth having

A `.dockerignore` that admits only `Cargo.toml`, `Cargo.lock` and `src/`. Without
it the build context includes `target/`, which is gigabytes, and every build
uploads it.
