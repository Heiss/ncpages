# One image: your generator, your hooks, and the ncpages binary.
#
# This is the whole customisation mechanism. ncpages ships a statically linked
# binary that imposes nothing on the base image, so you pick the base your
# generator and hooks need and copy the binary in. There is no family of base
# images to choose from, and nothing to guess about which runtime you wanted.
#
# Debian-based rather than Alpine on purpose: Zensical is maturin-based, and
# Python dependencies with C extensions find manylinux wheels here instead of
# compiling from source.

FROM python:3.13-slim

COPY --from=ghcr.io/astral-sh/uv:0.11.7 /uv /usr/local/bin/uv

# Dependencies are baked in from a frozen lockfile: nothing is fetched while a
# build runs, and changing a dependency is a deliberate image rebuild rather
# than silent drift.
WORKDIR /opt/recipe
COPY pyproject.toml uv.lock ./
ENV UV_PROJECT_ENVIRONMENT=/opt/venv \
    UV_LINK_MODE=copy
RUN uv sync --frozen --no-install-project \
 && rm -rf /root/.cache

COPY --from=ghcr.io/heiss/ncpages:main /usr/local/bin/ncpages /usr/local/bin/ncpages

# The build inherits this environment, which is why the image is the right place
# to configure the generator.
ENV PATH="/opt/venv/bin:${PATH}" \
    PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    # Local extension code — an Obsidian markdown extension, say — is imported
    # from the assembled build tree rather than installed, because editable
    # installs write absolute paths that break when the layout changes.
    PYTHONPATH=/work/build/src

RUN useradd --uid 10001 --create-home --shell /usr/sbin/nologin ncpages
USER 10001
WORKDIR /work

ENTRYPOINT ["ncpages"]
CMD ["run"]
