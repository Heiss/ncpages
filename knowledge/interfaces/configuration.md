---
type: Configuration Reference
title: ncpages.toml
description: Full configuration surface of the service, section by section, with the constraints each field carries.
resource: file:///etc/ncpages/ncpages.toml
tags: [configuration, reference, interface]
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

The config file lives in the read-only config directory, never in the vault. Values
below are placeholders; the reference recipe's concrete values are in
[zensical + obsidian](../recipes/zensical-obsidian.md).

`status: draft` — this surface is designed but not yet implemented; field names may
still move before `schema_version = 1` is frozen.

# Example

```toml
schema_version = 1

[source]
kind          = "webdav"                  # webdav | fs
url           = "http://nginx"            # the HTTP frontend, never the FPM container
host_header   = "cloud.example.org"       # real domain, for server_name/trusted_domains
path          = "Notes/blog"
user          = "publisher"
password_file = "/run/secrets/nc_app_password"
required      = false                     # start even if the source is unreachable

[triggers]
push   = "ws://notify-push:7867/ws"       # omit to disable
poll   = "30s"                            # safety net, stays on when push is active
timer  = "6h"                             # omit to disable
jitter = 0.1

[schedule]
debounce  = "10s"
max_delay = "120s"
on_busy   = "queue_latest"

[assemble]
overlay       = ["<generator config>", "<lockfile>", "overrides", "src"]
source_subdir = "docs"

[build]
url     = "http://builder:8080"
timeout = "10m"
output  = "site"

[[hooks.pre_build]]
run = "nav_from_frontmatter.py"
[[hooks.pre_build]]
run = "fetch_external_data.py"
env_passthrough = ["SOME_API_TOKEN"]

[[hooks.post_publish]]
run = "send_webmentions.sh"

[gate]
require_files = ["index.html", "sitemap.xml"]
min_pages     = 5
max_page_drop = 0.4
max_nav_churn = 10

[publish]
kind          = "symlink"
root          = "/work/releases"
keep_releases = 5

[report]
webdav_status_path = "Notes/_blog-status/build.md"   # OUTSIDE source.path
ntfy_topic         = "https://ntfy.sh/…"
```

# Constraints worth stating explicitly

**`source.url` must be an HTTP frontend.** With an FPM-based Nextcloud image, this
points at nginx. Pointing it at the FPM container yields errors that look like
authentication problems.

**`source.host_header`** exists because the internal URL is not the public one.
`server_name` matching and `trusted_domains` need the real domain.

**`source.required = false`** is the default posture: the working copy is
persistent, so an unreachable Nextcloud degrades the service rather than stopping
it. See [Source as working copy](../decisions/source-as-working-copy.md).

**`triggers.poll` stays enabled when `push` is set.** Push is an accelerator, not a
replacement — a dropped WebSocket must not mean a silently frozen site.

**`triggers.jitter`** spreads timer builds across installations so they do not all
hit the same external APIs at the same second.

**`schedule.on_busy`** has one supported value, `queue_latest`. Cancelling running
builds is unsafe by design.

**`assemble.overlay`** lists everything copied from the config directory into the
build tree. Anything executable or configuring belongs here; if it is in the vault
instead, the security model is void.

**`report.webdav_status_path` must not be inside `source.path`.** Otherwise:
write status → root ETag changes → trigger → build → write status → forever. Path
excludes do not help; the root ETag is path-blind.

**Fail-closed startup check.** If the hook directory resolves inside `source.path`,
the service refuses to start.

# Secrets

Secrets are passed as files (`*_file`), never as inline values, and reach hooks
only through explicit `env_passthrough`. The builder receives none of them.
