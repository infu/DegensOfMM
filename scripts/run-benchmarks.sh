#!/usr/bin/env bash
set -euo pipefail

workspace_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$workspace_root"

git_sha="$(git rev-parse --short HEAD 2>/dev/null || printf unknown)"
run_id="$(date +%Y%m%d-%H%M%S)-${git_sha}"
output_dir="${DOMM_BENCH_OUTPUT_DIR:-target/benchmarks/$run_id}"

mkdir -p "$output_dir"

printf "Running DoMM benchmark suite\n"
printf "Output: %s\n" "$output_dir"

DOMM_CANISTER_FEATURES=benchmark \
DOMM_BENCH_OUTPUT_DIR="$output_dir" \
CANIC_POCKET_IC_LOCK_NAMESPACE="domm-bench-$run_id" \
cargo test -p domm-pocket-ic-tests --test canister_endpoints \
    pocket_ic_gate_l_first_playable_canister_e2e_uses_public_endpoints_and_icydb_state \
    -- --nocapture

printf "\nBenchmark artifacts:\n"
printf "  %s\n" "$output_dir/run.json"
printf "  %s\n" "$output_dir/summary.json"
printf "  %s\n" "$output_dir/summary.md"
