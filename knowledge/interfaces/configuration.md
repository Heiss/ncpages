---
type: Configuration Reference
title: ncpages.toml
description: Full configuration surface, section by section, with the constraint each field carries and its status in the implementation.
resource: file:///etc/ncpages/ncpages.toml
tags: [configuration, reference, interface]
status: draft
generated: { by: claude-code/opus-5, at: 2026-08-15T13:30:00Z }
sources:
  - id: implementation
    resource: https://github.com/Heiss/ncpages/blob/main/src/config.rs
    title: src/config.rs
    author: human:heiss
    last_modified: 2026-08-15
  - id: concept
    resource: ../history/original-concept-note.md
    title: ncpages concept note, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
---

The config file lives in the read-only config directory, never in the vault.
Unknown keys are rejected rather than ignored, so a typo fails at startup instead
of silently disabling something.

`schema_version = 1` is not yet frozen: fields may still move before the first
release.

# Example

```toml
schema_version = 1

[source]
kind          = "webdav"                  # webdav | fs
url           = "http://nginx"            # the HTTP frontend, never the FPM container
host_header   = "cloud.example.org"       # real domain, for server_name/trusted_domains
path          = "Notes/blog"
required      = false                     # start even if the source is unreachable

# Either an account …
user          = "publisher"
password_file = "/run/secrets/nc_app_password"

# … or a public share link, which needs no account at all:
# share_token         = "abc123XYZ"       # the id from the share URL
# share_password_file = "/run/secrets/share_password"   # only if the share has one
# path                = ""                # a folder inside the share, usually empty

[paths]
src        = "/work/src"                  # vault working copy
build      = "/work/build"                # assembled build tree
state      = "/work/state"                # ETags, hashes, build history
config_dir = "/etc/ncpages"               # overlay + hooks/, read-only, outside the vault

[triggers]
push   = "ws://notify-push:7867/ws"       # omit to disable
poll   = "30s"                            # safety net, stays on when push is active
timer  = "6h"                             # omit to disable
jitter = 0.1

[schedule]
debounce  = "10s"
max_delay = "120s"
on_busy   = "queue_latest"                # the only accepted value

[assemble]
overlay       = ["zensical.toml", "uv.lock", "overrides", "src"]
source_subdir = "docs"

[build]
kind       = "agent"                      # agent | local
url        = "http://builder:8080"
token_file = "/run/secrets/build_token"
command    = ["zensical", "build", "--clean"]   # runs inside the builder
timeout    = "10m"
output     = "site"

[[hooks.pre_build]]
run = "nav_from_frontmatter.py"
[[hooks.pre_build]]
run = "fetch_external_data.py"
env_passthrough = ["SOME_API_TOKEN"]

[[hooks.post_publish]]
run = "send_webmentions.sh"

[gate]
require_files              = ["index.html", "sitemap.xml"]
min_pages                  = 5
max_page_drop              = 0.4
forbid_duplicate_basenames = true

[publish]
root          = "/work/publish"           # holds releases/ and current
keep_releases = 5

[serve]
enabled              = true
listen               = "0.0.0.0:8080"
cache_control_assets = "public, max-age=31536000, immutable"
cache_control_html   = "no-cache"

[health]
listen = "0.0.0.0:9090"

[report]
app        = true                         # probe for the companion Nextcloud app
app_url    = "…/apps/ncpages/api/v1/reports"   # derived from source.url when unset
ntfy_topic = "https://ntfy.sh/…"
```

# Constraints enforced at startup

These are checked in `Config::validate` and refuse to start rather than warn.

**The hook directory must be outside `paths.src`.** A build is code execution; a
hook inside the working copy would give a shell to everyone the folder is shared
with. The same applies to `paths.config_dir`.

**Nothing is ever written to the source.** There is no configuration for it,
because there is no code for it. See
[The watcher never writes to the source](../decisions/no-writes-to-the-source.md).

**`schedule.on_busy` accepts only `queue_latest`.** Cancelling a running build can
leave a published state whose irreversible `post_publish` effects half-fired.

**WebDAV sources require `url`, `user` and `password_file`;** agent builds require
`url`; local builds require `command`.

# Notes per section

**`source.url`** must be an HTTP frontend. With an FPM-based Nextcloud image that
is nginx — the FPM container speaks FastCGI, and pointing at it produces errors
that look like authentication failures.

## Choosing a credential

Both are fully supported. They differ in what an attacker gets if the file holding
them is read.

| | Share link | Account (app password) |
|---|---|---|
| Endpoint | `/public.php/dav/files/{token}` | `/remote.php/dav/files/{user}` |
| Scope | the shared folder only | everything that user can see |
| Rights | read-only by construction | read **and write**, unless narrowed |
| Setup | create a share, copy the id from the URL | create an app password |
| Revoking | one click on the share | revoke the app password |
| Requires | Nextcloud 29 or later | any version |
| Auth | **share id** as user + share password | user + app password |

**A share link authenticates exactly like an account, with different values.**
The id at the end of the share URL goes in the *username* field, and the share
password — if the share has one — in the password field. It is the same Basic
request, against a different endpoint. Newer Nextcloud documentation describes
the literal user `anonymous` for that endpoint instead, so a 401 on a
password-protected share is retried once that way rather than guessed at; both
forms are covered by tests.

The share link is the smaller blast radius and the smaller setup, and it matches
what ncpages does: read. An account is the right choice when the folder cannot be
shared — group folders, external storage, or a policy that forbids public links —
and when a recipe needs Nextcloud APIs beyond WebDAV.

Configure one or the other. Giving both in the same `[source]` is refused rather
than resolved by precedence, because a silent winner between two credentials is
the kind of thing nobody notices until it matters. Use two deployments if you
genuinely need both.

Requests carry `X-Requested-With: XMLHttpRequest`, which the public endpoint
requires for anything that is not a GET; without it Nextcloud answers 401 and the
cause is not obvious.

**`source.host_header`** exists because the internal URL is not the public one.

**`source.required = false`** is the default posture: the working copy is
persistent, so an unreachable source degrades the service rather than stopping it.

**`paths.build` and `publish.root` must share a filesystem**, or moving the build
output into a release degrades from a rename to a copy. `ncpages doctor` checks it.

**`triggers.poll` stays enabled when `push` is set.** A dropped WebSocket must not
mean a silently frozen site.

**`build.kind` defaults to `"local"`**: the generator runs as a subprocess of the
same container, so a crash in it is an exit code rather than a dead service, and
a deployment is one image and one container. `"agent"` moves the build into a
container with no credentials and no egress — worth it if the vault is shared or
the generator is large, and unnecessary when the credential is already a
read-only share token.

**`serve.cache_control_*`**: paths containing `/assets/` get the immutable value,
everything else the HTML value. Getting this backwards means old HTML referencing
asset names that no longer exist.

**`report.app`** costs one `OPTIONS` request per fifteen minutes when the
companion app is not installed, and nothing otherwise. See
[Status reporting](status-reporting.md).

## Secrets

Never in the config file itself. Every secret comes from one of two places, and
giving both for the same secret is refused:

| Secret | From a file | From the environment |
|---|---|---|
| account password | `source.password_file` | `source.password_env` |
| share password | `source.share_password_file` | `source.share_password_env` |
| builder token | `build.token_file` | `build.token_env` |

The `*_env` fields name a variable; ncpages reads it and trims whitespace, the
same as for a file. Files are the better default — they do not appear in `docker
inspect` or in a process listing — but environment variables are what most
container UIs actually offer, which makes them the difference between a setup
someone completes and one they abandon.

Two protections follow the environment route:

* **Hooks never see them.** Hooks start from a cleared environment; a secret
  reaches one only if that hook names it in `env_passthrough`.
* **The build never sees them.** The build inherits the container's environment,
  so every variable named in a `*_env` field is explicitly removed before the
  generator starts.

# Implementation status

| Section | State |
|---|---|
| `source` (`webdav` with an account or a share token, `fs`), `paths`, `assemble`, `build`, `gate`, `publish`, `serve`, `health` | implemented |
| `triggers.push` (notify_push), `triggers.poll`, `triggers.timer`, `triggers.jitter`, `schedule` | implemented |
| `hooks.*` with the four-phase contract | implemented |
| `report.ntfy_topic` | implemented for failures, gate refusals and conflict copies |
| `report.app`, `report.app_url` | client side implemented; the companion app itself is a separate project that does not exist yet |
| `gate.max_nav_churn` | **not implemented**; navigation is a recipe concern, so this may not belong in the core at all |
| writing anything to the source | **will not be implemented** |
