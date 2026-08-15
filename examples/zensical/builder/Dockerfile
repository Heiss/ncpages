# The builder image: build tools, no credentials, no route out.
#
# Dependencies are installed here, from a frozen lockfile, so that nothing is
# fetched at runtime — which is what makes `internal: true` on the build network
# possible in the first place. Changing a dependency is a deliberate image
# rebuild rather than silent drift.

# Slim, not Alpine: Zensical is maturin-based, and without musllinux wheels pip
# compiles the Rust part from source. Image size is the cheaper thing here.
FROM python:3.13-slim AS build

COPY --from=ghcr.io/astral-sh/uv:0.11.7 /uv /usr/local/bin/uv

WORKDIR /deps
COPY pyproject.toml uv.lock ./
ENV UV_PROJECT_ENVIRONMENT=/opt/venv \
    UV_LINK_MODE=copy
RUN uv sync --frozen --no-install-project

# ---------------------------------------------------------------------------

FROM python:3.13-slim

COPY --from=build /opt/venv /opt/venv

# The build agent is the same ncpages binary, in a different role. It is
# statically linked, so it runs here without any of Alpine's libc.
COPY --from=ghcr.io/heiss/ncpages:main /usr/local/bin/ncpages /usr/local/bin/ncpages

# Must match the watcher's UID, or the second build fails with EACCES on the
# releases directory — a failure that looks transient and is not.
ARG UID=10001
RUN useradd --uid ${UID} --create-home --shell /usr/sbin/nologin ncpages

ENV PATH="/opt/venv/bin:${PATH}" \
    PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    # Local extension code is made importable from the assembled build tree
    # rather than by an editable install, which writes absolute paths into .pth
    # files and breaks when the volume layout changes.
    PYTHONPATH=/work/build/src \
    UV_CACHE_DIR=/tmp/uv

USER ${UID}
WORKDIR /work

ENTRYPOINT ["ncpages"]
CMD ["build-agent", "--listen", "0.0.0.0:8080"]
