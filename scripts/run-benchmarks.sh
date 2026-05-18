#!/usr/bin/env bash
set -euo pipefail

workspace_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$workspace_root"

git_sha="$(git rev-parse --short HEAD 2>/dev/null || printf unknown)"
run_id="$(date +%Y%m%d-%H%M%S)-${git_sha}"
output_dir="${DOMM_BENCH_OUTPUT_DIR:-target/benchmarks/$run_id}"
case "$output_dir" in
    /*) ;;
    *) output_dir="$workspace_root/$output_dir" ;;
esac

mkdir -p "$output_dir"

readarray -t preexisting_pocket_ic_pids < <(pgrep -f '/tmp/pocket-ic-server.*/pocket-ic' || true)

cleanup_pocket_ic() {
    local pid existing
    while IFS= read -r pid; do
        [[ -z "$pid" ]] && continue
        existing=0
        for before in "${preexisting_pocket_ic_pids[@]}"; do
            if [[ "$pid" == "$before" ]]; then
                existing=1
                break
            fi
        done
        if ((existing == 0)); then
            kill "$pid" 2>/dev/null || true
        fi
    done < <(pgrep -f '/tmp/pocket-ic-server.*/pocket-ic' || true)
}
trap cleanup_pocket_ic EXIT

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
