# Interfaces

The contracts ncpages exposes. These are the surfaces that must stay stable; they
carry `status: draft` until the first implementation freezes them.

* [Configuration](configuration.md) - `ncpages.toml`, section by section, with the constraint each field carries
* [Hook contract](hook-contract.md) - four phases, the environment every hook receives, exit code semantics
* [Quality gate](quality-gate.md) - the checks between build and publish, and why the page-drop check matters most
* [Observability](observability.md) - `/healthz`, the status file written back to Nextcloud, push notifications
* [Builder API](builder-api.md) - the internal, token-authenticated build trigger
