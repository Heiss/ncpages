# Directory Update Log

## 2026-08-15

* **Creation**: [A service beside Nextcloud, not a Nextcloud app](decisions/service-not-nextcloud-app.md)
  — the objection "if an app ships anyway, why a separate service?" examined and
  rejected: the app-only shape removes no component, cannot host the build
  sandbox, has no execution model for long work, widens the blast radius into the
  publicly served directory, and forfeits share-link deployments. The half that
  holds — an app would add nothing to the *data* path — is recorded too, together
  with what the app is genuinely for.
  [Status reporting](interfaces/status-reporting.md) now states that the app is a
  sink, and the [Roadmap](roadmap.md) notes that RFC 6578 does not become due just
  by being the only remaining candidate for a change list, plus one unverified
  cost: whether the `REPORT` works over `public.php/dav` at all.
* **Correction**: [Index](index.md), [Observability](interfaces/observability.md)
  and the README — the notify_push trigger is built, and the status file written
  back to Nextcloud is gone by decision, not pending; what is outstanding is the
  companion app that receives the report. The record count in the index was stale
  as well.
* **Creation**: `AGENTS.md` at the repository root — a routing map into this
  bundle for agents working on the code: the invariants, a table from source file
  to the concept that governs it, and the rules for keeping the bundle true. It
  holds links and one-line hooks only; the text stays here.

* **Update**: [Roadmap](roadmap.md) gained a section for changes that need a
  measurement before they are worth making, starting with RFC 6578 collection
  sync as an alternative to descending by ETag — including the experiment that
  would settle it, progressive enhancement as the shape it should take, and a
  compile-flag-plus-second-image variant recorded as rejected for now, with the
  conditions under which to revisit it.
* **Update**: Public share links are a supported credential —
  `source.share_token` uses `/public.php/dav/files/{token}`, needs no account,
  and is read-only by construction. Documented in
  [Configuration](interfaces/configuration.md).
* **Update**: [Security model](architecture/security-model.md) gained a section
  on not trusting the source: paths from a `PROPFIND` response are validated
  before they are joined onto a local directory. A crafted `href` could
  previously have written outside the working copy.
* **Update**: The watcher no longer writes to Nextcloud at all.
  `report.webdav_status_path` is gone from the configuration surface, together
  with the validation that kept it outside `source.path` and the self-write
  fingerprint. Recorded as
  [The watcher never writes to the source](decisions/no-writes-to-the-source.md);
  the replacement channel is
  [Status reporting](interfaces/status-reporting.md), a companion Nextcloud app
  probed with `OPTIONS` and absent by default.
* **Update**: [Security model](architecture/security-model.md) corrected — the
  watcher makes exactly one kind of outward request, WebDAV to the source.
  Webmentions and comment APIs are hook scripts; the core knows nothing about
  them. The layer-3 section had attributed recipe work to the core.
* **Creation**: [A small static binary and two images](decisions/small-static-binary.md)
  — the binary went from 6.7 MB to 2.42 MB, and the runtime image is now Alpine
  with a static musl build, plus a `scratch` variant for the serve role.
* **Update**: [Overview](overview.md) gained the three-rung setup ladder:
  a WebDAV credential, then notify_push, then the companion app.

* **Creation**: [Test strategy](operations/test-strategy.md) — four layers, the
  mock Nextcloud that reproduces ETag propagation rather than stubbing it, and
  why the end-to-end stack uses the plain Nextcloud image instead of AIO.
* **Update**: notify_push is implemented; `triggers.push` is no longer inert.
  The end-to-end suite asserts it by setting `poll = "300s"` and requiring a
  change to go live within 60 seconds.
* **Creation**: [Command line](interfaces/cli.md), documenting the roles the
  binary can take.
* **Update**: [Configuration](interfaces/configuration.md) rewritten against the
  implementation — added `paths`, `serve`, `health`, `build.kind`/`command`/
  `token_file` and `gate.forbid_duplicate_basenames`, and a per-section status
  table. `triggers.push` and `report.webdav_status_path` are configured but not
  yet implemented; `gate.max_nav_churn` was dropped as a recipe concern.
* **Update**: This bundle is now the source of the published documentation site.
  Zensical builds it directly (`docs_dir = "knowledge"`) and GitHub Actions
  deploys it to GitHub Pages; `tools/check_bundle.py` gates the build on OKF
  conformance and link integrity.
* **Update**: Implementation choices recorded — Rust, repository
  `github.com/Heiss/ncpages` (public, Apache-2.0), and
  [One binary with roles](decisions/single-binary-roles.md), which moves serving
  in-process by default and supersedes the session's separate web container.
  [Overview](overview.md), [Topology](architecture/topology.md),
  [Delivery](architecture/delivery.md) and
  [Self-hosted delivery](decisions/self-hosted-delivery.md) annotated accordingly.
* **Update**: [Roadmap](roadmap.md) — removed the dogfooding item. The
  documentation site is built by GitHub Actions on GitHub Pages; ncpages is the
  wrong tool for content that lives in git.

* **Initialization**: Created the OKF v0.2 bundle from the design session of
  2026-08-15. Both primary sources imported verbatim as
  [Design session transcript](history/design-session-transcript.md) and
  [Original concept note](history/original-concept-note.md).
* **Creation**: [Overview](overview.md), [Glossary](glossary.md),
  [Open questions](open-questions.md), [Roadmap](roadmap.md).
* **Creation**: Architecture — [Pipeline](architecture/pipeline.md),
  [Scheduler state machine](architecture/state-machine.md),
  [Topology](architecture/topology.md),
  [Security model](architecture/security-model.md),
  [Delivery](architecture/delivery.md),
  [Build environment](architecture/build-environment.md).
* **Creation**: Interfaces — [Configuration](interfaces/configuration.md),
  [Hook contract](interfaces/hook-contract.md),
  [Quality gate](interfaces/quality-gate.md),
  [Observability](interfaces/observability.md),
  [Builder API](interfaces/builder-api.md).
* **Creation**: Thirteen [decision records](decisions/) covering change detection,
  security, navigation and delivery.
* **Creation**: Recipes — [Zensical + Obsidian](recipes/zensical-obsidian.md),
  [Navigation convention](recipes/nav-frontmatter-convention.md).
* **Creation**: Operations — [Cutover runbook](operations/cutover-runbook.md),
  [Failure modes](operations/failure-modes.md),
  [Doctor checks](operations/doctor-checks.md).
* **Creation**: History — [Design narrative](history/design-narrative.md),
  [Legacy workflow findings](history/legacy-workflow-findings.md).
* **Update**: Added [Audit of the current setup](history/current-setup-audit.md)
  after reading the live repository. Four assumptions from the session corrected:
  `fetch_comments.py` is a `post_build` hook, `google-genai` is not in the build
  path, navigation is already generated from a `category` front-matter key, and
  `/de/` has real content. Two new findings recorded: webmentions are discovered
  but never sent, and `.env` with an API key is committed.
* **Update**: [Open questions](open-questions.md) rewritten around the audit —
  three questions settled, four new ones opened.
  [Legacy workflow findings](history/legacy-workflow-findings.md) extended with the
  never-sent webmentions.
  [Navigation from frontmatter](decisions/navigation-from-frontmatter.md),
  [Navigation convention](recipes/nav-frontmatter-convention.md),
  [Zensical + Obsidian](recipes/zensical-obsidian.md) and
  [Cutover runbook](operations/cutover-runbook.md) annotated with the corrections.
