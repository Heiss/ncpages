# Hooks

Executables, run with a cleared environment plus the documented `NCPAGES_*`
variables and whatever `env_passthrough` names explicitly. Exit `0` for success,
`1` for a warning that should not stop the build, `2` to abort.

They run **outside** the builder, so they have network and whatever secrets you
pass them — and they must not write into the vault. Write into
`$NCPAGES_BUILD_DIR` instead; that is what gets built.

| Phase | Typical use |
|---|---|
| `pre_build` | generate navigation from front matter, fetch external data |
| `post_build` | post-process the HTML, before the gate checks it |
| `post_publish` | irreversible things: webmentions, cache purges, pings |

Make them executable (`chmod +x`); `ncpages doctor` checks that for you.
