---
type: Interface Contract
title: Status reporting
description: How build results leave the service — a companion Nextcloud app probed with OPTIONS, ntfy for failures, and never a file in the vault.
tags: [reporting, observability, interface, nextcloud-app]
status: draft
generated: { by: claude-code/opus-5, at: 2026-08-15T18:10:00Z }
sources:
  - id: operator
    resource: ../history/design-session-transcript.md
    title: Operator decision, 2026-08-15 (supersedes the session on this point)
    author: human:heiss
    last_modified: 2026-08-15
---

Build results leave through channels that do not touch the source. See
[The watcher never writes to the source](../decisions/no-writes-to-the-source.md)
for why the obvious option — a status file in the watched folder — is the wrong
shape.

# Channels

| Channel | Carries | Costs when unused |
|---|---|---|
| log + `/healthz` | everything | nothing; always on |
| companion Nextcloud app | full report, for a real UI | one `OPTIONS` per 15 minutes |
| ntfy | failures, refused gates, conflict copies | nothing |

# The companion app

A separate project: a Nextcloud app that receives reports and presents them. It
does not exist yet. ncpages is written so that this costs nothing until it does.

After a publish, ncpages sends `OPTIONS` to the report endpoint:

* **not installed** → the probe is the entire cost. Nothing is sent, nothing is
  logged above debug level, and the result is cached for fifteen minutes, so
  installing the app later takes effect without a restart.
* **installed** → the report is `POST`ed as JSON.

On the same machine this is free. Across a network it is one round trip after a
build that already took seconds. Either way it is more predictable than writing
into a folder that syncs to every device the author owns.

Authentication reuses the source credentials; the operator has already configured
them.

## Endpoint

Derived from `source.url` unless `report.app_url` says otherwise:

```
{source.url}/index.php/apps/ncpages/api/v1/reports
```

So installing the app is the whole setup. `report.app = false` switches the probe
off entirely.

## Wire contract

The app is a separate codebase, so this payload is an interface: adding fields is
compatible, renaming them is not.

```json
{
  "version": 1,
  "result": "published",
  "trigger": "push",
  "release": "20260815T161303Z",
  "pages": 47,
  "warnings": [],
  "violations": [],
  "conflict_copies": [],
  "error": null
}
```

| Field | Meaning |
|---|---|
| `version` | schema version of this payload, not of ncpages |
| `result` | `published`, `refused`, `skipped` or `failed` |
| `trigger` | `push`, `poll`, `timer`, `manual` |
| `release` | release id, absent when nothing was published |
| `pages` | page count of the release |
| `warnings` | hook exit code `1` messages, and gate warnings |
| `violations` | why the gate refused; empty otherwise |
| `conflict_copies` | files excluded because Nextcloud flagged a conflict |
| `error` | present only when `result` is `failed` |

`conflict_copies` deserves a prominent place in any UI built on this: its
presence means a version of someone's work is at risk right now.

# ntfy

Independent of Nextcloud on purpose — it has to work when Nextcloud is the thing
that broke. Fires only on failures, refused gates and conflict copies. A
notification per successful blog post would train the operator to ignore the
channel.

# What a UI could add later

The report is deliberately small and structured, which leaves room: build history
and duration trends, a diff of what changed between releases, request counts from
the serving side, a rollback button that repoints `current`. None of that belongs
in ncpages, and all of it is easier in an app that already has a Nextcloud
session and a front end.
