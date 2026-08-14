---
type: Decision Record
title: Scripts and build configuration never live in the vault
description: The single decision that shaped the architecture — content and executable configuration are separated, enforced fail-closed.
tags: [decision, security, architecture, vault]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-15T00:45:00Z }
sources:
  - id: session
    resource: ../history/design-session-transcript.md
    title: ncpages design session, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
---

# Context

The original sketch was "drop a bash script somewhere and run it on change". The
convenient place for that script is the watched folder itself — editable from a
phone, no SSH required.

# Decision

Scripts and build configuration live in a root-owned, read-only config directory
outside the vault. The vault supplies content, never execution logic. If the hook
directory resolves inside `source.path`, the service **refuses to start**.

# Rationale

A build is code execution by design. If `build.sh` were in the Nextcloud folder,
then everyone with write access to that folder would have a shell on the server.
That set includes:

* a compromised phone,
* an old sync client with a stored app password,
* every person the folder is ever shared with,
* every federated share.

The previous setup ran builds in GitHub Actions — also code execution, but behind
branch protection, commit signatures and an audit log. Removing all of that with
nothing in its place would not have been progress.

Fail-closed rather than a warning, because someone will otherwise take the
convenient path exactly once and forget.

# Consequences

* **You can no longer change the build from your phone.** That is the feature, not
  the cost.
* **Templates moved to the code side.** A `custom_dir` of Jinja templates is
  executable; it belongs in the config directory.
* **CSS stayed in the vault.** `stylesheets/extra.css` is inert, so appearance
  remains editable without SSH. The line is drawn at "can this become code", not
  at "is this a text file".
* This decision motivated the [watcher/builder split](watcher-builder-split.md) and
  immediately eliminated option (b) in the
  [navigation decision](navigation-from-frontmatter.md) — putting the generator
  config in the vault would have been vault-editable code execution.
* Changing build behaviour now requires server access. For a single-operator
  homelab that is acceptable; for a shared deployment it is the point.
