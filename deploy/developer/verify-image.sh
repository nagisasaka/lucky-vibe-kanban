#!/usr/bin/env bash
# Verify the exact candidate image without mounting any application data.
set -euo pipefail
image=${1:?Pass the candidate digest}
fixture=$(mktemp -d /tmp/lvk-candidate.XXXXXX)
name="lvk-candidate-$(basename "$fixture" | tr '[:upper:]' '[:lower:]')"
cleanup() {
  docker rm -f "$name" >/dev/null 2>&1 || true
  docker volume rm "$name-home" "$name-repos" "$name-work" >/dev/null 2>&1 || true
  rm -rf "$fixture"
}
trap cleanup EXIT
chmod 755 "$fixture"
mkdir "$fixture/tls"
chmod 755 "$fixture/tls"
openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=127.0.0.1 \
  -addext 'subjectAltName=IP:127.0.0.1' \
  -keyout "$fixture/tls/privkey.pem" -out "$fixture/tls/fullchain.pem" 2>/dev/null
# The host service deliberately uses UMask=0077. These disposable fixture
# TLS files must still be readable by the image's unprivileged app user.
# No htpasswd: exercise the new public locked setup, not the legacy proxy layer.
chmod 644 "$fixture/tls/fullchain.pem" "$fixture/tls/privkey.pem"
for suffix in home repos work; do docker volume create "$name-$suffix" >/dev/null; done
docker run -d --name "$name" --shm-size=256m -e NODE_EXTRA_CA_CERTS=/run/lvk-secrets/tls/fullchain.pem -e LVK_HOST=127.0.0.1 \
  --mount "type=bind,src=$fixture,dst=/run/lvk-secrets,readonly" \
  -v "$name-home:/home/appuser" -v "$name-repos:/repos" -v "$name-work:/var/tmp" \
  "$image" >/dev/null
healthy=false
for _ in $(seq 1 90); do
  if [[ "$(docker inspect -f '{{.State.Health.Status}}' "$name")" == healthy ]]; then
    healthy=true; break
  fi
  sleep 2
done
test "$healthy" = true
docker cp "$(dirname -- "$0")/verify-access.mjs" "$name:/tmp/smoke-access.mjs"
docker exec "$name" node /tmp/smoke-access.mjs candidate-only
docker exec "$name" curl -fsS --cacert /run/lvk-secrets/tls/fullchain.pem \
  --user test:candidate-only https://127.0.0.1:8443/api/info -H "Host: 127.0.0.1" >/dev/null
docker exec -e LVK_TEST_INTERNAL_TOKEN_FILE=/home/appuser/.local/share/vibe-kanban/access-internal.token -e LVK_E2E_BASE_URL=http://127.0.0.1:3000 "$name" node /opt/lvk-browser/smoke.mjs
