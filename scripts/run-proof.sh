#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export PATH="$HOME/.shakenup/bin:$PATH"
exec python3 "$ROOT/scripts/run_proof.py" "$@"
