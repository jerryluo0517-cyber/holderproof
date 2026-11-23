#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/lyquor-env.sh"
require_lyquor_localnet

exec bash "$LYQUOR_LDK_DIR/scripts/localnet.sh" "${1:-multi}"
