#!/bin/sh
# Stand-in generator: one page per note. The real recipe installs Zensical,
# Quartz or Hugo here; ncpages does not know the difference.
set -e
mkdir -p site
for f in docs/*.md; do
  [ -e "$f" ] || continue
  n=$(basename "$f" .md)
  printf '<!doctype html><h1>%s</h1>' "$n" > "site/$n.html"
done
printf '<!doctype html><h1>home</h1>' > site/index.html
