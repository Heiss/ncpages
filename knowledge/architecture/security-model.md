---
type: Threat Model
title: Security model
description: Why a build is code execution, how content is separated from code, how the builder is sandboxed, and where credentials are allowed to exist.
tags: [security, threat-model, sandbox, credentials]
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

The premise everything else follows from: **a build is code execution, triggered by
a change in a cloud folder.** Someone will eventually point this tool at a shared
team folder. The threat model therefore belongs *in front of* the installation
instructions, not in an appendix.[^session]

# Layer 1 — content and code are separated

The vault contains content only: markdown, images, `stylesheets/extra.css`.
Everything executable or configuring lives in `/etc/ncpages/`, root-owned and
mounted read-only.

If `build.sh` or the generator config lived in the Nextcloud folder, then everyone
with write access to that folder would have a shell on the server. That set
includes a compromised phone, an old sync client with a stored app password, every
person the folder is ever shared with, and every federated share.[^session]

The previous setup ran the build in GitHub Actions — also code execution, but with
branch protection, commit signatures and an audit log in front of it. Discarding
that with nothing in its place would not have been progress. See
[Code outside the vault](../decisions/code-outside-vault.md).

Two consequences that are easy to get wrong:

* **Templates are code.** A `custom_dir` of Jinja templates belongs on the config
  side, not in the vault.
* **CSS is not.** `stylesheets/extra.css` stays in the vault deliberately, so
  appearance can be changed from a phone without SSH.

The overlap check is **fail-closed**: if the hook directory resolves inside
`source.path`, ncpages refuses to start and explains why. Warning is not enough —
somebody will take the convenient path otherwise.[^concept]

# Layer 2 — the builder is sandboxed

`internal: true` (no egress), `read_only: true`, `cap_drop: ALL`,
`no-new-privileges`, non-root, memory and time limits, tmpfs on `/tmp`. Even if a
generator extension can execute code, it has nowhere to go.

`network_mode: none` was the first instinct but cannot be combined with a compose
network, and the builder needs one to receive its trigger. `internal: true` is the
honest limit of what Compose offers.[^session]

The builder is triggered over an internal HTTP endpoint with a shared token. Not a
Docker socket — mounting that would be equivalent to handing out root on the host.

# Layer 3 — credentials are isolated

The builder holds no secrets, and no route to the network. The watcher holds
exactly one credential — the source password — and makes exactly one kind of
outward request with it: WebDAV against the watched folder.

**Everything else that reaches outward is a hook script**, and the core knows
nothing about it. Sending webmentions, fetching comments, purging a CDN: those are
programs in the config directory, run as child processes with a cleared
environment plus whatever `env_passthrough` names explicitly. ncpages neither
knows what they contact nor what their secrets are for. See
[Hook contract](../interfaces/hook-contract.md).

This is stricter than the GitHub job it replaces, where the build and every token
shared one context — but it is worth being precise about where the boundary
actually runs:

| | Secrets | Network | Runs |
|---|---|---|---|
| builder | none | none | the generator |
| watcher | the source password | WebDAV to the source | the pipeline |
| hooks | only what is passed in | whatever the watcher's container can reach | recipe-specific work |

Hooks are **inside** the watcher's trust zone, not sandboxed from it: a hook can
reach anything the watcher container can, including Nextcloud. That is precisely
why hook scripts must live outside the vault — a hook is code, and the vault is
writable by everyone the folder is shared with.

# The watcher never writes to Nextcloud

The source is read-only to ncpages. It syncs down and nothing goes back up.

The tempting feature is a status file written into the cloud, so build results are
visible from the same place the content is edited. It is the wrong shape twice
over. It would sync back into the author's Obsidian vault, putting a machine's
output into a space that should only ever be touched by its owner. And writing
anything inside the watched folder changes the root ETag, which triggers a build,
which writes the status again — forever. Path excludes do not help, because the
root ETag is path-blind: it changes for *any* descendant.

Status reporting therefore leaves through a channel of its own. See
[Status reporting](../interfaces/status-reporting.md).

# The source is not trusted either

The vault is treated as hostile content, and so is the server serving it. Two
consequences in the sync path:

**Paths from the server are validated before they touch the filesystem.** A
`PROPFIND` response contains `href` values chosen by the server; one that decodes
to `../../etc/…` would, joined onto the destination unchecked, write wherever the
process can reach. Entries with `..` segments, absolute paths, backslashes or
drive letters are dropped and logged. There is a test that fails when the check is
removed.

**A share link is the least privilege that works.** Where an account credential
grants read *and write* to everything that account can see, a public share is
read-only by construction and revocable in one click. See
[Configuration](../interfaces/configuration.md).

# Supply chain

Every binary fetched into the builder image is pinned by version **and** sha256, at
image build time, not at runtime. The workflow being replaced downloaded two
binaries as `latest` without checksums and ran them as root in a job that had the
deploy credentials in its environment. See
[Legacy workflow findings](../history/legacy-workflow-findings.md).

[^session]: Design session, parts 1.4, 1.5, 2.4 and 3.4.
[^concept]: Concept note, section 3.2.
