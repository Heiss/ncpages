# Statically linked against musl, so the runtime image needs no libc at all and
# the binary cannot fail at startup because the build image shipped a newer
# glibc than the runtime — a real failure this project already hit once.
#
# Two targets:
#
#   docker build .                     → runtime: alpine + a shell, for run/watch
#                                        (hooks are scripts; they need one)
#   docker build --target serve .      → scratch: the binary alone, for the
#                                        serve role, which runs no hooks
#
# No architecture-specific steps, so buildx produces amd64 and arm64 from this
# file unchanged. A large part of the homelab audience runs arm64.

FROM rust:1.89-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked && strip target/release/ncpages

# ---------------------------------------------------------------------------
# serve: no shell, no package manager, nothing to attack that is not the binary
# ---------------------------------------------------------------------------
FROM scratch AS serve
COPY --from=build /src/target/release/ncpages /ncpages
USER 10001:10001
WORKDIR /work
ENTRYPOINT ["/ncpages"]
CMD ["serve"]

# ---------------------------------------------------------------------------
# runtime: the default. Includes a shell because hook scripts are the extension
# interface, and CA certificates because the source may be HTTPS.
# ---------------------------------------------------------------------------
FROM alpine:3.22 AS runtime
RUN apk add --no-cache ca-certificates

# Watcher and builder must share a UID, or the second build fails with EACCES on
# the releases directory — a failure that looks transient and is not.
ARG UID=10001
RUN adduser --uid ${UID} --disabled-password --no-create-home --shell /sbin/nologin ncpages

COPY --from=build /src/target/release/ncpages /usr/local/bin/ncpages

USER ${UID}
WORKDIR /work
ENTRYPOINT ["ncpages"]
CMD ["run"]
