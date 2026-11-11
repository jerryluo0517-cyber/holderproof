#!/usr/bin/env bash
set -euo pipefail

# Step 2 uses the official native multi-node localnet (4 nodes).
exec "$HOME/.shakenup/ldk/scripts/localnet.sh" "${1:-multi}"
