#!/usr/bin/env bash
# Source this file for the demo environment, or run it with a command.

export LYQUOR_LDK_DIR="${LYQUOR_LDK_DIR:-$HOME/.shakenup/ldk}"
if [[ "$LYQUOR_LDK_DIR" != /* ]]; then
    LYQUOR_LDK_DIR="$PWD/$LYQUOR_LDK_DIR"
fi
if [[ -n "${LYQUOR_BIN_DIR:-}" && "$LYQUOR_BIN_DIR" != /* ]]; then
    export LYQUOR_BIN_DIR="$PWD/$LYQUOR_BIN_DIR"
fi

# An explicit binary directory takes precedence. Keep the current PATH and
# append the standard locations for tools installed by the project guide.
export PATH="${LYQUOR_BIN_DIR:+$LYQUOR_BIN_DIR:}${PATH:+$PATH:}$HOME/.shakenup/bin:$HOME/.cargo/bin:$HOME/.foundry/bin"

require_lyquor_localnet() {
    local script="$LYQUOR_LDK_DIR/scripts/localnet.sh"
    if [[ ! -f "$script" || ! -r "$script" ]]; then
        printf 'Lyquor localnet script not found or unreadable at %s\n' "$script" >&2
        printf 'Set LYQUOR_LDK_DIR to the installed LDK directory.\n' >&2
        return 1
    fi
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    exec "$@"
fi
