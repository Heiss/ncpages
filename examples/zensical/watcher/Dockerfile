# A watcher image for hooks that need more than a shell.
#
# Hooks run in the watcher, so whatever they need has to exist there. The stock
# image is Alpine with a shell and CA certificates — enough for shell hooks and
# for a deployment with no hooks at all, and nothing more.
#
# Rather than shipping a family of base images and guessing which runtime you
# want, ncpages ships a statically linked binary that runs on any of them. Pick
# your own base and copy it in; that is the whole mechanism, and it is the same
# one the builder image uses.
#
#   docker build -t ncpages-watcher ./watcher
#
# Then in docker-compose.yml, replace the watcher's `image:` with:
#
#   build: ./watcher
#
# Debian rather than Alpine here on purpose: hook dependencies with C extensions
# find manylinux wheels and do not compile from source, which is the same reason
# the builder image is python:3.13-slim.
FROM python:3.13-slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/*

# Whatever your hooks import. Pin it; a hook that silently changes behaviour on
# a rebuild is worse than one that fails.
# COPY hook-requirements.txt /tmp/
# RUN pip install --no-cache-dir -r /tmp/hook-requirements.txt

# Statically linked, so it needs nothing from this image to run.
COPY --from=ghcr.io/heiss/ncpages:main /usr/local/bin/ncpages /usr/local/bin/ncpages

# The same UID as the builder, or the second build fails with EACCES on the
# releases directory.
ARG UID=10001
RUN useradd --uid ${UID} --create-home --shell /usr/sbin/nologin ncpages

USER ${UID}
WORKDIR /work
ENTRYPOINT ["ncpages"]
CMD ["run"]
