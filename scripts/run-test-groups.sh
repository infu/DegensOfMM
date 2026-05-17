#!/usr/bin/env bash
set -uo pipefail

workspace_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$workspace_root"

declare -A GROUP_COMMANDS
declare -A GROUP_PREBUILDS
declare -A GROUP_DESCRIPTIONS
GROUP_ORDER=()

add_group() {
    local name="$1"
    local description="$2"
    local prebuild="$3"
    local command="$4"

    GROUP_ORDER+=("$name")
    GROUP_DESCRIPTIONS["$name"]="$description"
    GROUP_PREBUILDS["$name"]="$prebuild"
    GROUP_COMMANDS["$name"]="$command"
}

add_group "pure" \
    "deterministic domm-game rules and fixture tests" \
    "cargo test -p domm-game --no-run" \
    "cargo test -p domm-game"
add_group "schema" \
    "schema macro compilation tests" \
    "cargo test -p domm-schema-macro-tests --no-run" \
    "cargo test -p domm-schema-macro-tests"
add_group "generated" \
    "generated-session harness tests" \
    "cargo test -p domm-generated-session-tests --no-run" \
    "cargo test -p domm-generated-session-tests"
add_group "canister-check" \
    "canister crate typecheck" \
    "" \
    "cargo check -p domm-degens-canister"
add_group "pocket-lock" \
    "PocketIC lock sharding probe" \
    "cargo test -p domm-pocket-ic-tests --test pic_lock --no-run" \
    "cargo test -p domm-pocket-ic-tests --test pic_lock -- --nocapture"
add_group "pocket-smoke" \
    "PocketIC package smoke without canister install" \
    "cargo test -p domm-pocket-ic-tests --test canister_smoke --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_smoke"
add_group "endpoint" \
    "endpoint inventory and public surface PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_canister_exposes_every_required_game_endpoint -- --nocapture"
add_group "gate-j" \
    "Gate J strategic loop and IcyDB persistence PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_gate_j_strategic_loop_persists_icydb_rows -- --nocapture"
add_group "gate-k" \
    "Gate K battle aftermath, victory, and history PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_gate_k_battle_aftermath_victory_history_persist_icydb_rows -- --nocapture"
add_group "gate-l" \
    "Gate L first-playable canister route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_gate_l_first_playable_canister_e2e_uses_public_endpoints_and_icydb_state -- --nocapture"
add_group "movement" \
    "movement crossing conflict PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_movement_crossing_conflict_uses_persisted_sync_cursor -- --nocapture"
add_group "stationary" \
    "stationary enemy blocker PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_stationary_enemy_blocker_starts_champion_encounter -- --nocapture"
add_group "week-two" \
    "week-two tavern and recruit growth PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_week_two_tavern_and_recruit_growth_materialize_on_turn_advance -- --nocapture"
add_group "gate-m" \
    "Gate M canister-backed web client probe" \
    "cargo test -p domm-pocket-ic-tests --test client_probe_canister --no-run" \
    "cargo test -p domm-pocket-ic-tests --test client_probe_canister gate_m_web_client_probe_runs_against_pocket_ic_canister_adapter -- --nocapture"
add_group "timer-jobs" \
    "timer jobs scheduling, repair, no-op, and lease recovery PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_timer_jobs_repair_deadlines_and_recover_expired_leases -- --nocapture"

FAST_GROUPS=("pure" "schema" "generated" "canister-check" "pocket-lock")
POCKET_GROUPS=(
    "pocket-lock"
    "pocket-smoke"
    "endpoint"
    "gate-j"
    "gate-k"
    "gate-l"
    "movement"
    "stationary"
    "week-two"
    "gate-m"
    "timer-jobs"
)

usage() {
    cat <<'USAGE'
Usage:
  scripts/run-test-groups.sh list
  scripts/run-test-groups.sh [fast|pocket|all-existing|GROUP...]

Environment:
  DOMM_TEST_JOBS      Parallel group limit. Defaults to min(nproc, 8).
  DOMM_TEST_LOG_DIR   Directory for per-group logs. Defaults under target/test-groups/.

Examples:
  DOMM_TEST_JOBS=8 scripts/run-test-groups.sh fast
  DOMM_TEST_JOBS=4 scripts/run-test-groups.sh gate-k week-two gate-m
USAGE
}

list_groups() {
    printf "Available test groups:\n"
    for group in "${GROUP_ORDER[@]}"; do
        printf "  %-15s %s\n" "$group" "${GROUP_DESCRIPTIONS[$group]}"
    done
}

selected_groups() {
    if (($# == 0)); then
        printf "%s\n" "${FAST_GROUPS[@]}"
        return
    fi

    case "$1" in
        fast)
            printf "%s\n" "${FAST_GROUPS[@]}"
            return
            ;;
        pocket)
            printf "%s\n" "${POCKET_GROUPS[@]}"
            return
            ;;
        all-existing)
            printf "%s\n" "${GROUP_ORDER[@]}"
            return
            ;;
    esac

    printf "%s\n" "$@"
}

max_jobs() {
    if [[ -n "${DOMM_TEST_JOBS:-}" ]]; then
        printf "%s\n" "$DOMM_TEST_JOBS"
        return
    fi

    local cores
    cores="$(nproc 2>/dev/null || printf "4")"
    if ((cores > 8)); then
        printf "8\n"
    elif ((cores < 1)); then
        printf "1\n"
    else
        printf "%s\n" "$cores"
    fi
}

now_ms() {
    date +%s%3N
}

format_ms() {
    local millis="$1"
    printf "%d.%03ds" "$((millis / 1000))" "$((millis % 1000))"
}

run_prebuilds() {
    local -a prebuilds=()
    local -A seen=()
    local group prebuild

    for group in "$@"; do
        prebuild="${GROUP_PREBUILDS[$group]}"
        if [[ -n "$prebuild" && -z "${seen[$prebuild]:-}" ]]; then
            prebuilds+=("$prebuild")
            seen["$prebuild"]=1
        fi
    done

    if ((${#prebuilds[@]} == 0)); then
        return
    fi

    printf "Prebuilding %d unique test target(s)...\n" "${#prebuilds[@]}"
    for prebuild in "${prebuilds[@]}"; do
        local started ended elapsed
        started="$(now_ms)"
        printf "  $ %s\n" "$prebuild"
        if ! bash -lc "$prebuild"; then
            printf "Prebuild failed: %s\n" "$prebuild" >&2
            exit 1
        fi
        ended="$(now_ms)"
        elapsed="$((ended - started))"
        printf "    prebuild time: %s\n" "$(format_ms "$elapsed")"
    done
}

run_group() {
    local group="$1"
    local command="${GROUP_COMMANDS[$group]}"
    local log_file="$LOG_DIR/$group.log"
    local result_file="$LOG_DIR/$group.result"
    local started ended elapsed status

    started="$(now_ms)"
    CANIC_POCKET_IC_LOCK_NAMESPACE="domm-$RUN_ID-$group" bash -lc "$command" \
        >"$log_file" 2>&1
    status=$?
    ended="$(now_ms)"
    elapsed="$((ended - started))"

    printf "%s\t%s\t%s\t%s\n" \
        "$group" "$status" "$(format_ms "$elapsed")" "$command" >"$result_file"
    return "$status"
}

case "${1:-}" in
    list)
        list_groups
        exit 0
        ;;
    help | --help | -h)
        usage
        exit 0
        ;;
esac

selected=()
while IFS= read -r group; do
    [[ -n "$group" ]] && selected+=("$group")
done < <(selected_groups "$@")

if ((${#selected[@]} == 0)); then
    usage >&2
    exit 2
fi

for group in "${selected[@]}"; do
    if [[ -z "${GROUP_COMMANDS[$group]:-}" ]]; then
        printf "Unknown test group: %s\n\n" "$group" >&2
        list_groups >&2
        exit 2
    fi
done

jobs="$(max_jobs)"
if ! [[ "$jobs" =~ ^[0-9]+$ ]] || ((jobs < 1)); then
    printf "DOMM_TEST_JOBS must be a positive integer, got: %s\n" "$jobs" >&2
    exit 2
fi

RUN_ID="$(date +%Y%m%d-%H%M%S)"
LOG_DIR="${DOMM_TEST_LOG_DIR:-target/test-groups/$RUN_ID}"
mkdir -p "$LOG_DIR"

printf "Running %d group(s) with DOMM_TEST_JOBS=%s\n" "${#selected[@]}" "$jobs"
printf "Logs: %s\n" "$LOG_DIR"

run_prebuilds "${selected[@]}"

running=0
overall=0
for group in "${selected[@]}"; do
    run_group "$group" &
    running="$((running + 1))"
    if ((running >= jobs)); then
        if ! wait -n; then
            overall=1
        fi
        running="$((running - 1))"
    fi
done

while ((running > 0)); do
    if ! wait -n; then
        overall=1
    fi
    running="$((running - 1))"
done

printf "\n| Test group | Status | Time | Command |\n"
printf "| --- | --- | ---: | --- |\n"
for group in "${selected[@]}"; do
    IFS=$'\t' read -r result_group status elapsed command <"$LOG_DIR/$group.result"
    if [[ "$status" == "0" ]]; then
        label="pass"
    else
        label="fail($status)"
        overall=1
    fi
    printf "| %s | %s | %s | \`%s\` |\n" "$result_group" "$label" "$elapsed" "$command"
done

if ((overall != 0)); then
    printf "\nFailed group log tails:\n" >&2
    for group in "${selected[@]}"; do
        IFS=$'\t' read -r _ status _ _ <"$LOG_DIR/$group.result"
        if [[ "$status" != "0" ]]; then
            printf "\n==> %s (%s)\n" "$group" "$LOG_DIR/$group.log" >&2
            tail -80 "$LOG_DIR/$group.log" >&2
        fi
    done
    exit "$overall"
fi
