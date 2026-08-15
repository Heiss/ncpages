# Zensical + Obsidian

A production-shaped deployment: an Obsidian vault in Nextcloud, built by
Zensical, served by ncpages. **One container, one Dockerfile.**

This is a template, not a turnkey site. What you bring is your content, your
generator configuration and your hooks; what this gives you is the wiring.

The Dockerfile is the whole customisation mechanism: ncpages ships a statically
linked binary that imposes nothing on the base image, so you pick the base your
generator needs and copy the binary in.

## What lives where

```
examples/zensical/
├── Dockerfile             your generator, your hooks, the ncpages binary
├── pyproject.toml         your build dependencies
├── uv.lock                pinned; changing a dependency is a rebuild
├── docker-compose.yml     one service, one volume, one port
├── Makefile               make net / up / doctor / build
├── etc/                   mounted read-only at /etc/ncpages
│   ├── ncpages.toml       the service configuration
│   ├── zensical.toml      generator config, without a nav tree
│   ├── build.sh           what the build phase runs
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

## If you want more separation

Everything below is the same image in a different role, added only if you want
what it buys.

**Isolate the build.** The generator processes vault content, so a bug in it runs
with whatever the container has. Splitting it out gives the build no credentials
and no route to the internet:

```yaml
  builder:
    build: .
    command: ["build-agent", "--listen", "0.0.0.0:8080"]
    volumes: [work:/work, ./etc:/etc/ncpages:ro]
    networks: [build]        # internal: true — no egress
    read_only: true
    tmpfs: [/tmp]
    cap_drop: [ALL]
```

Then set `kind = "agent"` in `ncpages.toml`. Both containers must run the **same
UID**, or the second build fails with `EACCES` on the releases directory — a
failure that looks transient and is not. With a read-only share token as the
credential, the blast radius of a generator bug is already "read that folder",
which is why this is optional rather than the default.

**Keep the site up when the watcher is not.** `run` hosts the server in the same
process, so a crash takes the site with it. Two roles instead:

```yaml
  watcher:
    build: .
    command: ["watch"]

  web:
    image: ghcr.io/heiss/ncpages:main-serve   # scratch: the binary, nothing else
    command: ["serve"]
    volumes: [work:/work, ./etc:/etc/ncpages:ro]
    ports: ["8080:8080"]
    # No depends_on, deliberately: the site survives everything else being down.
```

The serve container mounts the **parent** of the `current` symlink and resolves
it per request. Mounting the symlink itself makes Docker bind whatever it pointed
at during container start, and every later swap becomes invisible: the site
silently never updates.
