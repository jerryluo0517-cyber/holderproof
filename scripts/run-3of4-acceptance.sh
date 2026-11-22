#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

mkdir -p .run/acceptance
run_dir="$(mktemp -d ".run/acceptance/3of4-$(date +%Y%m%d-%H%M%S)-XXXXXX")"
printf 'Acceptance evidence directory: %s\n' "$run_dir"
git rev-parse HEAD | tee "$run_dir/commit.txt"
make check-env 2>&1 | tee "$run_dir/environment.log"

make e3 2>&1 | tee "$run_dir/e3.log"
cp .run/step4/certificate.json .run/step4/gate-cases.json "$run_dir/"
cp .run/acceptance/historical-balances.json .run/acceptance/e3-results.json "$run_dir/"

make epoch-check 2>&1 | tee "$run_dir/epoch.log"
cp .run/acceptance/epoch-checkpoints.json "$run_dir/"

make credit-check 2>&1 | tee "$run_dir/credit.log"
cp .run/acceptance/credit-results.json "$run_dir/"

make sepolia-check 2>&1 | tee "$run_dir/sepolia.log"
cp .run/acceptance/sepolia-certificate.json "$run_dir/"

printf 'All 3-of-4 acceptance checks passed. Evidence saved in %s\n' "$run_dir"
