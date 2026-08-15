#!/bin/sh
# Runs inside the builder: no network, no secrets, read-only root filesystem.
# The working directory is the assembled build tree.
set -eu

# --clean, because a leftover file from a previous build that no longer exists
# in either source would otherwise be published forever.
exec zensical build --clean
