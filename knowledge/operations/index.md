# Operations

Running it, migrating to it, and diagnosing it when someone else's deployment
misbehaves.

* [Cutover runbook](cutover-runbook.md) - phases 0 to 7, from the GitHub Actions chain to ncpages
* [Failure modes](failure-modes.md) - every break found in the red-team pass, and what handles it
* [Doctor checks](doctor-checks.md) - the same list as executable diagnostics

The unifying property of the failure catalogue: almost everything fails silently.
That is why the mitigations are structural rather than "watch the logs".
