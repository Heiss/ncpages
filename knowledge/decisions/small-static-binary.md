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

# What the base image has to satisfy

The binary is static, so it imposes nothing: it runs on Alpine, Debian, Ubuntu,
distroless and `scratch` alike. Only the *other* things in a container decide
what base is right, and they differ by role:

| Role | Needs from the base | Whose image |
|---|---|---|
| `serve` | nothing | ours (`scratch`) |
| `run`, `watch` | a shell, plus whatever the **hooks** need | ours by default, yours when hooks need more |
| `build-agent` | whatever the **generator** needs | always the recipe's |

The builder was never a question: a recipe brings its own image anyway, on
whatever base its generator wants, and takes the binary with one `COPY --from`.

The watcher is the one worth stating, because hooks run there. Shell hooks and
deployments with no hooks at all are fine on Alpine. A Python hook whose
dependencies carry C extensions is not: musl means no `manylinux` wheels, so pip
compiles from source — the same trap that keeps the reference builder on
`python:3.13-slim` rather than Alpine.

**The answer is not a family of base images.** Shipping `alpine`, `debian` and
`ubuntu` variants would multiply the build, scan and test matrix, and would still
be guessing which runtime someone wants preinstalled. Instead the binary is
portable enough that deriving an image is three lines:

```dockerfile
FROM python:3.13-slim
COPY --from=ghcr.io/heiss/ncpages:main /usr/local/bin/ncpages /usr/local/bin/ncpages
```

One mechanism, used identically for the builder and for a heavier watcher. There
is a worked example in `examples/zensical/watcher/Dockerfile`.

Two constraints survive into any derived image: the **same UID** as the builder,
or the second build fails with `EACCES` on the releases directory; and a
**writable `/tmp`** if the root filesystem is read-only.

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
