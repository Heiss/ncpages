#!/usr/bin/env bash
#
# End-to-end exercise of the whole pipeline against a temporary vault:
#
#   1. build, gate and publish
#   2. serve the published release
#   3. change the vault and watch the swap take effect live
#   4. collapse the vault and confirm the gate keeps the old site online
#
# Usage: examples/local-dev/smoke-test.sh [path-to-ncpages]

set -euo pipefail

BIN="${1:-./target/debug/ncpages}"
BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
D="$(mktemp -d)"
SITE=127.0.0.1:8099
HEALTH=127.0.0.1:9099
PID=""

cleanup() {
  [ -n "$PID" ] && kill "$PID" 2>/dev/null || true
  rm -rf "$D"
}
trap cleanup EXIT

fail() {
  echo "FAIL: $*" >&2
  [ -f "$D/run.log" ] && tail -30 "$D/run.log" >&2
  exit 1
}

mkdir -p "$D/vault" "$D/etc/hooks" "$D/work"
sed "s#REPLACED#$D#g" "$HERE/ncpages.toml" > "$D/etc/ncpages.toml"

# A stand-in generator. The real one is Zensical, Quartz, Hugo or whatever the
# recipe installs; ncpages does not know the difference.
cat > "$D/etc/build.sh" <<'EOF'
#!/bin/sh
set -e
mkdir -p site/assets
for f in docs/*.md; do
  [ -e "$f" ] || continue
  n=$(basename "$f" .md)
  printf '<h1>%s</h1>' "$n" > "site/$n.html"
done
printf '<h1>home</h1>' > site/index.html
printf 'body{}' > site/assets/main.abc123.css
printf '<urlset/>' > site/sitemap.xml
EOF

cat > "$D/etc/hooks/note.sh" <<'EOF'
#!/bin/sh
echo "trigger=$NCPAGES_TRIGGER out=$NCPAGES_OUT_DIR release=${NCPAGES_RELEASE_DIR:-none}" >> "$(dirname "$0")/../../hooks.log"
EOF
chmod +x "$D/etc/build.sh" "$D/etc/hooks/note.sh"

for i in $(seq 1 10); do echo "note $i" > "$D/vault/n$i.md"; done

echo "· config validates"
"$BIN" -c "$D/etc/ncpages.toml" check > /dev/null || fail "config did not validate"

echo "· one-shot build publishes"
"$BIN" -c "$D/etc/ncpages.toml" build > "$D/build.log" 2>&1 || fail "build failed"
grep -q "published" "$D/build.log" || fail "nothing was published"
[ -L "$D/work/publish/current" ] || fail "current is not a symlink"

echo "· doctor reports no failures"
"$BIN" -c "$D/etc/ncpages.toml" doctor > "$D/doctor.log" 2>&1 || fail "doctor reported a failure"

echo "· serving the published release"
"$BIN" -c "$D/etc/ncpages.toml" run > "$D/run.log" 2>&1 &
PID=$!
for _ in $(seq 1 40); do
  curl -fsS "http://$SITE/index.html" > /dev/null 2>&1 && break
  sleep 0.25
done
curl -fsS "http://$SITE/n1.html" | grep -q "n1" || fail "published page is not served"

echo "· caching headers differ for HTML and hashed assets"
curl -fsS -D- -o /dev/null "http://$SITE/index.html" | grep -qi "cache-control: no-cache" \
  || fail "HTML must not be cached"
curl -fsS -D- -o /dev/null "http://$SITE/assets/main.abc123.css" | grep -qi "immutable" \
  || fail "hashed assets should be immutable"

echo "· a new note goes live without a restart"
echo "brand new" > "$D/vault/fresh.md"
ok=""
for _ in $(seq 1 60); do
  if curl -fsS "http://$SITE/fresh.html" > /dev/null 2>&1; then ok=1; break; fi
  sleep 0.5
done
[ -n "$ok" ] || fail "the swap never became visible to the server"

echo "· healthz reports the last release"
curl -fsS "http://$HEALTH/healthz" | grep -q '"last_release"' || fail "healthz is missing state"

echo "· a collapsed vault does not reach the live site"
rm "$D"/vault/*.md
sleep 8
curl -fsS "http://$SITE/n1.html" | grep -q "n1" \
  || fail "the gate let a one-page site replace the published one"
grep -q "gate refused" "$D/run.log" || fail "the gate did not refuse the collapsed build"

echo
echo "smoke test passed"
