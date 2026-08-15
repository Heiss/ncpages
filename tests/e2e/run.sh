#!/usr/bin/env bash
#
# End-to-end test against a real Nextcloud with notify_push.
#
# What this proves that the mock cannot:
#
#   * ETag propagation works the way the design assumes, on a real Nextcloud
#   * notify_push actually delivers, and ncpages reacts to it — asserted by
#     setting the poll interval to five minutes, so a change that goes live in
#     seconds can only have come from the socket
#   * the builder really has no egress
#   * the whole topology works with separate watcher and builder containers
#
# Usage: tests/e2e/run.sh [--keep]

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$HERE"

COMPOSE="docker compose"
DAV="http://localhost:8081/remote.php/dav/files/admin/Notes/blog"
AUTH="admin:admin-password-123"
SITE="http://localhost:8099"
HEALTH="http://localhost:9099"
KEEP="${1:-}"

log() { printf '\n\033[1m· %s\033[0m\n' "$*"; }
fail() {
  printf '\n\033[31mFAIL: %s\033[0m\n' "$*" >&2
  $COMPOSE logs --tail 60 watcher builder notify-push 2>&1 | tail -120 >&2
  exit 1
}

cleanup() {
  if [ "$KEEP" != "--keep" ]; then
    $COMPOSE down -v --remove-orphans >/dev/null 2>&1 || true
  else
    echo "environment kept running; tear down with: (cd $HERE && docker compose down -v)"
  fi
}
trap cleanup EXIT

occ() { $COMPOSE exec -T --user www-data nextcloud php occ "$@"; }

# Wait until `cmd` succeeds, up to `timeout` seconds.
wait_for() {
  local what="$1" timeout="$2"
  shift 2
  local deadline=$((SECONDS + timeout))
  until "$@" >/dev/null 2>&1; do
    [ $SECONDS -lt $deadline ] || return 1
    sleep 2
  done
  echo "  ${what}: ready after $((timeout - (deadline - SECONDS)))s"
}

put() { curl -fsS -u "$AUTH" -T - "$DAV/$1" >/dev/null; }

log "starting Nextcloud, Postgres and Redis"
$COMPOSE up -d db redis nextcloud
wait_for "nextcloud" 300 curl -fsS "http://localhost:8081/status.php" \
  || fail "Nextcloud did not become healthy"
occ status | grep -q "installed: true" || fail "Nextcloud is not installed"

log "installing and configuring notify_push"
# The container subnet has to be trusted, or notify_push's self-test fails —
# the single most common reason for a non-working setup in the wild.
occ config:system:set trusted_proxies 0 --value=172.29.0.0/16
occ app:install notify_push || occ app:enable notify_push
$COMPOSE up -d notify-push
wait_for "notify_push" 120 curl -fsS "http://localhost:8081/status.php"
sleep 5
$COMPOSE ps notify-push | grep -q "Up\|running" || fail "notify_push is not running"

log "creating the watched folder and seeding notes"
curl -fsS -u "$AUTH" -X MKCOL "http://localhost:8081/remote.php/dav/files/admin/Notes" >/dev/null || true
curl -fsS -u "$AUTH" -X MKCOL "$DAV" >/dev/null || true
for i in 1 2 3 4 5; do
  echo "note $i" | put "n$i.md"
done

log "building the ncpages image and starting watcher and builder"
$COMPOSE up -d --build builder watcher

log "waiting for the first publish"
wait_for "site" 240 curl -fsS "$SITE/n1.html" || fail "nothing was published"
curl -fsS "$SITE/n1.html" | grep -q "n1" || fail "the published page has the wrong content"
curl -fsS "$HEALTH/healthz" | grep -q '"last_release"' || fail "healthz has no release"

log "the builder has no egress"
# busybox wget: the runtime image ships a shell for hooks, not a download tool.
if $COMPOSE exec -T builder wget -q -T 5 -O - https://example.com >/dev/null 2>&1; then
  fail "the builder reached the internet; internal: true is not in effect"
fi
echo "  confirmed: no route out of the build container"

log "post_publish ran after the swap"
$COMPOSE exec -T watcher cat /work/post_publish.log | grep -q "post_publish" \
  || fail "the post_publish hook did not run"

# The poll interval is 300s. Anything that goes live within 60s came from the
# WebSocket, not from polling.
log "a new note goes live through notify_push, not polling"
started=$SECONDS
echo "pushed note" | put "pushed.md"
wait_for "pushed.html" 60 curl -fsS "$SITE/pushed.html" \
  || fail "the change did not arrive within the push window (poll is 300s)"
elapsed=$((SECONDS - started))
[ "$elapsed" -lt 60 ] || fail "took ${elapsed}s, which polling could not explain either"
echo "  published ${elapsed}s after the upload"

log "a deleted note disappears from the site"
curl -fsS -u "$AUTH" -X DELETE "$DAV/n5.md" >/dev/null
# Note the absent -f: 404 is the expected outcome here, not a transport error.
status_is_404() { [ "$(curl -sS -o /dev/null -w '%{http_code}' "$SITE/n5.html")" = "404" ]; }
wait_for "removal" 60 status_is_404 \
  || fail "the deleted note is still being served"

log "a collapsed vault does not reach the live site"
for i in 1 2 3 4; do
  curl -fsS -u "$AUTH" -X DELETE "$DAV/n$i.md" >/dev/null
done
gate_refused() { $COMPOSE logs watcher 2>&1 | grep -q "gate refused"; }
wait_for "gate" 90 gate_refused \
  || fail "the gate did not refuse the collapsed build"
curl -fsS "$SITE/pushed.html" | grep -q "pushed" \
  || fail "the gate refused, but the live site changed anyway"
echo "  confirmed: the gate refused, the previous release is still served"

printf '\n\033[32me2e passed\033[0m\n'
