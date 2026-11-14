#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MODE="install"
REQUIRED_RUST="1.99.0"
MIN_RUST="1.80"

usage() {
    cat <<'EOF'
Usage: ./scripts/setup-env.sh [--check]

Default mode installs the Lyquor LDK, Rust toolchain, WASM target and Foundry,
then runs the project checks.

  --check   Only inspect the environment; do not install or modify anything.
  --help    Show this help.
EOF
}

for arg in "$@"; do
    case "$arg" in
        --check) MODE="check" ;;
        --help|-h) usage; exit 0 ;;
        *) echo "Unknown argument: $arg" >&2; usage >&2; exit 2 ;;
    esac
done

export PATH="$HOME/.shakenup/bin:$HOME/.cargo/bin:$PATH"

say() { printf '\n==> %s\n' "$*"; }
pass() { printf '  [PASS] %s\n' "$*"; }
warn() { printf '  [WARN] %s\n' "$*"; }
fail() { printf '  [FAIL] %s\n' "$*" >&2; exit 1; }

version_ge() {
    python3 - "$1" "$2" <<'PY'
import sys
current = tuple(int(x) for x in sys.argv[1].split(".")[:3])
minimum = tuple(int(x) for x in sys.argv[2].split(".")[:3])
raise SystemExit(0 if current >= minimum else 1)
PY
}

check_prerequisites() {
    say "Checking base tools"
    for tool in curl git python3 make; do
        command -v "$tool" >/dev/null 2>&1 || fail "missing required tool: $tool"
        pass "$tool"
    done
    python3 -c 'import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)' \
        || fail "Python 3.10+ is required"
    pass "Python $(python3 -c 'import platform; print(platform.python_version())')"
}

check_rust() {
    say "Checking Rust"
    command -v rustup >/dev/null 2>&1 || fail "rustup is not installed"
    local version
    version="$(rustc --version | awk '{print $2}')"
    version_ge "$version" "$MIN_RUST" || fail "Rust $MIN_RUST+ is required; found $version"
    pass "rustc $version (project pins $REQUIRED_RUST)"
    rustup target list --installed --toolchain "$REQUIRED_RUST" 2>/dev/null \
        | grep -qx 'wasm32-unknown-unknown' \
        || fail "wasm32-unknown-unknown is missing for Rust $REQUIRED_RUST"
    pass "wasm32-unknown-unknown"
}

check_ldk() {
    say "Checking Lyquor LDK"
    [[ -d "$HOME/.shakenup/ldk" ]] || fail "Lyquor LDK not found at ~/.shakenup/ldk"
    for tool in shaker lyquor ladle; do
        command -v "$tool" >/dev/null 2>&1 || fail "missing Lyquor tool: $tool"
        pass "$tool"
    done
    command -v forge >/dev/null 2>&1 || fail "Foundry forge is not installed"
    pass "forge $(forge --version 2>/dev/null | head -n1)"
}

install_environment() {
    say "Installing Lyquor development environment"
    if [[ -d "$HOME/.shakenup/ldk" ]]; then
        pass "Lyquor LDK already installed; skipping download"
    else
        command -v curl >/dev/null 2>&1 || fail "curl is required"
        local installer
        installer="$(mktemp)"
        trap 'rm -f "$installer"' RETURN
        curl -L https://ldk.lyquor.dev -o "$installer"
        bash "$installer"
        rm -f "$installer"
        trap - RETURN
    fi

    command -v rustup >/dev/null 2>&1 || fail "rustup installation failed"
    say "Installing Rust $REQUIRED_RUST and WASM target"
    rustup toolchain install "$REQUIRED_RUST" --profile minimal --no-self-update
    rustup default "$REQUIRED_RUST"
    rustup target add wasm32-unknown-unknown --toolchain "$REQUIRED_RUST"
    export PATH="$HOME/.shakenup/bin:$HOME/.cargo/bin:$PATH"
}

run_project_checks() {
    say "Running project checks"
    cd "$ROOT"
    make check
    shaker test lyquid-indexer/Cargo.toml
    shaker test holderproof-gate-core/Cargo.toml
    pass "all project checks passed"
}

if [[ "$MODE" == "check" ]]; then
    check_prerequisites
    check_rust
    check_ldk
    say "Environment check"
    pass "ready to use HolderProof"
    exit 0
fi

check_prerequisites
install_environment
check_rust
check_ldk
run_project_checks

say "Environment setup complete"
printf '  Next: ./scripts/reproduce-step4.sh\n'
