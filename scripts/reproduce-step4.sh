#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RUN_DIR="$ROOT/.run/step4"
STATE_FILE="$RUN_DIR/state.env"
RPC="ws://127.0.0.1:10087/ws"
ANVIL="http://127.0.0.1:8545"
ALICE="0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
BLOCK=1000
THRESHOLD=60000
ANVIL_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"

export PATH="$HOME/.shakenup/bin:$PATH"

require_tool() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "Missing required tool: $1" >&2
        echo "Install Lyquor LDK with: curl -L https://ldk.lyquor.dev | bash" >&2
        exit 1
    }
}

require_port_free() {
    local port="$1"
    if lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
        echo "Port $port is already in use. Run scripts/stop-step4.sh or stop the existing demo." >&2
        exit 1
    fi
}

wait_for_url() {
    local url="$1"
    local label="$2"
    local deadline=$((SECONDS + 90))
    while ! curl -fsS "$url" >/dev/null 2>&1; do
        ((SECONDS < deadline)) || {
            echo "Timed out waiting for $label at $url" >&2
            exit 1
        }
        sleep 1
    done
}

wait_for_bartender() {
    local deadline=$((SECONDS + 90))
    while ! shaker list --endpoint "$RPC" 2>/dev/null | grep -q '^Lyquid-'; do
        ((SECONDS < deadline)) || {
            echo "Timed out waiting for bartender registration" >&2
            exit 1
        }
        sleep 1
    done
}

deploy_json() {
    local manifest="$1"
    local reference="$2"
    shaker deploy \
        --endpoint "$RPC" \
        --reference "$reference" \
        --private-key "$ANVIL_KEY" \
        --output json \
        "$manifest"
}

json_field() {
    local field="$1"
    python3 -c '
import json
import sys
raw = sys.stdin.read().strip()
for line in reversed(raw.splitlines()):
    try:
        data = json.loads(line)
    except json.JSONDecodeError:
        continue
    print(data.get(sys.argv[1], ""))
    raise SystemExit(0)
raise SystemExit(1)
' "$field"
}

node_id_hex() {
    local port="$1"
    curl -fsS --data '{}' \
        --header 'content-type: application/json' \
        "http://127.0.0.1:${port}/lyquor.node.v1.NodeService/GetNodeInfo" \
        | python3 -c '
import base64
import json
import sys
payload = json.load(sys.stdin)
value = payload.get("node_id", {}).get("value") or payload.get("nodeId", {}).get("value")
if not value:
    raise SystemExit("missing node id")
if value.startswith("Node-"):
    value = value[5:]
value = value.upper()
value += "=" * ((8 - len(value) % 8) % 8)
raw = base64.b32decode(value, casefold=True)
print("0x" + raw[:32].hex())
'
}

oracle_epoch() {
    cast call --rpc-url "$RPC" "$INDEXER_CONTRACT" \
        '__lyquor_oracle_dest_epoch_info(string,bool) returns (uint64,bytes32,uint32,bytes)' \
        balance_proof false \
        | head -n1 | tr -d '[:space:]'
}

state_write() {
    {
        printf 'LOCALNET_PID=%q\n' "$LOCALNET_PID"
        printf 'SERVE_INDEXER_PID=%q\n' "$SERVE_INDEXER_PID"
        printf 'SERVE_CONSUMER_PID=%q\n' "$SERVE_CONSUMER_PID"
        printf 'SERVE_GOVERNANCE_PID=%q\n' "$SERVE_GOVERNANCE_PID"
        printf 'GOVERNANCE_CONTRACT=%q\n' "$GOVERNANCE_CONTRACT"
        printf 'GOVERNANCE_LYQUID=%q\n' "$GOVERNANCE_LYQUID"
        printf 'INDEXER_LYQUID=%q\n' "$INDEXER_LYQUID"
        printf 'INDEXER_CONTRACT=%q\n' "$INDEXER_CONTRACT"
        printf 'CONSUMER_LYQUID=%q\n' "$CONSUMER_LYQUID"
        printf 'CONSUMER_CONTRACT=%q\n' "$CONSUMER_CONTRACT"
    } > "$STATE_FILE"
}

for tool in shaker cast curl lsof python3; do
    require_tool "$tool"
done
for port in 8545 10087 11087 12087 13087 8080 8081 8082; do
    require_port_free "$port"
done

mkdir -p "$RUN_DIR"
rm -f "$STATE_FILE"

echo "[1/9] Starting four-node devnet..."
python3 - "$ROOT/scripts/start-devnet.sh" "$RUN_DIR/localnet.log" <<'PY' &
import os
import sys
os.setpgrp()
fd = os.open(sys.argv[2], os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o644)
os.dup2(fd, 1)
os.dup2(fd, 2)
os.execv(sys.argv[1], [sys.argv[1], "multi"])
PY
LOCALNET_PID=$!
SERVE_INDEXER_PID=""
SERVE_CONSUMER_PID=""
SERVE_GOVERNANCE_PID=""
GOVERNANCE_CONTRACT=""
GOVERNANCE_LYQUID=""
INDEXER_LYQUID=""
INDEXER_CONTRACT=""
CONSUMER_LYQUID=""
CONSUMER_CONTRACT=""
state_write
wait_for_url "http://127.0.0.1:10087/lyquor.node.v1.NodeService/GetNodeInfo" "Lyquor node 1"
wait_for_bartender
echo "      devnet pid: $LOCALNET_PID"

echo "[2/9] Deploying Indexer..."
INDEXER_OUTPUT="$(deploy_json "$ROOT/lyquid-indexer/Cargo.toml" "http://127.0.0.1:10087/lyquids:holderproof-indexer-local")"
INDEXER_LYQUID="$(printf '%s' "$INDEXER_OUTPUT" | json_field lyquid_id)"
INDEXER_CONTRACT="$(printf '%s' "$INDEXER_OUTPUT" | json_field contract)"
INDEXER_TARGET="$(shaker to-hex "$INDEXER_LYQUID")"
[[ "$INDEXER_LYQUID" == Lyquid-* && "$INDEXER_CONTRACT" == 0x* ]] || {
    echo "Failed to parse Indexer deployment: $INDEXER_OUTPUT" >&2
    exit 1
}
echo "      $INDEXER_LYQUID -> $INDEXER_CONTRACT"

echo "[3/9] Deploying Consumer..."
CONSUMER_OUTPUT="$(deploy_json "$ROOT/lyquid-consumer/Cargo.toml" "http://127.0.0.1:10087/lyquids:holderproof-consumer-local")"
CONSUMER_LYQUID="$(printf '%s' "$CONSUMER_OUTPUT" | json_field lyquid_id)"
CONSUMER_CONTRACT="$(printf '%s' "$CONSUMER_OUTPUT" | json_field contract)"
[[ "$CONSUMER_LYQUID" == Lyquid-* && "$CONSUMER_CONTRACT" == 0x* ]] || {
    echo "Failed to parse Consumer deployment: $CONSUMER_OUTPUT" >&2
    exit 1
}
echo "      $CONSUMER_LYQUID -> $CONSUMER_CONTRACT"

echo "      Deploying independent GovernanceGateLyquid..."
GOVERNANCE_OUTPUT="$(deploy_json "$ROOT/lyquid-governance-gate/Cargo.toml" "http://127.0.0.1:10087/lyquids:holderproof-governance-local")"
GOVERNANCE_LYQUID="$(printf '%s' "$GOVERNANCE_OUTPUT" | json_field lyquid_id)"
GOVERNANCE_CONTRACT="$(printf '%s' "$GOVERNANCE_OUTPUT" | json_field contract)"
[[ "$GOVERNANCE_LYQUID" == Lyquid-* && "$GOVERNANCE_CONTRACT" == 0x* ]] || exit 1
state_write

echo "[4/9] Reading four committee node IDs..."
NODE1="$(node_id_hex 10087)"
NODE2="$(node_id_hex 11087)"
NODE3="$(node_id_hex 12087)"
NODE4="$(node_id_hex 13087)"

echo "[5/9] Configuring 3-of-4 committee and query..."
cast send "$INDEXER_CONTRACT" 'configure_committee(bytes32[])' \
    "[$NODE1,$NODE2,$NODE3,$NODE4]" \
    --private-key "$ANVIL_KEY" --rpc-url "$RPC" --quiet
cast send "$INDEXER_CONTRACT" 'set_query(address,uint64)' "$ALICE" "$BLOCK" \
    --private-key "$ANVIL_KEY" --rpc-url "$RPC" --quiet
cast call --rpc-url "$RPC" "$INDEXER_CONTRACT" \
    '__lyquor_oracle_advance_epoch(string,address,bool) returns (bool)' \
    balance_proof "$INDEXER_TARGET" false >/dev/null

OLD_EPOCH=0
for _ in $(seq 1 60); do
    CURRENT_EPOCH="$(oracle_epoch || true)"
    if [[ "$CURRENT_EPOCH" =~ ^[0-9]+$ ]] && (( CURRENT_EPOCH > OLD_EPOCH )); then
        break
    fi
    sleep 1
done
cast call --rpc-url "$RPC" "$INDEXER_CONTRACT" \
    '__lyquor_oracle_finalize_epoch(string,address,bool) returns (bool)' \
    balance_proof "$INDEXER_TARGET" false >/dev/null

echo "[6/9] Generating certified balance..."
cast call --rpc-url "$RPC" "$INDEXER_CONTRACT" 'report_balance() returns (bool)' >/dev/null
for _ in $(seq 1 60); do
    CERT_JSON="$(cast call --rpc-url "$RPC" "$INDEXER_CONTRACT" \
        'get_certificate_json(address,uint64) returns (string)' "$ALICE" "$BLOCK" 2>/dev/null || true)"
    if [[ -n "$CERT_JSON" ]]; then
        break
    fi
    sleep 1
done
[[ -n "${CERT_JSON:-}" ]] || {
    echo "Certificate was not generated. See $RUN_DIR/localnet.log" >&2
    exit 1
}

echo "[7/9] Configuring Consumer..."
cast send "$CONSUMER_CONTRACT" 'configure(address,address,uint64,uint256)' \
    "$INDEXER_TARGET" "$ALICE" "$BLOCK" "$THRESHOLD" \
    --private-key "$ANVIL_KEY" --rpc-url "$ANVIL" --quiet
sleep 2
cast send "$CONSUMER_CONTRACT" 'check_eligible()' \
    --private-key "$ANVIL_KEY" --rpc-url "$ANVIL" --quiet

cast send "$GOVERNANCE_CONTRACT" 'configure(address,address,uint64,uint256,uint64)' \
    "$INDEXER_TARGET" "$ALICE" "$BLOCK" 70000 1 \
    --private-key "$ANVIL_KEY" --rpc-url "$ANVIL" --quiet
sleep 2
cast send "$GOVERNANCE_CONTRACT" 'check_eligible()' \
    --private-key "$ANVIL_KEY" --rpc-url "$ANVIL" --quiet

echo "[8/9] Starting HTTP diagnostics..."
shaker serve --endpoint "$RPC" --listen 127.0.0.1:8081 "$INDEXER_LYQUID" \
    > "$RUN_DIR/serve-indexer.log" 2>&1 &
SERVE_INDEXER_PID=$!
shaker serve --endpoint "$RPC" --listen 127.0.0.1:8080 "$CONSUMER_LYQUID" \
    > "$RUN_DIR/serve-consumer.log" 2>&1 &
SERVE_CONSUMER_PID=$!
shaker serve --endpoint "$RPC" --listen 127.0.0.1:8082 "$GOVERNANCE_LYQUID" \
    > "$RUN_DIR/serve-governance.log" 2>&1 &
SERVE_GOVERNANCE_PID=$!
state_write
wait_for_url "http://127.0.0.1:8081/lyquid/info" "Indexer proxy"
wait_for_url "http://127.0.0.1:8080/lyquid/info" "Consumer proxy"

wait_for_url "http://127.0.0.1:8082/lyquid/info" "Governance proxy"

echo "[9/9] Result"
echo "Certificate:"
curl -fsS http://127.0.0.1:8081/api/certificate
echo
echo "Consumer:"
curl -fsS http://127.0.0.1:8080/api/result
echo

state_write
python3 "$ROOT/scripts/verify_step4.py" --exercise-gates
printf '\nReproduction complete.\n'
printf '  Certificate: http://127.0.0.1:8081/api/certificate\n'
printf '  Result:      http://127.0.0.1:8080/api/result\n'
printf '  Governance:  http://127.0.0.1:8082/api/result\n'
printf '  Stop with:   ./scripts/stop-step4.sh\n'
