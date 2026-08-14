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

The builder holds no secrets. Every outward access — WebDAV, comment APIs,
webmentions — happens in the watcher. This is stricter than the GitHub job it
replaces, where build and token access shared one context.[^session]

# Special case: the self-triggering loop

The status report is written back to Nextcloud. If its path were inside the watched
folder, writing it would change the root ETag, which triggers a build, which writes
the status again — forever. Path excludes do not help, because the root ETag is
path-blind: it changes for *any* descendant.

Two mitigations, both required:

1. the status path must be a sibling folder, outside `source.path`;
2. the watcher keeps a fingerprint of the state it last wrote itself and ignores a
   change that matches it.

# Supply chain

Every binary fetched into the builder image is pinned by version **and** sha256, at
image build time, not at runtime. The workflow being replaced downloaded two
binaries as `latest` without checksums and ran them as root in a job that had the
deploy credentials in its environment. See
[Legacy workflow findings](../history/legacy-workflow-findings.md).

[^session]: Design session, parts 1.4, 1.5, 2.4 and 3.4.
[^concept]: Concept note, section 3.2.
