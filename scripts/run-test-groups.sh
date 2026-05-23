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
add_group "pure-property" \
    "perf1 pure domm-game property-style core rule matrix" \
    "cargo test -p domm-game --no-run" \
    "cargo test -p domm-game property_ -- --nocapture"
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
add_group "service-regression" \
    "perf1 fast canister service regression filter" \
    "cargo test -p domm-degens-canister --no-run" \
    "cargo test -p domm-degens-canister service_ -- --nocapture"
add_group "projection-recovery" \
    "perf1 projection flush and recovery service filters" \
    "cargo test -p domm-degens-canister --no-run" \
    "cargo test -p domm-degens-canister projection -- --nocapture && cargo test -p domm-degens-canister flush -- --nocapture"
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
add_group "endpoint-auth" \
    "endpoint anonymous and non-participant auth matrix PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_endpoint_auth_matrix_rejects_anonymous_and_non_participant_session_reads -- --nocapture"
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
add_group "setup" \
    "one-call setup progress, replay, and post-upgrade resume PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_one_call_setup_progress_replay_and_upgrade_resume -- --nocapture"
add_group "gate-m" \
    "Gate M canister-backed web client probe" \
    "cargo test -p domm-pocket-ic-tests --test client_probe_canister --no-run" \
    "cargo test -p domm-pocket-ic-tests --test client_probe_canister gate_m_web_client_probe_runs_against_pocket_ic_canister_adapter -- --nocapture"
add_group "timer-jobs" \
    "timer jobs scheduling, repair, no-op, and lease recovery PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_timer_jobs -- --nocapture"
add_group "end-turn" \
    "end-turn readiness, stale command, and replay PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_end_turn_closes_turn_and_blocks_stale_actions -- --nocapture"
add_group "battle-round" \
    "battle-round readiness, auto-defend, end-turn, and replay PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round -- --test-threads=1 --nocapture"
add_group "render-projection" \
    "render projection live objects, fog, cursors, and aftermath PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_render_projection -- --test-threads=1 --nocapture"
add_group "query-budget" \
    "query budget preview, submit, bounded render reads, and response sizes PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_query_budget_keeps_preview_submit_and_render_bounded -- --nocapture"
add_group "command-recovery" \
    "command recovery replay, ledger, and aftermath idempotency PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_command_recovery_replays_economy_and_battle_effects -- --nocapture"
add_group "visibility-redaction" \
    "visibility and public/private redaction PocketIC route" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run" \
    "cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_visibility_redaction_keeps_private_payloads_private -- --nocapture"

if [[ "${DOMM_ENABLE_FAILURE_ARTIFACT_SELF_TEST:-0}" == "1" ]]; then
    add_group "failure-artifact-self-test" \
        "hidden failing group used to verify failure artifact capture" \
        "" \
        "printf 'seed=artifact-self-test\nstep=before-failure\nlast successful view snapshot: none\ncommand_id=command:self-test event_id=event:self-test\nactive runtime diagnostic: synthetic\nprojection snapshot: synthetic\ntimer job snapshot: synthetic\n' && exit 7"
fi

FAST_GROUPS=("pure" "schema" "generated" "canister-check" "pocket-lock")
PERF1_FAST_GROUPS=("pure-property" "service-regression" "projection-recovery" "canister-check")
POCKET_PARALLEL_GROUPS=(
    "pocket-lock"
    "pocket-smoke"
    "endpoint-auth"
    "week-two"
    "setup"
    "timer-jobs"
    "end-turn"
    "battle-round"
    "render-projection"
    "query-budget"
    "command-recovery"
    "visibility-redaction"
)
POCKET_LONG_GROUPS=(
    "endpoint"
    "gate-j"
    "gate-k"
    "gate-l"
    "movement"
    "stationary"
)
POCKET_GATE_M_GROUPS=(
    "gate-m"
)
POCKET_SERIAL_GROUPS=(
    "${POCKET_LONG_GROUPS[@]}"
    "${POCKET_GATE_M_GROUPS[@]}"
)
POCKET_GROUPS=(
    "${POCKET_PARALLEL_GROUPS[@]}"
    "${POCKET_SERIAL_GROUPS[@]}"
)
PERF1_FOCUSED_GROUPS=(
    "endpoint-auth"
    "gate-l"
    "render-projection"
    "battle-round"
    "command-recovery"
    "visibility-redaction"
)
PERF1_LONG_FORM_GROUPS=(
    "gate-j"
    "gate-k"
    "gate-l"
    "movement"
    "stationary"
    "week-two"
    "gate-m"
)

usage() {
    cat <<'USAGE'
Usage:
  scripts/run-test-groups.sh list
  scripts/run-test-groups.sh [fast|perf1-fast|perf1-focused|perf1-long-form|pocket|pocket-parallel|pocket-long|pocket-gate-m|pocket-serial|all-existing|GROUP...]

Environment:
  DOMM_TEST_JOBS      Parallel group limit. Defaults to min(nproc, 8).
  DOMM_TEST_LOG_DIR   Directory for per-group logs. Defaults under target/test-groups/.
  DOMM_TEST_ARTIFACT_DIR
                      Directory for failure bundles. Defaults to target/test-artifacts/.

Examples:
  DOMM_TEST_JOBS=8 scripts/run-test-groups.sh fast
  DOMM_TEST_JOBS=4 scripts/run-test-groups.sh perf1-focused
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
        perf1-fast)
            printf "%s\n" "${PERF1_FAST_GROUPS[@]}"
            return
            ;;
        perf1-focused)
            printf "%s\n" "${PERF1_FOCUSED_GROUPS[@]}"
            return
            ;;
        perf1-long-form)
            printf "%s\n" "${PERF1_LONG_FORM_GROUPS[@]}"
            return
            ;;
        pocket)
            printf "%s\n" "${POCKET_GROUPS[@]}"
            return
            ;;
        pocket-parallel)
            printf "%s\n" "${POCKET_PARALLEL_GROUPS[@]}"
            return
            ;;
        pocket-long)
            printf "%s\n" "${POCKET_LONG_GROUPS[@]}"
            return
            ;;
        pocket-gate-m)
            printf "%s\n" "${POCKET_GATE_M_GROUPS[@]}"
            return
            ;;
        pocket-serial)
            printf "%s\n" "${POCKET_SERIAL_GROUPS[@]}"
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

artifact_root() {
    printf "%s\n" "${DOMM_TEST_ARTIFACT_DIR:-target/test-artifacts}"
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

collect_log_matches() {
    local output_file="$1"
    local pattern="$2"
    local description="$3"
    local group log_file

    {
        printf "# %s\n\n" "$description"
        printf "Run ID: %s\n\n" "$RUN_ID"
    } >"$output_file"

    for group in "${selected[@]}"; do
        log_file="$LOG_DIR/$group.log"
        {
            printf "## %s\n\n" "$group"
            if [[ -f "$log_file" ]]; then
                grep -Ein "$pattern" "$log_file" | tail -200 || printf "No matching lines.\n"
            else
                printf "No log file was written for this group.\n"
            fi
            printf "\n"
        } >>"$output_file"
    done
}

write_failure_artifacts() {
    local artifact_dir selected_text group result_file status elapsed command log_file
    local -a failed_groups
    artifact_dir="$(artifact_root)/$RUN_ID"
    mkdir -p "$artifact_dir/logs"

    failed_groups=()
    for group in "${selected[@]}"; do
        result_file="$LOG_DIR/$group.result"
        if [[ -f "$result_file" ]]; then
            IFS=$'\t' read -r _ status elapsed command <"$result_file"
            cp "$result_file" "$artifact_dir/logs/$group.result"
            if [[ "$status" != "0" ]]; then
                failed_groups+=("$group")
            fi
        else
            status="missing"
            elapsed="unknown"
            command="${GROUP_COMMANDS[$group]}"
            failed_groups+=("$group")
        fi
        log_file="$LOG_DIR/$group.log"
        if [[ -f "$log_file" ]]; then
            cp "$log_file" "$artifact_dir/logs/$group.log"
        fi
    done

    selected_text="${selected[*]}"
    {
        printf '# DOMM Test Failure Artifact\n\n'
        printf -- '- Run ID: `%s`\n' "$RUN_ID"
        printf -- '- Source log dir: `%s`\n' "$LOG_DIR"
        printf -- '- Selected groups: `%s`\n' "$selected_text"
        printf -- '- Failed groups: `%s`\n' "${failed_groups[*]:-none}"
        printf -- '- Artifact dir: `%s`\n\n' "$artifact_dir"
        printf '## Minimal Replay\n\n'
        for group in "${failed_groups[@]}"; do
            printf '```bash\n'
            printf 'DOMM_TEST_JOBS=1 scripts/run-test-groups.sh %s\n' "$group"
            printf '```\n\n'
        done
        printf '## Files\n\n'
        printf -- '- `seed.txt`: seed and environment context\n'
        printf -- '- `step-log.txt`: grouped command/status table and failed log tails\n'
        printf -- '- `last-successful-view-snapshots.txt`: extracted view/snapshot lines\n'
        printf -- '- `command-event-ids.txt`: extracted command/event/nonce lines\n'
        printf -- '- `active-runtime-diagnostics.txt`: extracted runtime/diagnostic lines\n'
        printf -- '- `projection-snapshot.txt`: extracted projection/flush/dirty lines\n'
        printf -- '- `timer-job-snapshot.txt`: extracted timer/job/deadline lines\n'
        printf -- '- `logs/`: full per-group logs and result rows\n'
    } >"$artifact_dir/failure-summary.md"

    {
        printf "RUN_ID=%s\n" "$RUN_ID"
        printf "SELECTED_GROUPS=%s\n" "$selected_text"
        printf "FAILED_GROUPS=%s\n" "${failed_groups[*]:-none}"
        printf "DOMM_TEST_SEED=%s\n" "${DOMM_TEST_SEED:-unset}"
        printf "DOMM_SCENARIO_SEED=%s\n" "${DOMM_SCENARIO_SEED:-unset}"
        printf "DOMM_BENCH_SEED=%s\n" "${DOMM_BENCH_SEED:-unset}"
        printf "CANIC_POCKET_IC_LOCK_NAMESPACE_PREFIX=domm-%s-<group>\n" "$RUN_ID"
        env | LC_ALL=C sort | grep -E '^(DOMM|CANIC|CARGO|RUST|CI|GITHUB)_' || true
        printf "\n# Extracted Seed Lines\n\n"
        for group in "${selected[@]}"; do
            log_file="$LOG_DIR/$group.log"
            printf "## %s\n\n" "$group"
            if [[ -f "$log_file" ]]; then
                grep -Ein 'seed|rng|random|scenario' "$log_file" | tail -200 || printf "No matching lines.\n"
            else
                printf "No log file was written for this group.\n"
            fi
            printf "\n"
        done
    } >"$artifact_dir/seed.txt"

    {
        printf "# Step Log\n\n"
        printf "| Test group | Status | Time | Command |\n"
        printf "| --- | --- | ---: | --- |\n"
        for group in "${selected[@]}"; do
            result_file="$LOG_DIR/$group.result"
            if [[ -f "$result_file" ]]; then
                IFS=$'\t' read -r _ status elapsed command <"$result_file"
            else
                status="missing"
                elapsed="unknown"
                command="${GROUP_COMMANDS[$group]}"
            fi
            printf '| %s | %s | %s | `%s` |\n' "$group" "$status" "$elapsed" "$command"
        done
        printf "\n# Failed Log Tails\n\n"
        for group in "${failed_groups[@]}"; do
            log_file="$LOG_DIR/$group.log"
            printf "## %s\n\n" "$group"
            if [[ -f "$log_file" ]]; then
                tail -120 "$log_file"
            else
                printf "No log file was written for this group.\n"
            fi
            printf "\n"
        done
    } >"$artifact_dir/step-log.txt"

    collect_log_matches "$artifact_dir/last-successful-view-snapshots.txt" \
        'last successful|view snapshot|game_view|visible_objects|visible_map|champion_view|town_view|battle_state' \
        "Last Successful View Snapshot Lines"
    collect_log_matches "$artifact_dir/command-event-ids.txt" \
        'command[_ -]?id|event[_ -]?(id|key|seq)|client_nonce|nonce|receipt|status' \
        "Command, Event, Nonce, Receipt Lines"
    collect_log_matches "$artifact_dir/active-runtime-diagnostics.txt" \
        'active runtime|runtime diagnostic|runtime|diagnostic|dirty queue|queue_len|lag' \
        "Active Runtime Diagnostic Lines"
    collect_log_matches "$artifact_dir/projection-snapshot.txt" \
        'projection|dirty|flush|lag|checkpoint|queue_len' \
        "Projection Snapshot Lines"
    collect_log_matches "$artifact_dir/timer-job-snapshot.txt" \
        'timer|system_job|job|deadline|wakeup|repair|lease' \
        "Timer and Job Snapshot Lines"

    {
        printf "#!/usr/bin/env bash\n"
        printf "set -euo pipefail\n"
        printf "cd %q\n" "$workspace_root"
        for group in "${failed_groups[@]}"; do
            printf "DOMM_TEST_JOBS=1 scripts/run-test-groups.sh %q\n" "$group"
        done
    } >"$artifact_dir/replay.sh"
    chmod +x "$artifact_dir/replay.sh"

    printf "\nFailure artifacts: %s\n" "$artifact_dir" >&2
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
    if [[ -f "$LOG_DIR/$group.result" ]]; then
        IFS=$'\t' read -r result_group status elapsed command <"$LOG_DIR/$group.result"
    else
        result_group="$group"
        status="missing"
        elapsed="unknown"
        command="${GROUP_COMMANDS[$group]}"
    fi
    if [[ "$status" == "0" ]]; then
        label="pass"
    else
        label="fail($status)"
        overall=1
    fi
    printf "| %s | %s | %s | \`%s\` |\n" "$result_group" "$label" "$elapsed" "$command"
done

if ((overall != 0)); then
    write_failure_artifacts
    printf "\nFailed group log tails:\n" >&2
    for group in "${selected[@]}"; do
        if [[ -f "$LOG_DIR/$group.result" ]]; then
            IFS=$'\t' read -r _ status _ _ <"$LOG_DIR/$group.result"
        else
            status="missing"
        fi
        if [[ "$status" != "0" ]]; then
            printf "\n==> %s (%s)\n" "$group" "$LOG_DIR/$group.log" >&2
            if [[ -f "$LOG_DIR/$group.log" ]]; then
                tail -80 "$LOG_DIR/$group.log" >&2
            else
                printf "No log file was written for this group.\n" >&2
            fi
        fi
    done
    exit "$overall"
fi
