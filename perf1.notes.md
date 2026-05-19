# perf1 Notes

This is the running notes and decision log for `perf1.todo.md`.

Keep this file current while working:

- Record what changed and why.
- Record benchmark run IDs and key numbers.
- Record decisions, reversals, and measured blockers.
- Keep `perf1.todo.md` as the checklist; keep this file as the narrative.

## 2026-05-19

### Current Goal

Reduce `submit_battle_action` from the pinned combined baseline of `26.9860B` average instructions toward the normal target of around `0.3B`.

The plan is benchmark-driven and intentionally fluid. If an implementation checkpoint does not reduce instructions enough, the design should change. The goal is not to defend the current architecture; the goal is to remove measured cost until the endpoint is in the expected range.

### Current Baseline

Saved baseline from `target/benchmarks/20260518-231144-9f17dcb`:

| source | calls | avg instructions | p95 instructions | avg memory delta |
| --- | ---: | ---: | ---: | ---: |
| Gate K | 25 | 27.4632B | 28.7961B | 173.88 MB |
| Gate L | 26 | 26.5272B | 28.8134B | 156.41 MB |
| combined | 51 | 26.9860B | 28.8344B | 164.97 MB |

### Decisions So Far

1. The main smell is row-normalized live gameplay state, not just missing indexes.
2. `submit_battle_action` should move toward loading/mutating one active battle aggregate instead of hydrating/diffing many tactical child rows.
3. Use benchmark gates, not fixed architecture, to drive the work:
   - Gate 0: traced baseline
   - Gate 1: under 10B
   - Gate 2: under 3B
   - Gate 3: under 1B
   - Gate 4: around 0.3B
4. First implementation should add benchmark attribution before major rewrites, so repo operation counts and phase costs explain what actually moved.
5. Initial active battle runtime can reuse `domm_game::BattleState`; do not invent a new rules model until measurements require it.
6. Keep durable rows for command idempotency, event feed, system jobs, strategic aftermath, and history.
7. Avoid per-action stable writes for tactical child rows if the goal is to reach the 0.3B range.

### Subagent Findings

Battle aggregate migration:

- Hot row hydration/persistence is centralized around `battle_rows::load_battle_state*` and `battle_rows::persist_battle_state`.
- Main call sites are `submit_battle_action`, `sync_battle`, timeout/round jobs, recovery, cast ability, readiness, and aftermath.
- Minimal compatibility path can hydrate runtime from existing rows first, then switch hot battle command paths to runtime.
- `Battle` should remain an indexed shell/projection for lookup, jobs, and references.

Upgrade/runtime storage:

- The canister currently has `init` and `post_upgrade`, but no `pre_upgrade`.
- IcyDB uses stable memory IDs `20`, `21`, `22`, and commit memory `119`; any custom upgrade snapshot must avoid conflicting with those.
- `domm_game::BattleState` already supports Candid/serde, so Candid serialization is viable.
- For first pass, prefer heap active state plus upgrade serialization. Durable snapshot rows are optional and should not be added if they reintroduce per-action stable writes.

API/projection compatibility:

- `get_battle_state` should read active runtime directly.
- `get_game_view` should stay projection/shell-oriented and should not decode full battle runtime.
- `get_champion_view`, `get_town_view`, visible objects, history, events, and command status should remain row/projection-backed.
- Active combat stack damage does not need to project into champion/town rows until aftermath.

Benchmark tracing:

- Extend benchmark metrics with repo operation records and phase markers.
- Instrument `repos/foundation.rs` wrappers first.
- Convert direct `foundation::storage_result(...)` repo queries to typed helpers over time so row counts are exact.
- Add `submit_battle_action` phase markers around auth, command begin, recovery, timeout, load/apply/persist, event fanout, readiness/schedule, and final response.

### Next Work

1. Implement benchmark-only repo operation tracing and phase markers.
2. Run a traced baseline and record the run ID here and in `perf1.todo.md`.
3. Use the traced baseline to choose the first code cut toward Gate 1.

### Gate 0 Instrumentation Checkpoint

Implemented benchmark-only attribution for the current `submit_battle_action` path before changing battle behavior.

What changed:

- `DiagnosticBenchmarkCallView` now carries nested phase records and aggregated repo operation records.
- `submit_battle_action` now marks auth/context, battle load, command begin, recovery, timeout processing, input normalization, command-applying update, battle-state load, validation, rule application, tactical persistence, event fanout, readiness/schedule, session reload, and final response phases.
- Repository foundation helpers now trace create/insert/update/load/delete/page operations.
- Battle hot repository lookups, command/event lookups, battle-round readiness lookups, and system-job lookups were converted from eager `storage_result(...)` calls to traced `storage_operation(...)` calls.
- Benchmark artifacts now include phase and repo-operation summaries in `summary.json`, `run.json`, and `summary.md`.

Verified:

- `cargo check -p domm-degens-canister`
- `cargo test -p domm-pocket-ic-tests --test canister_endpoints benchmark_summary --no-run`
- `cargo test -p domm-pocket-ic-tests --test canister_endpoints benchmark_summary -- --nocapture`
- `cargo test -p domm-degens-canister exported_candid -- --nocapture`

### Plan Evaluation And Update

The first plan was directionally right but too conservative. It treated the active battle aggregate as the main performance fix, but a heap aggregate alone probably cannot reach `0.3B` if each battle action still performs durable command writes, event fanout writes, battle timeout job upserts, readiness row writes, and battle header updates.

Decision: treat the battle aggregate as Gate 1, not the finish line.

Updated gate logic:

| gate | intended architecture cut |
| --- | --- |
| Gate 1 | remove active tactical child-row hydrate/diff/persist |
| Gate 2 | remove per-action durable battle header/job/readiness writes |
| Gate 3 | move active battle command receipts and active battle events into runtime, with durable flush/merge behavior |
| Gate 4 | optimize battle rule CPU and runtime data structures toward `0.3B` |

Important implications:

- Do not add a per-action durable `BattleRuntimeSnapshot` unless measurement proves it is cheap enough. Upgrade serialization is acceptable; per-action stable snapshots are suspicious until proven otherwise.
- Do not spend too much time converting every repo to perfect tracing before Gate 1. Add targeted phase tracing and hot repo tracing first, then deepen instrumentation when a gate misses.
- `GameCommand` and `GameEvent` are allowed to stop being the active battle command/event authority while a battle is active. Public APIs must still work by reading or merging runtime data.
- `BattleParticipantRoundReady`, battle timeout `SystemJob`, and active `Battle` header fields are also candidates for runtime authority if they remain visible in traces.
- If compatibility rows force high cost, update tests and projections rather than keeping the cost.

The plan in `perf1.todo.md` was updated to reflect this. The target remains around `0.3B`; missing an intermediate gate should trigger another architectural cut, not a long polishing pass.

### Codebase Re-Evaluation

I re-read the updated plan against the current canister implementation. The plan is still directionally right, but the implementation needs to be more explicit than "heap `BattleState`".

Current `submit_battle_action` shape in `canisters/degens/src/services/battle.rs`:

1. `require_active_session_caller` loads player/session/participant rows.
2. `load_battle_row` loads the durable `Battle` shell.
3. `begin_participant_command_guarded` checks durable command idempotency and creates a `GameCommand`.
4. `recover_applying_battle_commands` pages applying durable commands and can replay row-backed battle mutations.
5. `apply_due_timeouts` may create/update system `GameCommand`s and reload/persist battle rows.
6. `apply_player_action` updates the command to applying, loads full row-backed `BattleState`, validates, applies the rule command, persists `Battle`/`BattleStack`/`BattleOccupancy`, and appends events.
7. The submit path reloads `Battle`, recomputes readiness by loading full state again, reads/writes `BattleParticipantRoundReady`, may upsert a round job, and schedules timeout jobs.
8. Final response updates the durable command to complete.

This confirms Gate 1: removing active tactical child-row hydration/persistence should be a large cut. It also confirms Gate 1 alone will not approach `0.3B`, because durable command writes, event fanout, readiness, jobs, and command-status compatibility remain.

Concrete code-driven adjustments:

- `BattleRuntime` must wrap `domm_game::BattleState` with canister metadata: session id, battle id, participant/audience keys, command receipts, runtime event buffer, readiness set, active deadline state, and a session event sequence cursor.
- `get_battle_state` can move cleanly to runtime first; it already builds from a battle row plus stack rows and does not need durable events.
- `sync_battle`, timeout jobs, and round jobs cannot remain row-backed if submit is runtime-backed, or they will reintroduce row hydration and stale-state risk.
- `end_battle_turn` currently writes `BattleParticipantRoundReady`; if runtime readiness becomes authoritative, this endpoint must move too.
- `get_events_after` currently reads only durable `GameEvent`s. Runtime events need a merge path and must preserve session-level event sequence ordering.
- `get_command_status` and `get_command_status_by_nonce` currently read durable command rows. Runtime command receipts must be checked first once battle actions stop writing durable commands per submit.
- `apply_resolved_battle_aftermath` reads `Battle` and survivor `BattleStack` rows. Runtime finalization must either project survivor rows before calling existing aftermath or rewrite aftermath to consume runtime survivors directly.
- `append_new_battle_events` loads champion/town owners while computing audiences and writes one event per audience plus public, updating `GameSession.next_event_seq` per event. Gate 3 should precompute audiences and batch or avoid per-event session updates.
- `recompute_battle_round_readiness_and_schedule` can be worse than a simple row write: it reloads full battle state and may call `legal_actions_for_stack`, which always computes reachable tiles. Gate 2 must replace this with runtime alive/acted tracking and avoid legal-action generation for readiness unless a measured edge case needs it.
- `validate_player_action` calls `legal_actions_for_stack` for all non-cast actions, and that always runs reachability. Action-specific validation is a legitimate early CPU cut, especially for attack/defend/wait.
- `normalize_battle_action_input` reloads full state for `auto:enemy`; that should resolve from runtime in Gate 1.

Updated conclusion: build the first runtime path around the submit/sync/readiness/event/status/aftermath boundary, not only around `battle_rows`. Add phase tracing around the existing blocks first, then cut the row-backed tactical path. If Gate 1 does not clear `<10B`, skip cosmetic repo tracing and remove the durable command/event/job/readiness costs next.
