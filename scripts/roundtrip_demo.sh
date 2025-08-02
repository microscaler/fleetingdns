#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$REPO_ROOT"

# Ensure the compose stack is running
if ! docker compose ps -q dnsd >/dev/null 2>&1; then
    echo "Docker compose stack not detected, starting..."
    bash "$REPO_ROOT/scripts/compose_start.sh"
fi

# Register demo slot in Redis
cargo run -p slot-setter demo 127.0.0.1 --ttl 60

# Wait for dnsd to serve the slot
for i in {1..10}; do
    IP=$(dig @127.0.0.1 -p6353 demo.fdns.run +short || true)
    if [[ "$IP" == "127.0.0.1" ]]; then
        break
    fi
    sleep 1
done
if [[ "$IP" != "127.0.0.1" ]]; then
    echo "DNS query failed" >&2
    exit 1
fi

# Verify EdgeHub TLS endpoint
openssl s_client -connect 127.0.0.1:2222 -servername ssh </dev/null > /dev/null

# Simple HTTP echo to show round-trip
python3 -m http.server 8080 >/tmp/demo_http.log 2>&1 &
HTTP_PID=$!
sleep 1
curl -sf --resolve demo.fdns.run:8080:$IP http://demo.fdns.run:8080/ >/dev/null
kill $HTTP_PID

echo "Round-trip demo succeeded"
