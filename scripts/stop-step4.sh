#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE_FILE="$ROOT/.run/step4/state.env"

if [[ ! -f "$STATE_FILE" ]]; then
    echo "No Step 4 demo state found."
    exit 0
fi

# shellcheck disable=SC1090
source "$STATE_FILE"

kill_pid() {
    local pid="${1:-}"
    [[ -n "$pid" ]] || return 0
    kill "$pid" >/dev/null 2>&1 || true
}

kill_group() {
    local pid="${1:-}"
    [[ -n "$pid" ]] || return 0
    python3 - "$pid" <<'PY'
import os
import signal
import sys
try:
    os.killpg(os.getpgid(int(sys.argv[1])), signal.SIGTERM)
except ProcessLookupError:
    pass
PY
}

kill_port() {
    local port="$1"
    local pids
    pids="$(lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null || true)"
    [[ -z "$pids" ]] || kill $pids >/dev/null 2>&1 || true
}

echo "Stopping HTTP proxies..."
kill_pid "${SERVE_GOVERNANCE_PID:-}"
kill_pid "${SERVE_CONSUMER_PID:-}"
kill_pid "${SERVE_INDEXER_PID:-}"
for port in 8080 8081 8082; do kill_port "$port"; done
echo "Stopping devnet..."
kill_group "${LOCALNET_PID:-}"
for port in 10087 11087 12087 13087; do kill_port "$port"; done
sleep 2
echo "Stopped. Logs remain in $ROOT/.run/step4/"
