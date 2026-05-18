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
suite_output="$output_dir/test-output.log"
prebuild_output="$output_dir/prebuild.log"
: > "$suite_output"
: > "$prebuild_output"

readarray -t preexisting_pocket_ic_pids < <(pgrep -f '/tmp/pocket-ic-server.*/pocket-ic' || true)

cleanup_pocket_ic() {
    local pid existing before
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

default_jobs() {
    local cores
    cores="$(nproc 2>/dev/null || printf "4")"
    if ((cores > 4)); then
        printf "4\n"
    elif ((cores < 1)); then
        printf "1\n"
    else
        printf "%s\n" "$cores"
    fi
}

benchmark_jobs="${DOMM_BENCH_JOBS:-$(default_jobs)}"
if ! [[ "$benchmark_jobs" =~ ^[0-9]+$ ]] || ((benchmark_jobs < 1)); then
    benchmark_jobs=1
fi

GATES=("gate-j" "gate-k" "gate-l" "gate-m")
declare -A GATE_TEST_FILE=(
    ["gate-j"]="canister_endpoints"
    ["gate-k"]="canister_endpoints"
    ["gate-l"]="canister_endpoints"
    ["gate-m"]="client_probe_canister"
)
declare -A GATE_TEST_NAME=(
    ["gate-j"]="pocket_ic_gate_j_strategic_loop_persists_icydb_rows"
    ["gate-k"]="pocket_ic_gate_k_battle_aftermath_victory_history_persist_icydb_rows"
    ["gate-l"]="pocket_ic_gate_l_first_playable_canister_e2e_uses_public_endpoints_and_icydb_state"
    ["gate-m"]="gate_m_web_client_probe_runs_against_pocket_ic_canister_adapter"
)
declare -A GATE_DESCRIPTION=(
    ["gate-j"]="Strategic loop persistence"
    ["gate-k"]="Battle aftermath and victory history"
    ["gate-l"]="First-playable public endpoint route"
    ["gate-m"]="Canister-backed web client probe"
)

printf "Running DoMM benchmark suite\n"
printf "Output: %s\n" "$output_dir"
printf "Parallel gate jobs: %s\n" "$benchmark_jobs"

printf "Prebuilding benchmark test targets...\n"
{
    DOMM_CANISTER_FEATURES=benchmark \
        cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run
    DOMM_CANISTER_FEATURES=benchmark \
        cargo test -p domm-pocket-ic-tests --test client_probe_canister --no-run
} >"$prebuild_output" 2>&1

run_gate() {
    local gate="$1"
    local gate_dir="$output_dir/$gate"
    local gate_log="$gate_dir/test-output.log"
    local status_file="$gate_dir/status.txt"
    local started ended elapsed status

    mkdir -p "$gate_dir"
    : > "$gate_log"
    started="$(date +%s)"
    printf "started %s at %s\n" "$gate" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$status_file"

    set +e
    DOMM_CANISTER_FEATURES=benchmark \
    DOMM_BENCH_OUTPUT_DIR="$gate_dir" \
    DOMM_BENCH_QUERY_LOG_PATH="$gate_log" \
    CANIC_POCKET_IC_LOCK_NAMESPACE="domm-bench-$run_id-$gate" \
    cargo test -p domm-pocket-ic-tests --test "${GATE_TEST_FILE[$gate]}" \
        "${GATE_TEST_NAME[$gate]}" -- --nocapture >"$gate_log" 2>&1
    status=$?
    set -e

    ended="$(date +%s)"
    elapsed="$((ended - started))"
    if ((status == 0)); then
        printf "passed\nelapsed_seconds=%s\n" "$elapsed" > "$status_file"
    else
        printf "failed\nelapsed_seconds=%s\n" "$elapsed" > "$status_file"
    fi
    return "$status"
}

wait_for_slot() {
    while (($(jobs -rp | wc -l) >= benchmark_jobs)); do
        sleep 2
    done
}

for gate in "${GATES[@]}"; do
    wait_for_slot
    printf "Starting %s: %s\n" "$gate" "${GATE_DESCRIPTION[$gate]}"
    run_gate "$gate" &
done

set +e
wait
set -e

for gate in "${GATES[@]}"; do
    gate_log="$output_dir/$gate/test-output.log"
    {
        printf "\n===== %s =====\n" "$gate"
        cat "$gate_log"
    } >> "$suite_output"
done

jq_available=0
if command -v jq >/dev/null 2>&1; then
    jq_available=1
fi

format_scaled() {
    local value="$1"
    local scale="$2"
    awk -v value="$value" -v scale="$scale" 'BEGIN {
        scaled = value / scale;
        text = sprintf("%.4f", scaled);
        sub(/\.?0+$/, "", text);
        if (text == "-0") {
            text = "0";
        }
        print text;
    }'
}

summary_number() {
    local summary="$1"
    local filter="$2"
    if ((jq_available == 0)) || [[ ! -f "$summary" ]]; then
        return 1
    fi
    jq -r "$filter" "$summary" 2>/dev/null
}

summary_scaled() {
    local summary="$1"
    local filter="$2"
    local scale="$3"
    local value
    value="$(summary_number "$summary" "$filter" || true)"
    if [[ -z "$value" || "$value" == "null" ]]; then
        printf "n/a"
        return
    fi
    format_scaled "$value" "$scale"
}

gate_log_metric() {
    local log="$1"
    local key="$2"
    local line
    line="$(grep 'Gate M canister client metrics:' "$log" | tail -n 1 || true)"
    if [[ -z "$line" ]]; then
        return 1
    fi
    tr ' ' '\n' <<<"$line" | awk -F= -v key="$key" '$1 == key { print $2; found = 1 } END { exit found ? 0 : 1 }'
}

gate_query_instruction_billions() {
    local log="$1"
    awk '/DOMM_BENCH_QUERY/ {
        for (i = 1; i <= NF; i++) {
            if ($i ~ /^instruction_delta=/) {
                split($i, field, "=");
                total += field[2];
            }
        }
    } END {
        if (total == 0) {
            exit 1;
        }
        scaled = total / 1000000000;
        text = sprintf("%.4f", scaled);
        sub(/\.?0+$/, "", text);
        print text;
    }' "$log"
}

write_suite_markdown() {
    local suite_md="$output_dir/suite-summary.md"
    local gate gate_dir gate_log status summary calls scenarios required row_growth stable_pages instructions cycles memory artifacts

    {
        printf "# DoMM Benchmark Suite %s\n\n" "$run_id"
        printf -- "- Git: \`%s\`\n" "$git_sha"
        printf -- "- Parallel gate jobs: %s\n" "$benchmark_jobs"
        printf -- "- Output: \`%s\`\n\n" "$output_dir"
        printf "| Gate | Status | Calls | Scenarios | Required endpoints | Row growth | Stable pages | Instructions B | Cycles T | Memory MB | Artifacts |\n"
        printf "| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | --- |\n"

        for gate in "${GATES[@]}"; do
            gate_dir="$output_dir/$gate"
            gate_log="$gate_dir/test-output.log"
            status="$(head -n 1 "$gate_dir/status.txt" 2>/dev/null || printf unknown)"
            summary="$gate_dir/summary.json"
            calls="n/a"
            scenarios="n/a"
            required="n/a"
            row_growth="n/a"
            stable_pages="n/a"
            instructions="n/a"
            cycles="n/a"
            memory="n/a"
            artifacts="\`$gate/test-output.log\`"

            if [[ -f "$summary" && $jq_available -eq 1 ]]; then
                calls="$(summary_number "$summary" '.call_count' || printf "n/a")"
                scenarios="$(summary_number "$summary" '.scenario_count' || printf "n/a")"
                required="$(summary_number "$summary" '"\(.covered_required_endpoint_count)/\(.required_game_endpoint_count)"' || printf "n/a")"
                row_growth="$(summary_number "$summary" '.total_row_growth' || printf "n/a")"
                stable_pages="$(summary_number "$summary" '"\(.stable_memory_pages_start) -> \(.stable_memory_pages_final)"' || printf "n/a")"
                instructions="$(summary_scaled "$summary" '[.scenarios[].instruction_total] | add // 0' 1000000000)"
                cycles="$(summary_scaled "$summary" '[.scenarios[].cycle_cost_total] | add // 0' 1000000000000)"
                memory="$(summary_scaled "$summary" '[.scenarios[].memory_delta_bytes] | add // 0' 1048576)"
                artifacts="\`$gate/summary.md\`, \`$gate/run.json\`, \`$gate/test-output.log\`"
            elif [[ "$gate" == "gate-m" ]]; then
                local updates queries total_rows stable_start stable_final
                updates="$(gate_log_metric "$gate_log" "updates" || printf "n/a")"
                queries="$(gate_log_metric "$gate_log" "queries" || printf "n/a")"
                if [[ "$updates" =~ ^[0-9]+$ && "$queries" =~ ^[0-9]+$ ]]; then
                    calls="$((updates + queries))"
                fi
                row_growth="$(gate_log_metric "$gate_log" "row_growth" || printf "n/a")"
                total_rows="$(gate_log_metric "$gate_log" "total_rows" || printf "n/a")"
                stable_start="$(gate_log_metric "$gate_log" "stable_pages_start" || printf "n/a")"
                stable_final="$(gate_log_metric "$gate_log" "stable_pages_final" || printf "n/a")"
                stable_pages="${stable_start} -> ${stable_final}"
                instructions="$(gate_query_instruction_billions "$gate_log" || printf "n/a")"
                if [[ "$stable_start" =~ ^[0-9]+$ && "$stable_final" =~ ^[0-9]+$ ]]; then
                    memory="$(format_scaled "$(((stable_final - stable_start) * 65536))" 1048576)"
                fi
                scenarios="1"
                required="probe"
                artifacts="\`$gate/test-output.log\`"
            fi

            printf "| %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s |\n" \
                "$gate" "$status" "$calls" "$scenarios" "$required" "$row_growth" \
                "$stable_pages" "$instructions" "$cycles" "$memory" "$artifacts"
        done
    } > "$suite_md"
}

json_escape() {
    sed 's/\\/\\\\/g; s/"/\\"/g' <<<"$1"
}

write_suite_json() {
    local suite_json="$output_dir/suite-summary.json"
    local first=1 gate gate_dir gate_log status summary elapsed

    {
        printf '{\n'
        printf '  "run_id": "%s",\n' "$(json_escape "$run_id")"
        printf '  "git_sha": "%s",\n' "$(json_escape "$git_sha")"
        printf '  "parallel_gate_jobs": %s,\n' "$benchmark_jobs"
        printf '  "output_dir": "%s",\n' "$(json_escape "$output_dir")"
        printf '  "gates": [\n'
        for gate in "${GATES[@]}"; do
            gate_dir="$output_dir/$gate"
            gate_log="$gate_dir/test-output.log"
            status="$(head -n 1 "$gate_dir/status.txt" 2>/dev/null || printf unknown)"
            elapsed="$(awk -F= '$1 == "elapsed_seconds" { print $2 }' "$gate_dir/status.txt" 2>/dev/null || true)"
            [[ -z "$elapsed" ]] && elapsed=0
            summary="$gate_dir/summary.json"
            if ((first == 0)); then
                printf ',\n'
            fi
            first=0
            printf '    {\n'
            printf '      "name": "%s",\n' "$(json_escape "$gate")"
            printf '      "description": "%s",\n' "$(json_escape "${GATE_DESCRIPTION[$gate]}")"
            printf '      "status": "%s",\n' "$(json_escape "$status")"
            printf '      "elapsed_seconds": %s,\n' "$elapsed"
            printf '      "output_dir": "%s",\n' "$(json_escape "$gate_dir")"
            printf '      "test_output": "%s",\n' "$(json_escape "$gate_log")"
            if [[ -f "$summary" ]]; then
                printf '      "summary_json": "%s"\n' "$(json_escape "$summary")"
            else
                printf '      "summary_json": null\n'
            fi
            printf '    }'
        done
        printf '\n  ]\n'
        printf '}\n'
    } > "$suite_json"
}

write_suite_markdown
write_suite_json

failed=0
for gate in "${GATES[@]}"; do
    status="$(head -n 1 "$output_dir/$gate/status.txt" 2>/dev/null || printf unknown)"
    if [[ "$status" != "passed" ]]; then
        failed=1
    fi
done

printf "\nBenchmark artifacts:\n"
printf "  %s\n" "$output_dir/suite-summary.md"
printf "  %s\n" "$output_dir/suite-summary.json"
printf "  %s\n" "$suite_output"
for gate in "${GATES[@]}"; do
    if [[ -f "$output_dir/$gate/summary.md" ]]; then
        printf "  %s\n" "$output_dir/$gate/summary.md"
    else
        printf "  %s\n" "$output_dir/$gate/test-output.log"
    fi
done

if ((failed != 0)); then
    printf "\nAt least one benchmark gate failed. See per-gate logs above.\n" >&2
    exit 1
fi
