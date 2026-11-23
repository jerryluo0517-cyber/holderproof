#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
RUN_DIR="$ROOT/.run/windows"
PID_FILE="$RUN_DIR/runtime.pid"
export PATH="$HOME/.shakenup/bin:$HOME/.cargo/bin:$HOME/.foundry/bin:$PATH"
# Avoid slow Cargo builds on /mnt/c or /mnt/d, and Windows/Linux target mixing.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/holderproof/target}"
cd "$ROOT"
mkdir -p "$RUN_DIR"

owned_pid() {
    python3 - "$PID_FILE" "$ROOT" <<'PY'
import sys
from pathlib import Path
try:
    pid = int(Path(sys.argv[1]).read_text().strip())
    proc = Path('/proc') / str(pid)
    args = (proc / 'cmdline').read_bytes().split(b'\0')
    runner = str(Path(sys.argv[2]) / 'scripts/windows/runtime.sh').encode()
    if runner in args and b'serve' in args and (proc / 'cwd').resolve() == Path(sys.argv[2]).resolve():
        print(pid)
        sys.exit(0)
except (OSError, ValueError):
    pass
sys.exit(1)
PY
}

case "${1:-check}" in
    setup) bash scripts/setup-env.sh ;;
    check) bash scripts/setup-env.sh --check ;;
    test) make check guest-test ;;
    status) owned_pid ;;
    stop)
        if pid="$(owned_pid)"; then
            kill -TERM "$pid"
            for _ in $(seq 1 30); do
                owned_pid >/dev/null || { echo 'HolderProof stopped.'; exit 0; }
                sleep 1
            done
            echo 'Shutdown is still in progress; inspect runtime logs.' >&2
            exit 1
        fi
        echo 'No managed Windows runtime found for this checkout.'
        ;;
    serve)
        command -v flock >/dev/null || { echo 'Install util-linux (flock).' >&2; exit 1; }
        exec 9>"$RUN_DIR/runtime.lock"
        flock -n 9 || { echo 'Another runtime is already starting.' >&2; exit 1; }
        python3 - <<'PY'
import socket
with socket.socket() as sock:
    sock.bind(('127.0.0.1', 8765))
PY
        printf '%s\n' "$$" > "$PID_FILE"
        web_pid=''
        cleanup() {
            trap - EXIT INT TERM
            if [[ -n "$web_pid" ]]; then
                kill "$web_pid" 2>/dev/null || true
                wait "$web_pid" 2>/dev/null || true
            fi
            bash scripts/stop-step4.sh
            rm -f "$PID_FILE"
        }
        # An existing demo must not be taken over by a failed second launch.
        for port in 8545 10087 11087 12087 13087 8080 8081 8082 8083; do
            if lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
                rm -f "$PID_FILE"
                echo "Port $port is occupied. Stop the existing demo first." >&2
                exit 1
            fi
        done
        trap cleanup EXIT
        trap 'exit 130' INT
        trap 'exit 143' TERM
        bash scripts/reproduce-step4.sh
        python3 -u scripts/serve_web.py &
        web_pid=$!
        wait "$web_pid"
        ;;
    *) echo "Usage: $0 {setup|check|test|serve|status|stop}" >&2; exit 2 ;;
esac
