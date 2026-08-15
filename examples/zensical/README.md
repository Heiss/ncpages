# Zensical + Obsidian

A production-shaped deployment: an Obsidian vault in Nextcloud, built by
Zensical, served by ncpages. Watcher and builder are separate containers, and the
builder has no route to the internet.

This is a template, not a turnkey site. What you bring is your content, your
generator configuration and your hooks; what this gives you is the topology,
the isolation and the wiring.

## What lives where

```
examples/zensical/
├── docker-compose.yml     watcher + builder, networks, volumes
├── Makefile               make net / up / doctor / build
├── builder/
│   ├── Dockerfile         Zensical, baked in from a frozen lockfile
│   ├── pyproject.toml     your build dependencies
│   └── uv.lock            pinned; changing a dependency is a rebuild
├── etc/                   mounted read-only at /etc/ncpages
│   ├── ncpages.toml       the service configuration
│   ├── zensical.toml      generator config, without a nav tree
│   ├── build.sh           what runs inside the builder
│   ├── overrides/         Jinja templates — code, hence not in the vault
│   └── hooks/             your pre_build / post_build / post_publish scripts
└── secrets/               one file per secret, git-ignored
```

And in Nextcloud, the watched folder contains **only content**:

```
Notes/blog/
└── docs/
    ├── *.md
    ├── assets/
    └── stylesheets/extra.css      inert, so it stays editable from a phone
```

Nothing executable, nothing configuring. A build is code execution: if
`build.sh` or `zensical.toml` lived in the vault, everyone that folder is ever
shared with would have a shell on this host. ncpages refuses to start if the hook
directory resolves inside the watched path.

## Setup

1. **Create the bridge network** — once, and owned by neither stack:

   ```sh
   make net
   ```

2. **Give Nextcloud a way in.** Either create a share link for the folder and put
   its id in `share_token` (read-only, revocable in one click), or create an app
   password and use `user` + `password_file`. See
   [Configuration](https://heiss.github.io/ncpages/interfaces/configuration/).

3. **Attach Nextcloud to the bridge.** In the Nextcloud stack, add `cloud-bridge`
   as an external network to the container that speaks HTTP (nginx or the apache
   image) and to `notify-push`.

4. **Write the secrets:** see `secrets/README.md`.

5. **Fill in `etc/ncpages.toml`:** `source.url`, `source.host_header`,
   `source.path`, and the credential from step 2.

6. **Start it:**

   ```sh
   make up
   make doctor     # every check in the failure catalogue, as something you run
   make logs
   ```

Point your reverse proxy at port 8080. TLS, certificates and DNS stay its job —
ncpages does not do them and should not.

## Before you trust it

`make doctor` is the fastest way to find the things that break silently: an
unreachable source, ETag propagation that does not work in your setup (external
storage and some group folders), a builder with egress, a UID mismatch, a build
tree on a different filesystem from the releases.

Then let it run through a failed build, a restart and a timer cycle before you
switch DNS. Keep the old publishing path alive until then.

## Adding your hooks

The reference recipe uses three:

| Hook | Phase | Why that phase |
|---|---|---|
| generate the nav from front matter | `pre_build` | the build needs the tree before it runs |
| fetch comments and annotations | `post_build` | it rewrites built HTML, and the gate should check the result |
| send webmentions | `post_publish` | irreversible; it may only announce a state that is live |

Hooks receive `NCPAGES_SRC_DIR`, `NCPAGES_BUILD_DIR`, `NCPAGES_OUT_DIR`,
`NCPAGES_RELEASE_DIR`, `NCPAGES_PREV_DIR` and `NCPAGES_TRIGGER`, and must write
into the build tree rather than the vault. See
[Hook contract](https://heiss.github.io/ncpages/interfaces/hook-contract/).

**One warning about the first `post_publish` run.** A webmention sender diffs the
new release against `NCPAGES_PREV_DIR`. On a first run that directory is a
holding page, so *every* page counts as new and the whole backlog goes out at
once — to other people's servers, irreversibly. Seed it, or make the first run a
dry run.

## Splitting the serve role

`run` keeps the watcher and the HTTP server in one process, which means a crash
in the watcher takes the site down with it. To keep the site up regardless, run
two containers from the same image:

```yaml
watcher:
  image: ghcr.io/heiss/ncpages:main
  command: ["watch"]

web:
  image: ghcr.io/heiss/ncpages:main-serve   # scratch: the binary and nothing else
  command: ["serve"]
  volumes: [work:/work, ./etc:/etc/ncpages:ro]
  ports: ["8080:8080"]
  # No depends_on, deliberately: the site survives everything else being down.
```

The serve image mounts the **parent** of the `current` symlink and resolves it
per request. Mounting the symlink itself makes Docker bind whatever it pointed at
during container start, and every later swap becomes invisible: the site silently
never updates.
