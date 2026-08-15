---
type: Test Strategy
title: Test strategy
description: Four layers, what each one can and cannot prove, and why the mock Nextcloud reproduces ETag propagation rather than stubbing it.
tags: [testing, quality, ci, architecture]
status: stable
generated: { by: claude-code/opus-5, at: 2026-08-15T16:10:00Z }
sources:
  - id: implementation
    resource: https://github.com/Heiss/ncpages/tree/main/tests
    title: tests/
    author: human:heiss
    last_modified: 2026-08-15
  - id: session
    resource: ../history/design-session-transcript.md
    title: ncpages design session, 2026-08-15
    author: human:heiss
    last_modified: 2026-08-15
---

Almost every failure in the [failure catalogue](failure-modes.md) is silent: a
build that succeeds and publishes nothing useful, a swap that never becomes
visible, a webmention announcing a page nobody can reach. Tests here are chosen
for their ability to make silence audible, and each one is named after the claim
it falsifies.

# The four layers

| Layer | Where | Runs in | Proves |
|---|---|---|---|
| Unit | `src/**` inline `mod tests` | milliseconds | pure logic: gate arithmetic, debounce bounds, path containment, release ordering, the hook environment contract |
| Integration | `tests/webdav_sync.rs`, `tests/pipeline_integration.rs` | ~6 s | behaviour against a mock Nextcloud and a real filesystem: descend-by-ETag, phase ordering, atomic swap under a refused gate |
| Smoke | `examples/local-dev/smoke-test.sh` | ~15 s | the shipped binary as an operator uses it: serve, swap live, cache headers, `doctor` |
| End-to-end | `tests/e2e/` | ~5 min | a real Nextcloud with notify_push, in the production container topology |

The first three run on every push. E2E runs on `main`, nightly, and on demand —
it is too slow to gate a pull request and too valuable to skip.

# Why the mock reproduces ETag propagation

The cheap thing to build would be a stub returning fixed ETags. It would pass
while proving nothing, because the entire change-detection design rests on one
Nextcloud behaviour: **a collection's ETag changes whenever anything beneath it
changes.** That is what makes a single `PROPFIND Depth: 0` sufficient.

So the mock in `tests/support/mod.rs` derives directory ETags from every
descendant, exactly as Nextcloud does. Which turns the central efficiency claim
into an assertion that fails when someone breaks it:

```rust
assert_eq!(mock.requests().len(), 1,
    "ETag propagation should make one PROPFIND sufficient");
```

The mock also serves the two failure modes that must behave differently — `401`
stops immediately rather than retrying into brute-force protection, `503` is
reported as maintenance mode — and records the `Host` header, because
`server_name` matching and `trusted_domains` depend on it.

It can also play a hostile server: `poison_listing_with("../escaped.md")` injects
an `href` that escapes the destination, and serves content for it, so a missing
traversal guard results in a file landing outside the working copy rather than a
convenient 404. That test was checked the only way such a test is worth anything
— by removing the guard and watching it fail.

Both endpoints are covered: the account one and the public share
(`/public.php/dav/files/{token}`), including the assertion that an unchanged share
also costs exactly one request.

# What only the end-to-end layer can prove

* **ETag propagation is real**, not just faithfully mocked. If a future Nextcloud
  changes it, the mock keeps passing and E2E fails — which is the correct
  division of labour.
* **notify_push actually delivers.** The E2E config sets `poll = "300s"`, then
  asserts a change goes live within 60 seconds. Polling cannot explain that, so
  the assertion can only pass if the WebSocket path works end to end.
* **The builder has no egress.** A `curl` from inside the build container must
  fail. `internal: true` is a claim about a network, and only a real network can
  check it.
* **The two-container topology works** with a shared volume, one UID, and the
  symlink resolved inside the serving container.

# Why not Nextcloud AIO

AIO's mastercontainer manages its own containers through the Docker socket and
expects a domain, TLS and a web installer. It cannot be scripted in CI, and a
test built around it would mostly test AIO. The plain image exposes the same
interfaces — WebDAV, ETag propagation, notify_push.

AIO is still what a large share of self-hosters run, so a manual AIO pass belongs
in a release checklist. It is a compatibility check, not a regression test.

# Conventions

* **Name the test after the claim**, not the function:
  `a_refused_gate_never_reaches_post_publish`, not `test_gate_2`. A failing test
  name should tell you what broke about the system, not where.
* **Assert the consequence, not the mechanism.** After a refused gate, check that
  `current` still points at the old release — not that some internal flag is
  false.
* **Every entry in the failure catalogue should be reachable by a test**, or
  carry a note saying why it cannot be.
* **No sleeps as synchronisation** in unit and integration tests; the E2E layer
  polls with timeouts instead, because it waits on real convergence.
* **Fixtures are throwaway.** The E2E credentials are committed on purpose and
  match the compose file; nothing in `tests/e2e/etc/` belongs in a deployment.

# Gaps

* The status file written back to Nextcloud has no test, because it is not
  implemented yet. When it is, the self-trigger loop is the thing to assert: the
  service must not build in response to its own write.
* `queue_latest` under real load is exercised only indirectly. A test that fires
  many events during a slow build, then asserts exactly one extra build ran, is
  worth adding.
* Multi-arch images are built but not exercised on arm64 in CI.
