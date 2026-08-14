# Directory Update Log

## 2026-08-15

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
