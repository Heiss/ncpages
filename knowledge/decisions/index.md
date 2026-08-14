# Decision records

Each record states the context, the decision, the alternatives that were rejected,
and the consequences that follow. The rejected alternatives are the point: most of
them look reasonable until you know why they fail here.

# Change detection and scheduling

* [WebDAV over inotify](webdav-over-inotify.md) - filesystem events break under encryption, S3 storage and group folders
* [notify_push over webhook_listeners](notify-push-over-webhook-listeners.md) - the obvious app has five-minute latency; the push service has one second
* [Three trigger sources into one channel](trigger-composition.md) - why the timer is functionally required, not redundant
* [queue_latest over cancel](queue-latest-over-cancel.md) - a cancelled build can leave a state with no way back

# Security

* [Scripts and build config never live in the vault](code-outside-vault.md) - the decision that shaped everything else
* [Split watcher and builder](watcher-builder-split.md) - credentials and build tools never in the same container
* [Bake dependencies into the image](baked-dependencies.md) - what makes a network-less builder possible

# Content and navigation

* [Derive navigation from note frontmatter](navigation-from-frontmatter.md) - four options examined; the chosen one keeps the core generator-agnostic
* [Scripts instead of a plugin system](hooks-not-plugins.md) - an interface that still works in five years

# Delivery and deployment

* [ncpages serves the site itself](self-hosted-delivery.md) - the largest simplification of the design
* [One binary with roles](single-binary-roles.md) - serving runs in-process by default; the container split stays available
* [Symlink swap as the only publish backend](symlink-swap-publish.md) - atomicity does not exist over a network protocol
* [A manually created bridge network](external-bridge-network.md) - joining another stack's default network is a trap
* [The source directory is a working copy, not a cache](source-as-working-copy.md) - Nextcloud as a soft dependency
