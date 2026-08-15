# Multi-arch by construction: no architecture-specific steps, so buildx can
# produce amd64 and arm64 from the same file. A large part of the homelab
# audience runs arm64.

FROM rust:1.89-slim AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/*

# Watcher and builder must share a UID, or the second build fails with EACCES on
# the releases directory — a failure that looks transient and is not.
ARG UID=10001
RUN useradd --uid ${UID} --create-home --shell /usr/sbin/nologin ncpages

COPY --from=build /src/target/release/ncpages /usr/local/bin/ncpages

USER ${UID}
WORKDIR /work
ENTRYPOINT ["ncpages"]
CMD ["run"]
