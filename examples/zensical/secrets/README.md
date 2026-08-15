# Secrets

One file per secret, referenced by `*_file` in `ncpages.toml`. Never inline
values, and never in the vault.

```sh
head -c 32 /dev/urandom | base64 | tr -d '\n' > build_token
# and, depending on which credential you chose:
printf '%s' 'your-nextcloud-app-password' > nc_app_password
printf '%s' 'your-share-password'         > share_password
chmod 600 *
```

Everything here except this file and `*.example` is ignored by git.
