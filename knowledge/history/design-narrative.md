---
type: Design Narrative
title: How the design arrived where it did
description: English summary of the 2026-08-15 session — which ideas were discarded, where the direction changed, and which objection produced the largest simplification.
tags: [history, design, narrative, provenance]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-15T00:45:00Z }
sources:
  - id: session
    resource: design-session-transcript.md
    title: ncpages design session, 2026-08-15 (verbatim, German)
    author: human:heiss
    last_modified: 2026-08-15
---

The starting sketch was: listen to a Nextcloud folder, run a preconfigured routine
on change, a slim binary with two hooks (`install_script` at start, `run_script` on
change), and push the result to git-pages. All four points changed; two of them
fundamentally.

# Watching was never the problem

Filesystem watching is solved — `watchexec` or the `notify` crate do it in a few
lines. What does not exist off the shelf is the state machine behind it: debounce,
coalescing queue, build sandbox, quality check, atomic publish, status report.

The same step brought the second reinterpretation: don't look at the filesystem,
ask over WebDAV. A single `PROPFIND Depth: 0` on the root folder answers whether
anything below it changed, because Nextcloud propagates ETags upward. Cheaper than
a recursive watch, and independent of how Nextcloud stores the data. "Efficient
polling" turned out to be the better mechanism, not the compromise.

# Push: the obvious app was the wrong one

`webhook_listeners` sounds right and fires through background jobs with a five
minute default interval — worse latency than polling, with more moving parts.
notify_push instead: Redis pub/sub to WebSocket, about one second. It reports only
*that* something changed, which fits exactly: the socket wakes the watcher, the
ETag check decides whether it was real.

# The security point that determined the architecture

The original plan put a bash script "somewhere" — conveniently, in the vault. That
is the most consequential point of the session: **the build is code execution by
design.** A script in the Nextcloud folder means a shell on the server for everyone
with write access to that folder — a compromised phone, an old sync client, every
person the folder is ever shared with.

The previous setup had branch protection and an audit log in front of that.
Discarding it with nothing in its place would not have been progress. So: code and
configuration live outside the vault, read-only. The cost — no more editing the
build from a phone — is a feature.

That single decision later pulled the template directory to the code side,
motivated the watcher/builder split, and eliminated one navigation option outright.

# Three Docker traps

FPM does not speak HTTP, so both notify_push and the watcher must address the nginx
container. A bind-mounted symlink is resolved at container start, so every later
swap is invisible and the site silently never updates. And `network_mode: none`
cannot be combined with a compose network, so builder isolation is
`internal: true` — the honest limit of what Compose provides.

# Auditing the old workflow

One pleasant finding: there is no Obsidian preprocessing step at all; wikilinks are
handled by a Markdown extension inside the build. The migration is much smaller
than feared.

Three unpleasant ones: the workflow is stateful and its cache handling is broken;
the 12-hour cron is functionally required rather than redundant, because incoming
comments only appear when a build runs; and webmentions are irreversible, which
forces a hard ordering — gate, then publish, then send. That last observation
produced the four-phase hook structure, the actual core of ncpages. It is the
generalisation of one concrete constraint, not an invented plugin system.

# The premise breaks: navigation

The generator config contained a hand-curated navigation tree. New note, syncs,
builds — and does not appear in the menu until someone edits a file that now lives
on the server. SSH is worse than git. Four options were examined; aggregating
navigation from note frontmatter won. An initial assessment that this needed *more*
core logic was wrong: it needs less, because nothing configuration-shaped ever
arrives from the vault.

A benefit that only became visible later: URLs stay stable, so reordering the menu
does not break webmention targets.

# The turn on delivery

Until late, the design assumed an external publish target, which produced a menu of
backends with rsync, SSH keys and atomicity problems. The objection — if everything
goes over WebDAV, the machine should not matter — was right for the input side and
wrong for the output side: atomic directory swap does not exist over any network
protocol, and without it the gate and the webmention timing both become
meaningless.

The resolution came from the next idea: ncpages serves the site itself. That
removed every remote backend, every deploy secret and the entire atomicity
discussion, and shrank the user contract to two volumes and one port. It was the
largest simplification of the session, and it came from the objection rather than
from the analysis.

# What was built during the session

The navigation convention, a migration script and an aggregator, tested against the
real configuration: tree → frontmatter across 46 files → re-aggregate → compare.
Byte-identical, including under randomly shuffled input order — which matters,
because filesystem traversal has no guaranteed order.
