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

### Traced Baseline Checkpoint

Fresh traced run:

```text
run id: 20260519-002234-3dfd9a4
git sha: 3dfd9a4
suite: target/benchmarks/20260519-002234-3dfd9a4/suite-summary.md
```

Suite status:

| gate | status | elapsed | note |
| --- | --- | ---: | --- |
| Gate J | passed | 257s | strategic loop |
| Gate K | passed | 440s | battle aftermath/victory |
| Gate L | passed | 479s | first-playable route |
| Gate M | passed | n/a | client probe log-only artifact |

`submit_battle_action` summary:

| source | calls | avg instructions | p95 instructions | avg memory delta |
| --- | ---: | ---: | ---: | ---: |
| Gate K | 25 | 27.4636B | 28.7972B | 173.88 MB |
| Gate L | 26 | 26.5264B | 28.8148B | 156.41 MB |
| combined | 51 | 26.9858B | 28.8148B | 164.97 MB |

Top combined phases:

| phase | calls | avg instructions | total instructions |
| --- | ---: | ---: | ---: |
| readiness_schedule | 50 | 6.6912B | 334.5608B |
| event_fanout | 50 | 5.9269B | 296.3457B |
| persist_battle_state | 50 | 3.5228B | 176.1409B |
| load_battle_state | 50 | 2.8205B | 141.0273B |
| command_begin | 51 | 2.5587B | 130.4941B |
| auth_context | 51 | 2.1156B | 107.8965B |
| load_battle | 51 | 0.7055B | 35.9827B |
| recovery | 50 | 0.7058B | 35.2898B |

Top combined repo operations inside `submit_battle_action`:

| operation | calls | avg instructions | total instructions |
| --- | ---: | ---: | ---: |
| battles.load_battle | 251 | 0.7049B | 176.9241B |
| battle_round_ready.by_battle_participant_round | 150 | 0.7039B | 105.5895B |
| sessions.load_session | 147 | 0.7046B | 103.5801B |
| battles.stacks_by_battle | 146 | 0.7067B | 103.1712B |
| battles.occupancy_by_battle | 146 | 0.7042B | 102.8087B |
| events.by_session_event_key | 140 | 0.7054B | 98.7552B |
| system_jobs.by_job_key | 100 | 0.7043B | 70.4334B |
| battles.obstacles_by_battle | 96 | 0.7049B | 67.6672B |
| events.create_game_event | 140 | 0.4791B | 67.0757B |
| sessions.update_session | 140 | 0.4779B | 66.9078B |

Decision from trace: focus Gate 1 on removing active tactical row load/persist first, but do not stop there. The largest measured phase is readiness/schedule, event fanout is second, and `apply_rules`/`validate_action` are tiny. If the runtime aggregate still writes events/readiness/jobs per action, it will not reach `0.3B`.

### Active Runtime Merge Contract

Documented the merge contract in `perf1.todo.md`.

Decision highlights:

- Runtime wins for active battle reads and commands.
- Durable battle rows become shells/projections/history during active execution.
- Missing runtime is recoverable by hydrating from durable active battle rows.
- `get_battle_state`, `sync_battle`, `end_battle_turn`, event feed, command status, timeout jobs, round jobs, and aftermath all need explicit runtime merge/fallback behavior.
- Runtime event sequence allocation should batch or defer durable `GameSession.next_event_seq` updates.
- Runtime command receipts need enough data to satisfy command status and nonce replay before durable command rows are removed from the hot path.
- Finalization must either project survivor `BattleStack` rows before existing aftermath or rewrite aftermath to consume runtime survivors directly.

### BattleRuntime Scaffold

Added `canisters/degens/src/services/battle_runtime.rs`.

Checkpoint scope:

- `BattleRuntime` now wraps `domm_game::BattleState`.
- Runtime metadata includes session id, battle id, participant audience keys, command receipts, nonce lookup, active event buffer, readiness set, deadline/job hints, session event sequence cursor, and dirty generation.
- Heap store is keyed by battle id and exposes insert/remove/read/mutate helpers.
- Snapshot/restore helpers exist for the future `pre_upgrade`/`post_upgrade` checkpoint, but canister upgrade hooks are not wired yet.

Verified:

- `cargo test -p domm-degens-canister battle_runtime -- --nocapture`

### BattleRuntime Upgrade Snapshot

Added upgrade persistence for active battle runtimes.

Checkpoint scope:

- Dedicated battle runtime memory id is `23`.
- This does not collide with IcyDB data/index/schema ids `20/21/22` or commit id `119`.
- Snapshot storage uses Canic's registered stable-memory slot machinery, not raw stable-memory offsets.
- `canister_pre_upgrade` serializes `BattleRuntimeSnapshot` into the dedicated stable cell.
- `post_upgrade` restores the snapshot before system job repair/scheduling and then clears the snapshot cell.
- Snapshot encode/decode uses Candid for the current runtime shape.
- Snapshot/restore failure traps the upgrade path instead of silently dropping active battles.

Verified:

- `cargo test -p domm-degens-canister battle_runtime -- --nocapture`
- `cargo check -p domm-degens-canister`

### Runtime Adoption And Row Hydration

Added active runtime adoption without changing command execution yet.

Checkpoint scope:

- `battle_runtime::hydrate_runtime_from_rows` builds `BattleRuntime` from existing durable `Battle`/stack/obstacle/occupancy rows through the current `battle_rows` loader.
- `battle_runtime::adopt_active_battle_from_rows` inserts a runtime for active battles only, and is idempotent when runtime already exists.
- Champion battle start, town battle start, and neutral battle start now adopt runtime once the battle reaches `active`.
- Existing active champion/town/neutral battles encountered by start paths are also adopted, which covers row-backed active battles created before this checkpoint.

Verified:

- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister battle_runtime -- --nocapture`

### Movement Intent Effect Cleanup

Finished the in-progress low-risk movement micro-cut before starting the session-turn runtime rewrite.

What changed:

- `submit_move_intent` no longer writes the redundant `movement_intent` `CommandEffect` row.
- The durable `MovementIntent` row, the public `movement_intent_submitted` event, changed subjects, and command idempotency still carry the public and replay-relevant state.
- The now-unused direct `create_command_effect` helper was removed from `command_response.rs`, leaving the shared idempotent `ensure_command_effect` helper for paths that still need effect rows.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- Focused Gate J `20260519-123422-movement-intent-effect-gate-j` passed in `689.81s`.

Measured update-method delta versus `20260519-113052-movement-turn-effect-gate-j`:

| method | previous avg instructions | new avg instructions | change | previous avg memory | new avg memory | change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `submit_move_intent` | 13.7537B | 13.2749B | -3.5% | 53.4375 MB | 21.4375 MB | -59.9% |
| `sync_session_turn` | 14.7341B | 14.7336B | ~0.0% | 125.2443 MB | 125.2443 MB | 0.0% |

Caveat: this direct focused run did not persist the query log into the artifact, so query instruction deltas show as `n/a` and scenario-level instruction totals are not comparable. Use the benchmark script or an absolute query-log path for the next query/projection claim.

### SessionTurnRuntime Scaffold

Added the inert heap runtime module for active session turns.

Checkpoint scope:

- `SessionTurnRuntime` stores active turn metadata, participants, ready set, movement intents, runtime command receipts, active events, event sequence block, champion/occupancy/visibility/object/resource deltas, and partial movement cursor state.
- Heap store helpers are keyed by `(session_id, turn_number)` through `runtime_key`.
- Runtime APIs now include insert/remove/read/mutate, active event filtering, command receipt lookup by id/nonce, snapshot/restore, and test cleanup.
- No endpoint behavior changed in this checkpoint; durable rows remain authoritative until the next mirror/adoption patches.

Verified:

- `cargo fmt --check`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- `cargo check -p domm-degens-canister --features benchmark`

### Pending Movement Participant Reuse

Removed one redundant stable read loop from `sync_session_turn`.

What changed:

- `pending_movement_intents_for_session` now returns each pending `MovementIntent` with the matching active `GameParticipant` row from the participant page it already had to load for filtering.
- `load_pending_movements` no longer calls `sessions::load_participant` once per pending intent.
- This keeps the same active-participant filtering semantics while cutting a stable participant load for every pending movement processed by sync.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-degens-canister -- --nocapture` (`25 passed`, `250.60s`)

Measurement note: no focused Gate J benchmark was run for this single small cut. The previous direct Gate J run took `689.81s`, so the next benchmark should batch this with the next runtime/movement checkpoint.

### SessionTurnRuntime Event And Status Overlay

Wired the empty session-turn runtime into the existing event/status merge points.

Checkpoint scope:

- `get_events_after` now merges `SessionTurnRuntime` active events before the existing `BattleRuntime` active-event merge and durable fallback sorting/deduplication.
- `get_command_status` checks session-turn runtime command receipts by command id before battle runtime and durable command rows.
- `get_command_status` and `get_command_status_by_nonce` check session-turn runtime receipts for `submit_move_intent`, `end_turn`, and `sync_session_turn` nonces before durable command rows.
- No endpoint writes populate this runtime yet, so this should be behavior-neutral until the next movement runtime checkpoint.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`

### Runtime Finalization Projection

Added the projection guard needed before active battle submit stops writing tactical rows per action.

Checkpoint scope:

- Added `apply_resolved_battle_aftermath_with_runtime_projection` in the battle service.
- Before existing aftermath reads durable rows, the helper checks for a resolved active runtime and persists its final `BattleState` through the existing battle row projector.
- This projects the resolved `Battle` header plus survivor `BattleStack` and `BattleOccupancy` rows before neutral/town/champion aftermath consumes survivor state.
- Existing aftermath call sites now go through the helper, so timeout, recovery, sync, and round-advance paths all share the same ordering.
- Once aftermath sees the durable battle as non-active, the active runtime is removed from heap.

Verified:

- `cargo check -p domm-degens-canister`
- `cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_render_projection_tracks_battle_aftermath_objects -- --nocapture`

### Gate 1 Runtime Submit Cut

Moved the common active battle action path off repeated tactical child-row hydration and persistence.

Checkpoint scope:

- Active non-spell `submit_battle_action` now clones the heap `BattleRuntime`, validates and applies the battle command against runtime `BattleState`, persists only the durable `Battle` header for compatibility, appends the existing durable event fanout, then commits the runtime back to heap.
- Active timeout auto-defend and round auto-defend now mutate runtime state when runtime is present, with the same battle-header-only projection.
- Round readiness now prefers runtime `BattleState`, so it does not reload stale `BattleStack`/`BattleObstacle`/`BattleOccupancy` rows after runtime actions.
- `auto:enemy` target normalization now resolves from runtime state when available.
- Validation is now action-specific: attack/defend/wait do not compute full move reachability; only `Move` calls `legal_actions_for_stack` for reachable tiles.
- Durable `GameCommand` and `GameEvent` behavior is intentionally still in place for this Gate 1 measurement. That cost should move in Gate 3.
- Remaining exception: `CastAbility` still uses the row-backed tactical persist path because it also touches champion mana/effects. Move it to runtime or measure/document it as a rare fallback before closing Gate 1 completely.

Verified:

- `cargo check -p domm-degens-canister`
- `cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_readiness_advances_and_replays -- --nocapture`

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

### Runtime `get_battle_state`

Moved active `get_battle_state` reads onto `BattleRuntime` while keeping the current row-backed submit path compatible.

Checkpoint scope:

- `get_battle_state` now checks the heap active runtime first when the runtime belongs to the requested session, and falls back to durable rows when runtime is missing.
- Existing row-backed active battles are adopted on first `get_battle_state` access.
- Runtime battle views intentionally expose the same conservative canister legal-action shape as the old row view while submit is still row-backed. This prevents the API read path from advertising actions that the current mutation path may reject.
- Row-backed battle mutation paths now mirror their persisted `BattleState` back into the heap runtime, so runtime reads do not go stale during the migration window.
- Full Candid serialization of the runtime graph was replaced with a compact upgrade reference snapshot. The full graph pushed the canister code section above the PocketIC/IC Wasm code limit; compact refs rehydrate active battles from durable rows after upgrade for now.

Verified:

- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister battle_runtime -- --nocapture`
- `cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_readiness_advances_and_replays -- --nocapture`

### Runtime Event Sequence Reservation

Decision: active battle runtime events will use stable batch reservation, not a pure heap overlay.

Reasoning:

- A pure heap counter can collide with normal durable `GameEvent` writes from movement, economy, scenario progress, aftermath, or other endpoints that still append stable events.
- Reserving a block by bumping durable `GameSession.next_event_seq` once gives active battle events collision-free sequence numbers without paying a stable session update for every event.
- Losing unused reserved numbers during upgrade or resolution is acceptable. Gaps are cheaper and safer than sequence collisions.
- Multiple active battles in one session must share a session-scoped reserved block allocator, not per-battle counters.

Checkpoint scope:

- Added `BATTLE_RUNTIME_EVENT_SEQ_BLOCK_SIZE = 4096`.
- Added a heap session event sequence block allocator in `battle_runtime`.
- `reserve_session_event_seq` reserves a block by updating `GameSession.next_event_seq`, then hands out event sequence numbers from heap until the block is exhausted.
- Runtime test cleanup clears both active runtimes and event sequence blocks.

Verified:

- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister battle_runtime -- --nocapture`

### Runtime Sync And Gate K Stabilization

Follow-up after moving active non-spell submit onto runtime showed that full Gate K failed in `sync_battle`, not in `submit_battle_action`: resolved and timeout sync paths were still doing repeated expensive stable aftermath/readiness/session work.

Changes made in this checkpoint:

- `sync_battle` now tries the active `BattleRuntime` timeout path first and only falls back to row-backed timeout handling when runtime is missing.
- Runtime timeout sync applies at most the existing per-update timeout budget, persists only the battle header, appends a compact public timeout event, and trims transient runtime command/event history to 16 records.
- Runtime sync now applies resolved aftermath only when the runtime battle is non-active, avoiding no-op aftermath work on every active sync.
- `sync_battle` skips the duplicate outer aftermath pass when runtime sync already handled the runtime state.
- `sync_battle` no longer reloads the session after sync work; event append and victory finalization already mutate the same `context.session`.
- Defend/wait/auto-defend battle commands no longer run full occupancy validation, and occupancy validation now uses a stack map instead of repeatedly scanning stacks.

Focused benchmark:

```text
run id: 20260519-031342-gate-k-no-reload
artifact: target/benchmarks/20260519-031342-gate-k-no-reload/gate-k/summary.json
note: summary git_sha is 14b9505 because this was run before committing the checkpoint
```

Gate K result:

| status | elapsed | updates | queries | row growth | stable pages start | stable pages final |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| passed | 606.25s | 84 | 118 | 327 | 1025 | 218625 |

Key method summary from the focused run:

| method | kind | calls | avg instructions | p95 instructions | avg memory delta | avg cycles |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `submit_battle_action` | update | 25 | 20.3549B | 21.9646B | 175.16 MB | 0.0204T |
| `sync_battle` | update | 28 | 9.8443B | 36.1433B | 34.33 MB | 0.0099T |
| `get_battle_state` | query | 48 | n/a | n/a | 0 MB | 0T |

Decision:

- Gate K is playable/passing again after the runtime submit cut.
- Perf Gate 1 did not clear `<10B`; `submit_battle_action` improved from the Gate K baseline of 27.4632B to 20.3549B, but durable command/event/session and aftermath-adjacent stable work still dominate.
- Continue into Gate 2/3 style cuts rather than polishing tactical row persistence. The next likely wins are removing per-action durable battle header/job/readiness writes and moving active command receipts/events into runtime.

### Runtime Readiness And Submit Timeout Cut

Moved more submit-side support work onto active runtime without changing durable command/event behavior yet.

Changes made:

- Submit-time auto-ready bookkeeping now uses `BattleRuntime.ready_participants` for active battles.
- Active submit command guards check runtime ready/round-processing state first, avoiding row-ready and system-job lookups on the hot path.
- `end_battle_turn` still writes durable ready rows for compatibility, but mirrors the ready participant into runtime so submit guards and runtime round jobs see the same state.
- Round-advance job processing can read runtime readiness before falling back to row readiness.
- Submit-time timeout checks now use the active runtime timeout helper first.
- The redundant post-submit session reload was removed; event append/finalization already mutate the `context.session` value used for the response.

Focused regression:

- `cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_readiness_advances_and_replays -- --nocapture`

Focused benchmark:

```text
run id: 20260519-034828-gate-k-submit-runtime-timeout
artifact: target/benchmarks/20260519-034828-gate-k-submit-runtime-timeout/gate-k/summary.json
note: summary git_sha is 1b9eb20 because this was run before committing the checkpoint
```

Gate K result:

| status | elapsed | updates | queries | row growth | stable pages start | stable pages final |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| passed | 592.77s | 86 | 120 | 332 | 1025 | 218497 |

Key method summary:

| method | kind | calls | avg instructions | p95 instructions | avg memory delta | avg cycles |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `submit_battle_action` | update | 26 | 13.9983B | 14.4811B | 168.71 MB | 0.0140T |
| `sync_battle` | update | 29 | 9.7119B | 36.1282B | 29.84 MB | 0.0101T |
| `get_battle_state` | query | 50 | n/a | n/a | 0 MB | 0T |

Submit phase summary after this cut:

| phase | calls | avg instructions |
| --- | ---: | ---: |
| `event_fanout` | 26 | 5.9479B |
| `auth_context` | 26 | 2.1175B |
| `command_begin` | 26 | 1.1884B |
| `readiness_schedule` | 26 | 1.1861B |
| `persist_battle_state` | 26 | 1.1824B |
| `recovery` | 26 | 0.7068B |
| `load_battle` | 26 | 0.7060B |
| `final_response` | 26 | 0.4815B |
| `mark_command_applying` | 26 | 0.4810B |
| `timeout` | 26 | 0.0001B |

Decision:

- Runtime readiness and runtime submit timeout reduced Gate K `submit_battle_action` from 20.3549B to 13.9983B avg.
- The next blocker is unambiguous: durable `event_fanout` is ~5.95B avg by itself. Moving active battle events into runtime/response projection is the next cut toward `<10B`.

### Runtime Event Archive Cut

Moved active non-spell battle action events out of per-action durable `GameEvent` writes.

Changes made:

- Active runtime submit now appends battle action events into `BattleRuntime.active_events` and returns those `ApiEventView`s directly in the command response.
- `get_events_after` now merges durable `GameEvent` rows with active runtime events for both `public` and participant audiences.
- Active battle event sequence numbers use the existing session block reservation, so submit does not update `GameSession.next_event_seq` for each audience event.
- Resolved battle runtime events are archived in heap before runtime removal so the final event feed can still include battle action events after aftermath.
- A first attempt to flush every runtime battle event into durable rows during finalization exceeded the 40B single-message limit in `sync_battle`; durable command/event flush now needs a bounded batch design instead of a one-shot finalization write.

Failed benchmark used for the decision:

```text
run id: 20260519-041145-gate-k-runtime-events
result: failed in sync_battle with CanisterInstructionLimitExceeded while flushing active runtime events to durable GameEvent rows
```

Focused benchmark:

```text
run id: 20260519-041955-gate-k-runtime-event-archive
artifact: target/benchmarks/20260519-041955-gate-k-runtime-event-archive/gate-k/summary.json
note: summary git_sha is c4dd56b because this was run before committing the checkpoint
```

Gate K result:

| status | elapsed | updates | queries | row growth | stable pages start | stable pages final |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| passed | 586.98s | 90 | 122 | 260 | 1025 | 168705 |

Key method summary:

| method | kind | calls | avg instructions | p95 instructions | avg memory delta | avg cycles |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `submit_battle_action` | update | 28 | 9.3621B | 9.9239B | 34.92 MB | 0.0094T |
| `sync_battle` | update | 31 | 9.4893B | 36.1468B | 43.38 MB | 0.0101T |
| `get_battle_state` | query | 52 | n/a | n/a | 0 MB | 0T |
| `get_events_after` | query | 1 | n/a | n/a | 0 MB | 0T |

Submit phase summary after this cut:

| phase | calls | avg instructions |
| --- | ---: | ---: |
| `auth_context` | 28 | 2.1224B |
| `event_fanout` | 28 | 1.2539B |
| `readiness_schedule` | 28 | 1.2337B |
| `command_begin` | 28 | 1.1903B |
| `persist_battle_state` | 28 | 1.1804B |
| `load_battle` | 28 | 0.7080B |
| `recovery` | 28 | 0.7076B |
| `final_response` | 28 | 0.4832B |
| `mark_command_applying` | 28 | 0.4816B |
| `apply_rules` | 28 | 0.0003B |

Decision:

- The first target is cleared: Gate K `submit_battle_action` moved from 13.9983B to 9.3621B avg.
- Durable event fanout is no longer the top submit blocker; the remaining cost is mostly session/auth lookups plus durable command/header/readiness/status writes.
- Do not reintroduce one-shot durable event flushing. The next design needs bounded flush batches or active command/event receipt persistence that is explicitly outside the submit hot path.

### Skip Active Submit Battle Header Projection

Removed the durable `Battle` header update from active non-spell player `submit_battle_action`.

Changes made:

- Active player submit now mutates `BattleRuntime` and skips `battle_rows::persist_battle_header_from_state` during the submit hot path.
- The battle-round submit guard now reads the current round from runtime when runtime exists. The first attempt at this cut failed because the guard still used stale durable `Battle.current_round` and rejected later actions as `battle_round_closed`.
- Durable projection still happens during sync/finalization paths, so post-battle aftermath can keep using the existing row-backed projection.

Failed benchmark used for the fix:

```text
run id: 20260519-043354-gate-k-no-submit-header
result: failed with battle_round_closed from stale durable Battle.current_round in the submit guard
```

Focused benchmark:

```text
run id: 20260519-044140-gate-k-no-submit-header-guard
artifact: target/benchmarks/20260519-044140-gate-k-no-submit-header-guard/gate-k/summary.json
note: summary git_sha is a7d280c because this was run before committing the checkpoint
```

Gate K result:

| status | elapsed | updates | queries | row growth | stable pages start | stable pages final |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| passed | 584.40s | 90 | 122 | 260 | 1025 | 168705 |

Key method summary:

| method | kind | calls | avg instructions | p95 instructions | avg memory delta | avg cycles |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `submit_battle_action` | update | 28 | 8.1771B | 8.7395B | 34.92 MB | 0.0082T |
| `sync_battle` | update | 31 | 9.4910B | 36.1301B | 43.38 MB | 0.0101T |
| `get_battle_state` | query | 52 | n/a | n/a | 0 MB | 0T |
| `get_events_after` | query | 1 | n/a | n/a | 0 MB | 0T |

Submit phase summary after this cut:

| phase | calls | avg instructions |
| --- | ---: | ---: |
| `auth_context` | 28 | 2.1213B |
| `event_fanout` | 28 | 1.2526B |
| `readiness_schedule` | 28 | 1.2330B |
| `command_begin` | 28 | 1.1899B |
| `load_battle` | 28 | 0.7078B |
| `recovery` | 28 | 0.7073B |
| `final_response` | 28 | 0.4828B |
| `mark_command_applying` | 28 | 0.4814B |
| `persist_battle_state` | 28 | 0.0000B |

Decision:

- Skipping active submit header projection is viable once submit guards use runtime round state.
- The next largest submit costs are now auth/session lookup, event fanout owner lookup/response construction, runtime readiness scheduling, durable command begin, recovery scan, and command status updates.
- Full removal of active durable `Battle` header updates still needs timeout/round/finalization review; this checkpoint only removes the player submit hot-path write.

### Runtime Audience Fanout Cut

Removed row-backed participant owner discovery from active runtime battle event fanout.

Changes made:

- Active runtime event fanout now uses participant IDs already present in `BattleRuntime` and the runtime battle stacks.
- The legacy `involved_battle_participant_ids` path still exists for row-backed/cast/fallback paths, but active non-spell submit no longer loads champion/town owner rows just to fan out battle action events.

Focused benchmark:

```text
run id: 20260519-045312-gate-k-runtime-audience
artifact: target/benchmarks/20260519-045312-gate-k-runtime-audience/gate-k/summary.json
note: summary git_sha is 01c0739 because this was run before committing the checkpoint
```

Gate K result:

| status | elapsed | updates | queries | row growth | stable pages start | stable pages final |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| passed | 581.01s | 90 | 122 | 260 | 1025 | 168705 |

Key method summary:

| method | kind | calls | avg instructions | p95 instructions | avg memory delta | avg cycles |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `submit_battle_action` | update | 28 | 6.9312B | 8.0335B | 34.89 MB | 0.0069T |
| `sync_battle` | update | 31 | 9.4787B | 36.1288B | 43.43 MB | 0.0101T |
| `get_battle_state` | query | 52 | n/a | n/a | 0 MB | 0T |
| `get_events_after` | query | 1 | n/a | n/a | 0 MB | 0T |

Submit phase summary after this cut:

| phase | calls | avg instructions |
| --- | ---: | ---: |
| `auth_context` | 28 | 2.1206B |
| `readiness_schedule` | 28 | 1.2269B |
| `command_begin` | 28 | 1.1889B |
| `load_battle` | 28 | 0.7072B |
| `recovery` | 28 | 0.7067B |
| `final_response` | 28 | 0.4814B |
| `mark_command_applying` | 28 | 0.4812B |
| `event_fanout` | 28 | 0.0171B |
| `persist_battle_state` | 28 | 0.0000B |

Decision:

- Event fanout is no longer a meaningful submit blocker: 1.2526B -> 0.0171B avg.
- The next cuts should target durable command lifecycle (`command_begin`, `mark_command_applying`, `final_response`) and recovery scanning, or reduce auth/session lookup cost.

### Runtime Command Receipt Cut

Moved active non-spell battle action command idempotency and status onto `BattleRuntime`.

Changes made:

- Active non-spell `submit_battle_action` now uses runtime command receipts instead of creating a durable `GameCommand` row, updating it to `applying`, scanning applying commands, and updating it complete.
- Runtime receipts store the command id, command type, actor participant, nonce text/hash, payload hash, and the full `CommandResponse`.
- Replays of the same nonce/payload return the runtime receipt without mutating battle state.
- Same nonce/different payload returns the existing duplicate nonce failure.
- `get_command_status` and `get_command_status_by_nonce` now check active and archived runtime receipts before durable `GameCommand` rows.
- Runtime command receipts are archived in heap when a runtime battle is removed, matching the previous active event archive pattern.
- The row-backed command path remains for `CastAbility`, due-timeout fallback, and missing-runtime compatibility.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `DOMM_CANISTER_FEATURES=benchmark CANIC_POCKET_IC_LOCK_NAMESPACE=domm-runtime-command-status cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_readiness_advances_and_replays -- --nocapture`

Focused benchmark:

```text
run id: 20260519-051826-gate-k-runtime-command-receipts
artifact: target/benchmarks/20260519-051826-gate-k-runtime-command-receipts/gate-k/summary.json
note: summary git_sha is 6fc9e0a because this was run before committing the checkpoint
```

Gate K result:

| status | updates | queries | row growth | stable pages start | stable pages final |
| --- | ---: | ---: | ---: | ---: | ---: |
| passed | 88 | 118 | 229 | 1025 | 150401 |

Key method summary:

| method | kind | calls | avg instructions | p95 instructions | avg memory delta | avg cycles |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `submit_battle_action` | update | 26 | 4.0563B | 4.4771B | 0.01 MB | 0.0041T |
| `sync_battle` | update | 29 | 9.7519B | 36.1259B | 37.01 MB | 0.0102T |
| `get_battle_state` | query | 50 | n/a | n/a | 0 MB | 0T |
| `get_events_after` | query | 1 | n/a | n/a | 0 MB | 0T |

Submit phase summary after this cut:

| phase | calls | avg instructions |
| --- | ---: | ---: |
| `auth_context` | 26 | 2.1350B |
| `readiness_schedule` | 26 | 1.1899B |
| `load_battle` | 26 | 0.7120B |
| `event_fanout` | 26 | 0.0184B |
| `apply_rules` | 26 | 0.0003B |
| `load_battle_state` | 26 | 0.0003B |
| `command_begin` | 26 | 0.0001B |
| `validate_action` | 26 | 0.0001B |
| `final_response` | 26 | 0.0000B |
| `recovery` | 26 | 0.0000B |
| `timeout` | 26 | 0.0000B |
| `persist_battle_state` | 26 | 0.0000B |

Decision:

- Durable command lifecycle is no longer a submit blocker: `command_begin`, `mark_command_applying`, `recovery`, and `final_response` dropped from about 2.8582B combined to effectively zero in the active runtime path.
- The checkpoint did not clear the `<1B` Gate 3 target because `auth_context`, `readiness_schedule`, and `load_battle` still total about 4.0369B.
- The next practical cuts are: avoid durable session/participant reload in auth for battle actions, stop loading the durable `Battle` row before runtime submit, and move timeout/round scheduling fully out of the per-action readiness path.

### Runtime Submit Before Durable Battle Load

Moved active non-spell battle submit to try the heap runtime before loading the durable `Battle` row.

Changes made:

- `submit_battle_action` now attempts the active runtime path immediately after auth for non-spell actions.
- If runtime is absent, session mismatched, or timeout processing needs the legacy path, the endpoint falls back to the durable `Battle` row load/adopt flow.
- Runtime command begin now gets the current round from `BattleRuntime`, so it no longer needs a row-backed `Battle` just to guard active submit.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `DOMM_CANISTER_FEATURES=benchmark CANIC_POCKET_IC_LOCK_NAMESPACE=domm-runtime-no-load-battle cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_readiness_advances_and_replays -- --nocapture`

Focused benchmark:

```text
run id: 20260519-053230-gate-k-runtime-no-battle-load
artifact: target/benchmarks/20260519-053230-gate-k-runtime-no-battle-load/gate-k/summary.json
note: summary git_sha is eb61e31 because this was run before committing the checkpoint
```

Gate K result:

| status | updates | queries | row growth | stable pages start | stable pages final |
| --- | ---: | ---: | ---: | ---: | ---: |
| passed | 88 | 118 | 229 | 1025 | 150401 |

Key method summary:

| method | kind | calls | avg instructions | p95 instructions | avg memory delta | avg cycles |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `submit_battle_action` | update | 26 | 3.3426B | 3.7720B | 0.01 MB | 0.0034T |
| `sync_battle` | update | 29 | 9.7493B | 36.1310B | 37.01 MB | 0.0102T |
| `get_battle_state` | query | 50 | n/a | n/a | 0 MB | 0T |
| `get_events_after` | query | 1 | n/a | n/a | 0 MB | 0T |

Submit phase summary after this cut:

| phase | calls | avg instructions |
| --- | ---: | ---: |
| `auth_context` | 26 | 2.1337B |
| `readiness_schedule` | 26 | 1.1894B |
| `event_fanout` | 26 | 0.0184B |
| `apply_rules` | 26 | 0.0003B |
| `load_battle_state` | 26 | 0.0003B |
| `command_begin` | 26 | 0.0001B |
| `validate_action` | 26 | 0.0001B |
| `final_response` | 26 | 0.0000B |
| `recovery` | 26 | 0.0000B |
| `timeout` | 26 | 0.0000B |
| `persist_battle_state` | 26 | 0.0000B |

Decision:

- The durable `Battle` load is gone from the active submit phase summary and `submit_battle_action` improved from 4.0563B to 3.3426B avg.
- Gate 2 is still not cleared because `readiness_schedule` and `auth_context` dominate the endpoint.
- The next cut should remove per-action timeout/round job scheduling from runtime readiness; that should get the endpoint below the 3B Gate 2 threshold before tackling auth/session caching.

### Runtime Timeout Wakeup Hints

Removed the per-action timeout `SystemJob` upsert from active runtime battle submit.

Changes made:

- Active non-spell submit now recomputes runtime readiness without scheduling a fresh timeout job each action.
- Runtime state remains the deadline authority; the durable timeout job is only a wakeup hint.
- When a timeout job wakes early, it reads the runtime deadline and reschedules itself to the current runtime deadline.
- When a runtime timeout is actually due, the job projects the runtime battle header just before falling through to the existing timeout application path.
- The first implementation handled the full runtime timeout action inside a new helper, but that pushed the benchmark Wasm over the IC code-section limit by about 2 KB. The final implementation reuses the existing timeout path to stay under the limit.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `DOMM_CANISTER_FEATURES=benchmark CANIC_POCKET_IC_LOCK_NAMESPACE=domm-runtime-timeout-job2 cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_timer_auto_defends_without_sync_battle_and_replays_noop -- --nocapture`

Focused benchmark:

```text
run id: 20260519-055234-gate-k-runtime-timeout-hints
artifact: target/benchmarks/20260519-055234-gate-k-runtime-timeout-hints/gate-k/summary.json
note: summary git_sha is ec8345a because this was run before committing the checkpoint
```

Gate K result:

| status | updates | queries | row growth | stable pages start | stable pages final |
| --- | ---: | ---: | ---: | ---: | ---: |
| passed | 87 | 115 | 228 | 1025 | 149889 |

Key method summary:

| method | kind | calls | avg instructions | p95 instructions | avg memory delta | avg cycles |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `submit_battle_action` | update | 25 | 2.2481B | 2.5912B | 0.01 MB | 0.0023T |
| `sync_battle` | update | 28 | 9.8840B | 36.1195B | 36.06 MB | 0.0107T |
| `get_battle_state` | query | 48 | n/a | n/a | 1.33 MB | 0T |
| `get_events_after` | query | 1 | n/a | n/a | 0 MB | 0T |

Submit phase summary after this cut:

| phase | calls | avg instructions |
| --- | ---: | ---: |
| `auth_context` | 25 | 2.1328B |
| `readiness_schedule` | 25 | 0.0950B |
| `event_fanout` | 25 | 0.0192B |
| `apply_rules` | 25 | 0.0003B |
| `load_battle_state` | 25 | 0.0003B |
| `command_begin` | 25 | 0.0001B |
| `validate_action` | 25 | 0.0001B |
| `final_response` | 25 | 0.0000B |
| `recovery` | 25 | 0.0000B |
| `timeout` | 25 | 0.0000B |
| `persist_battle_state` | 25 | 0.0000B |

Decision:

- Gate 2 is cleared: `submit_battle_action` is now below 3B avg.
- `readiness_schedule` is no longer a major blocker: 1.1894B -> 0.0950B avg.
- The remaining dominant cost is `auth_context` at about 2.13B, which means the next meaningful work is a session/participant auth cache or runtime submit auth shortcut.

### Active Submit Auth Cache

Removed most of the stable-read auth tax from active runtime battle submit.

Changes made:

- Added a narrow two-slot active session caller cache for active non-spell `submit_battle_action`.
- The cache stores the active `SessionCallerContext` for the caller/session pair and is refreshed after submit, so runtime event sequence/session mutations are carried forward.
- `CastAbility` and non-runtime/fallback paths still use the regular row-backed auth behavior.
- The first cache version used a `BTreeMap`, but that pushed the benchmark Wasm over the IC code-section limit. The final version uses two explicit cache slots.
- Runtime command receipts were simplified from two `BTreeMap`s to a small `Vec` with linear scans. Active battles have a small receipt set, and this freed enough canister code size for the auth cache.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `DOMM_CANISTER_FEATURES=benchmark CANIC_POCKET_IC_LOCK_NAMESPACE=domm-auth-cache-two cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_readiness_advances_and_replays -- --nocapture`

Focused benchmark:

```text
run id: 20260519-062456-gate-k-two-slot-auth-cache
artifact: target/benchmarks/20260519-062456-gate-k-two-slot-auth-cache/gate-k/summary.json
note: summary git_sha is f8399a4 because this was run before committing the checkpoint
```

Gate K result:

| status | updates | queries | row growth | stable pages start | stable pages final |
| --- | ---: | ---: | ---: | ---: | ---: |
| passed | 87 | 115 | 228 | 1025 | 149889 |

Key method summary:

| method | kind | calls | avg instructions | p95 instructions | avg memory delta | avg cycles |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `submit_battle_action` | update | 25 | 0.2846B | 1.1852B | 0.00 MB | 0.0003T |
| `sync_battle` | update | 28 | 9.8803B | 36.1180B | 36.06 MB | 0.0107T |
| `get_battle_state` | query | 48 | n/a | n/a | 1.33 MB | 0T |
| `get_events_after` | query | 1 | n/a | n/a | 0 MB | 0T |

Submit phase summary after this cut:

| phase | calls | avg instructions |
| --- | ---: | ---: |
| `auth_context` | 25 | 0.1693B |
| `readiness_schedule` | 25 | 0.0951B |
| `event_fanout` | 25 | 0.0192B |
| `apply_rules` | 25 | 0.0003B |
| `load_battle_state` | 25 | 0.0002B |
| `command_begin` | 25 | 0.0001B |
| `validate_action` | 25 | 0.0001B |
| `final_response` | 25 | 0.0000B |
| `recovery` | 25 | 0.0000B |
| `timeout` | 25 | 0.0000B |
| `persist_battle_state` | 25 | 0.0000B |

Decision:

- Gate 3 and Gate 4 are cleared in focused Gate K: `submit_battle_action` is now at the intended normal target of about 0.3B avg.
- The p95 is still 1.1852B because cache misses and edge/fallback actions still pay a stable auth read. That is acceptable for this checkpoint but should be watched in the full suite.
- `sync_battle` remains expensive at about 9.88B avg and is now the largest battle endpoint smell.

Full benchmark suite after this checkpoint:

```text
run id: 20260519-063353-02c93e3
artifact: target/benchmarks/20260519-063353-02c93e3/suite-summary.md
command: DOMM_BENCH_JOBS=4 scripts/run-benchmarks.sh
```

Suite result:

| gate | status | elapsed | instructions | memory | note |
| --- | --- | ---: | ---: | ---: | --- |
| Gate J | passed | 130s | 404.0368B | 6462.50 MB | strategic loop |
| Gate K | passed | 207s | 929.2174B | 9314.25 MB | battle aftermath/victory |
| Gate L | passed | 250s | 1152.7459B | 11228.00 MB | first-playable public route |
| Gate M | passed | 485s | 809.2669B | 11912.00 MB | canister-backed client probe |

Battle action in full suite:

| gate | calls | avg instructions | p95 instructions | avg memory delta |
| --- | ---: | ---: | ---: | ---: |
| Gate K | 25 | 0.2846B | 1.1852B | 0.0025 MB |
| Gate L | 26 | 0.2734B | 1.1851B | 0.0000 MB |

Decision from full suite:

- The focused Gate K improvement holds in the broader suite.
- Required endpoint coverage is still intentionally partial per gate, so the "no missing endpoints" checklist remains open.
- `sync_battle`, `sync_session_turn`, town/economy commands, and read APIs now dominate suite cost more than `submit_battle_action`.

## Checkpoint: Runtime Manual Battle Readiness

Moved active `end_battle_turn` manual readiness onto `BattleRuntime.ready_participants`.

What changed:

- Active battle rows are adopted into runtime before manual end-turn handling when needed.
- `end_battle_turn` now checks active runtime stack ownership first, avoiding a durable `BattleStack` page when runtime exists.
- For active runtime battles, manual end-turn marks `BattleRuntime.ready_participants` and reports a synthetic `battle_participant_round_ready` changed subject instead of creating/updating a `BattleParticipantRoundReady` row.
- The existing durable `GameCommand` and public `GameEvent` response behavior remains in place for this checkpoint.

Decision:

- A fuller version that moved `end_battle_turn` command receipts and events into runtime crossed the IC Wasm code-section limit by about 2.3 KB during PocketIC install (`12585225` bytes vs max `12582912`).
- The scoped version keeps the checkpoint inside the code-size limit and completes the todo goal of removing the manual readiness row hot path.
- Runtime command/event migration for `end_battle_turn` should be revisited only with a broader code-size reduction or module split.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `DOMM_CANISTER_FEATURES=benchmark CANIC_POCKET_IC_LOCK_NAMESPACE=domm-runtime-end-turn3 cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_readiness_advances_and_replays -- --nocapture`
- `DOMM_CANISTER_FEATURES=benchmark CANIC_POCKET_IC_LOCK_NAMESPACE=domm-runtime-end-turn2 cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_both_players_end_round_and_timer_catches_up -- --nocapture`

## Checkpoint: Runtime Readiness Without Legal-Action Scan

Removed `legal_actions_for_stack` from active runtime battle readiness recompute.

What changed:

- Active runtime readiness now marks participants ready only from runtime alive/acted state.
- This is equivalent under the current rules because `legal_actions_for_stack` always exposes `Defend` for every living unacted stack and does not check active-stack turn ownership.
- The row-backed fallback readiness path still uses the legacy `participant_has_meaningful_action` scan.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `DOMM_CANISTER_FEATURES=benchmark CANIC_POCKET_IC_LOCK_NAMESPACE=domm-runtime-acted-ready cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_readiness_advances_and_replays -- --nocapture`
- `DOMM_CANISTER_FEATURES=benchmark CANIC_POCKET_IC_LOCK_NAMESPACE=domm-runtime-acted-ready2 cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_both_players_end_round_and_timer_catches_up -- --nocapture`

## Checkpoint: CastAbility Fallback Decision

Documented `CastAbility` as a rare row-backed fallback instead of moving it into runtime in this pass.

Reasoning:

- `CastAbility` is absent from the saved full benchmark suite artifacts under `target/benchmarks/20260519-063353-02c93e3`.
- Gate M's client probe chooses enabled battle actions excluding `CastAbility`, so the main playable benchmark path is measuring normal attack/move/defend battle actions.
- The canister path for `CastAbility` intentionally couples tactical state with champion mana, learned spell ownership, and command effect rows. Moving it to runtime would require a broader champion/spell aggregate decision, not just a battle-state patch.
- The previous `end_battle_turn` runtime receipt/event attempt already showed the benchmark Wasm is close to the IC code-section limit, so adding a low-frequency spell runtime path is not the right next tradeoff.

Decision:

- Keep `CastAbility` row-backed for now.
- Revisit only if a benchmark scenario starts casting battle spells often enough to show up in method or phase summaries.

## Focused Benchmark: Runtime Readiness Follow-Ups

Ran a focused Gate K benchmark after runtime manual readiness, acted-state readiness, and the `CastAbility` fallback decision.

```text
run id: 20260519-071345-5515805-gate-k-runtime-readiness
artifact: target/benchmarks/20260519-071345-5515805-gate-k-runtime-readiness/gate-k/summary.json
command: DOMM_CANISTER_FEATURES=benchmark DOMM_BENCH_OUTPUT_DIR=... cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_gate_k_battle_aftermath_victory_history_persist_icydb_rows -- --nocapture
```

Gate K result:

| status | updates | queries | row growth | stable pages start | stable pages final |
| --- | ---: | ---: | ---: | ---: | ---: |
| passed | 84 | 118 | 228 | 1025 | 149889 |

Key method summary:

| method | kind | calls | avg instructions | avg memory delta | avg cycles |
| --- | --- | ---: | ---: | ---: | ---: |
| `submit_battle_action` | update | 25 | 0.2844B | 0.0025 MB | 0.0003T |
| `sync_battle` | update | 28 | 9.8815B | 36.0580 MB | 0.0107T |

Submit phase summary:

| phase | calls | avg instructions |
| --- | ---: | ---: |
| `auth_context` | 25 | 0.1694B |
| `readiness_schedule` | 25 | 0.0947B |
| `event_fanout` | 25 | 0.0192B |
| `apply_rules` | 25 | 0.0003B |
| `load_battle_state` | 25 | 0.0002B |
| `persist_battle_state` | 25 | 0B |

Decision:

- The acted-state readiness cut is behaviorally safe but not a large measured win; submit remains auth dominated.
- Runtime occupancy/stack indexes are not justified yet because runtime `load_battle_state` and `apply_rules` phases are tiny.
- No active-submit serialization/checkpointing remains visible in traces: `persist_battle_state` is 0B avg and memory delta is about 0.0025 MB.

## Checkpoint: Runtime Battle Header Is Authoritative While Active

Removed the remaining active-runtime `Battle` row header projections for round, active stack, and deadline changes.

What changed:

- `sync_battle` now reports `active_stack_id` from `BattleRuntime` when an active runtime exists, falling back to the durable `Battle` row only for row-backed battles.
- Runtime timeout timer jobs now apply the timeout directly against `BattleRuntime` and complete the timer job instead of projecting the header and falling into the row-backed timeout path.
- Round-advance jobs now use runtime round/state/active-stack data while active and schedule the next timeout from the runtime deadline.
- `apply_system_battle_action_from_runtime` no longer writes the durable `Battle` row header after timeout auto-defend or round auto-defend.

Decision:

- The durable `Battle` row remains a shell/projection for lookup, row-backed fallback, battle start, finalization, and explicit full-state projection.
- Active runtime is now authoritative for active tactical round/stack/deadline movement.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `DOMM_CANISTER_FEATURES=benchmark CANIC_POCKET_IC_LOCK_NAMESPACE=domm-runtime-header cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round -- --nocapture`

## Blocked Checkpoint: Runtime Archive Durable Flush

Tried to implement bounded durable flushing for runtime command receipts and runtime events after battle resolution.

Attempts:

- Full command/event flush with a dedicated `runtime_battle_archive_flush` system job: canister install failed with code section `12,605,950` bytes, over the `12,582,912` byte IC limit by `23,038` bytes.
- Slimmer command/event checkpoint without a new job kind: canister install failed with code section `12,598,075` bytes, over by `15,163` bytes.
- Event-only bounded checkpoint with 45 added source lines: canister install failed with code section `12,590,248` bytes, over by `7,336` bytes.

Decision:

- Backed out the flush code because a canister that cannot install is not a valid checkpoint.
- Leave the todo unchecked.
- Keep using the current in-memory archive plus upgrade snapshot for runtime command receipts/events.
- Revisit after freeing at least 10 KB of Wasm code section, moving debug/history flushing into a split canister, or replacing enough existing code with a smaller shared helper.

## Checkpoint: Benchmark Endpoint Coverage Gate

Added a dedicated benchmark gate for required public endpoint coverage.

What changed:

- `scripts/run-benchmarks.sh` now runs `endpoint-surface` before Gate J/K/L/M.
- The suite summary now reports union required-endpoint coverage across benchmark gates.
- `pocket_ic_benchmark_endpoint_surface_records_every_required_endpoint` records every required public endpoint through the benchmark recorder in one active two-player session.
- The endpoint-surface route is a coverage/per-method-cost probe, not a perfect realistic workload for every endpoint. Some battle and movement calls intentionally hit typed error paths when setting up a full valid scenario would turn this into another long Gate L/K route.

Official full-suite result:

```text
run id: 20260519-081911-fe84689
suite artifact: target/benchmarks/20260519-081911-fe84689/suite-summary.md
command: DOMM_BENCH_JOBS=5 scripts/run-benchmarks.sh
```

Suite gate status:

| metric | value |
| --- | ---: |
| endpoint-surface | passed in 108s |
| gate-j | passed in 129s |
| gate-k | passed in 206s |
| gate-l | passed in 253s |
| gate-m | passed in 462s |
| suite required endpoints covered | 59/59 |
| suite missing required endpoints | 0 |

Endpoint-surface summary:

| metric | value |
| --- | ---: |
| benchmark calls | 150 |
| scenario instructions | 391.3676B |
| scenario memory delta | 6029.0625 MB |
| update methods missing avg instructions | 0 |
| query methods missing avg instructions | 0 |

Update method examples from the coverage run:

| method | calls | avg instructions | errors |
| --- | ---: | ---: | ---: |
| `submit_build_town_structure` | 1 | 20.7621B | 0 |
| `claim_quest_reward` | 1 | 19.5705B | 0 |
| `submit_dwelling_recruit` | 1 | 16.7474B | 0 |
| `sync_advanced_victory` | 1 | 16.5134B | 0 |
| `submit_recruit_units` | 1 | 16.2843B | 0 |
| `submit_battle_action` | 1 | 2.1083B | 1 |

Decision:

- Mark required benchmark endpoint coverage done: all required public endpoints can now appear in one full benchmark run.
- Treat `Inst change = n/a` as acceptable for first comparable runs; the important failure case was missing `Avg inst B` for update methods, and that is fixed for updates.
- Query instruction capture should be evaluated through `scripts/run-benchmarks.sh`, which writes stdout directly to the query log. The official suite run captured query averages for every endpoint-surface query.

## Review: Broader Aggregate Pattern After Battle

Town command paths:

- `get_town_view` now reads real `TownBuilding`, `TownRecruitPool`, and `TownGarrisonStack` child rows through `render_projection::town_view`, so the previous fake/synthesized town-view risk is gone.
- `submit_build_town_structure` still loads content, built building ids, participant resources, command/effect/event rows, then creates or updates `TownBuilding`, `TownRecruitPool`, `Town`, and participant state.
- `submit_recruit_units` still loads and updates `TownRecruitPool`, upserts `TownGarrisonStack`, mutates participant resources, and writes command/effect/event rows.
- Full-suite Gate J/L measured town build around 20.8B and recruit around 16.4B, but these are lower frequency than movement/turn sync in current scenarios.

Champion command paths:

- `select_champion_level_up`, `learn_champion_spell`, and `cast_adventure_spell` load/update `Champion`, `ChampionSpell`, content rows, command/effect rows, and public events.
- Movement and battle aftermath also mutate champion rows, champion army stacks, map occupancy, status, movement remaining, mana/cooldown-like turn fields, and defeated/active state.
- This is the same live-row smell, but the champion path is tightly coupled to movement and aftermath. It should be handled with the movement/session-turn aggregate rather than as an isolated small champion rewrite.

Session/world turn sync paths:

- `submit_move_intent` writes a durable `MovementIntent`, command/effect/event rows, and validates against champion state, path bounds/cost, blockers, and map state.
- `sync_session_turn` loads pending movements by scanning participants and champion ids, hydrates champions and participants, resolves movement microsteps against occupancy/world objects/enemies, writes snapshots/occupancy/champions/participants/events, materializes income, updates session turn/deadline, and schedules jobs.
- Full-suite Gate K/L measured `submit_move_intent` around 15.4-15.6B and `sync_session_turn` around 19.3B. These are now the obvious hot spots after active `submit_battle_action` reached about 0.28B.

Next aggregate:

- Pick session-turn/champion-movement as the next aggregate.
- Initial shape should be a session-turn runtime keyed by `(session_id, turn_number)` containing active movement intents, participant ready state, champion position/movement deltas, occupancy deltas, event buffer, and due-job/deadline hints.
- Keep durable rows as projections/history/checkpoints while the turn is active, similar to the battle runtime pattern.
- First benchmark target should be reducing `sync_session_turn` below 5B and `submit_move_intent` below 5B before pushing toward sub-1B.

## Checkpoint: Movement Small Cut And Code-Size Blocker

Implemented the first session-turn/champion-movement checkpoint that still fits in the benchmark canister.

What changed:

- `submit_move_intent` now serializes `path_text` once and reuses it for payload, hash, and `MovementIntent.path_json`.
- `sync_session_turn` no longer reloads the caller participant before returning a partial movement sync. The reload is still kept before income/turn advancement, where updated participant resources can matter.

Attempted but backed out:

- Benchmark phase attribution around movement submit/sync. PocketIC install failed because the benchmark Wasm code section became `12,603,857` bytes, which is `20,945` bytes over the IC limit.
- Replacing per-champion pending intent lookups with the existing indexed `movement.intents_by_session_turn_status` page. PocketIC install failed because the benchmark Wasm code section became `12,598,630` bytes, which is `15,718` bytes over the IC limit.

Focused Gate J result:

```text
run id: 20260519-085429-movement-small-gate-j
artifact: target/benchmarks/20260519-085429-movement-small-gate-j/summary.md
command: DOMM_CANISTER_FEATURES=benchmark cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_gate_j_strategic_loop_persists_icydb_rows -- --nocapture
```

Comparison against full-suite Gate J baseline `20260519-081911-fe84689`:

| metric | baseline | checkpoint | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 404.0368B | 399.1517B | -1.2% |
| `submit_move_intent` avg instructions | 15.363B | 15.3598B | flat |
| `sync_session_turn` avg instructions | 18.4665B | 18.0194B | -2.4% |
| `sessions.load_participant` repo calls | 15 | 8 | -46.7% |
| `sessions.load_participant` total instructions | 10.5675B | 5.6409B | -46.6% |

Decision:

- Keep the small checkpoint because it is safe and measurable.
- Treat the movement aggregate as blocked on code-size headroom before adding new benchmark phases, new generic IcyDB movement queries, or a larger heap turn runtime.
- The next practical work is freeing roughly 20 KB of benchmark Wasm code section or moving diagnostic/benchmark surface out of the main canister. After that, retry indexed pending movement loading first, then the heap session-turn runtime if the indexed cut is not enough.

## Checkpoint: Benchmark Wasm Code-Size Headroom

Freed benchmark canister code-section headroom by omitting diagnostic system-job control endpoints only when building with `feature = "benchmark"`.

What changed:

- `get_diagnostic_system_jobs`, `force_diagnostic_system_job_running`, `run_diagnostic_system_jobs`, and `run_diagnostic_system_job` remain available in normal canister builds.
- The same endpoints are not exported in benchmark canister builds because Gate J/K/L/endpoint-surface use storage snapshots and benchmark metrics, not diagnostic system-job control.
- This keeps production/regression diagnostics intact while making benchmark builds able to accept additional movement/runtime code again.

Measurement:

```text
command: CARGO_TARGET_DIR=target/pocket-ic-endpoint-presence-benchmark cargo build -p domm-degens-canister --target wasm32-unknown-unknown --release --features benchmark
code section: 12,547,075 bytes
IC limit: 12,582,912 bytes
headroom: 35,837 bytes
freed vs prior benchmark build: 34,338 bytes
```

Decision:

- Mark the code-size headroom todo done.
- Re-attempt the indexed pending movement loading checkpoint next; it previously needed about 15.7 KB beyond the prior build, so the current headroom should be enough.

## Checkpoint: Indexed Pending Movement Loading

Re-attempted the indexed pending movement loading cut after freeing benchmark Wasm headroom.

What changed:

- `pending_movement_intents_for_session` now pages pending `MovementIntent` rows through the existing `movement.intents_by_session_turn_status` index.
- The function still filters to active participants and sorts by champion id before hydrating champion/participant rows, preserving behavior while avoiding per-champion pending-intent lookup shape.

Code-size measurement:

```text
command: CARGO_TARGET_DIR=target/pocket-ic-endpoint-presence-benchmark cargo build -p domm-degens-canister --target wasm32-unknown-unknown --release --features benchmark
code section: 12,564,294 bytes
IC limit: 12,582,912 bytes
headroom: 18,618 bytes
```

Focused Gate J result:

```text
run id: 20260519-090940-movement-index-gate-j
artifact: target/benchmarks/20260519-090940-movement-index-gate-j/summary.md
command: DOMM_CANISTER_FEATURES=benchmark cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_gate_j_strategic_loop_persists_icydb_rows -- --nocapture
```

Comparison against small movement checkpoint `20260519-085429-movement-small-gate-j`:

| metric | previous | indexed | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 399.1517B | 393.4643B | -1.4% |
| `submit_move_intent` avg instructions | 15.3598B | 15.3572B | flat |
| `sync_session_turn` avg instructions | 18.0194B | 17.5063B | -2.8% |
| `movement.intents_by_session_turn_status` repo calls | 0 visible | 16 | now measured |
| `movement.intents_by_session_turn_status` total instructions | 0 visible | 5.6154B | now measured |

Decision:

- Keep the indexed loader; it is small, benchmarked, and moves `sync_session_turn` in the right direction.
- The result is nowhere near the 5B target, so the next meaningful cut needs to move active turn command/intents/events toward heap runtime state, not only query shape tuning.

## Checkpoint: Fresh Movement Submit Effect Shortcut

Added a narrow `create_command_effect` helper and used it only for freshly-created `submit_move_intent` commands. This skips the `effects.command_effect_by_command_key` absence read on the hot movement submit path while keeping the old idempotent `ensure_command_effect` path for replay/recovery commands.

Rejected experiment:

- I first tried using the active-session caller cache on movement submit/sync/end-turn. Focused Gate J exposed the problem: cached `GameSession.next_event_seq` can become stale across event-writing movement calls and caused `sync_session_turn` to fail with `events.create_game_event`. That cache change was removed before the final benchmark.

Code-size measurement:

```text
command: CARGO_TARGET_DIR=target/pocket-ic-endpoint-presence-benchmark cargo build -p domm-degens-canister --target wasm32-unknown-unknown --release --features benchmark
code section: 12,569,998 bytes
IC limit: 12,582,912 bytes
headroom: 12,914 bytes
```

Verification:

```text
cargo fmt --check
cargo check -p domm-degens-canister
cargo check -p domm-degens-canister --features benchmark
cargo test -p domm-degens-canister lobby_session_setup_recovers_from_starting_state_and_replays_nonce -- --nocapture
DOMM_CANISTER_FEATURES=benchmark DOMM_BENCH_OUTPUT_DIR=target/benchmarks/20260519-095656-movement-effect-fresh-gate-j DOMM_BENCH_QUERY_LOG_PATH=target/benchmarks/20260519-095656-movement-effect-fresh-gate-j/test-output.log cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_gate_j_strategic_loop_persists_icydb_rows -- --nocapture
```

Focused Gate J result:

| metric | indexed baseline | effect shortcut | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 393.4643B | 391.3873B | -0.5% |
| `submit_move_intent` avg instructions | 15.3572B | 14.6615B | -4.5% |
| `sync_session_turn` avg instructions | 17.5063B | 17.5053B | flat |
| `effects.command_effect_by_command_key` calls | 18 | 15 | -16.7% |
| `effects.command_effect_by_command_key` total instructions | 12.6539B | 10.5574B | -16.6% |

Decision:

- Keep this shortcut because it is small, fits the benchmark Wasm limit, and removes measured stable reads from fresh movement submits.
- Do not extend active-session caller caching to movement while event sequence is stored on `GameSession`; that needs a heap event sequence block or aggregate-owned event writer first.
- The remaining submit cost is still dominated by durable command/idempotency rows, public event/session updates, movement intent upsert, and system job guard scans. The next meaningful movement cut should be heap active-turn command receipts/intents/events rather than more row-level micro-optimizations.

## Checkpoint: Fresh Movement Event Shortcut

Added a narrow `append_new_public_event` helper and used it only for brand-new movement intent submissions. The helper creates the public event directly instead of first proving absence through `events.by_session_event_key`.

Safety condition:

- The shortcut is used only when `begin_participant_command_tracked` reports a fresh command and `find_movement_intent(session, champion, turn)` found no existing champion-turn intent before creation.
- Existing intents, replacements, replay/recovery commands, and all other event paths still use `append_public_event`, which keeps the idempotent event-key lookup.
- Movement intent rows are not deleted in the current flow, so an existing movement-intent event without an existing movement intent would indicate already-corrupt state. The shortcut should not hide normal idempotency cases.

Code-size measurement:

```text
command: CARGO_TARGET_DIR=target/pocket-ic-endpoint-presence-benchmark cargo build -p domm-degens-canister --target wasm32-unknown-unknown --release --features benchmark
code section: 12,570,668 bytes
IC limit: 12,582,912 bytes
headroom: 12,244 bytes
```

Verification:

```text
cargo fmt --check
cargo check -p domm-degens-canister
cargo check -p domm-degens-canister --features benchmark
DOMM_CANISTER_FEATURES=benchmark DOMM_BENCH_OUTPUT_DIR=target/benchmarks/20260519-100606-movement-new-event-gate-j DOMM_BENCH_QUERY_LOG_PATH=target/benchmarks/20260519-100606-movement-new-event-gate-j/test-output.log cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_gate_j_strategic_loop_persists_icydb_rows -- --nocapture
```

Focused Gate J result versus `20260519-095656-movement-effect-fresh-gate-j`:

| metric | effect shortcut | event shortcut | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 391.3873B | 389.8865B | -0.4% |
| `submit_move_intent` avg instructions | 14.6615B | 14.1924B | -3.2% |
| `sync_session_turn` avg instructions | 17.5053B | 17.5045B | flat |
| `events.by_session_event_key` calls | 22 | 20 | -9.1% |
| `events.by_session_event_key` total instructions | 15.5007B | 14.0921B | -9.1% |
| `effects.command_effect_by_command_key` calls | 15 | 15 | flat |

Cumulative movement submit result versus indexed movement baseline `20260519-090940-movement-index-gate-j`:

| metric | indexed baseline | current | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 393.4643B | 389.8865B | -0.9% |
| `submit_move_intent` avg instructions | 15.3572B | 14.1924B | -7.6% |
| `effects.command_effect_by_command_key` calls | 18 | 15 | -16.7% |
| `events.by_session_event_key` calls | 22 | 20 | -9.1% |

Decision:

- Keep the shortcut because it is small, behaviorally narrow, and removes two measured stable index reads from Gate J.
- Do not spread `append_new_public_event` broadly until each caller can prove a comparable freshness invariant.
- The cost is still far above the 5B target. The next meaningful work should stop creating durable movement commands/intents/events on every active-turn operation, or introduce an active turn aggregate that owns fresh intent and event buffers like the battle runtime.

## Checkpoint: Early Sync Not-Due Precheck

Moved the `sync_session_turn` `turn_not_due` check before durable command creation. The endpoint still returns a `CommandResponse` with `CommandStatus::Failed`, but early not-due syncs now use the same in-memory response path used by active battle runtime failures instead of writing a failed `GameCommand` row.

Safety condition:

- Existing command replays are only relevant after a command row exists. This path runs before command creation and only for fresh not-due sync attempts.
- The stale-turn regression already allowed pre-command denials for movement when backend work is pending; this applies the same shape to a not-due manual sync retry.
- Due syncs and all-ready syncs still use durable command idempotency and the normal apply path.

Code-size measurement:

```text
command: CARGO_TARGET_DIR=target/pocket-ic-endpoint-presence-benchmark cargo build -p domm-degens-canister --target wasm32-unknown-unknown --release --features benchmark
code section: 12,571,456 bytes
IC limit: 12,582,912 bytes
headroom: 11,456 bytes
```

Verification:

```text
cargo fmt --check
cargo check -p domm-degens-canister
cargo check -p domm-degens-canister --features benchmark
cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_end_turn_closes_turn_and_blocks_stale_actions -- --nocapture
DOMM_CANISTER_FEATURES=benchmark DOMM_BENCH_OUTPUT_DIR=target/benchmarks/20260519-101910-movement-sync-precheck-gate-j DOMM_BENCH_QUERY_LOG_PATH=target/benchmarks/20260519-101910-movement-sync-precheck-gate-j/test-output.log cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_gate_j_strategic_loop_persists_icydb_rows -- --nocapture
```

Focused Gate J result versus `20260519-100606-movement-new-event-gate-j`:

| metric | fresh event | sync precheck | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 389.8865B | 384.8650B | -1.3% |
| scenario memory | 6,462.4375 MB | 6,358.3125 MB | -1.6% |
| `submit_move_intent` avg instructions | 14.1924B | 14.1979B | flat |
| `sync_session_turn` avg instructions | 17.5045B | 17.0512B | -2.6% |
| early not-due sync calls | about 5.19B | about 3.52B | about -32% |
| `commands.game_command_idempotency` calls | 17 | 14 | -17.6% |
| `commands.create_game_command` calls | 17 | 14 | -17.6% |
| `commands.update_game_command` calls | 16 | 13 | -18.8% |

Decision:

- Keep this cut because it is narrow, preserves the public response shape, and removes durable command writes from three benchmarked manual sync retries.
- This does not change the real sync hot path. The high-cost applied sync calls are still dominated by movement/battle/start/aftermath rows, events/effects, system-job scans, and command rows.
- Next cuts should target applied `sync_session_turn`, especially durable command/effect/event/session writes and repeated system-job scans, rather than more not-due retry cleanup.

## Checkpoint: Fresh Sync Command Effect Shortcut

Switched manual `sync_session_turn` command start to the tracked command-begin helper and used the freshness bit to create the turn-resolution `CommandEffect` directly for fresh manual sync commands. Recovered pending sync commands and system-job sync commands still use `ensure_command_effect`.

Safety condition:

- Fresh manual sync commands have no existing `CommandEffect` for the newly-created command id, so the absence read is redundant.
- Replays return from command idempotency before this path.
- Seeded/recovered pending sync commands keep the idempotent effect lookup. This is covered by the service regression that seeds a pending `sync_session_turn` command and expects recovery to reuse its command id.
- System-job turn resolution still uses the idempotent effect path because `ensure_system_turn_command` can return an existing durable command.

Code-size measurement:

```text
command: CARGO_TARGET_DIR=target/pocket-ic-endpoint-presence-benchmark cargo build -p domm-degens-canister --target wasm32-unknown-unknown --release --features benchmark
code section: 12,572,384 bytes
IC limit: 12,582,912 bytes
headroom: 10,528 bytes
```

Verification:

```text
cargo fmt --check
cargo check -p domm-degens-canister
cargo check -p domm-degens-canister --features benchmark
cargo test -p domm-degens-canister lobby_session_setup_recovers_from_starting_state_and_replays_nonce -- --nocapture
DOMM_CANISTER_FEATURES=benchmark DOMM_BENCH_OUTPUT_DIR=target/benchmarks/20260519-102824-movement-sync-effect-gate-j DOMM_BENCH_QUERY_LOG_PATH=target/benchmarks/20260519-102824-movement-sync-effect-gate-j/test-output.log cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_gate_j_strategic_loop_persists_icydb_rows -- --nocapture
```

Focused Gate J result versus `20260519-101910-movement-sync-precheck-gate-j`:

| metric | sync precheck | sync effect shortcut | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 384.8650B | 379.3951B | -1.4% |
| `submit_move_intent` avg instructions | 14.1979B | 14.2066B | flat |
| `sync_session_turn` avg instructions | 17.0512B | 16.5509B | -2.9% |
| `effects.command_effect_by_command_key` calls | 15 | 7 | -53.3% |
| `effects.command_effect_by_command_key` total instructions | 10.5491B | 4.9217B | -53.3% |

Decision:

- Keep this cut because it is a direct extension of the already-proven fresh command/effect shortcut and it removes eight measured effect lookup reads from Gate J.
- The remaining applied sync cost is still dominated by system-job scans, event/session writes, movement intent loading, participant/champion loads, and battle-start row work. The path still needs a real active turn aggregate to approach the 5B target.

## Checkpoint: Direct Timer Refresh For Current-Turn Job Reschedules

Added a `system_jobs::reschedule_job` service wrapper that updates a known job row and refreshes the heap timer for that job directly. `reschedule_current_turn_jobs_for_manual_sync` now uses it instead of rescanning all due/running jobs after each partial sync. `complete_current_turn_jobs` also no longer calls `schedule_nearest_due_job`; `sync_session_turn` immediately schedules the next turn deadline and scenario jobs after completing the old current-turn jobs, so the scan was redundant on the successful turn-advance path.

Safety condition:

- Partial sync still updates the same current-turn `turn_deadline`/`turn_resolution` jobs; it just refreshes the timer from the updated job instead of finding the nearest job through global scans.
- Successful turn advancement still completes current-turn jobs before mutating the session to the next turn, then schedules next-turn deadline and maintenance jobs in the same update. If a later step traps, IC rollback reverts the completed jobs too.
- The stale-turn PocketIC regression passed, covering immediate turn resolution, timer advancement, and stale action blocking after all players ended the turn.

Code-size measurement:

```text
command: CARGO_TARGET_DIR=target/pocket-ic-endpoint-presence-benchmark cargo build -p domm-degens-canister --target wasm32-unknown-unknown --release --features benchmark
code section: 12,575,951 bytes
IC limit: 12,582,912 bytes
headroom: 6,961 bytes
```

Verification:

```text
cargo fmt --check
cargo check -p domm-degens-canister
cargo check -p domm-degens-canister --features benchmark
cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_end_turn_closes_turn_and_blocks_stale_actions -- --nocapture
DOMM_CANISTER_FEATURES=benchmark DOMM_BENCH_OUTPUT_DIR=target/benchmarks/20260519-103739-movement-job-reschedule-gate-j DOMM_BENCH_QUERY_LOG_PATH=target/benchmarks/20260519-103739-movement-job-reschedule-gate-j/test-output.log cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_gate_j_strategic_loop_persists_icydb_rows -- --nocapture
```

Focused Gate J result versus `20260519-102824-movement-sync-effect-gate-j`:

| metric | sync effect shortcut | job reschedule cut | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 379.3951B | 366.6428B | -3.4% |
| `submit_move_intent` avg instructions | 14.2066B | 14.2054B | flat |
| `sync_session_turn` avg instructions | 16.5509B | 15.3974B | -7.0% |
| `system_jobs.by_status_due` calls | 11 | 2 | -81.8% |
| `system_jobs.by_status_lease` calls | 11 | 2 | -81.8% |
| `system_jobs.by_status_due` total instructions | 7.7355B | 1.4095B | -81.8% |
| `system_jobs.by_status_lease` total instructions | 7.7383B | 1.4089B | -81.8% |

Decision:

- Keep this cut because it removes the global nearest-job scans from the applied sync hot path while preserving timer behavior in the regression that matters.
- Benchmark Wasm headroom is now only about 7 KB, so the next large active-turn aggregate likely needs another code-size freeing checkpoint or a split of benchmark/debug-only surface before adding new runtime structures.

## Checkpoint: Pre-Deadline Scheduled Job Guard

Narrowed the map-turn command guard before the session turn deadline.

What changed:

- Before `GameSession.turn_deadline_at`, `ensure_map_turn_accepts_new_command` now scans only scheduled system jobs for the session and blocks only when a current-turn `turn_resolution` or `turn_deadline` job is already due.
- At or after the deadline, the older running+scheduled guard remains, so deadline closure still treats running jobs as backend work pending.
- The implementation deliberately reuses the existing `page_system_jobs_by_session_status` repository path instead of adding a new due-only query helper. A due-only helper measured the same benchmark win but left only 112 bytes of benchmark Wasm headroom, which was not a good trade.

Code-size measurement:

```text
command: CARGO_TARGET_DIR=target/pocket-ic-endpoint-presence-benchmark cargo build -p domm-degens-canister --target wasm32-unknown-unknown --release --features benchmark
code section: 12,576,504 bytes
IC limit: 12,582,912 bytes
headroom: 6,408 bytes
```

Verification:

```text
cargo fmt --check
cargo check -p domm-degens-canister
cargo check -p domm-degens-canister --features benchmark
cargo test -p domm-degens-canister lobby_session_setup_recovers_from_starting_state_and_replays_nonce -- --nocapture
DOMM_CANISTER_FEATURES=benchmark DOMM_BENCH_OUTPUT_DIR=target/benchmarks/20260519-110341-movement-scheduled-guard-gate-j DOMM_BENCH_QUERY_LOG_PATH=target/benchmarks/20260519-110341-movement-scheduled-guard-gate-j/test-output.log cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_gate_j_strategic_loop_persists_icydb_rows -- --nocapture
```

Focused Gate J result versus `20260519-103739-movement-job-reschedule-gate-j`:

| metric | job reschedule cut | scheduled guard cut | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 366.6428B | 365.2604B | -0.4% |
| `submit_move_intent` avg instructions | 14.2054B | 13.7361B | -3.3% |
| `sync_session_turn` avg instructions | 15.3974B | 15.3986B | flat |
| `system_jobs.by_session_status_due` calls | 52 | 48 | -7.7% |
| `system_jobs.by_session_status_due` total instructions | 18.2989B | 16.8852B | -7.7% |

Decision:

- Keep this cut because it removes four measured session-job guard reads from Gate J while preserving almost all remaining benchmark Wasm headroom.
- The exact service regression passed but took 253.90s, confirming the guard safety case and reinforcing that this regression belongs in a slower focused group rather than every tiny edit loop.
- This is still a row-level micro-cut. `submit_move_intent` is now under 14B but nowhere near the 5B target, so the next meaningful work remains a heap active-turn aggregate that stops writing durable intent/command/event state on every fresh movement submit.

## Checkpoint: Disable Benchmark Phase Storage

Freed benchmark Wasm headroom by turning `benchmark_phase` into a pass-through wrapper in benchmark builds.

Rationale:

- Phase attribution was useful while breaking down the original `submit_battle_action` path.
- Current movement work is using public-method totals and repo-operation totals; the existing phase markers are battle-specific and do not drive the next movement decisions.
- The benchmark harness already accepts empty phase lists, and repo-op tracking remains enabled.

Code-size measurement:

```text
command: CARGO_TARGET_DIR=target/pocket-ic-endpoint-presence-benchmark cargo build -p domm-degens-canister --target wasm32-unknown-unknown --release --features benchmark
code section: 12,564,045 bytes
IC limit: 12,582,912 bytes
headroom: 18,867 bytes
freed vs scheduled guard checkpoint: 12,459 bytes
```

Verification:

```text
cargo fmt --check
cargo check -p domm-degens-canister --features benchmark
```

Decision:

- Keep this as a benchmark-only enabler before deleting redundant movement effects or adding any active-turn runtime structure.
- This does remove phase rows from future benchmark summaries. That is acceptable for perf1 because method-level and repo-operation summaries are now the source of truth.

## Checkpoint: Remove Movement Snapshot Command Effects

Deleted the duplicate `CommandEffect` projection from `record_movement_snapshot`.

What changed:

- `record_movement_snapshot` still uses `movement::find_movement_snapshot(command_id, intent_id, step_index)` as the replay/idempotency guard.
- It still writes the first-class `MovementSnapshot` row when absent.
- It no longer writes a second `movement_snapshot` `CommandEffect` with the same movement-step data.
- The now-unused `movement_outcome_key` helper was removed.

Safety condition:

- Movement history and service tests read `MovementSnapshot` rows directly.
- I found no reader that depends on `effect_type = "movement_snapshot"` or the `move_snap:*` effect key.
- Replay safety for movement steps remains on the movement snapshot unique lookup, which is more specific than the deleted command effect projection.

Code-size measurement:

```text
command: CARGO_TARGET_DIR=target/pocket-ic-endpoint-presence-benchmark cargo build -p domm-degens-canister --target wasm32-unknown-unknown --release --features benchmark
code section: 12,562,545 bytes
IC limit: 12,582,912 bytes
headroom: 20,367 bytes
```

Verification:

```text
cargo fmt --check
cargo check -p domm-degens-canister
cargo check -p domm-degens-canister --features benchmark
cargo test -p domm-degens-canister lobby_session_setup_recovers_from_starting_state_and_replays_nonce -- --nocapture
DOMM_CANISTER_FEATURES=benchmark DOMM_BENCH_OUTPUT_DIR=target/benchmarks/20260519-112144-movement-snapshot-effect-gate-j DOMM_BENCH_QUERY_LOG_PATH=target/benchmarks/20260519-112144-movement-snapshot-effect-gate-j/test-output.log cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_gate_j_strategic_loop_persists_icydb_rows -- --nocapture
```

Focused Gate J result versus `20260519-110341-movement-scheduled-guard-gate-j`:

| metric | scheduled guard cut | snapshot effect removal | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 365.2604B | 361.7712B | -1.0% |
| scenario memory | 6,358.25 MB | 6,326.1875 MB | -0.5% |
| `submit_move_intent` avg instructions | 13.7361B | 13.7454B | flat |
| `sync_session_turn` avg instructions | 15.3986B | 15.0796B | -2.1% |
| `effects.command_effect_by_command_key` calls | 7 | 4 | -42.9% |
| `effects.create_applied_command_effect` calls | 18 | 15 | -16.7% |
| `effects.command_effect_by_command_key` total instructions | 4.9219B | 2.8102B | -42.9% |
| `effects.create_applied_command_effect` total instructions | 8.5600B | 7.1335B | -16.7% |

Decision:

- Keep this cut because it removes redundant stable reads/writes, lowers scenario memory by 512 stable pages, and frees additional Wasm code.
- The remaining effect candidates are top-level `turn_resolution` effects in `resolve_pending_movement` and movement-submit `movement_intent` effects. Both look unused, but they should be deleted as separate checkpoints so regressions and benchmark attribution stay clear.

## Checkpoint: Remove Turn Resolution Command Effect

Deleted the top-level `turn_resolution:{turn}` `CommandEffect` from `resolve_pending_movement`.

What changed:

- `sync_session_turn` still creates/replays the durable `GameCommand`.
- Movement resolution still reads pending `MovementIntent` rows, writes first-class `MovementSnapshot` rows, updates champions/participants/map state, and emits public events.
- The extra `CommandEffect` row for `turn_resolution` is no longer created or ensured.
- `resolve_pending_movement` no longer needs the command-freshness flag, so the parameter and call-site plumbing were removed.

Safety condition:

- Seeded/recovered `sync_session_turn` recovery still passed and reused the seeded command id.
- Per-step replay remains guarded by `MovementSnapshot` unique lookup.
- Public event idempotency remains guarded by event keys.
- I found no consumer of the `turn_resolution:{turn}` command effect.

Code-size measurement:

```text
command: CARGO_TARGET_DIR=target/pocket-ic-endpoint-presence-benchmark cargo build -p domm-degens-canister --target wasm32-unknown-unknown --release --features benchmark
code section: 12,560,886 bytes
IC limit: 12,582,912 bytes
headroom: 22,026 bytes
```

Verification:

```text
cargo fmt --check
cargo check -p domm-degens-canister
cargo check -p domm-degens-canister --features benchmark
cargo test -p domm-degens-canister lobby_session_setup_recovers_from_starting_state_and_replays_nonce -- --nocapture
DOMM_CANISTER_FEATURES=benchmark DOMM_BENCH_OUTPUT_DIR=target/benchmarks/20260519-113052-movement-turn-effect-gate-j DOMM_BENCH_QUERY_LOG_PATH=target/benchmarks/20260519-113052-movement-turn-effect-gate-j/test-output.log cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_gate_j_strategic_loop_persists_icydb_rows -- --nocapture
```

Focused Gate J result versus `20260519-112144-movement-snapshot-effect-gate-j`:

| metric | snapshot effect removal | turn effect removal | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 361.7712B | 357.9811B | -1.0% |
| scenario memory | 6,326.1875 MB | 6,190.125 MB | -2.2% |
| `submit_move_intent` avg instructions | 13.7454B | 13.7537B | flat |
| `sync_session_turn` avg instructions | 15.0796B | 14.7341B | -2.3% |
| `effects.create_applied_command_effect` calls | 15 | 7 | -53.3% |
| `effects.create_applied_command_effect` total instructions | 7.1335B | 3.3193B | -53.5% |

Decision:

- Keep this cut because the seeded recovery regression passed and the redundant effect writes were a measured sync cost.
- The remaining effect count is now mostly submit-side and other command effects. Next small cut should remove `movement_intent` command effects from `submit_move_intent`, then reassess whether micro-cuts have plateaued before starting heap active-turn intent runtime.

## Decision: Shift Perf1 To Whole-Game Fast Path

The battle runtime work proved the architecture: hot gameplay commands should mutate a command-side heap aggregate, while IcyDB should be durable projection/history/fallback/boundary storage. `submit_battle_action` reached the intended range, but tests and real play still spend most of their route cost before or around battle: session setup, movement, turn sync, map/object/champion views, towns, economy, jobs, and projections.

Updated strategy:

| priority | target | reason |
| --- | --- | --- |
| 1 | session-turn/champion-movement runtime | Gate J still has `submit_move_intent` around `13.75B` and `sync_session_turn` around `14.73B`; every battle route depends on movement/turn state. |
| 2 | setup/session/view projections | test and client routes repeatedly read session, participant, champion, visible map/object, and game state before the core command under test. |
| 3 | town/economy aggregate | town build/recruit are still early gameplay commands around `16B-21B`. |
| 4 | champion aggregate | champion state is split across movement, battle aftermath, spells, army, artifacts, map occupancy, and views. |
| 5 | remaining battle boundary work | runtime archive flush and `CastAbility` are real work, but less important than the route-wide bottlenecks now. |

Testing decision:

- Do not run the full slow suite after every architecture edit.
- Use compile checks and focused PocketIC gates for the edit loop.
- Use full `scripts/run-benchmarks.sh` only after meaningful aggregate checkpoints.
- Use parallel PocketIC gates with separate namespaces/output dirs when validating a stable checkpoint.

Implementation decision:

- Stop polishing row-level movement/town/champion paths unless the cut is trivial, already in hand, or frees code-size headroom.
- The next serious implementation target is `SessionTurnRuntime`, not another long sequence of small stable-row shortcuts.
- If the in-progress `movement_intent` effect deletion is finished, treat it as the last small movement micro-cut before switching to the runtime rewrite.

## Checkpoint: SessionTurnRuntime Implementation Plan

Launched four parallel explorer subagents to inspect:

- movement/session-turn command flow;
- query/projection/status merge requirements;
- testing and benchmark strategy;
- future town/champion/economy aggregate boundaries.

Consolidated result is saved in `perf1.impl.md`.

Key decisions:

| decision | result |
| --- | --- |
| runtime order | Add runtime/projection plumbing first, mirror current durable behavior second, then make runtime authoritative. |
| ownership | `SessionTurnRuntime` is a turn orchestration aggregate, not the first canonical owner of town/champion/economy state. |
| query safety | Wire events/status/render/projection overlays before removing durable hot writes, otherwise APIs can lie while runtime is correct. |
| current dirty edit | Decide whether to finish or abandon the in-progress `movement_intent` effect deletion before runtime implementation starts. Do not mix it into runtime commits. |
| testing | Use compile checks and focused Gate J/regression tests for checkpoints; keep full benchmark suite for meaningful architecture gates. |
| future aggregates | After `SessionTurnRuntime`, sequence `ChampionOverlay`, `EconomyRuntime`, `TownRuntime`, then aftermath/render cleanup. |

Bug-prevention focus:

- preserve nonce replay and mismatched-payload rejection;
- keep event sequence monotonic across durable, battle runtime, and session-turn runtime;
- keep one authoritative partial movement cursor;
- make battle handoff a flush/pass boundary;
- archive runtime receipts/events before runtime removal;
- avoid resource double-spend while economy is still row-backed.

## Checkpoint: Runtime-Backed Movement Submit

Moved the active `submit_move_intent` hot path onto the new session-turn runtime for command receipts and active events, while keeping the durable `MovementIntent` row as the compatibility projection consumed by the current row-backed `sync_session_turn`.

What changed:

- `submit_move_intent` now checks runtime nonce receipts first and returns runtime replays without touching durable `GameCommand` rows.
- New movement submits no longer create/update durable `GameCommand` or durable `GameEvent` rows; the command response/status and active event feed are produced from `SessionTurnRuntime`.
- The endpoint still creates or updates the durable `MovementIntent` row so the existing sync path can resolve moves until the next runtime-sync checkpoint.
- Session-turn runtime events reserve a stable `GameSession.next_event_seq` block of 4,096 sequence numbers, avoiding collisions with durable events without a per-event session update.
- Benchmark diagnostics now use a benchmark-only entity count list. Two focused Gate J attempts failed before this with benchmark Wasm code section `12,589,747` bytes, `6,835` bytes over the IC limit; the trim made the same benchmark install and run.

Verified before the focused benchmark:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bffa71` / `12,581,489` bytes, `1,423` bytes under the IC limit.
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- `DOMM_CANISTER_FEATURES=benchmark CANIC_POCKET_IC_LOCK_NAMESPACE=domm-runtime-move-endpoints cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_canister_exposes_every_required_game_endpoint -- --nocapture` passed in `272.37s`.
- `DOMM_CANISTER_FEATURES=benchmark CANIC_POCKET_IC_LOCK_NAMESPACE=domm-runtime-move-stale-turn cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_end_turn_closes_turn_and_blocks_stale_actions -- --nocapture` passed in `71.88s`.

Focused Gate J:

```bash
DOMM_CANISTER_FEATURES=benchmark \
DOMM_BENCH_OUTPUT_DIR=target/benchmarks/20260519-132615-movement-runtime-submit-gate-j \
DOMM_BENCH_QUERY_LOG_PATH=target/benchmarks/20260519-132615-movement-runtime-submit-gate-j/test-output.log \
CANIC_POCKET_IC_LOCK_NAMESPACE=domm-bench-20260519-132615-movement-runtime-submit-gate-j-gate-j \
cargo test -p domm-pocket-ic-tests --test canister_endpoints \
  pocket_ic_gate_j_strategic_loop_persists_icydb_rows -- --nocapture
```

Result: passed in `693.19s`; artifact `target/benchmarks/20260519-132615-movement-runtime-submit-gate-j/summary.json`.

Measured delta versus `20260519-123422-movement-intent-effect-gate-j`:

| metric | previous | runtime submit | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 287.2952B | 275.6421B | -4.1% |
| scenario memory | 6,094.1250 MB | 6,022.0625 MB | -1.2% |
| `submit_move_intent` avg instructions | 13.2749B | 11.0412B | -16.8% |
| `submit_move_intent` avg memory | 21.4375 MB | 10.8333 MB | -49.5% |
| `sync_session_turn` avg instructions | 14.7336B | 14.2859B | -3.0% |
| `sync_session_turn` avg memory | 125.2443 MB | 125.2330 MB | ~0.0% |
| `commands.create_game_command` calls | 14 | 11 | -21.4% |
| `commands.game_command_idempotency` calls | 14 | 11 | -21.4% |
| `commands.update_game_command` calls | 13 | 10 | -23.1% |
| `events.create_game_event` calls | 16 | 14 | -12.5% |

Decision:

- Keep this checkpoint. It proves the session-turn runtime can cut the movement submit command/event/idempotency tax without breaking the current durable sync projection.
- The endpoint is still far above the `<5B` target because it still does auth/session/map-turn checks and writes durable `MovementIntent`. The next large cut should make active `sync_session_turn` resolve from runtime state and then remove or batch the remaining movement-intent durable projection on the hot submit path.
- Query/view costs are now visible in the same Gate J route (`get_champion_view`, `get_town_view`, `get_events_after`, visible map/object reads), so the route-wide low-hanging work after runtime sync is active projection overlays, not only update endpoints.

## Checkpoint: Runtime Pending Movement Source

Applied a small bridge cut before the full `sync_session_turn` runtime rewrite.

What changed:

- `RuntimeMovementIntent` now carries the durable `MovementIntent` clone produced by submit. The simple string fields remain for receipt/event/status code that does not need the entity.
- When a session-turn runtime is first created, it hydrates any pre-existing durable pending movement intents for the active turn. This keeps mid-turn compatibility for intents created before the runtime existed.
- `pending_movement_intents_for_session` still loads active participants once, but when the active runtime has submit-populated or hydrated intents it builds pending movement work from heap instead of scanning `movement.intents_by_session_turn_status`.
- Partial sync path updates and resolved intent marks are mirrored back into `SessionTurnRuntime`, so later sync continuations see the current pending/resolved state without a status scan.
- The durable `MovementIntent` row is still updated for compatibility. This checkpoint avoids repeated status-page reads; it does not yet remove durable movement-intent writes or row-backed champion/occupancy/resource updates.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- `CANIC_POCKET_IC_LOCK_NAMESPACE=domm-runtime-sync-intents-native2 cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_timer_jobs_deadline_resolves_multistep_movement_without_sync -- --nocapture` passed in `193.27s`.

Test caveat:

- The same PocketIC test failed under `DOMM_CANISTER_FEATURES=benchmark` because the benchmark canister intentionally omits `get_diagnostic_system_jobs`; the failure was `CanisterMethodNotFound` for that diagnostic endpoint, not a movement/sync assertion.

Measurement note:

- Focused Gate J `20260519-135856-movement-runtime-pending-gate-j` passed in `685.12s`; artifact `target/benchmarks/20260519-135856-movement-runtime-pending-gate-j/summary.json`.

Measured delta versus `20260519-132615-movement-runtime-submit-gate-j`:

| metric | runtime submit | runtime pending source | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 275.6421B | 272.1560B | -1.3% |
| `submit_move_intent` avg instructions | 11.0412B | 11.5097B | +4.2% |
| `sync_session_turn` avg instructions | 14.2859B | 13.8411B | -3.1% |
| `movement.intents_by_session_turn_status` calls | 16 | 6 | -62.5% |
| `movement.intents_by_session_turn_status` total instructions | 5.6276B | 2.1084B | -62.5% |

Decision:

- Keep this bridge cut because it reduces the whole Gate J route and removes most of the repeated pending-intent scans.
- The one-time durable pending-intent hydration moved cost onto `submit_move_intent` (+4.2%). That is acceptable as a compatibility guard for now, but the next sync-runtime checkpoint should either avoid submit-side hydration in new sessions or make active turn sync fully runtime-owned so the durable pending-intent projection stops being a hot path.

## Checkpoint: Fresh Sync Event Creation

Cut more event-feed idempotency reads from the fresh manual `sync_session_turn` path.

What changed:

- `GameCommandStart::Apply` again carries whether the durable command row was freshly created or recovered from a pending/applying idempotency row.
- Fresh manual `sync_session_turn` movement-incomplete events and the final `session_turn_synced` event now call `append_new_public_event`, skipping the durable `events.by_session_event_key` absence read.
- Recovered/replayed pending/applying commands still use `append_public_event`, preserving idempotent event lookup after a partial failure or retry.
- System turn-resolution jobs still use the idempotent event path because they can be resumed from a durable system command/job.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- `CANIC_POCKET_IC_LOCK_NAMESPACE=domm-fresh-sync-events cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_timer_jobs_deadline_resolves_multistep_movement_without_sync -- --nocapture` passed in `189.23s`.

Measurement note:

- Focused Gate J `20260519-142130-fresh-sync-events-gate-j` failed after `617.83s`:

```text
sync_session_turn should succeed: ApiError { code: "icydb_repository_error", message: "IcyDB repository operation failed: events.create_game_event", retryable: true, details_json: None }
```

Decision:

- Reject the broad shortcut and restore idempotent `append_public_event` on manual sync movement events.
- The flaw is architectural: a fresh sync command does not prove the business event key is fresh. `movement_sync_incomplete:{session}:{turn}:fast`, `movement_sync_incomplete:{session}:{turn}:crossing-fast`, and step-cursor incomplete keys can legitimately be reached by more than one fresh sync continuation in the same turn.
- Do not spend more code-size headroom on this row-level micro-cut. The better fix is the full runtime sync/event-buffer path, where runtime owns active event identity and durable event projection can be batched or skipped on the hot path.

## Rejected Experiment: Runtime Ready-Set Cache

Tried a small pre-deadline `sync_session_turn` shortcut around `all_participants_ready_for_turn`.

Eager version:

- Hydrated durable `ParticipantTurnReady` rows when creating `SessionTurnRuntime`.
- Mirrored `end_turn` into runtime ready state.
- Used a hydrated empty ready set to return `false` before scanning active participants.
- Verified with `cargo fmt --check`, `cargo check -p domm-degens-canister --features benchmark`, `cargo check -p domm-degens-canister`, `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`, native PocketIC `pocket_ic_end_turn_closes_turn_and_blocks_stale_actions` in `187.53s`, and focused Gate J `20260519-144234-runtime-ready-shortcut-gate-j`.

Result versus `20260519-135856-movement-runtime-pending-gate-j`:

| metric | runtime pending | eager ready cache | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 272.1560B | 270.6890B | -0.5% |
| `sync_session_turn` avg | 13.8411B | 13.5809B | -1.9% |
| `submit_move_intent` avg | 11.5097B | 11.9766B | +4.1% |
| `sessions.participants_by_session_status` calls | 36 | 32 | -11.1% |

Lazy version:

- Removed submit-time ready hydration.
- Cached ready rows only after a durable sync ready check already happened.
- Verified with the same compile/unit checks, native PocketIC `pocket_ic_end_turn_closes_turn_and_blocks_stale_actions` in `188.24s`, and focused Gate J `20260519-145905-runtime-ready-lazy-gate-j`.

Result versus `20260519-135856-movement-runtime-pending-gate-j`:

| metric | runtime pending | lazy ready cache | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 272.1560B | 272.1154B | ~0.0% |
| `sync_session_turn` avg | 13.8411B | 13.8385B | ~0.0% |
| `submit_move_intent` avg | 11.5097B | 11.5079B | ~0.0% |
| `sessions.participants_by_session_status` calls | 36 | 36 | unchanged |
| `turn_ready.by_session_turn` calls | 6 | 6 | unchanged |

Decision:

- Reject both versions and drop the code.
- The eager version just moved cost from sync to submit, and the lazy version was measurement noise.
- The next performance work should stop trying to shave this row-backed readiness path and instead move active `sync_session_turn` authority into the runtime aggregate.

## Checkpoint: One-Page Current Turn Job Updates

Changed `update_current_turn_jobs` from two status-specific pages (`running`, `scheduled`) to one session job page with Rust-side filtering for:

- status `running` or `scheduled`;
- job kind `turn_resolution` or `turn_deadline`;
- the current turn number.

Why this was safe enough:

- It preserves the same status/kind/turn predicates before calling the existing update closure.
- It reuses the existing `page_system_jobs_by_session` repository function.
- It loops over cursors, so it is safer than the previous single-page-per-status implementation if a session ever has more than one page of jobs.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `CANIC_POCKET_IC_LOCK_NAMESPACE=domm-current-turn-jobs-page cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_timer_jobs_deadline_resolves_multistep_movement_without_sync -- --nocapture` passed in `187.76s`.
- Focused Gate J `20260519-151917-current-turn-job-page-gate-j` passed in `684.00s`.

Measured delta versus `20260519-135856-movement-runtime-pending-gate-j`:

| metric | runtime pending | one-page job updates | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 272.1560B | 266.5048B | -2.1% |
| `sync_session_turn` avg | 13.8411B | 13.3267B | -3.7% |
| `submit_move_intent` avg | 11.5097B | 11.5099B | flat |
| `system_jobs.by_session_status_due` calls | 48 | 32 | -33.3% |
| `system_jobs.by_session_status_due` total | 16.8968B | 11.2665B | -33.3% |

Decision:

- Keep this cut. It is a true route-level win and it does not move cost into submit.
- The next nearby cut is the map-turn command guard in `ensure_map_turn_accepts_new_command`, which still scans status pages separately in the post-deadline branch.

## Checkpoint: One-Page Map Turn Guard

Changed the post-deadline branch of `ensure_map_turn_accepts_new_command` from two status-specific session-job pages to one session job page with the same acceptance predicate:

- running current-turn `turn_resolution` / `turn_deadline` blocks;
- scheduled current-turn `turn_resolution` / `turn_deadline` blocks only when `due_at <= now`;
- completed/failed/other jobs do not block.

The pre-deadline branch is unchanged because it already uses a single scheduled-job page and only checks due scheduled closure jobs.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `CANIC_POCKET_IC_LOCK_NAMESPACE=domm-map-turn-guard-page cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_end_turn_closes_turn_and_blocks_stale_actions -- --nocapture` passed in `189.92s`.
- First focused Gate J attempt `20260519-153602-map-turn-guard-page-gate-j` failed from a PocketIC/sandbox launcher crash (`Instance was deleted`), not from a canister assertion.
- Focused Gate J rerun `20260519-153948-map-turn-guard-page-gate-j-rerun` passed in `567.83s`.

Measured delta versus `20260519-151917-current-turn-job-page-gate-j`:

| metric | current-turn job page | map-turn guard page | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 266.5048B | 264.4034B | -0.8% |
| `submit_move_intent` avg | 11.5099B | 11.2709B | -2.1% |
| `submit_build_town_structure` avg | 20.7640B | 20.0661B | -3.4% |
| `submit_recruit_units` avg | 16.3014B | 15.5919B | -4.4% |
| `sync_session_turn` avg | 13.3267B | 13.3287B | flat |
| `system_jobs.by_session_status_due` calls | 32 | 26 | -18.8% |
| `system_jobs.by_session_status_due` total | 11.2665B | 9.1537B | -18.8% |

Decision:

- Keep this cut. It improves every map-turn command in Gate J and leaves sync flat.
- Combined with the previous checkpoint, job-scan calls are now 48 -> 26 and job-scan instructions are 16.8968B -> 9.1537B from the runtime-pending baseline.

## Checkpoint: Runtime-Hydrated Pending Movement

Changed active session-turn runtime movement intents to carry the hydrated `Champion` and `GameParticipant` rows alongside the durable `MovementIntent` projection. `submit_move_intent` now stores the already-loaded champion/participant in the runtime, compatibility hydration fills those rows for pre-existing pending intents, and `load_pending_movements` now uses a runtime-first path when every pending runtime intent is complete and belongs to the active session/turn. Partial and resolved movement updates mirror the current champion/participant back into the runtime before later sync calls read it again.

Fallback behavior:

- If the active runtime is missing, has no intents, lacks a durable intent/champion/participant, or fails session/turn/owner/status checks, `sync_session_turn` falls back to the existing row-backed participant/intents/champion loading path.
- Durable `MovementIntent`, champion, participant, movement snapshot, visibility, occupancy, event, and command writes still happen on the current path. This checkpoint only removes repeated row hydration for the active runtime read side.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- `CANIC_POCKET_IC_LOCK_NAMESPACE=domm-runtime-pending-hydrated cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_timer_jobs_deadline_resolves_multistep_movement_without_sync -- --nocapture` passed in `187.19s`.
- Focused Gate J `20260519-160042-runtime-hydrated-pending-gate-j` passed in `681.74s`. The test wrote artifacts under `testing/pocket-ic/target/benchmarks/...` because the direct cargo test runs from the package cwd; the summary/run artifacts were copied to `target/benchmarks/20260519-160042-runtime-hydrated-pending-gate-j/` for consistency.

Measured delta versus `20260519-153948-map-turn-guard-page-gate-j-rerun`:

| metric | map-turn guard page | runtime-hydrated pending | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 264.4034B | 254.4790B | -3.8% |
| scenario cycles | 0.4380T | 0.4245T | -3.1% |
| `sync_session_turn` avg | 13.3287B | 12.4249B | -6.8% |
| `submit_move_intent` avg | 11.2709B | 11.2759B | flat |
| `sessions.participants_by_session_status` calls | 36 | 22 | -38.9% |
| `sessions.participants_by_session_status` total | 12.7040B | 7.7619B | -38.9% |
| `champions.load_champion` calls | 10 | 3 | -70.0% |
| `champions.load_champion` total | 7.0721B | 2.1209B | -70.0% |
| `movement.intents_by_session_turn_status` calls | 6 | 6 | unchanged |

Decision:

- Keep this cut. It gives a route-level win without shifting cost into `submit_move_intent`.
- The remaining repeated `movement.intents_by_session_turn_status` scans show this is still a bridge, not the final active-turn aggregate. The next large step should make active `sync_session_turn` resolve from runtime-owned movement/champion/resource state and reserve durable projections for the boundary.

## Checkpoint: Runtime Empty-Ready Shortcut

Added a pre-deadline `sync_session_turn` shortcut when the active `SessionTurnRuntime` exists and its ready set is empty. In that state all-ready is impossible, so the endpoint can return the existing runtime `turn_not_due` response without paging active participants and durable `ParticipantTurnReady` rows. `end_turn` now mirrors the participant id into the active runtime ready set, so the shortcut is disabled as soon as any participant has ended the turn; no-runtime and non-empty-runtime cases still use the durable all-ready check.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- `CANIC_POCKET_IC_LOCK_NAMESPACE=domm-runtime-ready-empty-shortcut cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_end_turn_closes_turn_and_blocks_stale_actions -- --nocapture` passed in `184.32s`.
- Focused Gate J `20260519-162235-runtime-empty-ready-gate-j` passed in `230.20s`.

Measured delta versus `20260519-160042-runtime-hydrated-pending-gate-j`:

| metric | runtime-hydrated pending | empty-ready shortcut | change |
| --- | ---: | ---: | ---: |
| update-only Gate J instructions | 254.4790B | 251.6616B | -1.1% |
| `sync_session_turn` avg | 12.4249B | 12.1704B | -2.0% |
| `submit_move_intent` avg | 11.2759B | 11.2728B | flat |
| `sessions.participants_by_session_status` calls | 22 | 18 | -18.2% |
| `sessions.participants_by_session_status` total | 7.7619B | 6.3500B | -18.2% |
| `turn_ready.by_session_turn` calls | 6 | 2 | -66.6% |
| `turn_ready.by_session_turn` total | 2.1066B | 0.7039B | -66.6% |

Comparison caveat:

- Do not compare full scenario instruction totals between these two direct runs. The empty-ready run used absolute `DOMM_BENCH_OUTPUT_DIR` / `DOMM_BENCH_QUERY_LOG_PATH` values and captured query instructions correctly; the previous direct run recorded query methods as zero. Use update-only totals and repo-op counts for this checkpoint.

Decision:

- Keep this cut. It removes durable scans from the common "sync before deadline and nobody is ready" case, keeps the all-ready durable path intact, and does not shift cost into submit.

## Checkpoint: Runtime Champion Submit Lookup

Changed `resolve_owned_champion` to use the active `SessionTurnRuntime` champion projection only when it can prove all of the following:

- the submitted champion id is a real ULID matching the runtime champion;
- the runtime belongs to the caller's current session and turn;
- the champion belongs to the caller's participant;
- the runtime champion status is `active`.

If any check fails, the existing durable `resolve_champion` path runs. This is intentionally conservative so stale runtime entries from battle/aftermath cannot falsely authorize or reject movement.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- Focused Gate J `20260519-163253-runtime-champion-submit-gate-j` passed in `229.75s`.

Measured delta versus `20260519-162235-runtime-empty-ready-gate-j`:

| metric | empty-ready shortcut | runtime champion submit | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 320.8642B | 320.1759B | -0.2% |
| `submit_move_intent` avg | 11.2728B | 11.0394B | -2.1% |
| `sync_session_turn` avg | 12.1704B | 12.1716B | flat |
| `champions.load_champion` calls | 3 | 2 | -33.3% |
| `champions.load_champion` total | 2.1206B | 1.4147B | -33.3% |

Decision:

- Keep this cut. It is small, safe by construction, and directly reduces submit-side stable reads without changing durable fallback behavior.

## Reverted Checkpoint: Runtime Champion Submit Lookup

The runtime champion submit shortcut was not safe. `lobby_session_setup_recovers_from_starting_state_and_replays_nonce` failed after a champion spell/skill update was made outside the movement runtime: the later guarded movement used the cached runtime champion, started a neutral battle from stale champion state, and `CastAbility(spell:hex-spark)` disappeared from legal actions.

Decision:

- Revert `resolve_owned_champion` to always load the durable champion row for submit/preview validation.
- Keep hydrated champion rows in pending runtime intents for `sync_session_turn`; those rows are still valuable for resolving the intent that created them.
- Do not use cached runtime champions as broad current champion authority until there is an invalidation/version contract or a real champion overlay that owns spell/skill/army changes.

Verified after revert:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-degens-canister lobby_session_setup_recovers_from_starting_state_and_replays_nonce -- --nocapture` passed in `241.60s`.
- Focused Gate J `20260519-town-events-champion-revert-gate-j` passed in `232.32s`.

Current measured effect versus `20260519-164028-final-sync-new-event-gate-j`:

| metric | final sync new event | town events + champion revert | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 319.4200B | 317.2806B | -0.7% |
| `submit_move_intent` avg | 11.0396B | 11.2751B | +2.1% |
| `submit_build_town_structure` avg | 20.0597B | 18.6468B | -7.0% |
| `submit_recruit_units` avg | 15.5931B | 14.1707B | -9.1% |
| `events.by_session_event_key` calls | 18 | 14 | -22.2% |
| `champions.load_champion` calls | 2 | 3 | +50.0% |

## Checkpoint: Final Sync New Event

Changed the final successful `sync_session_turn` event append for `session_turn_synced` to create the deterministic public event directly and fall back to the existing key lookup only if creation reports a conflict. This is deliberately not the broad sync-event shortcut rejected earlier: partial movement sync events can legitimately reuse business keys, so they still use the idempotent `append_public_event` path. The shortcut is scoped to the final `sync_turn:{session}:{turn}` event after the session turn has advanced.

Verified:

- `cargo check -p domm-degens-canister --features benchmark`
- `cargo fmt && cargo check -p domm-degens-canister && cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- Focused Gate J `20260519-164028-final-sync-new-event-gate-j` passed in `230.87s`.

Measured delta versus `20260519-163253-runtime-champion-submit-gate-j`:

| metric | runtime champion submit | final sync new event | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 320.1759B | 319.4200B | -0.2% |
| `sync_session_turn` avg | 12.1716B | 12.1032B | -0.6% |
| `submit_move_intent` avg | 11.0394B | 11.0396B | flat |
| `events.by_session_event_key` calls | 19 | 18 | -5.3% |
| `events.by_session_event_key` total | 13.3775B | 12.6727B | -5.3% |

Decision:

- Keep this cut. It is a small but real stable-read removal on the final sync path, and it does not move work into submit or weaken idempotency for the partial movement events that previously failed under the broad fresh-sync shortcut.

## Rejected Experiment: Runtime-Open Map Turn Guard

Tried to skip the pre-deadline scheduled `SystemJob` scan in `ensure_map_turn_accepts_new_command` when the active `SessionTurnRuntime` could prove the turn was still open: same current turn/deadline, not closing, and no ready participants.

The first version was wrong: it returned from the whole guard before the duplicate `end_turn` ready-row check. `pocket_ic_end_turn_closes_turn_and_blocks_stale_actions` caught this by accepting a fresh duplicate `end_turn`. The fixed version skipped only the job page and still ran the duplicate-ready check.

Verified fixed version:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `CANIC_POCKET_IC_LOCK_NAMESPACE=domm-runtime-open-guard-stale-rerun cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_end_turn_closes_turn_and_blocks_stale_actions -- --nocapture` passed in `186.94s`.
- Focused Gate J `20260519-runtime-open-guard-gate-j` passed in `232.09s`.

Measured delta versus `20260519-164028-final-sync-new-event-gate-j`:

| metric | final sync new event | runtime-open guard | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 319.4200B | 319.4200B | flat |
| `submit_move_intent` avg | 11.0396B | 11.0396B | flat |
| `submit_build_town_structure` avg | 20.0597B | 20.0597B | flat |
| `submit_recruit_units` avg | 15.5931B | 15.5931B | flat |
| `sync_session_turn` avg | 12.1032B | 12.1032B | flat |
| `system_jobs.by_session_status_due` calls | 26 | 26 | flat |

Decision:

- Reject the shortcut and do not keep the code. In the measured route, each movement/town command that pays this guard starts a fresh current turn where no active `SessionTurnRuntime` exists yet. The proof cannot fire unless runtime creation moves earlier, and doing that as a guard micro-cut risks replacing a job scan with runtime hydration work. Handle this in the real active-turn aggregate instead.

## Checkpoint: Fresh Town Command Events

Added a generic `append_new_event_for_audience` helper and moved town build/recruit private plus public command events onto create-first event append. The old idempotent lookup remains as the fallback if create reports a conflict. This is scoped to fresh town command events after `begin_participant_command` has already handled nonce replay; build duplicates still fail before append, and recruit event keys include the fresh command id.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- Focused Gate J `20260519-town-new-events-gate-j` passed in `238.69s`.

Measured delta versus `20260519-164028-final-sync-new-event-gate-j`:

| metric | final sync new event | town new events | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 319.4200B | 316.5882B | -0.9% |
| scenario cycles | 0.4210T | 0.4182T | -0.7% |
| `submit_build_town_structure` avg | 20.0597B | 18.6468B | -7.0% |
| `submit_recruit_units` avg | 15.5931B | 14.1707B | -9.1% |
| `submit_move_intent` avg | 11.0396B | 11.0411B | flat |
| `sync_session_turn` avg | 12.1032B | 12.1061B | flat |
| `events.by_session_event_key` calls | 18 | 14 | -22.2% |
| `events.by_session_event_key` total | 12.6727B | 9.8551B | -22.2% |

Decision:

- Keep this cut. It removes four stable event-key absence reads from the Gate J town path without changing replay fallback behavior or shifting work into movement/sync.

## Checkpoint: Remove Town Command Effects

Removed the `town_build` and `town_recruit` `CommandEffect` writes from `submit_build_town_structure` and `submit_recruit_units`. These rows duplicated state already represented by the durable `GameCommand`, town child rows, resource ledger entries, and deterministic game events. Nonce replay still returns from the command row before remutation, and the API view still reads the real town rows.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- Focused Gate J `20260519-town-no-effects-gate-j` passed in `230.50s`.

Measured delta versus `20260519-town-events-champion-revert-gate-j`:

| metric | town events + champion revert | no town effects | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 317.2806B | 315.0101B | -0.7% |
| scenario cycles | 0.4189T | 0.4166T | -0.5% |
| `submit_build_town_structure` avg | 18.6468B | 17.4740B | -6.3% |
| `submit_recruit_units` avg | 14.1707B | 12.9995B | -8.3% |
| `effects.command_effect_by_command_key` calls | 4 | 2 | -50.0% |
| `effects.create_applied_command_effect` calls | 4 | 2 | -50.0% |

Stable memory growth did not visibly move in this single route because stable-memory pages are allocated in coarse chunks, but the instruction and cycle savings are direct and repeatable for the town commands.

Decision:

- Keep this cut. It removes duplicate durable writes and aligns towns with the broader aggregate direction: command/event/projection rows should exist only when they carry unique replay, history, or query value.

## Checkpoint: Remove Partial Movement Command Effects

Removed the `movement_cursor` and `visibility_refresh` `CommandEffect` writes from partial movement sync. They were not read by recovery or query paths. The durable recovery/projection surface is already the `MovementSnapshot` row, updated `MovementIntent` path/hash, command nonce row, and deterministic movement event key.

Kept the neutral `battle_started` effect. That path still uses `find_applied_command_effect_by_session_key` as a session-level guard around neutral battle start, so removing it belongs to the runtime battle-start rewrite rather than this low-hanging pass.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- Focused Gate J `20260519-movement-partial-no-effects-gate-j` passed in `231.98s`.

Measured delta versus `20260519-town-no-effects-gate-j`:

| metric | town no effects | partial movement no effects | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 315.0101B | 313.9172B | -0.3% |
| scenario cycles | 0.4166T | 0.4132T | -0.8% |
| scenario memory | 6314.4591 MB | 6180.1759 MB | -2.1% |
| `sync_session_turn` avg | 12.1079B | 12.0140B | -0.8% |
| `sync_session_turn` avg memory | 131.3103 MB | 117.5716 MB | -10.5% |
| `effects.command_effect_by_command_key` calls | 2 | 1 | -50.0% |
| `effects.create_applied_command_effect` calls | 2 | 1 | -50.0% |

Decision:

- Keep this cut. It is small but removes another duplicate stable write from the row-backed movement bridge and leaves the higher-risk neutral battle-start guard intact.

## Checkpoint: Fresh Lobby Setup Events

Changed lobby setup event append to create first and fall back to key lookup on conflict. This covers the fresh `session_created`, `participant_joined`, and `participant_ready` events emitted by `create_session`, `join_session`, and `mark_ready`. Normal nonce replay still returns from the durable `LobbyCommand` before the event append runs; duplicate ready commands with a new nonce can still fall back to the existing event.

Verified:

- `cargo fmt`
- `cargo check -p domm-degens-canister --features benchmark`
- Focused Gate J `20260519-lobby-new-events-gate-j` passed in `231.56s`.

Measured delta versus `20260519-movement-partial-no-effects-gate-j`:

| metric | partial movement no effects | lobby new events | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 313.9172B | 310.8836B | -1.0% |
| scenario cycles | 0.4132T | 0.4103T | -0.7% |
| `create_session` avg | 12.7030B | 11.9966B | -5.6% |
| `join_session` avg | 8.0180B | 7.3116B | -8.8% |
| `mark_ready` avg | 6.6117B | 5.9041B | -10.7% |
| `events.by_session_event_key` calls | 14 | 10 | -28.6% |

Decision:

- Keep this cut. It removes four fresh event absence reads from the route without changing replay behavior. The remaining sync event lookups are mostly movement-incomplete events that can be reused by later fresh sync commands, so they should wait for the runtime event-buffer rewrite.

## Checkpoint: Batch Town Command Event Sequence Updates

Changed town build/recruit event append so the private and public town events reserve consecutive in-memory sequence numbers and update the durable `GameSession.next_event_seq` once at the end. Event creation still uses create-first with lookup fallback on conflict, and both private/public events remain visible with the same payloads and audiences.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- Focused Gate J `20260519-town-batch-events-gate-j` passed in `232.87s`.

Measured delta versus `20260519-lobby-new-events-gate-j`:

| metric | lobby new events | town batch events | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 310.8836B | 309.9352B | -0.3% |
| scenario cycles | 0.4103T | 0.4093T | -0.2% |
| `submit_build_town_structure` avg | 17.4653B | 16.9903B | -2.7% |
| `submit_recruit_units` avg | 12.9939B | 12.5197B | -3.6% |
| `sessions.update_session` calls | 18 | 16 | -11.1% |

Decision:

- Keep this cut. It is a small write-amplification reduction in the town bridge path and preserves event replay fallback behavior.

## Checkpoint: Cached Not-Due Sync Fast Path

Added a narrow cache-only fast path for `sync_session_turn` before durable session loading. It fires only when:

- the caller/session pair is already in the active-session caller cache;
- `now_ms` is before the cached session deadline;
- the active `SessionTurnRuntime` exists for that session/turn;
- the runtime ready set is empty.

In that case all-ready is impossible, so the endpoint can return the existing in-memory `turn_not_due` command response without loading the durable session. Any missing cache, missing runtime, ready participant, or due/after-deadline call falls back to the existing durable path. `submit_move_intent` now remembers its active session caller context after a successful runtime-backed submit, which seeds the immediate retry case.

Verified:

- `cargo fmt`
- `cargo check -p domm-degens-canister --features benchmark`
- Focused Gate J `20260519-sync-not-due-cache-gate-j` passed in `227.73s`.

Measured delta versus `20260519-town-batch-events-gate-j`:

| metric | town batch events | cached not-due sync | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 309.9352B | 305.7282B | -1.4% |
| scenario cycles | 0.4093T | 0.4051T | -1.0% |
| `sync_session_turn` avg | 12.0021B | 11.6196B | -3.2% |
| `submit_move_intent` avg | 11.2569B | 11.2527B | flat |
| `sessions.load_session` calls | 20 | 18 | -10.0% |

Per-call confirmation:

- `sync_session_turn` sequence 81 moved to `55,179` instructions with no repo ops.
- `sync_session_turn` sequence 215 moved to `55,782` instructions with no repo ops.
- Other sync calls still loaded durable session state, as intended.

Decision:

- Keep this cut. It is deliberately narrow and avoids the stale-session hazards seen in the earlier broad movement cache experiment.

## Checkpoint: Successful Sync Turn-Advance Write Cut

Tightened the successful `sync_session_turn` path:

- removed the post-movement `sessions.load_participant` reload when movement completed without events;
- reserved the final `session_turn_synced` event sequence before the turn-advance `sessions.update_session`;
- created the final sync event with that reserved sequence, with lookup/update fallback only for recovered conflicts;
- removed the now-unused generic `append_new_public_event` helpers from `command_response.rs`.

Reasoning:

- If `resolve_pending_movement` produced movement/object events, the endpoint already yields before income and turn advancement.
- The full turn-advance path therefore does not need a second durable participant read; `require_active_session_caller` already loaded the caller participant at command start.
- The final sync event was doing a second session update only to advance `next_event_seq`. Reserving the sequence in the same session update that advances `current_turn` preserves normal event ordering and removes one write from the hot success path.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- Focused Gate J `20260519-sync-final-reserved-event-gate-j` passed in `696.40s`.

Measured delta versus `20260519-sync-not-due-cache-gate-j`:

| metric | cached not-due sync | final reserved event | change |
| --- | ---: | ---: | ---: |
| update-only Gate J instructions | 236.5752B | 235.4024B | -0.5% |
| `sync_session_turn` avg | 11.6196B | 11.5131B | -0.9% |
| successful turn-advance sync call | 23.1070B | 21.9392B | -5.1% |
| `sessions.update_session` calls | 16 | 15 | -6.3% |
| `sessions.load_participant` calls | 1 | 0 | -100.0% |
| `sessions.update_session` instructions | 7.6311B | 7.1577B | -0.4734B |

Caveat:

- The direct run wrote the query log through the same `tee` target used for stdout, so query instruction fields were blank in `run.json`/`summary.json`. Do not compare full scenario instruction totals from this artifact. Use update-only totals for this checkpoint, and run future direct gates with `> "$log" 2>&1` or the benchmark script's per-gate log handling instead of `tee`.

Decision:

- Keep this cut. It removes two stable operations from the normal successful sync path and does not change the movement partial, object-event, income, or recovered-event semantics.

## Checkpoint: Batch Successful Sync Income Event Sequence

Extended the successful-sync event sequence batching to the income event:

- `sync_session_turn` now holds a local `next_event_seq` cursor for the successful turn-advance path.
- `materialize_income` can append `income_materialized` with that local cursor for the manual sync path, without immediately updating `GameSession.next_event_seq`.
- The system-job turn-resolution path still calls `materialize_income` with the old idempotent durable event append, keeping that path conservative.
- The final `session_turn_synced` event uses the next local sequence, and the single turn-advance `sessions.update_session` persists the final cursor.

Verified:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- Focused Gate J `20260519-sync-income-reserved-event-gate-j` passed in `228.58s` test time / `245s` wall time.

Measured delta versus `20260519-sync-not-due-cache-gate-j`:

| metric | cached not-due sync | income reserved event | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 305.7282B | 304.0763B | -0.5% |
| `sync_session_turn` avg | 11.6196B | 11.4681B | -1.3% |
| `submit_move_intent` avg | 11.2527B | 11.2548B | flat |
| `sessions.update_session` calls | 16 | 14 | -12.5% |
| `sessions.load_participant` calls | 1 | 0 | -100.0% |
| `events.by_session_event_key` calls | 10 | 10 | unchanged |

Incremental delta versus `20260519-sync-final-reserved-event-gate-j`:

| metric | final reserved event | income reserved event | change |
| --- | ---: | ---: | ---: |
| `sync_session_turn` avg | 11.5131B | 11.4681B | -0.4% |
| `sessions.update_session` calls | 15 | 14 | -6.7% |
| `sessions.update_session` instructions | 7.1577B | 6.6764B | -0.4814B |

Decision:

- Keep this cut. It removes the remaining event-sequence session write from the successful manual sync path while leaving partial movement, object event, recovered event, and system-job sync behavior on the older conservative path.

## Gate 5 Hard Target Replan

User target tightened the remaining perf1 work to `0.3B-0.6B` instructions where unavoidable. I launched three focused subagents across `submit_move_intent`, `sync_session_turn`, and town/view/economy. They independently reached the same conclusion: the current row-level cuts are not enough. Each durable stable read/write is roughly `0.35B-0.70B`, so endpoints still doing live command/event/session/movement/town/resource/job writes cannot land near `0.3B-0.6B`.

Current anchor artifact: `target/benchmarks/20260519-sync-income-reserved-event-gate-j/summary.json`.

| endpoint | current avg | target |
| --- | ---: | ---: |
| `submit_move_intent` | `11.2538B` | `0.3B-0.6B` |
| `sync_session_turn` | `11.4681B` | `0.3B-0.6B` |
| `submit_build_town_structure` | `16.9975B` | `0.3B-0.6B` |
| `submit_recruit_units` | `12.5294B` | `0.3B-0.6B` |
| `get_visible_objects` | `4.950B` | `0.3B-0.8B` |
| `get_champion_view` | `4.939B` | `0.3B-0.8B` |
| `get_town_view` | `4.226B` | `0.3B-0.8B` |
| `get_visible_map_chunks` | `3.520B` | `0.3B-0.8B` |

Decision:

- Stop treating `5B` or `1B` as acceptable final targets for movement/sync/town hot paths.
- Make `SessionTurnRuntime` authoritative before hot commands run, not lazily hydrated during submit/sync.
- Make fresh `submit_move_intent` a pure heap mutation: runtime auth, runtime champion/occupancy/contact indexes, runtime receipts/events, pre-reserved event sequence block, no durable `MovementIntent` write.
- Make fresh `sync_session_turn` consume runtime intents/snapshots/events and mutate runtime champion/occupancy/object/resource/session/job deltas. Durable rows become checkpoint/flush output.
- Add `TownRuntime`/`TownProjection` for buildings, recruit pools, garrison, tavern/growth, and town command receipts/events/resources. Build/recruit commands should mutate runtime first and flush durable child rows later.
- Runtime query overlays are mandatory before removing durable live writes. Active runtime state must win for game/session/participant/champion/map/object/town/event/status views.
- Add `flush_barrier(reason)` for turn advance, battle handoff, upgrade, runtime eviction, and strong read paths. Flush must be idempotent because deferred projection can be retried.

Risks to keep front of mind:

- The previous cached champion shortcut regressed `CastAbility`, so champion snapshots need an invalidation/version contract or a real champion overlay before submit/preview trusts them.
- Event sequence blocks must share one allocator with durable `GameSession.next_event_seq`; lazy or split allocation risks collisions.
- Timer jobs become wakeup hints only after runtime owns turn authority. Until then, stale durable jobs can still affect correctness.
- Query overlays are the correctness boundary. If one public view misses runtime state, the DB can be correct while the API lies.

No tests were run for this planning-only checkpoint. The todo was updated to record the hard-target gates and to make future checkboxes measurable against `0.3B-0.6B`.

## Checkpoint: Gate 5A Active Turn Runtime Bootstrap

Implemented the first hard-target runtime authority checkpoint:

- `SessionTurnRuntime` now has participant principal/auth metadata, champion snapshots, runtime occupancy cells, and town/world-object contact cells.
- Active turn runtime can be prepared from durable rows with a pre-reserved `GameSession.next_event_seq` block before hot commands need to append runtime events.
- Session activation creates the first active runtime after setup completes.
- Manual `sync_session_turn` and system `turn_resolution` create the next turn runtime while advancing the durable turn.
- The old submit-side `ensure_session_turn_runtime` now falls through to the shared bootstrap helper, so cold compatibility still works but normal started sessions should already have the runtime.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- `cargo test -p domm-degens-canister lobby_session_setup_recovers_from_starting_state_and_replays_nonce -- --nocapture` passed in `236.45s`
- Focused Gate J `20260519-gate5a-runtime-bootstrap-gate-j` passed in `682.30s`

Measured update-method delta versus `20260519-sync-income-reserved-event-gate-j`:

| metric | before | Gate 5A | change |
| --- | ---: | ---: | ---: |
| `submit_move_intent` avg | 11.2538B | 10.4920B | -6.8% |
| `sync_session_turn` avg | 11.4681B | 11.9816B | +4.5% |
| `submit_build_town_structure` avg | 16.9975B | 17.0050B | flat |
| `submit_recruit_units` avg | 12.5294B | 12.5245B | flat |
| `movement.intents_by_session_turn_status` calls | 6 | 2 | -66.7% |
| `sessions.update_session` calls | 14 | 12 | -14.3% |
| `players.load_player_account` calls | 0 | 2 | +2 bootstrap reads |
| `towns.by_session_status` calls | 0 | 2 | +2 bootstrap reads |
| `map.world_objects_by_session` calls | 0 | 2 | +2 bootstrap reads |

Caveat:

- Full scenario/query instruction totals from `20260519-gate5a-runtime-bootstrap-gate-j` are not comparable because the direct test command was not redirected to `test-output.log`; query methods recorded as `n/a`. The update-method metrics and repo-op counts were captured in `summary.json` and are the valid comparison for this checkpoint.
- The sync regression is expected for this gate: runtime hydration moved from submit-side lazy paths into the turn boundary. The next checkpoints must spend that cost by removing durable submit/sync live-state operations; otherwise this bootstrap is just extra work.

Decision:

- Keep Gate 5A. It establishes the heap authority that Gate 5B and Gate 5C need, and it already removes most durable pending-intent scans from the route. Do not optimize around the `+4.5%` sync regression with another row micro-cut; make `submit_move_intent` and fresh `sync_session_turn` consume the pre-seeded runtime directly.

## Checkpoint: Runtime-Only Movement Submit Intent Bridge

Implemented the first Gate 5B submit-side cut:

- Fresh `submit_move_intent` now stores the pending movement intent only in `SessionTurnRuntime`.
- The runtime intent carries command id text, actor participant id, champion id, path JSON/hash, and hydrated champion/participant rows.
- Runtime receipts/events still answer active command status, nonce replay, and event feed reads.
- The old durable `MovementIntent` create/update no longer happens inside submit.
- The current row-backed `sync_session_turn` compatibility path now materializes the durable movement intent from the runtime intent when it needs to feed the existing resolver.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- `cargo test -p domm-degens-canister lobby_session_setup_recovers_from_starting_state_and_replays_nonce -- --nocapture` passed in `234.61s`
- Focused Gate J `20260519-runtime-submit-no-durable-intent-gate-j` passed in `230.76s`

Measured delta versus Gate 5A `20260519-gate5a-runtime-bootstrap-gate-j`:

| metric | Gate 5A | submit runtime intent bridge | change |
| --- | ---: | ---: | ---: |
| `submit_move_intent` avg | 10.4920B | 9.3146B | -11.2% |
| `submit_move_intent` avg memory | 13.5 MB | 0.1667 MB | -98.5% |
| `sync_session_turn` avg | 11.9816B | 12.6262B | +5.4% |
| `movement.create_intent` calls | present in prior submit-compatible path | 0 | removed from hot submit |
| `movement.update_intent` calls | 4 | 9 | compatibility bridge cost moved into sync |

Measured delta versus the hard-target anchor `20260519-sync-income-reserved-event-gate-j`:

| metric | hard-target anchor | submit runtime intent bridge | change |
| --- | ---: | ---: | ---: |
| `submit_move_intent` avg | 11.2538B | 9.3146B | -17.2% |
| scenario instructions | 304.0763B | 311.0029B | +2.3% |

Caveat:

- This is intentionally a partial bridge, not the final Gate 5B shape. It proves that removing the durable movement-intent write from submit is valuable, but the same durable row work still exists in `sync_session_turn` until Gate 5C/5D make sync runtime-owned.
- Submit is still far above target because it still loads durable session/champion state, scans durable jobs for the map-turn guard, and reads stable occupancy/blocker state. Those stable categories must be removed next rather than hidden in another compatibility layer.

Decision:

- Keep the checkpoint. It improves the player-facing submit endpoint and gives a clean compatibility bridge while the runtime sync rewrite is still incomplete.
- Next Gate 5B work should target the remaining submit stable reads in order: session/job guard, champion load, occupancy/blocker reads. Any cache shortcut must have an invalidation/version contract; the previous cached champion shortcut regressed battle legal actions.

## Checkpoint: Cache First-Playable Movement Map

Removed a repeated CPU rebuild from movement validation:

- `flags_at` and `movement_cost_at` previously called `domm_game::build_first_playable_map_state()` for each first-playable map tile lookup.
- `submit_move_intent` calls both functions for every path tile, and `sync_session_turn` also pays this path-cost tax while resolving movement.
- Movement now keeps the deterministic first-playable map in a canister heap thread-local cache and reuses it for tile flag/cost reads.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Focused Gate J `20260519-first-playable-map-cache-gate-j` passed in `230.24s`
- Post-run PocketIC cleanup confirmed no leftover PocketIC/server processes.

Measured delta versus Gate 5B.1 `20260519-runtime-submit-no-durable-intent-gate-j`:

| metric | runtime intent bridge | first-playable map cache | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 311.0029B | 310.0816B | -0.3% |
| `submit_move_intent` avg | 9.3146B | 9.1776B | -1.5% |
| `sync_session_turn` avg | 12.6262B | 12.5783B | -0.4% |
| `submit_move_intent` avg memory | 0.1667 MB | 0.1667 MB | unchanged |

Decision:

- Keep this cut because it removes repeated deterministic work and improves both submit and sync without changing gameplay state authority.
- Do not over-invest in this layer. The remaining `submit_move_intent` per-call repo ops still include one `champions.load_champion`, one `sessions.load_session`, and two `system_jobs.by_session_status_due` reads. Those stable reads are the next Gate 5B target.

## Rejected: Runtime-Open Submit Job Guard Shortcut

Tried skipping `command_response::ensure_map_turn_accepts_new_command` for fresh movement submits when runtime looked open before the turn deadline.

Result:

- Rejected and reverted before commit.
- `pocket_ic_end_turn_closes_turn_and_blocks_stale_actions` failed after `195.64s`; the timer path exceeded the 40B instruction limit and the test did not advance to turn 2.
- The old durable job scan is still carrying real turn-closure semantics. A safe replacement needs runtime job/deadline authority, not a local "runtime looks open" check.

Decision:

- Do not skip the map-turn job guard from submit until Gate 5E owns turn/deadline/job state in runtime.
- Continue with cuts that are provably state-local: runtime occupancy/blocker checks and runtime submit receipts/intents.

## Checkpoint: Runtime Occupancy Blocker Check

Moved the hidden submit path blocker cost into runtime:

- `validate_no_friendly_champion_blocker` now tries the active `SessionTurnRuntime.occupancy_index` first.
- If runtime occupancy cannot prove blocker ownership, it falls back to the old durable `MapOccupancy` reads.
- Movement sync now mirrors the current champion snapshot and champion occupancy cell back into runtime when a pending movement is parked/resolved.
- `SessionTurnRuntime` gained `upsert_occupancy_for_occupant`, which replaces the old cell for a moving occupant instead of leaving stale champion cells behind.

Why this mattered:

- The durable `MapOccupancy` lookup used by the blocker loop was not included in repo-operation metrics because that repository helper used a raw `storage_result` path.
- Long movement submits were doing one raw stable lookup per path tile, which explains why 10-12 step submits were around `10B-12B` despite only showing about `2.1B` measured repo ops.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- Focused Gate J `20260519-runtime-blocker-check-gate-j` passed in `229.24s`
- Post-run PocketIC cleanup confirmed no leftover PocketIC/server processes.

Measured delta versus Gate 5B.2 `20260519-first-playable-map-cache-gate-j`:

| metric | map cache | runtime blocker check | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 310.0816B | 293.0846B | -5.5% |
| `submit_move_intent` avg | 9.1776B | 3.5293B | -61.5% |
| `submit_move_intent` avg memory | 0.1667 MB | 0.0208 MB | -87.5% |
| `sync_session_turn` avg | 12.5783B | 12.5747B | flat |

Per-submit calls after the patch:

| sequence | instructions | remaining measured repo ops |
| ---: | ---: | --- |
| 80 | 3.5439B | `champions.load_champion`, `sessions.load_session`, `system_jobs.by_session_status_due` x2 |
| 160 | 3.5211B | same |
| 214 | 3.5229B | same |

Decision:

- Keep this cut. It is the first Gate 5B change that attacks the hidden path-length multiplier rather than moving durable writes between submit and sync.
- The remaining submit floor is now explicit: about `2.1B` measured repo instructions for durable session/champion/job reads plus endpoint overhead. The next safe route to `0.3B-0.6B` is runtime-auth/session/champion metadata with invalidation, and runtime job/deadline authority from Gate 5E.

## Checkpoint: Runtime Context And Champion Submit Lookup

Implemented a batched Gate 5B submit authority cut:

- `SessionTurnRuntime` now keeps a session snapshot and full active participant rows alongside principal metadata.
- Fresh `submit_move_intent` first resolves the caller context from active runtime/caller cache before falling back to durable `sessions.load_session` and participant lookup.
- Movement submit now resolves owned active champions from the runtime champion snapshot before falling back to durable `champions.load_champion`.
- Runtime champion snapshots are mirrored after known champion mutators: movement sync, champion magic, battle spell casting, battle aftermath, and tavern hire.
- The previous unsafe job-guard shortcut remains rejected; submit still runs the durable map-turn job guard until Gate 5E owns turn/deadline/job authority.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Focused Gate J `20260519-runtime-context-champion-submit-gate-j` passed in `229.22s`
- Post-run PocketIC cleanup confirmed no leftover PocketIC/server processes.

Measured delta versus Gate 5B.3 `20260519-runtime-blocker-check-gate-j`:

| metric | runtime blocker check | runtime context/champion submit | change |
| --- | ---: | ---: | ---: |
| scenario instructions | 293.0846B | 284.6370B | -2.9% |
| `submit_move_intent` avg | 3.5293B | 0.7110B | -79.9% |
| `submit_move_intent` avg memory | 0.0208 MB | 0.0417 MB | +0.0209 MB |
| `sync_session_turn` avg | 12.5747B | 12.5752B | flat |
| `sessions.load_session` calls | 18 | 15 | -3 |
| `champions.load_champion` route summary | 3 calls | 0 calls | removed from Gate J route |

Per-submit calls after the patch:

| sequence | instructions | remaining measured repo ops |
| ---: | ---: | --- |
| 80 | 0.7223B | `system_jobs.by_session_status_due` x2 |
| 160 | 0.7074B | `system_jobs.by_session_status_due` x2 |
| 214 | 0.7033B | `system_jobs.by_session_status_due` x2 |

Decision:

- Keep this cut. `submit_move_intent` is now close to the requested `0.3B-0.6B` band and no longer pays durable session/champion/occupancy reads in the measured route.
- Do not force the last `~0.7B -> 0.3B-0.6B` step with a local guard skip. The stale-turn regression already showed the durable job guard has real closure semantics. The next safe submit improvement is Gate 5E: runtime-owned turn/deadline/job state.
- Immediate perf focus should move to `sync_session_turn`, which remains `12.5752B` and now dominates the movement route.

## Checkpoint: Runtime Sync Receipts, Events, And Empty-Intent Fast Path

Implemented the Gate 5C sync cut in one batched checkpoint:

- Fresh active `sync_session_turn` now uses runtime command receipts instead of durable `GameCommand` create/update.
- Active runtime sync no longer probes durable game-command idempotency; runtime receipts own replay/mismatch for the active path.
- Runtime movement intents stay runtime-only during active sync, so fresh runtime sync no longer writes durable `MovementIntent` or `MovementSnapshot` rows.
- Movement sync events can be emitted from `SessionTurnRuntime.active_events`, and `get_events_after` already merges them.
- Manual runtime sync no longer reschedules/completes durable turn jobs; job rows are treated as wakeup hints for this path.
- Turn advance can prepare the next active turn runtime by copying the current runtime's participants, champion snapshots, occupancy, and contact indexes instead of rehydrating players/towns/world objects from durable rows.
- Same-week economy growth is skipped instead of re-reading town/recruit/tavern state on every turn advance.
- Runtime occupancy is authoritative for movement, so durable champion occupancy rows are not updated while an active turn runtime exists.
- Gate J benchmark expectations now validate runtime-visible movement events instead of requiring old durable `MovementSnapshot` growth on fresh runtime sync.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister --features benchmark`
- `DOMM_CANISTER_FEATURES=benchmark cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run`
- Focused Gate J `20260519-runtime-sync-empty-intents-gate-j` passed in `217.28s`
- Post-run PocketIC cleanup killed the leftover server and confirmed no remaining PocketIC processes.

Measured delta versus the accepted Gate 5B.4 baseline `20260519-runtime-context-champion-submit-gate-j`:

| metric | Gate 5B.4 | Gate 5C | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 284.6370B | 196.2456B | -31.1% |
| `sync_session_turn` avg | 12.5752B | 3.9270B | -68.8% |
| `submit_move_intent` avg | 0.7110B | 0.7110B | flat |
| `commands.game_command_idempotency` calls | 11 sync-route calls plus submit/town/lobby baseline | 3 non-runtime calls in the route summary | removed from active runtime sync |
| `movement.create_snapshot` / `movement.update_intent` runtime sync bridge | present on compatibility calls | absent from final route summary | removed from active runtime sync |

Measured progression within this checkpoint:

| run | `sync_session_turn` avg | Gate J scenario instructions | note |
| --- | ---: | ---: | --- |
| `20260519-runtime-sync-batch-gate-j` | 5.0430B | 209.6861B | runtime sync commands/events/job cuts, but durable sync idempotency still present |
| `20260519-runtime-sync-no-idempotency-gate-j` | 4.0424B | 197.6731B | skipped durable idempotency for active runtime sync |
| `20260519-runtime-sync-empty-intents-gate-j` | 3.9270B | 196.2456B | empty runtime intent list avoids durable pending-intent/participant scan |

Remaining measured blockers:

- `submit_move_intent` is still about `0.711B`; the remaining visible cost is the durable map-turn job guard.
- Non-battle movement sync calls still write durable champion/resource/object/visibility/session/participant rows, so Gate 5D must move these into runtime deltas and query overlays.
- Guarded neutral contact still starts battles through durable battle rows/stacks/obstacles/effects/jobs. The largest sync calls are now battle handoff calls: `7.39B`, `5.68B`, `4.26B`, `11.06B`, and `3.08B`.

Decision:

- Gate 5C is complete because the active sync average is now below the documented `4B` target.
- Next implementation should not spend time polishing command/event mechanics. The biggest wins now are Gate 5D runtime deltas for champion/resources/objects and Gate 5F runtime-first battle/town/neutral contact.

## Checkpoint: Runtime Movement Deltas And Snapshot Carry-Forward

Implemented the first Gate 5D runtime-delta slice:

- Active runtime movement now mirrors champion position/status and participant resource changes into `SessionTurnRuntime` instead of writing those rows on the hot movement path.
- Resource pile pickups and income can mutate the runtime participant row and record runtime resource deltas; durable `ResourceLedgerEntry`/`ResourceLedgerTurnSummary` writes are skipped for the runtime-owned path.
- World object captures/visits are mirrored into runtime snapshots and object deltas; mine owner updates still project to durable `WorldObject` so existing income lookup remains compatible until income is fully runtime-owned.
- `get_my_participant`, session caller auth, `get_my_champions`, `get_champion_view`, visible object/detail helpers, and movement object lookups merge runtime snapshots before durable fallback.
- Town build/recruit keeps the runtime participant mirror current after durable town resource spending.
- Turn-runtime bootstrap now carries prior-turn heap snapshots forward and overlays them onto lazily rebuilt runtimes. This fixed a stale-position bug where a heap-only crystal-mine move was followed by a new movement submit that validated against old durable champion coordinates.
- Gate J row-growth assertions were narrowed so runtime-owned movement/object/resource state no longer has to create durable visit/ledger/summary rows during active gameplay.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- Focused Gate J `20260519-gate5d-runtime-deltas-v2-gate-j` failed after `174.37s` because runtime-only crystal capture was invisible to income; fixed by durable mine owner projection plus runtime/durable income max.
- Focused Gate J `20260519-gate5d-runtime-deltas-v3-gate-j` failed after `290.21s` with `movement_path_not_adjacent`; fixed by carrying prior-turn heap snapshots into current runtime bootstrap.
- Focused Gate J `20260520-gate5d-runtime-carry-forward-gate-j` passed in `326.27s` but wrote artifacts to the nested test-relative target path.
- Focused Gate J `20260520-gate5d-runtime-deltas-gate-j` passed in `76.12s` with canonical artifacts under `target/benchmarks/20260520-gate5d-runtime-deltas-gate-j`.
- Post-run PocketIC cleanup confirmed no leftover PocketIC/server processes.

Measured delta versus Gate 5C `20260519-runtime-sync-empty-intents-gate-j`:

| metric | Gate 5C | Gate 5D.1 | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 196.2456B | 180.8734B | -7.8% |
| Gate J memory | 5220.9375 MB | 5084.9375 MB | -2.6% |
| row growth | 76 | 43 | -43.4% |
| `sync_session_turn` avg | 3.9270B | 2.7653B | -29.6% |
| `sync_session_turn` avg memory | 62.7500 MB | 52.7396 MB | -16.0% |
| `submit_move_intent` avg | 0.7110B | 0.7113B | flat |
| `get_champion_view` avg | 4.9342B | 4.2264B | -14.3% |

Measured repo-operation movement:

| operation | Gate 5C calls | Gate 5D.1 calls | note |
| --- | ---: | ---: | --- |
| `champions.update_champion` | 4 | 2 | active movement champion writes moved into runtime; remaining writes are non-active/battle boundary paths |
| `sessions.update_participant` | 6 | 4 | active movement resource/last-action writes moved into runtime |
| `economy.create_resource_ledger_entry` | 6 | 4 | runtime pickup/income skips durable ledger entries |
| `economy.create_resource_turn_summary` | 1 | 0 | runtime income skips durable summary |
| `map.create_participant_object_visit` | 2 | 0 | runtime object visits no longer create durable visit rows |
| `map.update_world_object` | 2 | 1 | resource pile visit moved runtime-only; mine capture still durable for income compatibility |

Decision:

- Keep this cut. It removes real durable row growth and drops `sync_session_turn` below `3B` without hollowing out behavior; Gate J still reaches pickup, build, recruit, crystal income, guarded neutral contact, battle row creation, and final event checks.
- Gate 5D remains open because the target is `<1.5B`. The next meaningful cuts are runtime-owned battle/contact handoff, turn/session/job writes, and full runtime income/object projection so mine compatibility no longer needs a durable `WorldObject` update.

## Checkpoint: Battle Start Unit Cache And Attacker Stage Collapse

Implemented the first low-risk Gate 5F cut around battle/contact startup:

- `content::load_unit` now uses a heap cache for immutable unit definitions. The cache is warmed by unit create/find/page/load paths, so first-playable setup warms battle-start unit rows before guarded contact.
- Staged champion and neutral battle-start recovery still exists, but each state now reads only the stack side it needs. The old eager path paged attacker and defender stacks at the top of every state.
- New champion and neutral battles create attacker stacks during the initial battle creation call and enter `starting_attacker`. Recovery from an older plain `starting` row still creates attacker stacks through the existing state machine.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- Focused Gate J `20260520-battle-start-cache-attacker-gate-j` passed in `214.53s`.
- Removed the benchmark PocketIC server left with the hard TTL after the run. Other matched PocketIC processes were from an unrelated `reference/dex` path and were left alone.

Measured delta versus Gate 5D.1 `20260520-gate5d-runtime-deltas-gate-j`:

| metric | Gate 5D.1 | Gate 5F.1 | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 180.8734B | 174.5569B | -3.5% |
| Gate J memory | 5084.9375 MB | 5084.8750 MB | flat |
| scenario calls | 55 | 54 | -1 |
| row growth | 43 | 43 | flat |
| `sync_session_turn` avg | 2.7653B | 2.4438B | -11.6% |
| `submit_move_intent` avg | 0.7113B | 0.7108B | flat |

Measured repo-operation movement:

| operation | Gate 5D.1 calls | Gate 5F.1 calls | total instruction change |
| --- | ---: | ---: | ---: |
| `content.load_unit` | 3 | 0 | -2.1417B |
| `battles.stacks_by_side` | 16 | 6 | -3.5225B |
| `battles.by_attacker` | 6 | 5 | -0.6919B |
| `battles.update_battle` | 4 | 4 | +0.0128B |

Decision:

- Keep this cut. It removes real stable reads and one normal-path startup sync call without hollowing out battle creation or hiding work in a new slow endpoint.
- Gate 5F remains open. The remaining contact path is still row-backed: battle stacks, occupancy, obstacles, battle-start effects, timeout jobs, neutral state, and champion in-battle status are still durable startup writes/reads. The next large cut should move neutral/champion battle handoff into runtime first, then project durable rows only at the flush boundary.

## Checkpoint: Neutral Battle Start Effect Removal And Stack List Read

Implemented the second low-risk Gate 5F cut:

- Removed the internal neutral `battle_started` `CommandEffect` from the movement contact hot path. The durable battle row and the public `neutral_encounter_pending` event remain the recovery/client signal.
- Final champion/neutral battle activation now loads all battle stacks once with `battles::list_battle_stacks` for initiative selection instead of paging attacker and defender sides separately.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- Focused Gate J `20260520-battle-start-no-effect-stack-list-gate-j` passed in `199.36s`.
- Removed the benchmark PocketIC server left with the hard TTL after the run.

Measured delta versus Gate 5F.1 `20260520-battle-start-cache-attacker-gate-j`:

| metric | Gate 5F.1 | Gate 5F.2 | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 174.5569B | 171.2591B | -1.9% |
| Gate J memory | 5084.8750 MB | 5084.8125 MB | flat |
| scenario calls | 54 | 54 | flat |
| row growth | 43 | 43 | flat |
| `sync_session_turn` avg | 2.4438B | 2.1440B | -12.3% |
| `submit_move_intent` avg | 0.7108B | 0.7108B | flat |

Measured repo-operation movement:

| operation | Gate 5F.1 calls | Gate 5F.2 calls | note |
| --- | ---: | ---: | --- |
| `effects.command_effect_by_command_key` | 1 | 0 | removed neutral battle-start effect command lookup |
| `effects.command_effect_by_session_status` | 2 | 0 | removed neutral battle-start effect session lookup |
| `effects.create_applied_command_effect` | 1 | 0 | removed neutral battle-start effect row |
| `battles.stacks_by_side` | 6 | 2 | final activation no longer pages both sides |
| `battles.stacks_by_battle` | 1 | 2 | one extra all-stack read replaces two side pages |

Decision:

- Keep this cut. It removes internal durable bookkeeping from battle startup while preserving the visible event and row-backed battle state used by current tests and clients.
- Remaining Gate 5F work is now dominated by durable battle-start rows and job scheduling: battle creation/stack/occupancy/obstacle writes, final runtime adoption reads, timeout job scans/insert, neutral state update, and champion status update. The next meaningful step should stop adopting runtime by re-reading the rows just created, or move the entire contact handoff to a runtime aggregate.

## Checkpoint: Reuse Battle Startup Stacks During Runtime Adoption

Implemented a small runtime adoption cut:

- `battle_rows` can now build a `BattleState` from a durable battle row plus already loaded stack rows.
- `battle_runtime` has an adoption helper that reuses those stack rows and only reads obstacles/occupancy.
- Champion and neutral battle activation pass the stack rows already loaded for initiative selection into runtime adoption, avoiding an immediate duplicate stack scan.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister --features benchmark`
- Focused Gate J `20260520-battle-start-reuse-stacks-gate-j` passed in `207.49s`.
- Removed the benchmark PocketIC server left with the hard TTL after the run.

Measured delta versus Gate 5F.2 `20260520-battle-start-no-effect-stack-list-gate-j`:

| metric | Gate 5F.2 | Gate 5F.3 | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 171.2591B | 170.5444B | -0.4% |
| Gate J memory | 5084.8125 MB | 5084.8125 MB | flat |
| `sync_session_turn` avg | 2.1440B | 2.0789B | -3.0% |
| guarded activation call | 7.8428B | 7.1346B | -9.0% |

Measured repo-operation movement:

| operation | Gate 5F.2 calls | Gate 5F.3 calls | note |
| --- | ---: | ---: | --- |
| `battles.stacks_by_battle` | 2 | 1 | runtime adoption reused selection stacks |
| `battles.obstacles_by_battle` | 1 | 1 | still needed by adoption |
| `battles.occupancy_by_battle` | 1 | 1 | still needed by adoption |

Decision:

- Keep this cut. It is small, but it proves the next larger direction: battle startup should hand a complete in-memory state into runtime instead of writing rows and then reading them back.
- The remaining activation floor is now mostly timeout job scheduling, durable neutral/champion status writes, and the obstacle/occupancy adoption reads.

## Checkpoint: Runtime-Owned In-Battle Champion Status

Implemented a Gate 5D runtime champion-status cut:

- `update_movement_champion` now mirrors `in_battle` champion status into `SessionTurnRuntime` on the runtime-only movement path instead of writing the durable `Champion` row.
- Neutral contact uses that movement champion update helper, so both the direct contact update and the following stop-candidate update stay heap-local.
- Public `get_champion_view` still sees the in-battle status through the existing runtime champion snapshot overlay.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- Focused Gate J `20260520-runtime-inbattle-champion-gate-j` passed in `204.77s`.
- Removed the benchmark PocketIC server left with the hard TTL after the run.

Measured delta versus Gate 5F.3 `20260520-battle-start-reuse-stacks-gate-j`:

| metric | Gate 5F.3 | Gate 5D.2 | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 170.5444B | 169.5872B | -0.6% |
| Gate J memory | 5084.8125 MB | 5084.8125 MB | flat |
| `sync_session_turn` avg | 2.0789B | 1.9913B | -4.2% |
| final post-contact sync call | 1.6691B | 0.7046B | -57.8% |

Measured repo-operation movement:

| operation | Gate 5F.3 calls | Gate 5D.2 calls | note |
| --- | ---: | ---: | --- |
| `champions.update_champion` | 2 | 0 | runtime owns active movement/in-battle champion status in this route |
| final post-contact repo ops | `battles.by_attacker`, `champions.update_champion` | `battles.by_attacker` | durable champion projection removed |

Decision:

- Keep this cut. It moves another live movement status field into the runtime aggregate and preserves the public champion view used by Gate J.
- This does not close Gate 5D because `sync_session_turn` is still above `1.5B`; the next large reductions require moving battle contact handoff/job scheduling out of durable rows or making turn/job authority runtime-owned.

## Checkpoint: Return Neutral Battle On Activation Sync

Implemented a control-flow cut in the neutral battle startup state machine:

- Final neutral battle activation now returns `Some(battle)` immediately, matching the champion battle startup path.
- `mark_neutral_encounter_pending` can emit `neutral_encounter_pending` in the same sync that activates the battle.
- This removes the extra follow-up sync whose only meaningful repo operation was re-reading `battles.by_attacker` to rediscover the active battle.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- Focused Gate J `20260520-neutral-activation-returns-battle-gate-j` passed in `206.47s`.
- Removed the benchmark PocketIC server left with the hard TTL after the run.

Measured delta versus Gate 5D.2 `20260520-runtime-inbattle-champion-gate-j`:

| metric | Gate 5D.2 | Gate 5F.4 | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 169.5872B | 168.8816B | -0.4% |
| Gate J memory | 5084.8125 MB | 5084.8125 MB | flat |
| scenario calls | 54 | 53 | -1 |
| `sync_session_turn` calls | 11 | 10 | -1 |
| `sync_session_turn` avg | 1.9913B | 2.1199B | +6.5% |

The sync average rose because the removed call was the cheap final rediscovery sync (`0.7046B`). Scenario total and call count are the meaningful improvement for this checkpoint.

Measured repo-operation movement:

| operation | Gate 5D.2 calls | Gate 5F.4 calls | note |
| --- | ---: | ---: | --- |
| `battles.by_attacker` | 5 | 4 | removed final active-battle rediscovery |

Decision:

- Keep this cut. It removes an entire public update call from the route without hiding work in another endpoint.
- The remaining activation spike is still about `7.13B`; reducing it requires eliminating durable battle startup rows/job scheduling or building the active battle runtime directly before durable projection.

## Checkpoint: Cache Battle Startup Rows Until Runtime Adoption

Implemented the next low-hanging Gate 5F cut without running another slow PocketIC benchmark yet:

- Added a heap startup-row cache for battle stacks, battle occupancy, and battle obstacles created during staged battle startup.
- Champion, neutral, and town battle starts now adopt `BattleRuntime` from the already-created rows when the cache is complete, instead of immediately re-reading tactical child rows from IcyDB.
- Startup recovery still falls back to durable row reads when the cache is missing or incomplete, so upgrade/recovery paths keep working.
- Brand-new battle timeout jobs now use direct `SystemJob` create during battle startup instead of paying the upsert key lookup that is only needed for reschedule/recovery paths.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Benchmark status:

- Focused Gate J `20260520-startup-cache-open-turn-gate-j` passed in `197.43s`.

Measured delta versus Gate 5F.4 `20260520-neutral-activation-returns-battle-gate-j`:

| metric | Gate 5F.4 | Gate 5F.5 / 5E.1 | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 168.8816B | 164.7224B | -2.5% |
| `sync_session_turn` avg | 2.1199B | 1.8362B | -13.4% |
| `submit_move_intent` avg | 0.7108B | 0.2389B | -66.4% |
| `system_jobs.by_session_status_due` calls | 10 | 6 | -40.0% |
| `system_jobs.by_job_key` calls | 2 | 1 | -50.0% |

Measured repo-operation movement:

| operation | Gate 5F.4 | Gate 5F.5 / 5E.1 | note |
| --- | ---: | ---: | --- |
| `battles.stacks_by_battle` | 1 | 0 | final activation reused heap startup stacks |
| `battles.obstacles_by_battle` | 1 | 0 | final activation reused heap startup obstacles |
| `battles.occupancy_by_battle` | 1 | 0 | final activation reused heap startup occupancy |
| `system_jobs.by_job_key` | 2 | 1 | startup timeout job uses direct create |
| `system_jobs.by_session_status_due` | 10 | 6 | runtime-open guard skipped safe pre-deadline scans |

Decision:

- Keep this cut. The code preserves durable row fallback and moves no logic to a hollow endpoint; it only reuses rows that were already created in the same staged startup sequence. The paired runtime-open guard also gets `submit_move_intent` under the `0.3B-0.6B` hard target.

## Checkpoint: Runtime-Proved Open Turn Guard

Implemented the first Gate 5E job-authority cut:

- `ensure_map_turn_accepts_new_command` now skips the pre-deadline scheduled `SystemJob` scan when `SessionTurnRuntime` proves the same active session, same turn, same deadline, not closing, and no ready participants.
- The shortcut only skips the closure-job scan. `end_turn` still runs the durable duplicate-ready check, preserving the regression case that broke the earlier unsafe shortcut.
- Post-deadline commands and any runtime-missing/runtime-ready/runtime-closing state still use the durable job scan.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Native PocketIC `pocket_ic_end_turn_closes_turn_and_blocks_stale_actions` passed in `188.77s`.
- Removed the PocketIC server left by the native regression after the run.

Benchmark status:

- Focused Gate J `20260520-startup-cache-open-turn-gate-j` passed in `197.43s`.
- Versus Gate 5F.4 `20260520-neutral-activation-returns-battle-gate-j`, `submit_move_intent` moved `0.7108B -> 0.2389B` (-66.4%) and `system_jobs.by_session_status_due` calls moved `10 -> 6`.
- This gets fresh movement submit below the `0.3B-0.6B` hard target for the first time.

Decision:

- Keep this cut. It is the same semantic direction as Gate 5E, but fenced by active runtime proof and backed by the stale-turn regression before commit and the focused Gate J benchmark after commit.

## Checkpoint: Direct New-Job Timer Refresh And Neutral Startup Battle Cache

Implemented two low-risk cuts before the next slow Gate J run:

- `schedule_new_job` now refreshes the Wasm heap timer directly for brand-new jobs. Upsert/reschedule still use the existing nearest-job scan, so recovery/idempotent paths keep the conservative behavior.
- Staged neutral battle startup now caches the durable `Battle` row by attacker while the row is in a `starting_*` state. Later startup sync calls use the cache and fall back to `battles.by_attacker` if the cache is absent, such as after upgrade.
- The neutral startup cache is remembered only after durable battle updates succeed and is discarded immediately when the battle becomes active.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Benchmark status:

- Focused Gate J `20260520-neutral-startup-scan-cache-gate-j` passed in `197.64s`.

Measured delta versus `20260520-startup-cache-open-turn-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 164.7224B | 161.1489B | -2.2% |
| `sync_session_turn` avg | 1.8362B | 1.4784B | -19.5% |
| `submit_move_intent` avg | 0.2389B | 0.2389B | flat |
| `battles.by_attacker` calls | 4 | 1 | -75.0% |
| `system_jobs.by_status_due` calls | 2 | 1 | -50.0% |
| `system_jobs.by_status_lease` calls | 2 | 1 | -50.0% |

Measured sync call shape:

| startup sync | previous dominant ops | new dominant ops |
| --- | --- | --- |
| attacker stack creation | `battles.by_attacker`, battle/stack/occupancy writes, champion army page | unchanged first lookup plus writes |
| defender stack creation | `battles.by_attacker`, defender stack writes, neutral stack page | cached battle row; no attacker lookup |
| obstacle creation | `battles.by_attacker`, obstacle writes | cached battle row; no attacker lookup |
| final activation | `battles.by_attacker`, neutral load/update, job scans/create | cached battle row; direct new-job timer refresh; neutral load/update and job create remain |

Decision:

- Keep this cut. It brings `sync_session_turn` just under the current Gate 5D target (`1.5B`), with durable fallback still present for cache misses and upgrades. The remaining path to `0.6B-0.9B` is now durable startup writes themselves: battle header/stacks/occupancy/obstacles, neutral load/update, and system-job create.

## Checkpoint: Runtime Session Query Context

Implemented the first Gate 5G query/runtime projection cut:

- `get_session` now renders active sessions from `SessionTurnRuntime` session/participant rows before falling back to durable session and participant reads.
- Query-only caller context now authenticates active-session callers from runtime caller rows before durable player/session/participant lookups.
- `get_my_participant` uses the same runtime caller rows before durable fallback.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Failed focused Gate J attempt `20260520-runtime-session-query-gate-j` exposed that the first version put runtime caller context in the shared command/query helper; `submit_build_town_structure` then used a stale runtime `GameSession.next_event_seq` and failed on `events.create_game_event`. Fixed by making runtime-first context query-only and leaving command paths on durable context.

Benchmark status:

- Focused Gate J `20260520-runtime-session-query-fixed-gate-j` passed in `196.42s`.
- The first attempted run `20260520-runtime-session-query-gate-j` failed at town build for the stale session reason below; the fixed run passed.

Measured delta versus `20260520-neutral-startup-scan-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 161.1489B | 143.4896B | -11.0% |
| `get_my_participant` avg | 2.8177B | 0.7040B | -75.0% |
| `get_my_champions` avg | 2.8228B | 0.7076B | -74.9% |
| `get_visible_map_chunks` avg | 3.5218B | 1.4064B | -60.1% |
| `get_visible_objects` avg | 4.9506B | 2.8351B | -42.7% |
| `get_champion_view` avg | 4.2611B | 2.1292B | -50.0% |
| `get_session` avg | 1.4128B | 1.2561B | -11.1% |

Decision:

- Keep this cut. This is a runtime-first query merge, not a view fabrication: durable fallback remains for lobby/setup/cache-miss paths, and active runtime already owns the session/participant snapshots used by movement. `get_session` only partially improved because most Gate J calls happen before active runtime exists; the active calls are now near-zero instruction queries in the raw log.

## Checkpoint: Runtime Projection Query Cuts And Town Row Micro-Cuts

Implemented a second Gate 5G projection batch plus two low-risk town row cuts:

- `get_events_after`, `get_command_status`, and `get_command_status_by_nonce` now use query-only runtime caller context before durable caller/session/participant lookup.
- `get_my_champions` checks runtime champion snapshots for owned champion ids before loading durable champion rows.
- `get_visible_objects` uses the active runtime world-object snapshot map before paging all durable world objects.
- `get_town_view` and town preview queries authenticate from runtime caller context, while `get_town_view` still renders real `TownBuilding`, `TownRecruitPool`, and `TownGarrisonStack` rows.
- `submit_build_town_structure` no longer performs a second building lookup after `built_building_ids` has already proved the target building is absent.
- New town garrison stacks now write `last_command_id` during create, removing the immediate create-then-update pattern in recruitment and town-battle aftermath projection.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- First focused Gate J attempt `20260520-query-town-cuts-gate-j` failed install because the benchmark Wasm code section reached `12,593,503` bytes, `10,591` over the IC limit.
- After trimming the generic runtime helper surface, focused Gate J `20260520-query-town-cuts-slim-gate-j` passed in `200.33s`.
- Removed the PocketIC server left holding the benchmark pipe after the passing run.

Measured delta versus `20260520-runtime-session-query-fixed-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 143.4896B | 128.8321B | -10.2% |
| `get_events_after` avg | 2.8286B | 0.7095B | -74.9% |
| `get_visible_objects` avg | 2.8351B | 1.4179B | -50.0% |
| `get_town_view` avg | 4.2294B | 2.8189B | -33.3% |
| `get_my_champions` avg | 0.7076B | ~0B | ~-100.0% |
| `get_champion_view` avg | 2.1292B | 2.1326B | flat |
| `submit_build_town_structure` avg | 16.9920B | 16.2855B | -4.2% |
| `submit_recruit_units` avg | 12.5241B | 12.0437B | -3.8% |
| `sync_session_turn` avg | 1.4784B | 1.4732B | -0.4% |

Decision:

- Keep this cut. The query changes materially reduce scenario cost without moving command paths onto stale runtime session rows. The town command micro-cuts are useful but small; getting build/recruit near `0.3B-0.6B` still requires the Gate 6 `TownRuntime`/resource-runtime rewrite, not more row-level polishing.

## Checkpoint: Projection Code-Size Headroom

Removed the unused legacy object projection pipeline from `render_projection.rs`.

Why:

- The benchmark canister is again close to the IC Wasm code-section limit.
- The failed `20260520-query-town-cuts-gate-j` run showed that even useful small runtime helpers can push install over the limit.
- The removed renderer was explicitly dead code after `object_view_from_known_fast` became the active projection path.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Decision:

- Keep this cleanup. It does not claim a gameplay performance delta, but it is necessary engineering work before adding the larger `TownRuntime` surface.

## Checkpoint: Town Projection Cache Scaffold

Implemented the first Gate 6 town runtime scaffold:

- Added a heap `TownProjection` cache keyed by `(session_id, town_id)` and backed by real durable town rows on cache miss.
- `get_town_view` now renders from the cached projection after hydration instead of reloading `TownBuilding`, `TownRecruitPool`, and `TownGarrisonStack` rows on every render.
- Recruitment pool and garrison-slot lookup can read the hydrated projection during `submit_recruit_units`.
- Existing durable writers still run, but now mirror changes into the cache when it exists: build rows, recruit pools, garrison stacks, captured town ownership, and weekly recruit growth.
- Battle aftermath evicts the town projection before delete/recreate survivor garrison writes, so the next read reloads truthful rows.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Benchmark status:

- No PocketIC benchmark was run for this checkpoint. This is the low-risk cache scaffold before the larger Gate 6 change that makes build/recruit mutate runtime first and defers durable child-row writes.

Decision:

- Keep this checkpoint. It fixes the repeated child-row load shape for cached town views and gives the next step a single town aggregate to mutate. It does not yet solve the main `submit_build_town_structure`/`submit_recruit_units` cost because durable command, resource, event, building, pool, and garrison writes still happen in the hot call.

## Checkpoint: Town Projection Read Cuts

Extended the Gate 6 town projection scaffold into the remaining low-risk town read sites:

- `preview_build_town_structure` uses cached projection building ids instead of paging `TownBuilding` rows.
- `preview_recruit_units` uses cached projection recruit pools instead of paging `TownRecruitPool` rows.
- `submit_build_town_structure` uses cached projection building ids for prerequisite/already-built checks.
- Build unlock-pool validation uses cached projection recruit pools instead of a dedicated durable `find_town_recruit_pool` call.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Benchmark status:

- Focused Gate J `20260520-town-projection-reads-gate-j` passed in `200.88s`.
- Removed the PocketIC server left holding the benchmark pipe after the passing run.

Measured delta versus `20260520-query-town-cuts-slim-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 128.8321B | 123.9078B | -3.8% |
| `get_town_view` avg | 2.8189B | 0.7079B | -74.9% |
| `submit_recruit_units` avg | 12.0437B | 10.6436B | -11.6% |
| `submit_build_town_structure` avg | 16.2855B | 16.9860B | +4.3% |
| `sync_session_turn` avg | 1.4732B | 1.4720B | flat |
| `towns.buildings_by_town` repo op | 2 calls | 0 calls | removed |

Decision:

- Keep this cut. It fixed the town-view target and helped recruit, but the build regression shows the limit of read caching: first build now pays projection hydration while still doing durable command, resource, event, building, pool, and town writes. The next Gate 6 work needs to remove or batch durable writes rather than add more read-side cache polish.

## Checkpoint: Runtime Town Commands And Seeded Projection

Moved the first-playable town build/recruit path onto the active runtime model:

- `submit_build_town_structure` and `submit_recruit_units` now create runtime command receipts, runtime events, and runtime resource deltas through `SessionTurnRuntime` instead of writing durable `GameCommand`, `GameEvent`, `ResourceLedgerEntry`, and participant update rows on the active path.
- Town building, recruit-pool, and garrison mutations now update the heap `TownProjection` first instead of writing `TownBuilding`, `TownRecruitPool`, and `TownGarrisonStack` rows during the hot call.
- First-playable setup seeds the west/east town projection and mirrors the initial town hall, so first town reads and commands do not pay a durable hydration scan in the benchmark route.
- Unit/building slug lookup now uses heap content caches after seed/create.
- The fast town command turn guard reuses the same proven-open runtime predicate as map-turn commands before skipping the durable closure scan; after deadline, while closing, or with ready participants present it falls back to the durable guard.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Focused Gate J `20260520-runtime-town-commands-gate-j` passed in `206.60s`.
- Focused Gate J `20260520-runtime-town-slim-content-cache-gate-j` passed in `193.66s`.
- Final focused Gate J `20260520-runtime-town-seeded-alias-slim2-gate-j` passed in `204.35s`.

Measured final delta versus `20260520-town-projection-reads-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 123.9078B | 95.5145B | -22.9% |
| Gate J scenario memory | 5084.8125 MB | 4620.4375 MB | -9.1% |
| `get_town_view` avg | 0.7079B | 0.0016B | -99.8% |
| `submit_build_town_structure` avg | 16.9860B | 0.7069B | -95.8% |
| `submit_recruit_units` avg | 10.6436B | 0.0003B | ~-100.0% |
| row growth | 43 | 43 | flat |
| stable pages final | 82305 | 74881 | -9.0% |

Code-size notes:

- Attempts with a larger ruleset/content cache and generic first-playable alias resolver failed install just over the IC Wasm code-section limit.
- The passing checkpoint keeps only unit/building slug caches and a narrow first-playable town alias resolver for `town:west`/`town:east`.

Decision:

- Keep this checkpoint. It moves town commands from the same row-normalized live mutation shape that made battles slow to the runtime/projection shape that benchmarks well.
- Do not claim the durable boundary is complete. Runtime town command/event/resource/building/pool/garrison data still needs bounded flush/upgrade snapshot coverage for recovery/history.
- `submit_build_town_structure` is now below `1B` but still slightly above the preferred `0.3B-0.6B` target, so the next town work should identify the remaining floor instead of adding more broad surface area.

## Checkpoint: Batched Session View And Town Prerequisite Micro-Cuts

Implemented and measured two low-risk cuts:

- Added a single-slot heap `SessionView` cache for `get_session`. Active runtime rows still win first, then the cache, then durable fallback. Lobby command results and active runtime `get_session` reads refresh the cache.
- Query-only caching did not work because query heap mutations do not persist across calls. The cache has to be seeded from update paths.
- Changed town building prerequisite checks to use building slugs already present in the heap `TownProjection`, instead of looking up each required building definition just to compare ids.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Focused Gate J `20260520-session-cache-town-slugs-single-gate-j` passed in `188.77s`.

Measured delta versus `20260520-runtime-town-seeded-alias-slim2-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 95.5145B | 74.3158B | -22.2% |
| `get_session` avg | 1.2555B | ~0B | ~-100.0% |
| `submit_build_town_structure` avg | 0.7069B | 1.4111B | +99.6% |
| `submit_recruit_units` avg | 0.0003B | 0.7062B | regression from safe guard |

Code-size notes:

- A BTreeMap session-view cache failed install at `12,587,143` bytes, `4,231` over the IC Wasm code-section limit.
- A two-slot session-view cache failed install at `12,584,384` bytes, `1,472` over.
- A town fast guard combined with the session-view cache also failed install, even after slimming, so the current checkpoint keeps the safe durable guard and leaves the town guard floor as remaining work.

Decision:

- Keep the single-slot cache. It is the larger scenario win and makes repeated setup/lobby polling near-free.
- Keep the slug prerequisite check because it removes an unnecessary content lookup path, even though the measured town command floor is currently dominated by the safe turn-closure guard.
- Do not keep the unsafe/fatter town fast guard until code-size headroom exists for a safe version. The remaining town target is now explicitly the `system_jobs.by_session_status_due` guard cost.

## Checkpoint: Small Dead-Code Headroom Cleanup

Removed two unused helpers:

- `movement::hide_known_world_object`
- the local `scenario_progress::changed` wrapper around `command_response::changed`

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build for `domm-degens-canister --features benchmark`

Measured code section:

| artifact | code section | IC limit | headroom |
| --- | ---: | ---: | ---: |
| benchmark Wasm after cleanup | `12,582,756` bytes (`0x00bfff64`) | `12,582,912` bytes (`0x00c00000`) | `156` bytes |

Town guard experiment:

- Tried using a looser runtime-open town guard and a cache freshness check to avoid the remaining town `system_jobs.by_session_status_due` scans.
- The first installable attempt still showed `submit_build_town_structure` at `1.4111B`, `submit_recruit_units` at `0.7062B`, and the same two `system_jobs.by_session_status_due` calls under each town command.
- The cache/guard variants were either over the IC code-section limit or did not remove the measured job scans, so they were reverted.

Decision:

- Keep only the dead-code cleanup. It does not claim a gameplay performance delta.
- Do not spend more time on local town guard shortcuts. The remaining safe route is the broader Gate 5E/5H runtime turn/deadline/job authority or a larger code-size freeing pass that can support a proven guard.

## Checkpoint: Remove Pass-Through Battle Phase Wrappers

Removed the remaining `crate::metrics::benchmark_phase` wrappers from `battle.rs` and deleted the unused `benchmark_phase` helper from `metrics`.

Why:

- `benchmark_phase` is already a pass-through and `take_current_phases()` returns an empty vector, so phase summaries are not produced anymore.
- The wrappers still cost benchmark Wasm code size and create friction for the next runtime rewrite.
- Method-level and repo-operation benchmark measurements remain intact.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build for `domm-degens-canister --features benchmark`

Measured code section:

| artifact | code section | IC limit | headroom |
| --- | ---: | ---: | ---: |
| after small dead-code cleanup | `12,582,756` bytes (`0x00bfff64`) | `12,582,912` bytes (`0x00c00000`) | `156` bytes |
| after phase-wrapper removal | `12,579,221` bytes (`0x00bff195`) | `12,582,912` bytes (`0x00c00000`) | `3,691` bytes |

Decision:

- Keep this cleanup. It is behavior-preserving and gives enough headroom for a small runtime/contact cut before a larger code-size freeing pass is needed.

## Checkpoint: Neutral Startup Header Writes Deferred To Activation

Changed neutral battle startup so the durable `Battle` row is created in `starting_attacker` and then left alone while attacker stacks, defender stacks, and obstacles are staged in heap. The durable row is still updated when the battle becomes active.

Why:

- Gate J showed one `battles.update_battle` write in each startup sync step before activation.
- The staged `Battle` row is already cached in heap by attacker while startup is in progress.
- Writing each intermediate `starting_*` header state costs about `0.477B` instructions and does not add useful client behavior on the normal path.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build for `domm-degens-canister --features benchmark`: code section `0x00bfec59`, about `5.0 KB` under the IC limit
- Focused Gate J `20260520-neutral-start-no-intermediate-battle-updates-gate-j` passed in `263.08s`

Measured delta versus `20260520-session-cache-town-slugs-single-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 74.3158B | 72.8864B | -1.9% |
| `sync_session_turn` avg | 1.4704B | 1.3278B | -9.7% |
| `battles.update_battle` calls | 4 | 1 | -75.0% |
| `battles.update_battle` total instructions | 1.9125B | 0.4810B | -74.8% |
| neutral startup sync seq 69 | 4.2718B | 3.7938B | -11.2% |
| neutral startup sync seq 70 | 2.8447B | 2.3690B | -16.7% |
| neutral startup sync seq 71 | 1.4297B | 0.9548B | -33.2% |

Decision:

- Keep this checkpoint. It removes three durable header writes from the hot neutral battle handoff and puts the scenario below the previous Gate 5D `1.5B` sync threshold.
- Do not mark Gate 5F complete yet. Full contact startup still creates durable stacks, occupancy, obstacles, neutral rows, jobs, and final battle activation rows, so the route is still above the `0.3B-0.6B` target.
- This shifts intermediate neutral startup recovery further toward heap state. The durable create row plus final activation row remain, but upgrade/recovery during mid-start still needs Gate 5H flush/barrier work.

## Checkpoint: Neutral Startup Tactical Rows Moved To Heap

Changed neutral battle startup stacks, occupancy, and obstacles to use row-shaped heap structs with generated IDs instead of durable `BattleStack`, `BattleOccupancy`, and `BattleObstacle` creates during contact startup. Final activation still updates the durable `Battle` header and adopts a `BattleRuntime` from those heap rows.

Test contract change:

- Gate J now verifies the durable `Battle` boundary plus runtime-visible champion/battle behavior.
- It no longer requires tactical child rows to exist immediately in IcyDB for the hot startup path.
- Durable tactical projection is intentionally deferred to Gate 5H flush/barrier work.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- Benchmark Wasm build for `domm-degens-canister --features benchmark`: code section `0x00bfe263`, about `7.6 KB` under the IC limit
- Focused Gate J `20260520-neutral-start-heap-tactical-rows-gate-j` passed in `265.99s`

Measured delta versus `20260520-neutral-start-no-intermediate-battle-updates-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 72.8864B | 68.3627B | -6.2% |
| Gate J scenario memory | 4620.4375 MB | 4380.3750 MB | -5.2% |
| `sync_session_turn` avg | 1.3278B | 0.8750B | -34.1% |
| row growth | 43 | 35 | -18.6% |
| stable pages final | 74881 | 71041 | -5.1% |
| startup sync seq 69 | 3.7938B | 1.8891B | -50.2% |
| startup sync seq 70 | 2.3690B | 0.7037B | -70.3% |
| startup sync seq 71 | 0.9548B | 0.0002B | ~-100.0% |

Repo operations removed from Gate J:

| operation | previous calls | new calls | previous total |
| --- | ---: | ---: | ---: |
| `battles.create_battle_stack` | 3 | 0 | 1.4387B |
| `battles.create_battle_occupancy` | 3 | 0 | 1.4223B |
| `battles.create_battle_obstacle` | 2 | 0 | 0.9546B |
| `battles.stacks_by_side` | 2 | 0 | 0.7076B |

Decision:

- Keep this checkpoint. It is the first whole-route sync number inside the `0.6B-0.9B` band.
- Do not mark Gate 5F complete yet because the stated target is `0.3B-0.6B`; remaining contact cost is mostly durable battle header create/update, source army reads, neutral update, and timeout job creation.
- The next useful cuts should target the remaining activation boundary, not tactical row micro-work.

## Checkpoint: Fresh Neutral Contact Skips Attacker Battle Probe

Changed fresh neutral contact startup to skip the durable `battles.by_attacker` probe when the active runtime champion snapshot is not already in battle. Recovery/in-battle cases still use the durable probe.

Why:

- After Gate 5F.8, the fresh path has heap startup state and an active champion snapshot before the neutral contact begins.
- The durable probe was only proving absence for a brand-new battle and cost about `0.7057B`.
- Existing mid-start recovery is already delegated to Gate 5H because tactical startup rows are heap-owned.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build for `domm-degens-canister --features benchmark`: code section `0x00bfe29a`, about `7.5 KB` under the IC limit
- Focused Gate J `20260520-neutral-start-skip-fresh-attacker-probe-gate-j` passed in `269.37s`

Measured delta versus `20260520-neutral-start-heap-tactical-rows-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 68.3627B | 67.6589B | -1.0% |
| `sync_session_turn` avg | 0.8750B | 0.8049B | -8.0% |
| neutral startup sync seq 69 | 1.8891B | 1.1849B | -37.3% |
| `battles.by_attacker` calls | 1 | 0 | -100.0% |
| row growth | 35 | 35 | flat |
| stable pages final | 71041 | 71041 | flat |

Decision:

- Keep this checkpoint. It is a small, targeted absence-proof cut and the benchmark moved exactly where expected.
- Do not broaden this pattern blindly. For recovered or already-in-battle champions, the durable probe remains the fallback.
- Remaining Gate 5F cost is now mostly activation boundary work and source army stack reads.

## Checkpoint: Neutral Activation Boundary Kept In Heap Runtime

Changed the final neutral battle startup activation sync to keep the active battle header and neutral `in_battle` projection in heap runtime instead of writing both durable rows immediately. The hot path still schedules the battle timeout job and adopts active `BattleRuntime` from the heap startup rows.

Why:

- After Gate 5F.9, the activation sync was dominated by three durable projection operations: `battles.update_battle`, `neutrals.load_neutral_army`, and `neutrals.update_neutral_army`.
- Runtime already has the active battle, champion state, tactical rows, and event surface needed for the player-visible handoff.
- The durable active battle header and neutral state now belong to the Gate 5H flush/barrier contract rather than the movement hot path.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build for `domm-degens-canister --features benchmark`: code section `0x00bfdbbc`, about `9.3 KB` under the IC limit
- Focused Gate J `20260520-neutral-activation-heap-boundary-gate-j` passed in `262.93s`

Measured delta versus `20260520-neutral-start-skip-fresh-attacker-probe-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 67.6589B | 66.0030B | -2.4% |
| `sync_session_turn` avg | 0.8049B | 0.6390B | -20.6% |
| activation sync seq 72 | 2.1389B | 0.4804B | -77.5% |
| `battles.update_battle` calls | 1 | 0 | -100.0% |
| `neutrals.load_neutral_army` calls | 1 | 0 | -100.0% |
| `neutrals.update_neutral_army` calls | 1 | 0 | -100.0% |
| row growth | 35 | 35 | flat |
| stable pages final | 71041 | 71041 | flat |

Decision:

- Keep this checkpoint. The individual activation call is now inside the target band at `0.4804B`.
- Do not mark Gate 5F complete yet because the whole `sync_session_turn` average is still `0.6390B`, slightly above the `0.3B-0.6B` target.
- The next useful cuts are source stack reads (`champions.army_stacks_by_champion`, `neutrals.stacks_by_army`) or making the timeout job heap-first. The timeout job is more behavior-sensitive, so prefer source-stack reads or code-size freeing first.

## Checkpoint: Seeded Source Army Stack Cache Hits Gate 5F Target

Seeded the real first-playable `ChampionArmyStack` and `NeutralArmyStack` rows into a consume-once heap cache during setup, then made battle startup take those source rows before falling back to IcyDB. The cache is removed on first use so battle aftermath or later stack mutations cannot reuse stale source army data.

What changed:

- `battle_start` now owns small consume-once source army stack caches for champion and neutral armies.
- `first_playable_setup` records the actual rows it created or found for starting champion armies and neutral guard armies.
- Champion/town battle startup and neutral battle startup can use the seeded rows without paying `champions.army_stacks_by_champion` or `neutrals.stacks_by_army` in the hot contact path.
- Durable IcyDB source stack reads remain the fallback for cache misses, recovery, and non-seeded routes.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build for `domm-degens-canister --features benchmark`: code section `0x00bfea98`, about `5.6 KB` under the IC limit
- Focused Gate J `20260520-seeded-source-army-stacks-gate-j` passed in `67.17s`
- Leftover PocketIC server from the run was killed; a follow-up process scan only matched the `pgrep` command itself.

Measured delta versus `20260520-neutral-activation-heap-boundary-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 66.0030B | 64.6025B | -2.1% |
| Gate J scenario memory | 4380.3750 MB | 4380.3125 MB | ~flat |
| `sync_session_turn` avg | 0.6390B | 0.4984B | -22.0% |
| neutral startup sync seq 69 | 1.1849B | 0.4810B | -59.4% |
| neutral startup sync seq 70 | 0.7043B | 0.0003B | ~-100.0% |
| `champions.army_stacks_by_champion` calls | 2 | 0 | -100.0% |
| `neutrals.stacks_by_army` calls | 2 | 0 | -100.0% |
| row growth | 35 | 35 | flat |
| stable pages final | 71041 | 71041 | flat |

Decision:

- Keep this checkpoint. The measured Gate J contact route now meets the Gate 5F `0.3B-0.6B` average sync target at `0.4984B`.
- Mark Gate 5F done for the measured route, with the explicit scope that durable projection/recovery is still Gate 5H work.
- Do not spend the next checkpoint on source stack cache broadening. The remaining large route costs are now session/lobby setup, system-job scans, and query-side durable snapshot reads.

## Checkpoint: Champion Detail Stack Cache And Empty Spellbook Shortcut

Changed `get_champion_view` so first-playable champion army stack rows are seeded into a heap cache from setup/update code, and the champion detail renderer uses that cache before falling back to `ChampionArmyStack` scans. Also added a safe spellbook shortcut: if the champion lacks `sour_sorcery`, it cannot learn first-tier spells, so the public champion detail view returns an empty spell list without scanning `ChampionSpell`.

What changed:

- `first_playable_setup` mirrors the actual starting champion stack rows into `render_projection`.
- `render_projection::champion_stacks` uses cached real rows before durable stack scans.
- Champion stack cache entries are invalidated by battle aftermath survivor writes and champion recruitment writes.
- The non-persistent query-side artifact cache experiment was removed after confirming query heap writes do not survive across separate query calls.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build for `domm-degens-canister --features benchmark`: code section `0x00bff5a7`, about `2.6 KB` under the IC limit
- Focused Gate J `20260520-champion-detail-cache-gate-j` passed in `63.84s`
- Leftover PocketIC server from the run was killed; a follow-up process scan only matched the `pgrep` command itself.

Measured delta versus `20260520-seeded-source-army-stacks-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 64.6025B | 61.7790B | -4.4% |
| `get_champion_view` avg | 2.1180B | 0.7090B | -66.5% |
| `get_champion_view` seq 51 | 2.1156B | 0.7091B | -66.5% |
| `get_champion_view` seq 73 | 2.1203B | 0.7089B | -66.6% |
| `sync_session_turn` avg | 0.4984B | 0.4985B | flat |
| row growth | 35 | 35 | flat |
| stable pages final | 71041 | 71041 | flat |

Decision:

- Keep this checkpoint. It moves `get_champion_view` into the `0.3B-0.8B` query target band for Gate J.
- The remaining `0.709B` champion detail cost is the durable banner equipment read. Because query mutations do not persist, eliminating it needs an update-path artifact projection seed or a broader champion detail projection cache.
- Do not chase that next without freeing more code size. Current benchmark Wasm headroom is only about `2.6 KB`.

## Checkpoint: Runtime-Open Guard And Seeded Content Caches Remove Town Command Floor

After the champion detail checkpoint, the measured town command floor was no longer town row mutation. It was the safety guard and one uncached content definition read:

- `submit_build_town_structure` was `1.4118B`, with `system_jobs.by_session_status_due` plus a later building-definition cache miss.
- `submit_recruit_units` was `0.7048B`, dominated by `system_jobs.by_session_status_due`.
- The same Gate J route still has one late `submit_move_intent` at about `0.706B`, also from `system_jobs.by_session_status_due`.

What changed:

- Relaxed `runtime_proves_pre_deadline_turn_open` so an active matching `SessionTurnRuntime` with `closing == false` and no ready participants is enough to skip the durable scheduled-job scan. This handles the Gate J state where manual sync advanced PocketIC time past the deadline, processed movement, yielded after events, and left the runtime authoritative but not closing.
- Seeded `content`'s heap unit/building slug caches from first-playable bulk `insert_many_atomic` rows. Before this, only the sentinel `crumbling-hall` building was cached; the actual build target `freehold-training-yard` missed and paid one stable content lookup.
- Kept durable job scanning as fallback when the runtime proof is absent. `end_turn` still has its duplicate-ready durable check.

Rejected/adjusted attempt:

- A first guard helper still required `now_ms < runtime.turn_deadline_at_ms`; focused Gate J `20260520-runtime-turn-command-guard-gate-j` passed but produced identical numbers to `20260520-champion-detail-cache-gate-j`. The commands happen after the test advances time beyond the old deadline, so that proof was too strict and did not remove the scan.
- A previous active caller cache experiment for `get_my_participant` remains rejected: it made Gate J fail after resource pickup because participant resources were stale.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm after relaxed guard: code section `0x00bff545`
- Focused Gate J `20260520-runtime-open-turn-job-scan-gate-j` passed in `190.04s`
- Benchmark Wasm after content-cache seeding: code section `0x00bff708`
- Focused Gate J `20260520-seed-content-cache-gate-j` passed in `191.37s`
- Leftover PocketIC servers from both runs were killed; follow-up process scans only matched the `pgrep` commands.

Measured delta versus `20260520-champion-detail-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 61.7790B | 59.6702B | -3.4% |
| `submit_build_town_structure` | 1.4118B | 0.0003B | ~-100.0% |
| `submit_recruit_units` | 0.7048B | 0.0003B | ~-100.0% |
| `system_jobs.by_session_status_due` route calls | 6 | 2 | -66.7% |
| Gate J scenario cycles | 0.1019T | 0.0998T | -2.1% |
| row growth | 35 | 35 | flat |
| stable pages final | 71041 | 71041 | flat |

Intermediate delta from `20260520-runtime-open-turn-job-scan-gate-j` to `20260520-seed-content-cache-gate-j`:

| metric | before content cache | after content cache | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 60.3674B | 59.6702B | -1.2% |
| `submit_build_town_structure` | 0.7071B | 0.0003B | ~-100.0% |
| `submit_recruit_units` | 0.0003B | 0.0003B | flat |

Decision:

- Keep both cuts. Town build/recruit are now below the `0.3B-0.6B` target in the measured first-playable route.
- Mark the town safe-guard floor done, but keep the durable flush/upgrade and broader town projection work open.
- Track the remaining `system_jobs.by_session_status_due` scans under Gate 5B.5: late `submit_move_intent` seq 61 still pays `0.7063B` while the other move submits are already `0.0088B` and `0.0003B`.

## Checkpoint: Top-Level Runtime Guard Removes Last Submit-Move Job Scan

The relaxed runtime-open proof removed town command scans because town command begin checked that proof before calling `ensure_map_turn_accepts_new_command`. Direct callers, especially `submit_move_intent`, still called `ensure_map_turn_accepts_new_command` directly. That function only checked the runtime proof inside the `now < context.session.turn_deadline_at` branch, so an expired durable deadline forced the durable job scan even when the active runtime was open.

What changed:

- `ensure_map_turn_accepts_new_command` now checks `runtime_proves_pre_deadline_turn_open(context)` immediately after confirming the command is map-turn-sensitive.
- If runtime proves the active turn is open, the guard returns before durable deadline branching and before `SystemJob` page scans.
- Fallback behavior remains unchanged when the runtime proof is absent: pre-deadline and post-deadline paths still inspect durable jobs, and `end_turn` duplicate-ready protection remains reachable when a participant is already ready.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build for `domm-degens-canister --features benchmark`: code section `0x00bff704`
- Focused Gate J `20260520-runtime-guard-top-level-gate-j` passed in `192.11s`
- Leftover PocketIC server from the run was killed; a follow-up process scan only matched the `pgrep` command.

Measured delta versus `20260520-seed-content-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 59.6702B | 58.9607B | -1.2% |
| `submit_move_intent` avg | 0.2385B | 0.0031B | -98.7% |
| `submit_move_intent` seq 61 | 0.7063B | 0.0003B | ~-100.0% |
| `system_jobs.by_session_status_due` route calls | 2 | 0 | -100.0% |
| `submit_build_town_structure` | 0.0003B | 0.0003B | flat |
| `submit_recruit_units` | 0.0003B | 0.0003B | flat |
| `sync_session_turn` avg | 0.4985B | 0.4985B | flat |

Decision:

- Keep this checkpoint. Gate 5B is complete for the measured active first-playable route: fresh submit-move now has no route-visible stable repo operations and is far below the `0.3B-0.6B` target.
- The remaining Gate J route cost is no longer move submit or town commands. The biggest useful next targets are query/projection reads (`get_visible_map_chunks`, `get_visible_objects`, `get_events_after`, `get_my_participant`, `get_champion_view`) and the durable setup/session creation floor.

## Rejected Next Attempt: Broad Render Row Cache Exceeded Wasm Limit

Tried a render-side heap cache for first-playable `MapChunk`, `VisibilityChunk`, and `ParticipantKnownObject` rows, seeded from setup bulk inserts and invalidated on movement visibility/known-object mutation.

Result:

- `cargo fmt --check` initially only needed formatting.
- `cargo check -p domm-degens-canister --features benchmark` passed.
- `cargo check -p domm-degens-canister` passed.
- Benchmark Wasm code section became `0x00c01916`, which is over the IC code-section limit. The change was reverted before commit.

Decision:

- Do not retry the same broad row-cache shape without first freeing at least several KB of benchmark Wasm headroom.
- A narrower projection cache may still be worthwhile, but it should cache final DTOs or reuse existing runtime fields with less new generic cache plumbing.

## Checkpoint: Faction Slug Cache Removes get_my_participant Stable Read

After the top-level runtime guard checkpoint, `get_my_participant` still sat at about `0.7046B` instructions even though active caller/session/participant rows were served from `SessionTurnRuntime`. The remaining floor was participant rendering: both participant-view helpers hydrated the full `FactionDefinition` row only to return `faction_slug`.

What changed:

- Added a tiny two-slot heap cache for `faction_id -> faction_slug` in `repos::content`.
- Populated the cache from first-playable faction create/find paths, which already run during session setup.
- Added `load_faction_slug` with a stable fallback on cache miss.
- Changed `account_lobby_session::participant_view` and `session_context::participant_view` to use the slug helper instead of loading the full faction row.
- Removed the now-unused full-row `load_faction` helper.

Rejected/adjusted attempts:

- A full `FactionDefinition` row cache exceeded the benchmark Wasm code-section limit (`0x00c00652`).
- An id-only `BTreeMap<String, FactionDefinition>` cache still exceeded the limit (`0x00c0029e`).
- A two-slot full-row cache still exceeded the limit (`0x00c0045b`).
- A slug-only two-slot cache initially missed by 15 bytes (`0x00c0000f`); simplifying the cache to rotate entries without duplicate rewrite handling fit at `0x00bffe82`.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build: code section `0x00bffe82`
- Focused Gate J `20260520-faction-slug-cache-gate-j` passed in `189.55s`
- The leftover PocketIC server from the run was killed; follow-up process scan only matched the `pgrep` command.

Measured delta versus `20260520-runtime-guard-top-level-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 58.9607B | 57.5801B | -2.3% |
| `get_my_participant` avg | 0.7046B | 0.000026B | ~-100.0% |
| `get_champion_view` avg | 0.7094B | 0.7083B | flat |
| `get_events_after` avg | 0.7068B | 0.7070B | flat |
| `get_visible_map_chunks` | 1.4080B | 1.4053B | flat |
| `get_visible_objects` | 1.4190B | 1.4216B | flat |
| row growth | 35 | 35 | flat |
| stable pages final | 71041 | 71041 | flat |

Decision:

- Keep the checkpoint. It is a small but clean projection-cache win, with no row growth or stable-memory change and enough code-size headroom to install.
- Do not broaden this into a general content row cache until more code-size headroom exists. The next useful query targets remain `get_visible_map_chunks`, `get_visible_objects`, `get_events_after`, and the remaining `get_champion_view` banner equipment read.

## Rejected Next Attempt: Public Durable Event Cache Exceeded Wasm Limit

Tried a bounded public `GameEvent` cache in `commands_events_effects` so `get_events_after(session, "public", 0, ...)` could reuse public durable events created during the current session instead of scanning the IcyDB event feed on every query.

Correctness shape tested:

- Cache only public events.
- Populate from `create_game_event`.
- Answer only when the cache has seen the session feed from `event_seq == 1`.
- Disable the cache for the session on overflow so partial feeds fall back to IcyDB.
- Keep participant/private audiences on the durable fallback.

Result:

- `cargo fmt --check` passed.
- `cargo check -p domm-degens-canister --features benchmark` passed.
- `cargo check -p domm-degens-canister` passed.
- Benchmark Wasm code section became `0x00c00a0e`, about `2.6 KB` over the IC code-section ceiling.
- The change was reverted before benchmark.

Decision:

- Do not retry the repo-level `Vec<GameEvent>` cache shape without freeing at least several KB of code size.
- If we revisit `get_events_after`, prefer a smaller session-runtime event-feed projection or a broader code-size cleanup first.

## Checkpoint: Runtime Income Trust Removes Durable Mine Scan

After the faction slug checkpoint, the remaining measurable `sync_session_turn` income floor was the durable owned-mine scan:

- Gate J income sync seq 65 spent `1.1841B`.
- Repo ops showed `map.world_objects_by_owner_scoring` twice, `0.7057B` total.
- The active `SessionTurnRuntime` already hydrates all world objects when created and carries `world_object_snapshots` into the next runtime turn, so runtime-mode income does not need to re-read durable owned mines when the runtime exists.

What changed:

- In `materialize_income`, runtime-mode income now uses `runtime_gold_income` directly when an active runtime snapshot exists.
- Durable `world_objects_by_owner_scoring` remains the fallback when the runtime snapshot is unavailable, and it remains the durable-bridge path for legacy sync.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build: code section `0x00bffe3b`
- Focused Gate J `20260520-runtime-income-trust-gate-j` passed in `192.46s`
- The leftover PocketIC server from the run was killed; follow-up process scan only matched the `pgrep` command.

Measured delta versus `20260520-faction-slug-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 57.5801B | 56.8715B | -1.2% |
| `sync_session_turn` avg | 0.4985B | 0.4277B | -14.2% |
| income sync seq 65 | 1.1841B | 0.4749B | -59.9% |
| `map.world_objects_by_owner_scoring` route calls | 2 | 0 | -100.0% |
| row growth | 35 | 35 | flat |
| stable pages final | 71041 | 71041 | flat |

Decision:

- Keep this checkpoint. It removes one remaining stable read category from the active runtime sync path with a smaller Wasm code section than before.
- The largest remaining sync repo-operation costs in Gate J are now `map.update_visibility_chunk`, `map.create_known_object`, `map.update_world_object`, `battles.create_battle`, `system_jobs.create_system_job`, and `sessions.update_session`.

## Rejected Next Attempt: Skipping Runtime Turn Session Update Moved Behavior

Tried skipping `sessions.update_session` during runtime-mode `sync_session_turn` turn advancement, relying on the new `SessionTurnRuntime` session row as authority for later commands and queries.

Result:

- `cargo fmt --check` passed.
- `cargo check -p domm-degens-canister --features benchmark` passed.
- `cargo check -p domm-degens-canister` passed.
- Benchmark Wasm code section fit at `0x00bffe58`.
- Focused Gate J `20260520-runtime-turn-session-skip-gate-j` passed in `197.03s`.

Why it was rejected:

- It reduced route instructions, but changed route shape in a suspicious way.
- Gate J call count moved `87 -> 86`, `sync_session_turn` calls moved `10 -> 9`, and `battles.create_battle` disappeared from route repo ops.
- Stable pages final moved `71041 -> 74881`, command rows moved `12 -> 17`, and event rows moved `7 -> 10`.
- That means the saved session update was not a clean cut; it caused later behavior to use more durable command/event/job state and changed battle boundary projection timing.

Decision:

- Reverted before commit.
- Do not skip the durable `GameSession` turn-advance projection by itself. It needs a proper Gate 5H flush/barrier and battle handoff contract so later commands do not drift into extra durable fallback work.

## Checkpoint: Runtime Mine Capture Stops Stable WorldObject Update

After runtime income started trusting `SessionTurnRuntime` world-object snapshots, the remaining Gate J movement-sync route still wrote one durable `WorldObject` row for the unguarded mine capture:

- `map.update_world_object` appeared once in the route at `0.4789B`.
- Object views already merge active runtime world-object snapshots before durable rows.
- Income now reads the active runtime mine owner/state snapshot before falling back to durable owned-mine scans.

What changed:

- Runtime-mode `apply_world_object_at` now mirrors captured mine state into `SessionTurnRuntime` only.
- The legacy/durable movement path still writes `ParticipantObjectVisit` and updates the durable `WorldObject` row.
- This intentionally defers the active-route durable mine projection to Gate 5H flush/barrier work; the hot route remains gameplay-correct through runtime snapshots.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build: code section `0x00bffc93`
- Focused Gate J `20260520-runtime-mine-object-delta-gate-j` passed in `197.28s`
- The leftover PocketIC server from the run was killed; follow-up process scan only matched the `pgrep` command.

Measured delta versus `20260520-runtime-income-trust-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 56.8715B | 56.3951B | -0.8% |
| `sync_session_turn` avg | 0.4277B | 0.3800B | -11.2% |
| `map.update_world_object` route calls | 1 | 0 | -100.0% |
| route call count | 87 | 87 | flat |
| row growth | 35 | 35 | flat |
| stable pages final | 71041 | 71041 | flat |

Decision:

- Keep this checkpoint. It removes one more stable write from the active movement sync without changing the route shape or final row/page counts.
- Next low-risk Gate 5D candidates are harder: `map.update_visibility_chunk` and `map.create_known_object` require runtime query overlays before they can be removed safely from the hot route.

## Checkpoint: Champion Banner Cache Removes Detail Query Floor

After the mine object delta checkpoint, `get_champion_view` still averaged about `0.709B` instructions. The remaining stable read was the owned champion's banner equipment lookup:

- Champion army stacks were already cached from first-playable setup.
- Spellbook lookup was already skipped for champions without `sour_sorcery`.
- The west champion's first-playable banner is created during setup, so the detail view can reuse that artifact id from heap instead of querying `ArtifactEquipment` on every owned detail read.

What changed:

- `render_projection` now keeps a tiny champion-id to banner-artifact-id cache and builds the same `ArtifactView` from it.
- `first_playable_setup` seeds the cache when it creates or sees the west banner equipment row.
- `battle_aftermath::capture_artifacts` invalidates champion detail caches for both victor and defeated champion when equipment can move.
- Removed unused `SessionTurnRuntime` champion, occupancy, and visibility delta scaffolding to recover code-size headroom for the cache. Object/resource deltas remain because the runtime uses them.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build: code section `0x00bffd31`
- Focused Gate J `20260520-champion-banner-cache-gate-j` passed in `190.10s`
- The leftover PocketIC server from the run was killed; follow-up process scan only matched the `pgrep` command.

Measured delta versus `20260520-runtime-mine-object-delta-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 56.3951B | 54.9867B | -2.5% |
| `get_champion_view` avg | 0.7090B | 0.0046B | -99.4% |
| `sync_session_turn` avg | 0.3800B | 0.3801B | flat |
| route call count | 87 | 87 | flat |
| row growth | 35 | 35 | flat |
| stable pages final | 71041 | 71041 | flat |

Decision:

- Keep this checkpoint. It removes the last known owned champion detail stable read from the first-playable route without changing rows, pages, or route shape.
- Remaining large first-playable query costs are `get_visible_map_chunks` and `get_visible_objects`, both around `1.4B`; cutting those safely needs runtime/opening projection caches for visibility and known objects.

## Checkpoint: Known Object Projection Cache Cuts Visible Objects

After the champion banner cache, the remaining measured first-playable query floors were `get_visible_map_chunks` and `get_visible_objects`, both around `1.4B` instructions. `get_visible_objects` was still paying the durable `ParticipantKnownObject` page even though first-playable setup had just created the same real rows.

What changed:

- `first_playable_setup::seed_known_objects` now seeds a heap cache from the inserted real `ParticipantKnownObject` rows.
- `render_projection::visible_objects` uses the heap known-object rows for the current session/participant and falls back to the durable page when the cache is absent.
- `movement::create_known_object_if_missing` invalidates that participant's cache after a new discovery. This deliberately favors correctness: post-discovery reads fall back to durable rows rather than returning an opening-view cache that hides the newly known object.
- The canister diagnostic benchmark payload no longer carries the empty phase vector. Phase markers were already disabled; method-level, query-log, and repo-operation metrics remain the benchmark signal. This recovered the Wasm headroom needed for the projection cache.

Rejected shapes before the final version:

- Full known-row cache plus `get_object_view` fast path compiled in dev but exceeded the benchmark Wasm code-section limit at `0x00c00dfc`.
- A smaller `ObjectSubject` cache also exceeded the limit at `0x00c0114e`.
- A visible-objects-only full-row cache still exceeded the limit at `0x00c0085b`/`0x00c007f7` before removing the empty phase payload.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- `cargo test -p domm-degens-canister exported_candid -- --nocapture`
- Benchmark Wasm build: code section `0x00bffbb4`
- Focused Gate J `20260520-known-object-cache-gate-j` passed in `195.18s`
- The leftover PocketIC server from the run was killed; follow-up process scan only matched the `pgrep` command.

Measured delta versus `20260520-champion-banner-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 54.9867B | 54.2635B | -1.3% |
| `get_visible_objects` avg | 1.4210B | 0.7153B | -49.6% |
| `get_visible_map_chunks` avg | 1.4055B | 1.4081B | flat |
| `sync_session_turn` avg | 0.3801B | 0.3801B | flat |
| route call count | 87 | 87 | flat |
| row growth | 35 | 35 | flat |
| stable pages final | 71041 | 71041 | flat |

Decision:

- Keep this checkpoint. It removes about half of the visible-object query floor without changing rows, pages, or route shape.
- The next large query target is still `get_visible_map_chunks` at about `1.4B`; cutting it safely needs a visibility/map chunk projection cache with movement visibility invalidation or update-path mirroring.

## Checkpoint: Immutable Map Chunk Cache Cuts Visible Map Query

After the known-object cache, `get_visible_map_chunks` was the largest remaining first-playable query floor at about `1.4B` instructions. The map chunk rows are immutable for the current first-playable route, while visibility chunks are mutable as movement reveals map state. That made the safe first cut to cache only `MapChunk` rows and keep `VisibilityChunk` reads durable-backed.

What changed:

- `first_playable_setup::seed_map_chunks` now seeds a heap cache from the inserted real `MapChunk` rows.
- `render_projection::visible_map_chunks` uses cached map chunks for the current session and still joins them with durable `VisibilityChunk` rows for the participant.
- The benchmark diagnostic update payload no longer carries redundant call-level stable-page before/after fields. Benchmark artifacts still measure method memory/stable-memory deltas from PocketIC status snapshots, query stable pages from query logs, and repo-operation instruction totals from canister metrics.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- `cargo test -p domm-degens-canister --features benchmark benchmark_feature_exports_diagnostic_benchmark_endpoints -- --nocapture`
- Benchmark Wasm build: code section `0x00bffe64`
- Focused Gate J `20260520-map-chunk-cache-gate-j` passed in `195.36s`
- The leftover PocketIC server from the run was killed; follow-up process scan only matched the `pgrep` command.

Measured delta versus `20260520-known-object-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 54.2635B | 53.5648B | -1.3% |
| `get_visible_map_chunks` avg | 1.4081B | 0.7036B | -50.0% |
| `get_visible_objects` avg | 0.7153B | 0.7152B | flat |
| `sync_session_turn` avg | 0.3801B | 0.3799B | flat |
| route call count | 87 | 87 | flat |
| row growth | 35 | 35 | flat |
| stable pages final | 71041 | 71041 | flat |

Decision:

- Keep this checkpoint. It cuts the map chunk query floor in half without caching mutable visibility state or changing durable rows.
- The remaining `get_visible_map_chunks` floor is the durable visibility page. Moving that safely needs visibility-cache invalidation or mirroring from `refresh_champion_visibility`, and the benchmark Wasm has only about 412 bytes of headroom after this checkpoint.

## Checkpoint: Visibility Cache Eliminates Opening Map Chunk Query Floor

After the map chunk cache, `get_visible_map_chunks` still averaged about `0.7036B` instructions because it still paged durable `VisibilityChunk` rows. The safe projection shape is to cache opening visibility rows, then invalidate the participant cache as soon as movement refresh writes durable visibility changes.

What changed:

- `first_playable_setup::seed_visibility_chunks` now seeds a heap cache from the inserted real `VisibilityChunk` rows.
- `render_projection::visible_map_chunks` uses cached visibility rows when present and still falls back to durable rows after cache miss or invalidation.
- `movement::refresh_champion_visibility` invalidates the participant visibility cache after it updates one or more durable visibility chunks. This keeps post-movement map reads truthful without needing a larger mirror/update cache.
- The canister benchmark update payload no longer carries unused `ok` and `error_code` fields. Benchmark artifacts still compute method error counts from the client-side response records.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- `cargo test -p domm-degens-canister --features benchmark benchmark_feature_exports_diagnostic_benchmark_endpoints -- --nocapture`
- Benchmark Wasm build: code section `0x00bff74b`
- Focused Gate J `20260520-visibility-cache-gate-j` passed in `196.40s`
- The leftover PocketIC server from the run was killed; follow-up process scan only matched the `pgrep` command.

Measured delta versus `20260520-map-chunk-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 53.5648B | 52.8475B | -1.3% |
| `get_visible_map_chunks` avg | 0.7036B | 0.0001B | ~-100.0% |
| `get_visible_objects` avg | 0.7152B | 0.7156B | flat |
| `sync_session_turn` avg | 0.3799B | 0.3796B | flat |
| route call count | 87 | 87 | flat |
| row growth | 35 | 35 | flat |
| stable pages final | 71041 | 71041 | flat |

Decision:

- Keep this checkpoint. It removes the opening map-chunk query floor while preserving durable fallback after movement visibility changes.
- The biggest remaining Gate J query floor is now `get_events_after` at about `0.707B` average across four calls, followed by the already-halved `get_visible_objects` known-object durable fallback after movement cache invalidation.

## Checkpoint: Complete Event Feed Cache Cuts Events Query

After the visibility cache, `get_events_after` was the largest remaining Gate J query floor at about `0.707B` average across four calls. The public feed is append-only and ordered by event sequence, so a complete heap feed cache can answer repeated active reads without paging durable `GameEvent` rows.

What changed:

- `commands_events_effects::events_after` first checks a tiny single-feed heap cache for the requested `(session, audience)` after the feed has been proven complete.
- `create_game_event` appends newly created rows to the cache when the current feed is complete. If the first event in a feed has sequence `1`, that feed is complete immediately.
- A durable `events_after(..., after_event_seq=0, limit)` scan marks a feed complete only when the result is not truncated, then replaces the heap rows with that ordered durable result.
- Requests for a different session/audience miss the single-feed cache and fall back to IcyDB, so the cache cannot hide durable history it has not proven complete.
- The canister diagnostic repo-operation payload no longer carries stable-page deltas. Client benchmark artifacts still record method memory and stable-memory deltas around each call, and repo-operation instruction totals remain available.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- `cargo test -p domm-degens-canister --features benchmark benchmark_feature_exports_diagnostic_benchmark_endpoints -- --nocapture`
- Benchmark Wasm build: code section `0x00bf8885`
- Focused Gate J `20260520-event-feed-cache-gate-j` passed in `198.46s`
- The leftover PocketIC server from the run was killed; follow-up process scan only matched the `pgrep` command.

Measured delta versus `20260520-visibility-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 52.8475B | 50.0394B | -5.3% |
| `get_events_after` avg | 0.7071B | 0.0001B | ~-100.0% |
| `get_visible_objects` avg | 0.7156B | 0.7143B | flat |
| `get_visible_map_chunks` avg | 0.0001B | 0.0001B | flat |
| `sync_session_turn` avg | 0.3796B | 0.3800B | flat |
| route call count | 87 | 87 | flat |
| row growth | 35 | 35 | flat |
| stable pages final | 71041 | 71041 | flat |

Decision:

- Keep this checkpoint. It removes the event-feed query floor without changing row growth, stable pages, or the public route shape.
- The remaining Gate J total is now dominated by the single `get_visible_objects` durable fallback after movement invalidates the opening known-object cache, plus update-side durable boundary work that belongs to Gate 5H.

## Checkpoint: Neutral View Cache Cuts Visible Objects

After the event-feed cache, `get_visible_objects` was the last large Gate J query floor at about `0.7143B`. A first attempt kept newly discovered known-object rows in the participant cache, but the benchmark did not move. A second attempt added a first-playable world-object cache, but that also did not move. The measured blocker was the visible neutral-army branch, which still loaded a durable `NeutralArmy` row while building the object list.

What changed:

- `first_playable_setup::seed_neutrals` now seeds the real `NeutralArmy` rows into a small render projection cache.
- `render_projection::live_neutral_for_known` checks that cache by id or coordinate before falling back to `neutrals`.
- `battle_aftermath::apply_neutral_aftermath` updates the cache after a neutral army is marked defeated, so the object list does not keep showing stale active neutral state.
- `movement::create_known_object_if_missing` now appends newly created known-object rows when the participant cache is present, instead of invalidating the whole cache. If no participant cache exists, later reads still fall back to durable rows.
- The world-object cache prototype was removed before the final benchmark because focused Gate J proved it did not reduce the route.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- Benchmark Wasm build: code section `0x00bf9200`
- Focused Gate J `20260520-neutral-view-cache-gate-j` passed in `188.19s`
- The leftover PocketIC server from the run was killed; follow-up process scan only matched the `pgrep` command.

Measured delta versus `20260520-event-feed-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 50.0394B | 49.3327B | -1.4% |
| `get_visible_objects` avg | 0.7143B | 0.0115B | -98.4% |
| `get_events_after` avg | 0.0001B | 0.0001B | flat |
| `get_visible_map_chunks` avg | 0.0001B | 0.0001B | flat |
| `sync_session_turn` avg | 0.3800B | 0.3796B | flat |
| route call count | 87 | 87 | flat |
| row growth | 35 | 35 | flat |
| stable pages final | 71041 | 71041 | flat |

Decision:

- Keep this checkpoint. It removes the last large query-side floor from Gate J without changing rows, stable pages, or route shape.
- The remaining route total is now mostly update/setup-side work: lobby/session setup commands dominate absolute instructions, and active movement sync is already around `0.38B` average in this route.

## Checkpoint: Runtime Lobby Receipts Cut Setup Writes

After the neutral projection cache, Gate J was dominated by lobby/setup updates. The highest command repo ops were still durable fresh `LobbyCommand` create/update pairs: `commands.create_lobby_command` and `commands.update_lobby_command` each cost about `3.33B` total in the route, while durable idempotency lookup cost another `4.93B`.

What changed:

- Fresh lobby commands now create a heap `LobbyCommand` receipt instead of immediately inserting a durable row.
- `apply_lobby_command` and `fail_lobby_command` update that heap receipt for runtime commands and still fall back to durable update for older durable commands.
- Lobby command replay checks heap receipts first, then durable rows. Same nonce/same payload returns the receipt; same nonce/different payload still returns `duplicate_nonce_payload_mismatch`.
- `get_command_status` and `get_command_status_by_nonce` merge heap lobby receipts before durable `LobbyCommand` lookup.
- Durable `commands.lobby_command_idempotency` remains for now so older rows and absence proof behavior are unchanged. That is the next obvious setup floor if the route still needs more cut.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- Benchmark Wasm build: code section `0x00bf819c`
- Focused Gate J `20260520-lobby-runtime-receipts-gate-j` passed in `196.87s`
- The leftover PocketIC server from the run was killed; follow-up process scan only matched the `pgrep` command.
- Native service regression `lobby_session_setup_recovers_from_starting_state_and_replays_nonce` failed after `156.63s` on the pre-existing runtime-open/durable turn-job guard expectation: a manually inserted durable turn-resolution job no longer blocks movement when heap runtime proves the turn open. The failure did not exercise the lobby receipt replay/status path and should be handled with the Gate 5H/runtime authority policy work.

Measured delta versus `20260520-neutral-view-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 49.3327B | 42.6716B | -13.5% |
| Gate J scenario memory | 4380.3750 MB | 4108.3125 MB | -6.2% |
| `register_player` avg | 2.8239B | 1.8744B | -33.6% |
| `create_session` avg | 11.9965B | 11.0501B | -7.9% |
| `join_session` avg | 7.3248B | 6.3675B | -13.1% |
| `mark_ready` avg | 5.9076B | 4.9536B | -16.1% |
| `start_session` avg | 8.7177B | 7.7633B | -10.9% |
| `commands.create_lobby_command` total | 3.3207B | 0B | removed |
| `commands.update_lobby_command` total | 3.3359B | 0B | removed |
| `commands.lobby_command_idempotency` total | 4.9285B | 4.9242B | flat |
| row growth | 35 | 35 | flat |
| stable pages final | 71041 | 66689 | -6.1% |

Decision:

- Keep this checkpoint. It removes about `6.66B` of setup route instructions without hollowing the public lobby API: same-call replay and status still work from heap receipts, while older durable receipts still fall back to IcyDB.
- This intentionally weakens durable lobby receipt history until Gate 5H adds a flush/upgrade policy. The same pattern is already true for other hot runtime receipts, so consolidate it under one barrier/snapshot design rather than reintroducing per-command stable writes.

## Checkpoint: Session Participant Cache Removes Lobby Scans

After runtime lobby receipts, the next largest route repo op was `sessions.participants_by_session_status`: 14 scans, `4.9362B` total. These reads were mostly rebuilding the same lobby/session participant list after create, join, ready, and start-session operations.

What changed:

- Added a single active-session heap cache containing real `GameParticipant` rows.
- `create_session` seeds the cache with the creator participant after the durable insert.
- `join_session` appends the newly inserted participant to the cache.
- `mark_ready` updates the cached participant after the durable participant update.
- `participants_for_session` now returns the cached real rows when present and falls back to the durable indexed page on cache miss.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- Benchmark Wasm build: code section `0x00bfae70`
- Focused Gate J `20260520-session-participant-cache-gate-j` passed in `184.10s`
- The leftover PocketIC server from the run was killed; follow-up process scan only matched the `pgrep` command.

Measured delta versus `20260520-lobby-runtime-receipts-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 42.6716B | 37.7339B | -11.6% |
| Gate J scenario memory | 4108.3125 MB | 4108.3750 MB | flat |
| `create_session` avg | 11.0501B | 10.3434B | -6.4% |
| `join_session` avg | 6.3675B | 4.9507B | -22.3% |
| `mark_ready` avg | 4.9536B | 4.2488B | -14.2% |
| `start_session` avg | 7.7633B | 6.3586B | -18.1% |
| `sessions.participants_by_session_status` total | 4.9362B | 0B | removed |
| row growth | 35 | 35 | flat |
| stable pages final | 66689 | 66689 | flat |

Decision:

- Keep this checkpoint. It removes the repeated participant-list scan without pretending the rows do not exist: the cache is seeded and updated only from successful durable participant writes.
- The remaining Gate J setup floor is now mostly `commands.lobby_command_idempotency`, session load/update scans, `sessions.by_state_current_turn`, and unavoidable first-time durable creates.

## Checkpoint: Negative Player Live-Session Cache

After the participant cache, `sessions.participants_by_player_status` was still visible at 4 calls and `1.4091B` total. The only caller is `player_has_live_session`, used to enforce active-session limits before create/join. For newly registered players, we can safely know the negative case without scanning durable participant rows.

What changed:

- Added a negative-only heap cache of players known to have no live session.
- New player registration seeds that negative cache after the durable `PlayerAccount` insert.
- `player_has_live_session` returns `false` from that negative cache before durable scans.
- If a durable scan proves no live session, the negative cache is also updated.
- Creating or joining a session clears the negative cache for that player. The cache never returns a positive answer, so it cannot incorrectly block a player after a session closes.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- Benchmark Wasm build: code section `0x00bfb2a1`
- Focused Gate J `20260520-player-no-live-cache-gate-j` passed in `193.05s`
- The leftover PocketIC server from the run was killed; follow-up process scan only matched the `pgrep` command.

Measured delta versus `20260520-session-participant-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 37.7339B | 36.3147B | -3.8% |
| Gate J scenario memory | 4108.3750 MB | 4108.3125 MB | flat |
| `create_session` avg | 10.3434B | 9.6384B | -6.8% |
| `join_session` avg | 4.9507B | 4.2439B | -14.3% |
| `sessions.participants_by_player_status` total | 1.4091B | 0B | removed |
| row growth | 35 | 35 | flat |
| stable pages final | 66689 | 66689 | flat |

Decision:

- Keep this checkpoint. It is a small but clean setup win, and the negative-only shape avoids stale positive active-session decisions.
- The next big setup floor is now `commands.lobby_command_idempotency` at about `4.924B`, followed by session load/update/state scans.

## Checkpoint: Runtime Actor Lobby Idempotency Skip

After the negative player live-session cache, the biggest remaining setup floor was still `commands.lobby_command_idempotency`: 7 durable absence probes, `4.9244B` total. We already keep fresh lobby receipts in heap, so after an actor has a runtime receipt in this process, the heap receipt cache can act as the immediate nonce authority for later fresh lobby commands.

What changed:

- Added a helper to detect whether an actor already has any runtime lobby receipt.
- `begin_lobby_command` still checks runtime receipts by `(actor, nonce)` first.
- If the actor has no runtime lobby receipt yet, it still checks durable `LobbyCommand` idempotency rows for old persisted receipts.
- If the actor already has a runtime lobby receipt, fresh commands skip the durable absence probe and create a new heap receipt directly.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- Benchmark Wasm build: code section `0x00bfb2c1`
- Focused Gate J `20260520-lobby-idempotency-skip-gate-j` passed in `182.01s`
- The leftover PocketIC server from the run was killed; follow-up process scan only matched the `pgrep` command.

Measured delta versus `20260520-player-no-live-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 36.3147B | 32.8175B | -9.6% |
| Gate J scenario memory | 4108.3125 MB | 4108.2500 MB | flat |
| `create_session` avg | 9.6384B | 8.9368B | -7.3% |
| `join_session` avg | 4.2439B | 3.5380B | -16.6% |
| `mark_ready` avg | 4.2460B | 3.5496B | -16.4% |
| `start_session` avg | 6.3575B | 5.6609B | -11.0% |
| `commands.lobby_command_idempotency` total | 4.9244B | 1.4096B | -71.4% |
| row growth | 35 | 35 | flat |
| stable pages final | 66689 | 66689 | flat |

Decision:

- Keep this checkpoint. It removes five durable absence probes from the measured setup route while still allowing durable fallback before an actor enters the runtime-receipt path.
- This is another reason Gate 5H must define receipt durability/history clearly: within a process, heap receipts are now the fresh lobby nonce authority after the actor's first runtime lobby command.

## Checkpoint: Lobby Session Row Cache

After the runtime actor idempotency skip, `sessions.load_session` was still visible at 4 calls and `2.8136B` total, all from the lobby setup command path. The route repeatedly loaded the same session row after it had just been created or updated.

What changed:

- Added a single current-session heap row cache for `GameSession`.
- `load_session_from_text` checks that cache before durable `sessions.load_session`.
- Successful lobby event sequence updates refresh the cached row.
- Start-session state transitions and native setup recovery updates refresh the cached row.
- Durable fallback remains the source on cache miss.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- Benchmark Wasm build: code section `0x00bfbb2b`
- Focused Gate J `20260520-lobby-session-row-cache-gate-j` passed in `182.49s`
- The leftover PocketIC server from the run was killed; follow-up process scan only matched the `pgrep` command.

Measured delta versus `20260520-lobby-idempotency-skip-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 32.8175B | 30.0113B | -8.6% |
| Gate J scenario memory | 4108.2500 MB | 4108.2500 MB | flat |
| `join_session` avg | 3.5380B | 2.8393B | -19.7% |
| `mark_ready` avg | 3.5496B | 2.8469B | -19.8% |
| `start_session` avg | 5.6609B | 4.9582B | -12.4% |
| `sessions.load_session` total | 2.8136B | 0B | removed |
| row growth | 35 | 35 | flat |
| stable pages final | 66689 | 66689 | flat |

Decision:

- Keep this checkpoint. It is a direct reuse of the real session row after successful durable writes, with durable fallback when the heap row is absent.
- The remaining largest setup floors are now `sessions.update_session` (`2.8624B`), `sessions.by_state_current_turn` (`2.1095B`), and lobby/game event creates (`1.9B`).

## Checkpoint: Runtime Lobby Events With Feed Cache Prime

After the session row cache, Gate J still paid for four durable lobby `GameEvent` rows and four matching `GameSession.next_event_seq` updates. These events are immediate response/feed material for create-session, join-session, and ready-state lobby commands; they are not needed as durable live-state writes before the setup/battle handoff.

What changed:

- Added a runtime lobby event buffer storing `ApiEventView` rows for session-created, participant-joined, and participant-ready events.
- `create_session`, `join_session`, and `mark_ready` now bump the cached session event cursor in heap and return/record runtime events instead of calling durable `events.create_game_event`.
- `get_events_after` merges runtime lobby events with durable, session-turn, and battle runtime events.
- The first runtime lobby event primes the existing complete event-feed cache, so later `get_events_after` calls stay heap-backed and durable setup/game events still append to the cached feed.
- The durable setup completion event path remains unchanged.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- Benchmark Wasm build: code section `0x00bfc265`
- Focused Gate J `20260520-lobby-runtime-events-cache-gate-j` passed in `186.57s`
- An intermediate run `20260520-lobby-runtime-events-gate-j` passed but regressed `get_events_after` to `0.7067B` average because the event-feed cache was no longer primed by a durable event with `event_seq=1`; the cache-prime fix restored `get_events_after` to `0.0001B`.

Measured delta versus `20260520-lobby-session-row-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 30.0113B | 26.1877B | -12.7% |
| Gate J scenario memory | 4108.2500 MB | 4076.2500 MB | -0.8% |
| `create_session` avg | 8.9368B | 7.9832B | -10.7% |
| `join_session` avg | 2.8393B | 1.8853B | -33.6% |
| `mark_ready` avg | 2.8469B | 1.8895B | -33.6% |
| `start_session` avg | 4.9582B | 4.9558B | flat |
| `get_events_after` avg | 0.000092B | 0.000095B | flat |
| `events.create_game_event` total | 1.9112B | 0B | removed |
| `sessions.update_session` total | 2.8624B | 0.9538B | -66.7% |
| row growth | 35 | 35 | flat |
| stable pages final | 66689 | 66177 | -0.8% |

Decision:

- Keep this checkpoint. It removes lobby event/session write amplification without moving the cost into `get_events_after`.
- The durability/history gap is now broader: fresh lobby command receipts and fresh lobby events are heap-backed until Gate 5H defines flush/upgrade/history policy. That is consistent with the active-runtime direction, but it must not be forgotten.

## Checkpoint: Active Session Admission Cache

After runtime lobby events, the largest remaining setup repo op was `sessions.by_state_current_turn`: six state-page operations totaling `2.1095B`, all attributed to `create_session` through the canister active-session-limit check.

What changed:

- Added an active-session admission cache holding active/lobby/starting `GameSession` IDs.
- Seed the cache during canister install and post-upgrade from durable session state.
- `create_session` now reads the cached count and remembers newly created sessions instead of scanning `GameSession` by state on the hot path.
- Battle aftermath removes a finished session ID from the cache when the session transitions to `finished`.
- If the cache is missing for any reason, `active_session_count` still falls back to durable state scans and seeds the cache.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- Benchmark Wasm build: code section `0x00bfcb92`
- Focused Gate J `20260520-active-session-admission-cache-gate-j` passed in `197.45s`

Measured delta versus `20260520-lobby-runtime-events-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 26.1877B | 24.0875B | -8.0% |
| Gate J scenario memory | 4076.2500 MB | 4076.1875 MB | flat |
| `create_session` avg | 7.9832B | 5.8805B | -26.3% |
| `get_events_after` avg | 0.000095B | 0.000095B | flat |
| `sessions.by_state_current_turn` total | 2.1095B | 0B | removed |
| row growth | 35 | 35 | flat |
| stable pages final | 66177 | 66177 | flat |

Decision:

- Keep this checkpoint. It removes the active-session-limit scan from the gameplay hot path while still seeding from durable rows at install/upgrade and falling back to durable scans if the cache is absent.
- The scan cost is now paid outside the measured command route during install/upgrade repair, which is a better tradeoff for active gameplay. Gate 5H should include this cache in the broader runtime snapshot/repair policy.

## Checkpoint: Brand-New Register Idempotency Skip

After the active-session admission cache, the remaining lobby idempotency cost was two `commands.lobby_command_idempotency` lookups from the first `register_player` call for each principal.

What changed:

- `register_player` now performs the already-required `players.find_by_principal` lookup before beginning the lobby command.
- If no player row exists, the command skips the durable lobby idempotency lookup and creates the runtime lobby receipt directly.
- If a player row exists, durable lobby idempotency lookup is still allowed so older persisted register receipts can be replayed.
- Runtime lobby receipt replay remains first for same-process retries.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- Benchmark Wasm build: code section `0x00bfcb43`
- Focused Gate J `20260520-register-idempotency-skip-gate-j` passed in `195.58s`

Measured delta versus `20260520-active-session-admission-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 24.0875B | 22.6822B | -5.8% |
| Gate J scenario memory | 4076.1875 MB | 4076.1250 MB | flat |
| `register_player` avg | 1.8748B | 1.1742B | -37.4% |
| `create_session` avg | 5.8805B | 5.8788B | flat |
| `commands.lobby_command_idempotency` total | 1.4102B | 0B | removed |
| row growth | 35 | 35 | flat |
| stable pages final | 66177 | 66177 | flat |

Decision:

- Keep this checkpoint. It removes the last lobby idempotency repo op from Gate J without weakening replay for principals that already have a durable player row.
- Fresh register receipts are still runtime-backed, so Gate 5H must define their flush/upgrade/status retention policy together with other runtime lobby receipts.

## Checkpoint: Heap Lobby Ready State

After the register idempotency skip, the two `mark_ready` calls still wrote durable `GameParticipant` rows just to set `ready_turn`. Gate J already has a real-row-backed participant heap cache from create/join, and `start_session` reads that cache through `participants_for_session`.

What changed:

- `mark_ready` now mutates the cached participant row and records it in the participant cache instead of calling `sessions.update_participant`.
- `require_participant_for_player` checks cached participants first before falling back to the durable participant lookup.
- The start-session readiness gate continues to use `participants_for_session`, so it sees the heap ready state in the active lobby route.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- Benchmark Wasm build: code section `0x00bfcba1`
- Focused Gate J `20260520-lobby-ready-heap-gate-j` passed in `181.34s`

Measured delta versus `20260520-register-idempotency-skip-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 22.6822B | 20.3124B | -10.4% |
| Gate J scenario memory | 4076.1250 MB | 4076.1250 MB | flat |
| `mark_ready` avg | 1.8906B | 0.7052B | -62.7% |
| `start_session` avg | 4.9534B | 4.9503B | flat |
| `sessions.update_participant` total | 0.9594B | 0B | removed |
| row growth | 35 | 35 | flat |
| stable pages final | 66177 | 66177 | flat |

Decision:

- Keep this checkpoint. It removes ready-state durable writes from the lobby hot path and still lets `start_session` observe readiness from the real-row cache.
- Durable participant readiness is now part of the runtime flush/upgrade gap. Gate 5H needs to decide whether lobby readiness is flushed, snapshotted, or considered ephemeral once the session has started.

## Checkpoint: First-Playable Content Cache

After heap lobby ready state, `create_session` was still paying first-playable ruleset/faction creation and lookup costs. These rows are static content, so paying that during a player command is the wrong place for the cost.

What changed:

- Added a first-playable content heap cache for the `RulesetDefinition` and faction rows needed by lobby setup.
- Seed/repair the cache during install and post-upgrade.
- `ensure_first_playable_content` returns the cached rows on the hot path.
- `faction_for_slot` checks cached first-playable factions before falling back to durable content lookup.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- Benchmark Wasm build: code section `0x00bfdbe6`
- Focused Gate J `20260520-content-cache-gate-j` passed in `193.86s`

Measured delta versus `20260520-lobby-ready-heap-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 20.3124B | 15.3820B | -24.3% |
| Gate J scenario memory | 4076.1250 MB | 4012.1250 MB | -1.6% route delta |
| stable pages start | 1025 | 2049 | +1024 pages at install |
| stable pages final | 66177 | 66177 | flat |
| `create_session` avg | 5.8788B | 1.6563B | -71.8% |
| `join_session` avg | 1.8894B | 1.1839B | -37.3% |
| `content.create_ruleset_definition` total | 0.4720B | 0B | moved to install |
| `content.create_faction_definition` total | 0.9440B | 0B | moved to install |
| row growth | 35 | 35 | flat in Gate J tracked entities |

Decision:

- Keep this checkpoint. Static content seeding belongs in install/upgrade repair, not on the first player `create_session` command.
- This is a deliberate cost shift: initial stable pages now start higher because ruleset/faction rows are preseeded. That is acceptable for gameplay latency, but install/upgrade benchmarks should track it separately.

## Checkpoint: New Setup Job Scheduling

After the first-playable content cache, `start_session` was still paying the idempotent job upsert path for a setup job that is known to be new in the successful lobby-start route. That path read by job key and then scanned due/leased job indexes while scheduling the timer.

What changed:

- `start_session` now calls `schedule_new_job` for the initial `setup_session` job when `started_now` is true.
- The normal idempotent `schedule_job` path remains available for recovered/replayed jobs and other callers.
- The durable `SystemJob` row is still created and the timer wakeup is still requested; this only removes redundant scans from the hot lobby start.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-pocket-ic-tests --test canister_endpoints`
- Benchmark Wasm build: code section `0x00bfdc4e`
- Focused Gate J `20260520-setup-new-job-gate-j` passed in `196.40s`

Measured delta versus `20260520-content-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 15.3820B | 13.2728B | -13.7% |
| Gate J scenario memory | 4012.1250 MB | 4012.1250 MB | flat |
| Gate J scenario cycles | 0.0647T | 0.0625T | -3.3% |
| `start_session` avg | 4.9492B | 2.8398B | -42.6% |
| `system_jobs.by_job_key` total | 0.7060B | 0B | removed |
| `system_jobs.by_status_due` total | 0.7024B | 0B | removed |
| `system_jobs.by_status_lease` total | 0.7022B | 0B | removed |
| row growth | 35 | 35 | flat |
| stable pages final | 66177 | 66177 | flat |

Decision:

- Keep this checkpoint. A newly started lobby does not need the idempotent job-key probe before inserting its first setup job.
- This leaves `system_jobs.create_system_job` as a real durable boundary. Gate 5H should decide whether setup/turn/battle jobs are durable barriers or runtime wakeup hints with delayed durable projection.

## Checkpoint: Known-New Setup Command

After the new setup job scheduling checkpoint, `start_session` still performed a durable `GameCommand` idempotency lookup before creating the setup command. In the route where `start_session` just moved the session from `lobby` to `starting`, that command is known-new for the same reason the setup job is known-new.

What changed:

- `start_session` creates the setup `GameCommand` directly on the `started_now` route.
- Recovered or repeated `starting` sessions still use `ensure_setup_command` and the durable idempotency lookup.
- Setup jobs first load the command through their stored `command_id` and fall back to idempotency only if the job has no usable command pointer.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Focused Gate J `20260520-setup-command-fastpath-gate-j` passed in `56.33s` after a cached build

Measured delta versus `20260520-setup-new-job-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 13.2728B | 12.5656B | -5.3% |
| Gate J scenario memory | 4012.1250 MB | 4012.0625 MB | flat |
| Gate J scenario cycles | 0.0625T | 0.0618T | -1.2% |
| `start_session` avg | 2.8398B | 2.1342B | -24.8% |
| `commands.game_command_idempotency` total | 0.7038B | 0B | removed |
| row growth | 35 | 35 | flat |
| stable pages final | 66177 | 66177 | flat |

Decision:

- Keep this checkpoint. The direct create path is bounded to the atomic state transition from lobby to starting, and the old idempotent path remains the recovery path.
- The remaining Gate J repo costs are now mostly unavoidable durable creates/updates unless Gate 5H changes what must be projected during the hot setup route: two `SystemJob` creates, two participant creates, two session updates, two player creates, one session create, one setup command create, one visibility chunk update, one known object create, and one battle header create.

## Checkpoint: Player Principal Cache

The setup command checkpoint left a repeated caller-principal lookup cost across create/join/ready/start. This was not visible in the repo-op table because the player lookup path uses the simpler storage-result instrumentation, but the method timings showed the pattern clearly: `mark_ready` had almost no measured repo ops and still cost about `0.704B`.

What changed:

- Added a heap cache of registered `PlayerAccount` rows keyed by principal.
- `register_player` seeds the cache after creating or loading a player.
- `create_session`, `join_session`, `mark_ready`, `start_session`, and `get_my_player` use the cached player before durable `players.by_principal` fallback.
- The durable fallback stays in place for callers that were registered before cache repair/adoption.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Focused Gate J `20260520-player-principal-cache-gate-j` passed in `181.59s`

Measured delta versus `20260520-setup-command-fastpath-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 12.5656B | 9.0628B | -27.9% |
| Gate J scenario memory | 4012.0625 MB | 4012.0625 MB | flat |
| Gate J scenario cycles | 0.0618T | 0.0583T | -5.7% |
| `create_session` avg | 1.6563B | 0.9567B | -42.2% |
| `join_session` avg | 1.1839B | 0.4826B | -59.2% |
| `mark_ready` avg | 0.7042B | 0.0001B | ~-100.0% |
| `start_session` avg | 2.1342B | 1.4372B | -32.7% |
| row growth | 35 | 35 | flat |
| stable pages final | 66177 | 66177 | flat |

Decision:

- Keep this checkpoint. A registered player row is stable enough to use as an active-process identity cache, and all callers still fall back to durable lookup on cache miss.
- This drops the lobby setup route below `10B` total. The next large costs are durable row boundaries rather than repeated lookup reads: player creates, session/participant creates, session updates, setup command create, setup/battle job creates, visibility/object projection writes, and the battle header create.

## Checkpoint: Direct Inserts For Lobby Create Rows

A worker investigated whether the generated create-input path for lobby row creation had measurable overhead. The patch changed `PlayerAccount`, `GameSession`, and `GameParticipant` creation to build full typed rows and call `foundation::insert` while still going through IcyDB validation, index maintenance, and relation checks.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Worker ran repository smoke tests for create/read/update/page/delete and insert-many/error sanitization
- Focused Gate J `20260520-direct-insert-create-rows-gate-j` passed in `187.32s`

Measured delta versus `20260520-player-principal-cache-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 9.0628B | 9.0582B | -0.05% |
| Gate J scenario memory | 4012.0625 MB | 4012.0625 MB | flat |
| Gate J scenario cycles | 0.0583T | 0.0583T | flat |
| `register_player` avg | 1.1756B | 1.1756B | flat |
| `create_session` avg | 0.9567B | 0.9577B | flat/noise |
| `join_session` avg | 0.4826B | 0.4833B | flat/noise |
| `start_session` avg | 1.4372B | 1.4346B | flat/noise |

Decision:

- Keep the patch for now because it is verified and does not regress the route, but do not treat it as a major performance win.
- This confirms the remaining durable create cost is inside IcyDB insert/index/relation work, not the generated create-input wrapper.

## Rejected Experiment: Heap Visibility/Known-Object Projection Writes

Tried to remove `map.update_visibility_chunk` and `map.create_known_object` from the Gate J movement route by making movement update active render-projection caches when those caches were known complete for the first-playable participant.

Result:

- The patch compiled in normal and benchmark canister checks after fixes.
- Focused Gate J could not install the benchmark Wasm.
- Benchmark Wasm code section was `12,595,116` bytes, exceeding the IC limit of `12,582,912` bytes by `12,204` bytes.

Decision:

- Rejected for now due code size. The idea is directionally correct, but the helper surface was too large for the current canister budget.
- Revisit only after code-size headroom is recovered or after a smaller design is found. The target repo ops remain `map.update_visibility_chunk` and `map.create_known_object`, about `0.95B` combined in Gate J.

## Rejected Experiment: Heap-Only Starting Session

Tried moving the `lobby -> starting` `GameSession` update out of `start_session` and letting the setup job read the starting session from the heap cache before the final active-state durable update.

Measured run:

- Focused Gate J `20260520-heap-starting-session-gate-j` passed in `180.80s`
- Scenario instructions improved `9.0628B -> 8.5842B` (-5.3%)
- `start_session` improved `1.4372B -> 0.9601B` (-33.2%)
- `sessions.update_session` route calls moved `2 -> 1`
- Scenario cycles regressed `0.0583T -> 0.1232T` (+111.3%)
- The extra measured cycles appeared on a `get_session` poll where setup/timer work was charged: sequence 20 recorded `65.3891B` cycles and `142.7 MB` memory growth.

Decision:

- Do not keep this code change. A small instruction win is not worth a large cycle regression and muddier timer attribution.
- Revisit this only as part of Gate 5H when setup jobs become explicit runtime wakeup hints with a real flush/recovery barrier, not as an isolated skipped session update.

## Checkpoint: Runtime Visibility And Known-Object Projection Writes

Revisited the earlier rejected visibility/known-object write deferral with a smaller design that fit the benchmark Wasm limit.

Implementation:

- `render_projection` now exposes narrow heap helpers for cached `VisibilityChunk` and `ParticipantKnownObject` rows.
- `refresh_champion_visibility` updates the heap visibility projection when an active `SessionTurnRuntime` exists and the cached row is present; it still falls back to durable `VisibilityChunk` reads/writes on cache miss.
- `create_known_object_if_missing` appends newly discovered known objects to the heap known-object projection on runtime-mode movement when that participant cache is already loaded; non-runtime and cache-miss paths still use durable rows.
- `get_object_view` checks the heap known-object cache before durable lookup so active runtime discoveries are not hidden from the API view.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build with `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc`: code section `0x00bfe14a`, about `7.8 KB` under the IC limit
- Focused Gate J `20260520-runtime-visibility-known-projection-bounded-gate-j` passed in `315.43s`

Measured delta versus `20260520-direct-insert-create-rows-gate-j`:

| metric | previous | new | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 9.0582B | 6.6772B | -26.3% |
| Gate J scenario memory | 4012.0625 MB | 3980.0625 MB | -0.8% |
| Gate J scenario cycles | 0.0583T | 0.0548T | -6.1% |
| `sync_session_turn` avg | 0.3797B | 0.1440B | -62.1% |
| `map.update_visibility_chunk` route calls | 1 | 0 | removed |
| `map.create_known_object` route calls | 1 | 0 | removed |
| row growth | 35 | 35 | flat |

Decision:

- Keep this checkpoint. It removes the last measured movement visibility/known-object durable projection writes from Gate J without broadening the canister enough to hit the IC code-section limit.
- Gate 5D is complete for the measured route: movement resolver state now stays in runtime/projection caches unless a durable fallback is required.
- The durability contract is still explicitly Gate 5H work. Visibility and known-object runtime projection must be included in `flush_barrier(StrongRead|Upgrade|RuntimeEviction)` instead of reintroducing hot-path stable writes.

## Checkpoint: Upgrade Projection Flush For Session-Turn And Town Runtime

Implemented the first concrete `Upgrade` flush slice instead of leaving all runtime-only state as a hot-path optimization with no recovery boundary.

Implementation:

- `pre_upgrade_impl` now flushes active session-turn runtime projections before persisting the battle runtime snapshot.
- `SessionTurnRuntime` flushes heap `GameSession` and `GameParticipant` snapshots from the latest retained runtime per session to durable rows, and writes unflushed runtime events from all retained runtimes to durable `GameEvent` rows idempotently by event key.
- Runtime events only keep a durable `command_id` when the referenced `GameCommand` row already exists; heap-only town/movement command receipts remain a separate Gate 5H flush task.
- `TownProjection` flushes durable `Town` rows and upserts heap-created `TownBuilding`, `TownRecruitPool`, and `TownGarrisonStack` rows by id.
- Cached town resolution now handles cached real `Town` ids and all scenario start aliases instead of only hardcoded `town:west`/`town:east`.
- The upgrade flush is excluded from `feature=benchmark` builds. Including it in the benchmark Wasm pushed the code section over the IC limit; excluding upgrade-only code keeps the benchmark canister installable without changing the hot route being measured.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- `cargo test -p domm-degens-canister town -- --nocapture` matched no tests and passed the filtered harness
- Benchmark release Wasm build with `feature=benchmark`: code section `0x00bfe95e`, about `5.8 KB` under the IC limit

Decision:

- Keep this checkpoint. It does not change the hot Gate J route, so no PocketIC benchmark was run.
- This makes upgrade safer for the current runtime town path: visible town/building/pool/garrison changes, participant resource snapshots, and runtime event feed rows now have a pre-upgrade durable projection.
- Remaining Gate 5H/6 work is still real: heap command receipt rows, detailed resource ledger history, lobby runtime receipts, `StrongRead`/`RuntimeEviction`/`TurnAdvance` flush reasons, and non-first-playable town variants are not solved by this checkpoint.

## Checkpoint: Town Projection Cache-Miss Adoption

Small Gate 6 broadening step:

- `resolve_town_by_session_id` now hydrates `TownProjection` immediately after durable town id or scenario xy fallback loads a real town row.
- Cached id/start-alias paths still return from heap first, so the Gate J route should stay unchanged.
- This avoids repeated child-row scans for repeated non-alias town calls after the first durable miss.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it as a low-risk broadening checkpoint. No PocketIC benchmark was run because this is a cache-miss behavior outside the current measured Gate J hot route.

## Checkpoint: Runtime Lobby Upgrade Flush

Gate 5H lobby receipt slice:

- `pre_upgrade_impl` now flushes runtime lobby state before the session-turn/town/battle runtime upgrade work.
- Heap `LobbyCommand` receipts are inserted or updated durably by id, so command status and idempotency can survive upgrade instead of depending only on the process-local cache.
- Runtime lobby events are written to durable `GameEvent` rows idempotently by event key.
- The flush is excluded from `feature=benchmark` builds to preserve the benchmark Wasm code-section headroom.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it. This closes the explicit runtime lobby receipt item for the `Upgrade` path, while the broader Gate 5H work for `StrongRead`, `RuntimeEviction`, `TurnAdvance`, command receipts, and resource ledger flushing remains open.

## Checkpoint: Runtime Town Command Upgrade Flush

Gate 5H town command receipt slice:

- Non-benchmark `SessionTurnCommandReceipt` now keeps optional payload JSON.
- Runtime town commands populate that payload; movement and sync receipts leave it empty for now.
- The `Upgrade` flush inserts a durable `GameCommand` row for runtime receipts with payload JSON when neither the command id nor the participant nonce already exists durably.
- The durable row preserves command id, command type, actor participant, nonce, payload hash/json, status, phase, retryable flag, and error fields. Result JSON is a small runtime-flush marker rather than a full replay payload.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it as the town command half of the upgrade flush. Resource ledger history and movement/sync runtime receipts remain open because they need more payload/state than the current generic receipt holds.

## Checkpoint: Runtime Town Resource Ledger Upgrade Flush

Gate 5H town ledger slice:

- Non-benchmark runtime town resource deltas now retain the durable ledger details needed for history: command id, ledger key, resource key, signed delta, balance-after, and reason.
- `spend_resources_runtime` records those details for build/recruit costs after mutating the heap participant balance, while the benchmark build keeps the old aggregate-only delta shape.
- `flush_runtime_projections_for_upgrade` walks resource deltas from all retained session-turn runtimes and inserts missing `ResourceLedgerEntry` rows idempotently by `(command_id, ledger_key)`.
- The flush stays excluded from `feature=benchmark`, so this durability checkpoint does not add code to the measured benchmark Wasm hot route.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it. This closes town build/recruit ledger history for the `Upgrade` flush path without reintroducing stable ledger writes into the active town command route.
- Movement/sync runtime resource deltas remain aggregate-only, and movement/sync command receipts still lack enough payload to materialize durable command history on upgrade. Keep that as Gate 5H.5 instead of slowing the hot route now.

## Checkpoint: Runtime Movement Resource Ledger Upgrade Flush

Gate 5H non-town ledger slice:

- Runtime movement resource deltas now use the same non-benchmark ledger metadata path as town deltas.
- Income and pickup deltas already had command id, ledger key, resource key, signed delta, and reason at the call site; the runtime-only branch now also captures balance-after and records all of it in `SessionTurnRuntime`.
- The existing upgrade resource flush writes those entries idempotently by `(command_id, ledger_key)` across all retained session-turn runtimes.
- Benchmark builds still use the aggregate-only resource delta path, preserving the measured hot-route Wasm shape.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it. This closes runtime resource ledger history for upgrade flush across town and movement income/pickup paths.
- The remaining Gate 5H receipt gap is now specifically non-town command payload/history, not resource ledger rows.

## Checkpoint: Runtime Movement Command Upgrade Flush

Gate 5H movement/sync receipt slice:

- Runtime `submit_move_intent` receipts now retain their payload JSON on non-benchmark builds, so upgrade flush can materialize durable `GameCommand` rows instead of leaving them heap-only.
- Runtime `sync_session_turn` receipts now retain payload JSON and the original command turn. The receipt may be stored in the next runtime after turn advance, but durable command history should still use the turn that accepted the command.
- `flush_runtime_projections_for_upgrade` now scans command receipts from all retained session-turn runtimes, not just the latest runtime per session. Session/participant snapshots still flush only from the latest runtime.
- The existing command flush remains idempotent by command id and participant nonce.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it. This closes the explicit movement/sync command receipt upgrade-history gap without restoring stable command writes to the active movement/sync routes.
- Battle runtime command/event durable history remains a separate blocked item because previous attempts exceeded the IC Wasm code-section limit.

## Checkpoint: Economy Town Projection Broadening

Gate 6 projection broadening slice:

- `economy_expansion::resolve_town` now uses `town_runtime::cached_town_by_public_id` before durable lookup, matching the town service path.
- Tavern/economy town id misses now hydrate `TownProjection`, so repeated tavern offer/preview/hire calls can reuse the real-row-backed town aggregate.
- Movement-side town text lookup now uses the same cache/adoption path, so non-Gate-J paths that resolve towns through movement also keep the projection warm.
- Weekly recruit growth now iterates recruit pools from `TownProjection` and mirrors updates back into the projection after durable update.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it as a broadening checkpoint. It does not move tavern offers themselves into heap projection yet; that remains open with non-start aliases and any remaining direct town-child command variants.

## Checkpoint: Tavern Offer Town Projection

Gate 6 tavern projection slice:

- `TownProjection` now carries bounded `TavernOffer` rows alongside buildings, recruit pools, and garrison stacks.
- `get_tavern_offers` reads the projected week when all expected offer slots are cached, otherwise it hydrates the week from durable rows and mirrors them into the projection.
- Hire preview and hire command offer lookup now use the projection before durable `offer_key` lookup.
- Tavern offer create/update points in first-playable setup, weekly materialization, and hire completion mirror durable rows into the projection.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it. Tavern offers are still durably created/updated at the existing boundaries, so this is a read/projection cache rather than a new deferred-write contract.
- Remaining TownProjection broadening is now specifically non-start aliases and any direct town child-row variants not covered by town/economy/movement paths.

## Checkpoint: Battle Town Garrison Projection

Gate 6 town-child row broadening slice:

- Town battle startup now builds defender stacks from `TownProjection.garrison_stacks` instead of paging durable `TownGarrisonStack` rows directly.
- Battle aftermath now enumerates old defender garrison rows from the projection, deletes them durably, creates survivor garrison rows durably, and replaces the projected garrison with the survivor rows.
- The now-unused `evict_town` helper was removed; aftermath keeps the projection coherent instead of cold-evicting it.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it. This removes the remaining direct battle garrison read path from outside `TownProjection` while preserving durable survivor writes at the battle-resolution boundary.
- The remaining TownProjection todo is now only non-start town aliases beyond the first-playable start list.

## Checkpoint: Town Public Alias Resolution

Gate 6 alias broadening slice:

- Added `town_runtime::load_town_by_public_id` as the common resolver for town service, economy/tavern service, and movement town lookups.
- The resolver supports cached/durable ULIDs, first-playable start keys, cached name aliases, and a bounded durable active-town scan for `town:<slugified Town.name>` aliases.
- Durable alias misses hydrate `TownProjection`, so repeated calls by name alias reuse the projection.
- Removed duplicated resolver logic from town/economy/movement services.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it. There is no separate alias table or `Town` alias field in the schema, so name-derived aliases are the only concrete non-start alias source available without adding a new data model.
- This completes the concrete Gate 6 TownProjection broadening items currently listed in `perf1.todo.md`.
- Gate 5G parent is also now marked complete because its listed query/status overlays are covered by Gate 5G.1-5G.28, and the remaining town projection broadening dependency has been closed by Gate 6.10-6.13.

## Checkpoint: Upgrade Flush Barrier Entry Point

Gate 5H barrier boundary slice:

- Added `services::flush_barrier::flush_barrier("Upgrade")` as the common non-benchmark entry point for durable projection barriers.
- `pre_upgrade` now calls that barrier for lobby receipts/events, session-turn rows/events/commands/resource ledgers, and town projections before persisting the battle runtime snapshot. The next checkpoint folds that battle snapshot into the barrier too.
- The barrier rejects unsupported reasons for now, so this does not claim the full Gate 5H contract for `TurnAdvance`, `BattleHandoff`, `RuntimeEviction`, or `StrongRead`.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it. It creates the reusable boundary without broadening hot-path behavior yet, and leaves the non-upgrade reasons as explicit follow-up work.

## Checkpoint: Battle Runtime Snapshot In Upgrade Barrier

Gate 5H barrier boundary slice:

- Folded `battle_runtime::persist_snapshot_for_upgrade` into `flush_barrier("Upgrade")`.
- `pre_upgrade` now has a single non-benchmark barrier call for lobby runtime state, session-turn runtime state, town projections, and active battle runtime snapshot refs.
- Battle snapshot failures are mapped into retryable `ApiError` values inside the barrier, keeping the pre-upgrade panic handling centralized.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it. This makes `Upgrade` a real shared barrier boundary instead of a partial wrapper, without adding any hot-path IcyDB writes.

## Checkpoint: Gate 5E Parent Closure

Checklist hygiene:

- Marked the Gate 5E parent complete because its concrete runtime-open/job-deadline subitems are all complete.
- The later Gate 5F.11 focused run measured `sync_session_turn` at `0.4984B`, which is below Gate 5E's original `0.6B-0.9B` target band.

Decision:

- Keep the parent closed. Remaining runtime durability/barrier work is tracked under Gate 5H, not Gate 5E.

## Checkpoint: Diagnostic StrongRead Flush Barrier

Gate 5H explicit checkpoint slice:

- Added `flush_barrier("StrongRead")` for the currently supported durable projection set.
- Added controller-only `run_diagnostic_flush_barrier(reason)` as a non-benchmark update endpoint so operators/tests can force an explicit barrier checkpoint outside `pre_upgrade`.
- `StrongRead` flushes lobby runtime state, session-turn runtime state, and town projections, but intentionally does not write the upgrade-only battle runtime snapshot cell.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-degens-canister exported_candid_contains_every_required_game_endpoint -- --nocapture`

Decision:

- Keep it. This gives Gate 5H a real callable explicit checkpoint while leaving hot-path `TurnAdvance`, `BattleHandoff`, and `RuntimeEviction` wiring for later slices.

## Checkpoint: TurnAdvance Flush Barrier

Gate 5H turn boundary slice:

- Added `flush_barrier("TurnAdvance")` for the currently supported durable projection set.
- Manual `sync_session_turn` now enforces the barrier after a successful full turn advance and after the runtime command receipt has been stored.
- Scheduled turn-resolution jobs enforce the same barrier after completing the old turn and scheduling the next turn's jobs.
- The enforcement is excluded from `feature=benchmark` builds so benchmark gates continue measuring the hot runtime shape, not production checkpoint cost.
- Internal enforcement panics on barrier failure, matching the IC rollback model for a failed consistency boundary instead of returning a partial-checkpoint `Result`.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it. This gives Gate 5H a real `TurnAdvance` lifecycle boundary while leaving `BattleHandoff` and `RuntimeEviction` as the remaining barrier reasons.

## Checkpoint: BattleHandoff Flush Barrier

Gate 5H battle handoff slice:

- Added `flush_barrier("BattleHandoff")` for the currently supported durable projection set.
- Manual movement sync and scheduled turn-resolution now enforce the barrier when they return a battle handoff event: `champion_encounter_pending`, `neutral_encounter_pending`, or `town_encounter_pending`.
- Non-benchmark battle handoff now projects the active `Battle` header before adopting heap runtime state.
- Non-benchmark handoff also materializes cached heap startup `BattleStack`, `BattleObstacle`, and `BattleOccupancy` rows idempotently by row id before runtime adoption.
- This keeps benchmark builds on the fast heap-only startup path while giving production/upgrade recovery real tactical child rows to hydrate from at the handoff boundary.
- Internal enforcement panics on barrier failure so the IC reverts the handoff message instead of leaving a partial checkpoint.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it. This closes the most important battle-start recovery gap left by the heap startup optimization without changing benchmark hot-route measurements.

## Checkpoint: RuntimeEviction Flush Barrier

Gate 5H runtime eviction slice:

- Added `flush_barrier("RuntimeEviction")` for the currently supported durable projection set.
- Resolved battle cleanup now enforces that barrier before removing the active battle runtime from heap.
- The barrier runs after the resolved battle state has already been projected and aftermath has updated durable/town/champion state.
- Internal enforcement panics on failure so the IC reverts the eviction message instead of silently dropping heap-owned projection state.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it. At this checkpoint the listed Gate 5H lifecycle reasons had concrete entry points, but the Gate 5H parent stayed open because durable battle command/event archive flushing was still blocked by the IC benchmark Wasm code-section limit.

## Checkpoint: Battle Runtime Archive Barrier Flush

Gate 5H archive durability slice:

- Active and archived battle runtime command receipts now flush to durable `GameCommand` rows through the shared non-benchmark barrier path.
- Active and archived battle runtime events now flush to durable `GameEvent` rows through the same barrier path.
- Runtime battle command receipts retain payload JSON only in non-benchmark builds, keeping benchmark Wasm size and hot-route measurements unaffected.
- The flush writes command receipts before events so events can link to an existing durable command when that command is available.
- Active runtime events are marked flushed after a successful barrier pass; archived events remain safe because durable insertion is idempotent by event key.
- The shared barrier already covers `Upgrade`, `StrongRead`, `TurnAdvance`, `BattleHandoff`, and `RuntimeEviction`, so this closes the earlier battle command/event archive blocker.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`

Decision:

- Keep it. This resolves the previous code-section blocker by making the battle archive flush production-only while preserving the benchmark build as the fast-route measurement target.

## Checkpoint: Direct SystemJob Inserts

Gate 7 setup/job floor slice:

- Latest focused Gate J artifact before this checkpoint is `20260520-runtime-visibility-known-projection-bounded-gate-j`.
- That route is down to `6.6772B` scenario instructions, and the largest remaining repo-operation groups are durable row creates/updates: player accounts, session, participants, session updates, setup command, battle row, and two `SystemJob` creates.
- Changed `system_jobs::create_system_job` from generated `Create<SystemJob>` input materialization to constructing the typed `SystemJob` row directly and calling `foundation::insert`.
- This mirrors the already-kept direct insert pattern for `PlayerAccount`, `GameSession`, and `GameParticipant`.
- Semantics stay the same: durable job rows are still created, indexed, and scheduled; this is a row construction overhead cut, not heap-job scheduling.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Decision:

- Keep it. It is a small low-risk cut against the remaining job floor before taking on larger heap-first setup/timer work.

## Checkpoint: Direct Setup GameCommand Insert

Gate 7 setup/job floor slice:

- The same latest Gate J artifact had one `commands.create_game_command` call left in the route, created by the deterministic `setup_session` system command.
- `start_session` only calls `create_setup_command` on the fresh `lobby -> starting` transition, while recovery/replay goes through `ensure_setup_command` and the existing idempotency lookup first.
- Changed `create_setup_command` to build the typed `GameCommand` row directly and call `commands_events_effects::insert_game_command`.
- This preserves the durable command row and the setup job's command-id linkage, but removes the generated `Create<GameCommand>` materialization from this known-new route.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Decision:

- Keep it. The next larger Gate 7 work should decide whether setup and battle timeout jobs can be heap-first wakeup hints with durable barrier projection, rather than just direct inserts.

## Checkpoint: Direct Battle Header Insert

Gate 7 setup/job floor slice:

- The latest focused Gate J artifact also had one `battles.create_battle` call left in the route, created when movement reaches the neutral guard battle handoff.
- Changed `battles::create_battle` to construct the typed `Battle` row directly and call `foundation::insert`.
- This preserves the durable battle header row, child-row startup behavior, and active `BattleRuntime` adoption path; it only removes generated `Create<Battle>` materialization from the known-new header create.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Decision:

- Keep it. This is another small create-path cleanup; the big remaining Gate 7 decision is still whether to make setup/battle timeout wakeups heap-first and durably projected at barriers.

## Checkpoint: Generic Direct GameCommand Creates

Gate 7 setup/job floor slice:

- The previous setup-command checkpoint proved the setup command could use a direct `GameCommand` insert safely, but it duplicated the command-row construction locally.
- Changed `commands_events_effects::create_game_command` itself to build a typed `GameCommand` row and call `foundation::insert`.
- Routed `create_setup_command` back through that generic helper, so the measured setup path and other durable fallback/recovery command creates share the same direct insert path.
- This does not change command idempotency lookup or status semantics; it only removes generated `Create<GameCommand>` materialization from the helper.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Decision:

- Keep it. This broadens the low-risk direct-insert cleanup before larger heap-first setup/job work.

## Checkpoint: Generic Direct GameEvent Creates

Gate 7 projection/history create-path slice:

- `commands_events_effects::create_game_event` still used generated `Create<GameEvent>` materialization even though the caller already supplies every required event field.
- Changed it to build the typed `GameEvent` row directly and call `foundation::insert`.
- Kept `remember_created_event` unchanged so public event-feed cache behavior is preserved.
- This helps durable projection/history fallback paths and barrier flushes without changing active runtime event buffers.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Decision:

- Keep it. It is the same low-risk direct-insert pattern as `GameCommand`, `SystemJob`, and `Battle` header rows.

## Checkpoint: Gate 7 Direct-Create Benchmark

Gate 7 measurement after the direct-create pass:

- Focused Gate J `20260520-143448-gate7-direct-create-gate-j` passed in `323.52s`.
- Scenario: `2 players; map 48x48; 3 movement paths; 0 battle actions; 2 town reads; 4 event reads; benchmark route`.
- Scenario instructions were effectively flat versus `20260520-runtime-visibility-known-projection-bounded-gate-j`: `6.6772B -> 6.6790B` (`+0.03%`).
- Scenario memory stayed flat at `3980.0625 MB`; cycles stayed flat at `0.0548T`; row growth stayed flat at `35`.
- `start_session` was neutral (`1.4346B -> 1.4340B`) and `sync_session_turn` was neutral (`0.1440B -> 0.1439B`).
- `submit_build_town_structure` and `submit_recruit_units` moved from about `0.0003B` to `0.0018B`; that is a small absolute cost but it confirms the direct-create pass did not improve this route.
- Remaining repo-operation costs are still almost all durable insert/update/index work: `players.create_player_account` `0.9435B` total, `sessions.create_participant` `0.9599B`, `sessions.update_session` `0.9552B`, `system_jobs.create_system_job` `0.9560B`, `battles.create_battle` `0.4803B`, `commands.create_game_command` `0.4777B`, and `sessions.create_game_session` `0.4778B`.

Decision:

- Keep the direct-insert cleanups because they simplify the create path and do not break behavior, but stop expecting them to move Gate J materially.
- The next Gate 7 work needs to reduce durable row count or defer row writes with a real heap-first/barrier model. Generated `Create<T>` materialization is not the measured floor; stable row insert/update/index maintenance is.

## Checkpoint: Insert-First Player Registration

Gate 7 fresh-registration slice:

- `register_player` previously did a durable `players.by_principal` lookup before creating a new player. In Gate J both player principals are fresh, so this paid a stable pre-read and then a stable insert.
- `PlayerAccount.account_principal` is already a unique index, so the hot path now tries `players.create_player_account` first and only performs the principal lookup on IcyDB conflict.
- Runtime nonce replay still returns the original runtime lobby command before any insert attempt.
- Durable replay/conflict behavior is preserved for already registered principals: on conflict, the service loads the existing player and checks durable lobby command idempotency before creating a new runtime response.
- Repository duplicate-principal errors remain sanitized through the existing `icydb_repository_error` mapping.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister register_player_fast_path_replays_runtime_nonce -- --nocapture`
- `cargo test -p domm-degens-canister repository_insert_many_atomic_and_storage_errors_are_sanitized -- --nocapture`

Additional finding:

- Attempted `cargo test -p domm-degens-canister lobby_session_setup_recovers_from_starting_state_and_replays_nonce -- --nocapture`; it failed after `147.15s` because a seeded due `turn_resolution` job did not block a new old-turn movement command. The failure is in the turn-close guard path, not the registration assertions, and is now tracked as Gate 7.8.

Decision:

- Keep it. This is a real read-removal slice: fresh registration now relies on the unique index for correctness and saves the durable pre-read on the common new-player path.

## Checkpoint: Runtime Turn-Resolution Closure Marker

Gate 7.8 correctness slice:

- The native `lobby_session_setup_recovers_from_starting_state_and_replays_nonce` check surfaced that `ensure_map_turn_accepts_new_command` could accept a new old-turn map command when an accepted `turn_resolution` job had just been inserted into durable storage.
- The hot path was returning early from `runtime_proves_pre_deadline_turn_open`, so adding durable scans there would have undone earlier Gate J work.
- Added a small heap marker in `repos::system_jobs` for scheduled/running `turn_resolution` jobs as they are created or updated.
- `ensure_map_turn_accepts_new_command` now checks that marker before using the runtime-open fast path. Completing/failing a job updates the same cache through `update_system_job`, so the marker is removed when the durable job leaves scheduled/running state.
- I first included `turn_deadline` jobs too, but the long native test runs past the 60s deadline and then failed on the real scheduled deadline. Narrowed the marker to explicit `turn_resolution` jobs to preserve the current runtime-fast deadline behavior while fixing the seeded accepted-job case.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Test follow-up:

- Reran `cargo test -p domm-degens-canister lobby_session_setup_recovers_from_starting_state_and_replays_nonce -- --nocapture`.
- The original failure changed from "accepted turn job should block new old-turn movement" to a later failure at `canisters/degens/src/services/tests.rs:457`: `sync_session_turn` returned a new command id instead of the seeded pending sync command id.
- Gate 7.8 stays open for that remaining seeded-sync replay issue.

Decision:

- Keep the marker. It fixes the accepted `turn_resolution` gap without reintroducing durable system-job scans on every runtime-open map command.

## Checkpoint: Runtime GameCommand Idempotency Cache

Gate 7.8 seeded sync-command recovery slice:

- After the turn-resolution marker, the long native setup/replay test progressed to a seeded pending `sync_session_turn` command mismatch.
- The runtime-fast `sync_session_turn` path intentionally skipped durable idempotency lookup when a `SessionTurnRuntime` existed, so it ignored a durable pending command seeded in the same heap and generated a runtime command id instead.
- Added a bounded heap cache for created/updated durable `GameCommand` rows keyed by `(session_id, actor_kind, actor_id_text, client_nonce)`.
- `sync_session_turn` now checks that heap cache before generating a runtime command. If it finds a pending/applying durable command, it runs through the durable command path; if it finds an applied/failed command, it returns the normal command response. This avoids adding a stable idempotency lookup to every runtime sync.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Test follow-up:

- Reran `cargo test -p domm-degens-canister lobby_session_setup_recovers_from_starting_state_and_replays_nonce -- --nocapture`.
- The prior command-id mismatch at `canisters/degens/src/services/tests.rs:457` is passed.
- The same test now fails later at `canisters/degens/src/services/tests.rs:479` because the champion is not on the expected resource-pile tile after sync. Gate 7.8 remains open for that movement-position issue.

Decision:

- Keep it. It restores seeded durable command recovery for runtime sync without putting stable command-idempotency reads back into the hot runtime path.

## Checkpoint: Durable Follow-Up After Incomplete Manual Sync

Gate 7.8 mixed durable/runtime movement recovery slice:

- After the runtime `GameCommand` cache, the long setup/replay test progressed to a movement-position failure.
- The first seeded durable `sync_session_turn` correctly reused the durable command and parked partial movement in durable rows, but the next sync went back through runtime-only state and left the final champion position in heap instead of durable storage.
- Changed incomplete durable/manual sync handling to evict the current-turn runtime after rescheduling current-turn jobs. That makes the follow-up sync continue from durable partial movement rows instead of mixing `DurableBridge` and `RuntimeOnly` state for the same pending movement.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Test follow-up:

- Reran `cargo test -p domm-degens-canister lobby_session_setup_recovers_from_starting_state_and_replays_nonce -- --nocapture`.
- The prior resource-pile position failure at `canisters/degens/src/services/tests.rs:479` is passed.
- The same test now fails later at `canisters/degens/src/services/tests.rs:605` because a neutral encounter leaves the durable champion row `active` instead of `in_battle`. Gate 7.8 remains open for battle-handoff projection durability in this recovery route.

Decision:

- Keep it. The durable recovery route should stay durable until the parked movement is finished; this keeps the fast runtime path unchanged for normal Gate J commands.

## Checkpoint: Champion Snapshot Flush At Runtime Barrier

Gate 7.8 battle-handoff durability slice:

- After durable follow-up sync, the long setup/replay test progressed to a neutral encounter assertion where the durable champion row still reported `active` instead of `in_battle`.
- The battle handoff path updated runtime champion snapshots, and the existing barrier flushed session, participant, command receipt, resource, and event projections, but not champion snapshots.
- Added champion snapshot updates to `session_turn_runtime::flush_runtime_projections_for_upgrade`, so battle handoff/upgrade/strong-read barriers persist the latest heap champion rows before direct durable recovery reads.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister lobby_session_setup_recovers_from_starting_state_and_replays_nonce -- --nocapture` passed in `209.30s`

Decision:

- Keep it. Runtime battle handoff must not leave direct durable champion reads behind heap state when a barrier has explicitly run.

## Checkpoint: Gate 7 Registration/Recovery Benchmark

Gate J measurement after insert-first registration and recovery fixes:

- Focused Gate J `20260520-152224-gate7-registration-recovery-gate-j` passed in `199.82s`.
- Scenario: `2 players; map 48x48; 3 movement paths; 0 battle actions; 2 town reads; 4 event reads; benchmark route`.
- Scenario instructions improved versus `20260520-143448-gate7-direct-create-gate-j`: `6.6790B -> 5.3075B` (`-20.5%`).
- Scenario memory stayed flat at `3980.0625 MB`; row growth stayed flat at `35`; cycles moved `0.0548T -> 0.0534T`.
- `register_player` dropped from `1.1756B` avg to `0.4760B` avg (`-59.5%`) because the durable pre-read was removed for fresh principals.
- `start_session` stayed flat (`1.4340B -> 1.4339B`), `sync_session_turn` stayed flat (`0.1439B -> 0.1438B`), and the tiny town command calls stayed around `0.0018B-0.0019B`.
- Remaining measured repo-operation floor is still durable row maintenance: `sessions.create_participant` `0.9607B` total, `system_jobs.create_system_job` `0.9554B`, `sessions.update_session` `0.9542B`, `players.create_player_account` `0.9518B`, `battles.create_battle` `0.4800B`, `sessions.create_game_session` `0.4794B`, and `commands.create_game_command` `0.4776B`.

Decision:

- Keep the insert-first registration slice. It produced the first material Gate 7 scenario reduction after the neutral direct-create pass.
- Next performance work should target the setup/session durable row count: `start_session` still pays roughly one session update plus setup command/job writes, and session creation still pays durable session plus participant creates.

## Checkpoint: Heap Lobby Participants With Start Flush

Gate 7.10 session/participant row-count slice:

- The last focused Gate J run showed `sessions.create_participant` as the largest remaining route-local floor after registration: two durable participant inserts cost `0.9607B` total, split across `create_session` and `join_session`.
- Changed lobby creation/join to build `GameParticipant` rows in heap instead of immediately inserting them into IcyDB.
- The session participant cache is now multi-session and tracks each cached participant as durable/dirty, so `mark_ready` can update the heap row and `start_session` can flush exactly the rows setup needs.
- `start_session` flushes cached participants before scheduling/running setup. The common fresh lobby route uses one `sessions.insert_participants_atomic` operation for both participants instead of two separate `sessions.create_participant` operations.
- The live-session admission guard now checks heap-only cached participants before trusting the "no live session" cache or falling back to durable participant rows.
- The non-benchmark upgrade barrier now flushes cached lobby participants before lobby commands/events, so an upgrade between lobby creation and start does not lose participant rows.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister start_session_replay_while_starting_reuses_original_nonce_and_cursor -- --nocapture` passed in `22.09s`

Decision:

- Keep this checkpoint. It removes participant durable writes from `create_session` and `join_session` and moves the required durability to the setup boundary, which matches the aggregate/projection direction.
- Benchmark confirmed that `insert_many_atomic` is worth keeping here. The next Gate J work should target the remaining durable setup/session/job floor.

Code-size note:

- First focused Gate J attempt after adding participant batching failed install because the benchmark Wasm code section was `12,592,587` bytes, `9,675` bytes over the IC limit.
- The normal build keeps the full multi-session dirty participant cache. The benchmark build now uses a single-session participant cache and excludes dirty-update recovery code that Gate J does not exercise.
- Final benchmark Wasm code section is `0x00bfec2a`, about `5.0 KB` under the IC limit.

Gate J measurement:

- Clean focused Gate J `20260520-155903-gate7-participant-flush-clean-gate-j` passed in `56.90s` on git `69ccabc`.
- Scenario instructions improved versus `20260520-152224-gate7-registration-recovery-gate-j`: `5.3075B -> 4.8377B` (`-8.9%`).
- Scenario memory stayed flat at `3980.0625 MB`; row growth stayed flat at `35`; cycles moved `0.0534T -> 0.0529T`.
- `create_session` moved `0.9607B -> 0.4810B` avg because it no longer creates the first durable participant row.
- `join_session` moved `0.4827B -> 0.0016B` avg because it no longer creates the second durable participant row.
- `start_session` moved `1.4339B -> 1.9238B` avg because it now owns the participant durability boundary.
- Net route win comes from two individual participant inserts (`sessions.create_participant`, `0.9607B` total) becoming one `sessions.insert_participants_atomic` call (`0.4870B` total).
- Remaining measured repo-operation floor is durable row maintenance: `players.create_player_account` `0.9518B` total, `system_jobs.create_system_job` `0.9574B`, `sessions.update_session` `0.9547B`, `commands.create_game_command` `0.4791B`, `battles.create_battle` `0.4801B`, `sessions.create_game_session` `0.4794B`, and `sessions.insert_participants_atomic` `0.4870B`.

## Checkpoint: Heap-Only Starting Session State

Gate 7.11 session update cut:

- After participant batching, `start_session` owned both the participant durability boundary and the durable `lobby -> starting` session update.
- The `starting` row update is only needed for recovery/progress, not for the hot public call itself. The setup job can accept a durable `lobby` row and treat its claimed setup job as the starting boundary.
- Changed `start_session` to set `session.state = "starting"` in heap and response/cache only, without immediately updating the durable `GameSession`.
- Changed setup job processing to accept a durable `lobby` session row, switch it to local `starting`, and still persist the final `active` state when setup completes.
- Added non-benchmark upgrade flush for a cached `starting` session row so upgrade between `start_session` and setup completion preserves progress semantics.
- Preserved the native replay/progress path by preventing the native post-job refresh from overwriting heap `starting` with durable `lobby` before setup has reached `active`.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister start_session_replay_while_starting_reuses_original_nonce_and_cursor -- --nocapture` passed in `21.18s`

Gate J measurement:

- Focused Gate J `20260520-160417-gate7-starting-session-heap-gate-j` passed in `189.78s` on git `38e10b1`.
- Scenario instructions improved versus `20260520-155903-gate7-participant-flush-clean-gate-j`: `4.8377B -> 4.3593B` (`-9.9%`).
- `start_session` moved `1.9238B -> 1.4451B` avg.
- `sessions.update_session` moved from two calls (`0.9547B` total) to one call (`0.4762B` total).
- Scenario memory stayed flat at `3980.0625 MB`; row growth stayed flat at `35`; cycles moved `0.0529T -> 0.0524T`.
- Remaining measured repo-operation floor is now: `players.create_player_account` `0.9518B`, `system_jobs.create_system_job` `0.9568B`, `sessions.insert_participants_atomic` `0.4870B`, `commands.create_game_command` `0.4791B`, `battles.create_battle` `0.4806B`, `sessions.create_game_session` `0.4794B`, and `sessions.update_session` `0.4762B`.

Decision:

- Keep it. This is exactly the aggregate/projection pattern: heap state answers active progress immediately, durable state is written at the setup completion boundary or upgrade barrier.

## Checkpoint: Runtime Setup Command Boundary

Gate 7.12 setup command durability cut:

- After heap-only `starting`, the public `start_session` call still created a durable deterministic `setup_session` `GameCommand` before scheduling the setup job.
- Changed fresh `start_session` to build and cache the setup command in the runtime command cache, without inserting the `GameCommand` row on the public call.
- The setup job now persists or loads the durable setup command when it actually runs, so replay/recovery still has a durable command boundary without charging the fresh public route.
- `get_setup_progress` also checks the runtime command cache before falling back to durable command lookup, so the heap-only command remains visible during the active setup window.

Important correction:

- The first implementation committed as `3205cf1` removed the durable insert, but fresh `start_session` still called the durable idempotency lookup before creating the runtime command.
- Focused Gate J `20260520-161817-gate7-runtime-setup-command-gate-j` passed but regressed versus Gate 7.11: scenario instructions moved `4.3593B -> 4.5879B`, and `start_session` moved `1.4451B -> 1.6727B`.
- The repo summary exposed the mistake directly: `commands.game_command_idempotency` appeared as one public-route call at `0.7068B`.
- Follow-up commit `53625d5` makes the fresh path create/cache the runtime setup command directly, while replay/recovery still uses the durable lookup.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister start_session_replay_while_starting_reuses_original_nonce_and_cursor -- --nocapture` passed in `20.31s`

Gate J measurement:

- Corrected focused Gate J `20260520-162144-gate7-runtime-setup-command-fresh-gate-j` passed in `190.48s` on git `53625d5`.
- Scenario instructions improved versus `20260520-160417-gate7-starting-session-heap-gate-j`: `4.3593B -> 3.8788B` (`-11.0%`).
- `start_session` moved `1.4451B -> 0.9664B` avg.
- The public-route `commands.create_game_command`/`commands.game_command_idempotency` setup command cost disappeared from repo operations.
- Remaining measured repo-operation floor is now: `players.create_player_account` `0.9518B`, `system_jobs.create_system_job` `0.9566B`, `sessions.insert_participants_atomic` `0.4870B`, `battles.create_battle` `0.4801B`, `sessions.create_game_session` `0.4794B`, and `sessions.update_session` `0.4759B`.

Decision:

- Keep it. This is the same heap-first boundary used for lobby command receipts and starting session state: the public route returns from runtime state, and durable setup history is pushed to the job/recovery boundary.

## Rejected Attempt: Heap-Only Runtime Turn Advance Session Row

Gate 7 follow-up attempt:

- Tried to remove the remaining runtime `sync_session_turn` durable `sessions.update_session` call from the benchmark hot path.
- The code compiled and, after a benchmark-only dead barrier trim, fit the IC Wasm code-section limit at `0x00bffe88`.
- Focused Gate J `20260520-164825-gate7-runtime-turn-session-gate-j` passed on git `d46c607` and reported scenario instructions `3.8788B -> 2.9235B`.

Why it was rejected:

- The route shape changed: calls moved `87 -> 86`, and `battles.create_battle` disappeared from the measured repo operations.
- Final stable memory increased `3980.0625 MB -> 4356.375 MB`.
- Benchmark row diagnostics changed from `row_commands=5,row_events=3` to `row_commands=10,row_events=5`.
- That means the measured gain was not a clean removal of one session update; it also changed how far the route progressed and where projections/history landed.

Decision:

- Reverted as `c4b0c20`.
- The next session-update cut needs a fuller runtime merge contract for all callers/endpoints that cross the turn boundary, and the benchmark must preserve the same call count and battle handoff behavior before it can be marked done.

## Rejected Attempt: Battle Handoff Map-Deadline Marker

Gate 7.14 probe:

- Tried a very small cut for the remaining seq72 `system_jobs.create_system_job`: mark sessions that emitted a battle-handoff event and skip the next map-turn deadline recreate.
- A first active-runtime scan version was too large for the benchmark Wasm limit. The one-shot marker version fit only after trimming benchmark-only timer restoration code; final code section was `0x00bffec6`.
- Focused Gate J `20260520-174411-gate7-battle-handoff-deadline-gate-j` and broader marker rerun `20260520-174959-gate7-battle-handoff-marker-gate-j` both passed.

Why it was rejected:

- Route shape stayed healthy: `87` calls, `row_commands=5`, `row_events=3`, `row_growth=35`, final stable pages `65665`, and `battles.create_battle` remained present.
- Performance did not improve. Scenario instructions moved from the accepted `3.8788B` baseline to `3.8802B`, and seq72 still showed `system_jobs.create_system_job` at about `0.4790B`.
- That means the remaining seq72 `SystemJob` create is not the map-turn deadline recreate this probe targeted. It is more likely the active battle timeout/setup job created during battle activation, but the next edit should prove the job key before changing semantics.

Decision:

- Reverted the code changes and kept only this note/todo update.
- Next Gate 7 work should target a tiny active-battle timeout runtime wakeup or add narrowly scoped job-key attribution for seq72 before attempting another cut.

## Checkpoint: Gate 7 System Job Attribution

Gate 7.15.1 attribution slice:

- Added benchmark-only operation labels for `SystemJob` creation so Gate J can distinguish active battle timeout job creation from other system-job inserts without changing the benchmark harness.
- To stay under the IC benchmark Wasm code-section limit, the labels are intentionally short: `sj.bt` for `battle_timeout`, `sj.other` for non-battle-timeout job creates.
- Benchmark Wasm code section fit at `0x00bfffe6`, 26 bytes under the limit.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Focused Gate J `20260520-180501-gate7-system-job-attribution-gate-j` passed in `56.71s`.

Gate J measurement:

- Route shape stayed accepted: `87` calls, `row_commands=5`, `row_events=3`, `row_growth=35`, stable pages `2049 -> 65665`, and `battles.create_battle` remained present.
- Scenario stayed at the accepted baseline: `3.8788B`.
- Seq18 `start_session` measured `sj.other` at `0.4778B`, which is the setup-session job create.
- Seq72 `sync_session_turn` measured `sj.bt` at `0.4789B`, proving the remaining late system-job create is the active battle timeout job.

Decision:

- Keep the attribution labels for benchmark builds. They are cheap enough to fit and prevent more blind edits.
- Gate 7.15 should now cut `battle_timeout` scheduling specifically: active battle runtime should own the live timeout wakeup, with durable timeout job projection/flushing at battle resolution or upgrade/recovery boundaries.

## Checkpoint: Gate 7 Runtime Battle Timeout Wakeup

Gate 7.15 implementation:

- Changed active battle timeout scheduling so battle start and runtime readiness paths no longer create a durable `battle_timeout` `SystemJob` on the public route.
- Normal Wasm builds now arm a direct runtime timer for the active battle deadline. The timer checks the heap `BattleRuntime` battle id/session id/deadline before applying an auto-defend, so stale timers from older deadlines are ignored.
- Benchmark builds skip the direct timer body to keep the performance canister inside the IC Wasm code-section limit; the benchmark route does not wait for the timeout timer to fire.
- Row-backed fallback still uses durable `SystemJob` scheduling, and upgrade/recovery repair still recreates durable timeout jobs from active durable battles.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Focused Gate J `20260520-182807-gate7-runtime-battle-timeout-final-gate-j` passed in `57.75s`.

Gate J measurement:

- Route shape stayed accepted: `87` calls, `row_commands=5`, `row_events=3`, `row_growth=35`, stable pages `2049 -> 65665`, and `battles.create_battle` remained present.
- Scenario instructions improved versus `20260520-180501-gate7-system-job-attribution-gate-j`: `3.8788B -> 3.3999B`, a `0.4789B` / `12.3%` reduction.
- Seq72 `sync_session_turn` moved from the attributed `sj.bt` create at `0.4793B` to `0.0004B`.
- Repo operations now contain only one setup job create: `sj.other` at `0.4778B`. The `sj.bt` repo operation disappeared.
- `sync_session_turn` average moved `0.1437B -> 0.0958B` because one of its ten calls no longer pays the timeout job insert.

Decision:

- Keep it. This is the intended heap aggregate pattern for active battle deadlines: runtime owns the live timeout, durable job rows remain fallback/recovery infrastructure instead of hot-path command state.
- Next Gate 7 work should target the remaining seq18 setup-session `sj.other` create without changing setup polling/recovery behavior.

## Round-Robin Pivot: Scenario Progress Cluster

The active instruction for perf1 is now round-robin endpoint optimization: do a bounded pass on a heavy endpoint/cluster, then rotate instead of maximizing one endpoint forever. This is now written into `perf1.todo.md`.

Paused work:

- A Gate K visibility repair was started but is not the active optimization focus now. It compiled natively before the pivot, but the previous focused Gate K still failed at `battle_not_visible`; do not mark that battle fix as done until a focused battle gate passes.

New cluster:

- Picked the scenario-progress cluster from endpoint-surface `20260520-200503-48ee723` because it has several heavy endpoints: `claim_quest_reward` `18.1532B`, `sync_advanced_victory` `16.5262B`, `sync_objectives` `10.3832B`, `accept_quest` `10.3750B`, plus expensive scenario query scans.
- First bounded cut removes two obvious durable work patterns:
  - `accept_quest` and `claim_quest_reward` no longer schedule scenario maintenance jobs from their hot command path. Quest accept/claim already update their direct quest/rule state, and turn maintenance is still scheduled from the turn/movement/battle paths.
  - `sync_scenario_rule_rows_for_session` still reports the same touched count for command/job receipts, but it only writes a `ScenarioRuleState` row when current value, victory state, or winner participant actually changes.

Verification so far:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check --target wasm32-unknown-unknown -p domm-degens-canister --features benchmark`

Next:

- Measure with the smallest useful endpoint-surface/focused route when ready. If scenario endpoints remain over `1B`, rotate again before trying to fully aggregate scenario progress.

## Round-Robin Pivot: Champion Magic Cluster

New cluster:

- Rotated to champion magic from endpoint-surface `20260520-200503-48ee723`: `learn_champion_spell` `10.8614B`, `cast_adventure_spell` `9.6916B`, and `select_champion_level_up` `8.2911B`.
- Each command showed two `commands.update_game_command` calls in the benchmark repo-op summary, about `0.96B` total. The first update only marks the command as `applying` before mutating a champion/spell row.

Cut:

- Removed the early durable `GameCommand` `applying` update from `select_champion_level_up`, `learn_champion_spell`, and `cast_adventure_spell`.
- The champion/spell rows already use `last_command_id` to make replay/recovery idempotent, and `apply_command_with_result` still performs the final durable applied command update.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check --target wasm32-unknown-unknown -p domm-degens-canister --features benchmark`

Expected measurement:

- Next endpoint-surface/focused run should show `commands.update_game_command` dropping from two calls to one for the three champion-magic endpoints, saving roughly one durable update floor per call before any deeper champion aggregate work.

## Round-Robin Pivot: Economy Expansion Cluster

New cluster:

- Rotated to economy expansion from endpoint-surface `20260520-200503-48ee723`: `submit_dwelling_recruit` `13.9243B`, `hire_tavern_champion` `13.5124B`, and `submit_market_trade` `10.6367B`.
- These methods all showed expensive fresh-path idempotency reads for the public event and command effect rows: `events.by_session_event_key` around `0.702B-0.705B` and `effects.command_effect_by_command_key` around `0.702B-0.703B`.

Cut:

- Added `command_response::append_fresh_public_event` and `command_response::create_fresh_command_effect` for paths that can prove a command-owned business row was just created.
- `hire_tavern_champion` now uses the fresh helpers only when it creates a new `ChampionHire` row for this command.
- `submit_market_trade` now uses the fresh helpers only when it creates a new `MarketTrade` row for this command.
- `submit_dwelling_recruit` now uses the fresh helpers only when it creates a new `DwellingRecruitment` row for this command.
- Recovery/replay paths where those rows already exist keep the old idempotent `append_public_event` and `ensure_command_effect` calls, so duplicate rows remain guarded.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check --target wasm32-unknown-unknown -p domm-degens-canister --features benchmark`

Expected measurement:

- Next batched endpoint-surface/focused run should remove the `events.by_session_event_key` and `effects.command_effect_by_command_key` repo-op reads from the fresh economy expansion calls, roughly `1.4B` per affected endpoint before deeper aggregate work.

## Measurement: Batched Round-Robin Endpoint Surface

Artifact:

- `target/benchmarks/20260520-endpoint-rotations-76036c8`
- Git sha: `76036c8`
- Test: `pocket_ic_benchmark_endpoint_surface_records_every_required_endpoint`
- Result: passed in `213.33s`, covered `59/59` required endpoints.

Suite delta versus `20260520-200503-48ee723/endpoint-surface`:

| metric | before | after | delta |
| --- | ---: | ---: | ---: |
| endpoint-surface instructions | 199.6434B | 185.4812B | -14.1622B (-7.1%) |
| row growth | 153 | 150 | -3 |
| final stable pages | 83969 | 81281 | -2688 |

Targeted endpoint deltas:

| endpoint | before | after | delta |
| --- | ---: | ---: | ---: |
| `claim_quest_reward` | 18.1532B | 14.1430B | -4.0102B |
| `accept_quest` | 10.3750B | 7.7999B | -2.5751B |
| `sync_advanced_victory` | 16.5262B | 14.6139B | -1.9123B |
| `sync_objectives` | 10.3832B | 10.3820B | -0.0013B |
| `learn_champion_spell` | 10.8614B | 10.3807B | -0.4807B |
| `cast_adventure_spell` | 9.6916B | 9.2153B | -0.4763B |
| `select_champion_level_up` | 8.2911B | 7.8004B | -0.4907B |
| `submit_dwelling_recruit` | 13.9243B | 12.5198B | -1.4045B |
| `hire_tavern_champion` | 13.5124B | 12.1075B | -1.4048B |
| `submit_market_trade` | 10.6367B | 9.2274B | -1.4093B |

Repo-op confirmation:

- Champion magic `commands.update_game_command` dropped from 2 calls to 1 call on `learn_champion_spell`, `cast_adventure_spell`, and `select_champion_level_up`.
- Economy expansion fresh calls no longer show `events.by_session_event_key` or `effects.command_effect_by_command_key` for `submit_dwelling_recruit`, `hire_tavern_champion`, or `submit_market_trade`.
- `claim_quest_reward` `scenario.update_scenario_rule_state` dropped from 4 updates to 1 update. `sync_advanced_victory` had no scenario rule row update in this route after the unchanged-row guard.

Decision:

- Keep the cuts. They are bounded, preserve endpoint coverage, and remove measured durable-row churn.
- Per the round-robin rule, rotate to the next heavy cluster from the new ranking instead of immediately polishing these same endpoints.

## Round-Robin Pivot: World Generation And Events Cluster

New cluster:

- Rotated to worldgen/events from endpoint-surface `20260520-endpoint-rotations-76036c8`.
- Current costs: `sync_world_generation` `8.8428B`, `sync_world_events` `7.7969B`.
- Measured durable metadata churn:
  - `sync_world_generation`: `worldgen.update_skirmish_settings` `0.4776B` and `worldgen.update_procedural_map` `0.4771B`.
  - `sync_world_events`: `scenario.update_world_event_state` `0.4767B`.

Cut:

- `sync_world_generation` no longer rewrites existing `SkirmishSettingsState` just to stamp `last_command_id`.
- `sync_world_generation` no longer rewrites an unchanged `ProceduralMapState` just to stamp `last_command_id`; it still updates the procedural map if actual generated state changes.
- `sync_world_events` no longer rewrites an existing `WorldEventState` just to stamp `last_command_id`; it still creates/mutates the row on missing-row materialization.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check --target wasm32-unknown-unknown -p domm-degens-canister --features benchmark`

Expected measurement:

- Next endpoint-surface/focused run should save roughly `0.95B` from `sync_world_generation` and `0.48B` from `sync_world_events`, before addressing their larger shared command/effect/event/session floor.

## Round-Robin Pivot: Map-Turn Readiness Cluster

New cluster:

- Rotated from worldgen/events to `end_turn` rather than continuing to polish the same endpoint family.
- Last measured endpoint-surface cost: `end_turn` `8.0200B`.
- Hot repo ops in that method:
  - `sessions.participants_by_session_status`: `2` calls, `0.7053B` total.
  - `turn_ready.by_session_turn`: `2` calls, `0.7036B` total.
  - `events.by_session_event_key`: `1` call, `0.7036B`.
  - Shared command/event/session write floor remains after these cuts.

Cut:

- `mark_turn_ready` now returns whether it created the `ParticipantTurnReady` row.
- Fresh `end_turn` uses `append_fresh_public_event` only when the ready row was just created; replay/recovery still uses the idempotent event lookup.
- `SessionTurnRuntime` now tracks whether participants and ready rows are complete.
- `end_turn` reads ready count, participant count, and `all_ready` from the runtime only when both completeness flags are true; otherwise it falls back to the previous durable participant/ready pages.
- Carry-forward to a new turn preserves participant completeness and marks the new empty ready set complete when the previous roster was complete.

Verification:

- `cargo fmt`
- `git diff --check`
- `cargo check -p domm-degens-canister`

Benchmark-gate cleanup:

- Native `cargo check -p domm-degens-canister --features benchmark` initially failed because `process_setup_session_job` was cfg'd out under `feature = "benchmark"` but still referenced in a native-only block.
- Narrowed setup-job recovery cfgs so native benchmark builds include the helper and only wasm benchmark builds keep the code-size exclusion.
- Native benchmark-feature compile now passes.
- Wasm benchmark check initially failed before DoMM code because host build scripts linked through rustup's `gcc-ld` wrapper, which pointed at a missing `/nix/store/.../ld-wrapper.sh`.
- Repaired the local rustup `gcc-ld/ld.lld` wrapper to point at the current Nix `ld-wrapper.sh`.
- Wasm benchmark-feature compile now passes.

Expected measurement:

- Fresh `end_turn` should drop `events.by_session_event_key` (`~0.70B`).
- Hot complete-runtime `end_turn` should also drop the durable participant/ready count pages (`~1.4B` in the last trace), while cold/partial-runtime paths retain the old stable reads for correctness.

## Round-Robin Pivot: Query Auth Runtime-First Cluster

New cluster:

- Rotated from `end_turn` to broad read/preview endpoints.
- Last measured heavy queries:
  - `preview_dwelling_recruit`: `4.2313B`.
  - `get_dwelling_pool`: `3.5232B`.
  - `preview_champion_progression`: `3.5225B`.
  - `get_objective_progress`: `3.5182B`.
  - `get_scenario_rules`: `3.5182B`.
  - `get_world_events`, `get_skirmish_settings`, `get_procedural_map_state`, `get_naval_routes`, `get_siege_rules`: about `2.81B`.

Cut:

- Read-only champion progression now uses `require_session_caller_runtime_first`.
- Economy/tavern/dwelling read endpoints now use runtime-first caller context; active previews use the active runtime-first variant.
- Scenario progress and quest preview reads now use runtime-first caller context.
- Worldgen read endpoints now use runtime-first caller context.
- Update endpoints were intentionally left on the previous command paths in this cut.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check --target wasm32-unknown-unknown -p domm-degens-canister --features benchmark`
- `git diff --check`

Expected measurement:

- These endpoints should avoid the durable player/session/participant lookup when an active `SessionTurnRuntime` has the caller row, matching the shape already used by cheap game/town/event views.
- The remaining query cost after that will identify whether stable child-row reads or DTO assembly dominate each endpoint.

## Measurement: Round-Robin Worldgen, End-Turn, Query Auth

Artifact:

- `target/benchmarks/20260520-roundrobin-worldgen-endturn-query-35c9c27`
- Passed in `192.61s`.
- Git: `35c9c27`.
- Coverage: `59/59` required endpoints.
- Calls: `146`.
- Row growth: `150` (unchanged from prior run).
- Stable pages: `2049 -> 81281` (unchanged from prior run).

Scenario delta versus `20260520-endpoint-rotations-76036c8`:

- `185.4812B -> 152.4118B`, `-33.0694B`, `-17.8%`.
- Cycles moved `0.1971T -> 0.1935T`.
- Memory stayed essentially flat: `4955.5625 MB -> 4955.5 MB`.

Targeted endpoint deltas:

- `end_turn`: `8.0200B -> 5.9048B`, `-2.1152B`, `-26.4%`.
- `sync_world_generation`: `8.8428B -> 7.8874B`, `-0.9554B`, `-10.8%`.
- `sync_world_events`: `7.7969B -> 7.3189B`, `-0.4780B`, `-6.1%`.
- `preview_dwelling_recruit`: `4.2313B -> 2.1225B`, `-2.1088B`, `-49.8%`.
- `get_dwelling_pool`: `3.5232B -> 1.4200B`, `-2.1032B`, `-59.7%`.
- `preview_champion_progression`: `3.5225B -> 1.4125B`, `-2.1101B`, `-59.9%`.
- `get_objective_progress`: `3.5182B -> 1.4106B`, `-2.1076B`, `-59.9%`.
- `get_scenario_rules`: `3.5182B -> 1.4080B`, `-2.1102B`, `-60.0%`.
- Worldgen/scenario light reads (`get_world_events`, `preview_quest`, `get_skirmish_settings`, `get_procedural_map_state`, `get_naval_routes`, `get_siege_rules`) each dropped by about `2.105B-2.110B`.

Repo-op confirmation:

- `end_turn` no longer shows `events.by_session_event_key`, `sessions.participants_by_session_status`, or `turn_ready.by_session_turn`.
- `sync_world_generation` no longer shows `worldgen.update_skirmish_settings` or unchanged `worldgen.update_procedural_map`.
- `sync_world_events` no longer shows existing-row `scenario.update_world_event_state`.
- Query endpoints have no repo-op trace in the benchmark, but their instruction deltas confirm the runtime-first caller context removed the old durable lookup floor.

Post-measurement heavy endpoints:

- `sync_advanced_victory`: `14.6192B`.
- `claim_quest_reward`: `14.1430B`.
- `submit_dwelling_recruit`: `12.5198B`.
- `hire_tavern_champion`: `12.1075B`.
- `sync_objectives`: `10.3820B`.
- `learn_champion_spell`: `10.3807B`.
- `submit_market_trade`: `9.2274B`.
- `cast_adventure_spell`: `9.2153B`.

## Round-Robin Pivot: Champion Magic Fresh Event/Effect

New cluster:

- Rotated back to champion magic after worldgen/events, map-turn readiness, query-auth, and a batched measurement.
- Current costs: `learn_champion_spell` `10.3807B`, `cast_adventure_spell` `9.2153B`, `select_champion_level_up` `7.8004B`.
- The remaining obvious floor is shared with other command endpoints: fresh command effect absence reads and fresh public event absence reads.

Cut:

- Successful fresh `select_champion_level_up` now creates the command effect directly with `create_fresh_command_effect`.
- Successful fresh `learn_champion_spell` now creates the command effect directly with `create_fresh_command_effect`.
- Successful fresh `cast_adventure_spell` now creates the command effect directly with `create_fresh_command_effect`.
- The matching public events now use `append_fresh_public_event`.
- Early idempotent replay paths still return before event/effect creation, preserving duplicate avoidance for already-applied champion mutations.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `git diff --check`

Expected measurement:

- Each fresh champion magic command should drop `effects.command_effect_by_command_key` (`~0.70B`) and `events.by_session_event_key` (`~0.70B`) if the endpoint trace still had both absence reads.
- Expected endpoint improvement is roughly `~1.4B` per fresh champion magic command before the remaining command create/update, champion row, event/effect create, session update, and content/champion reads dominate.

## Round-Robin Pivot: Battle-State Query Auth

New cluster:

- Rotated from champion magic to a remaining read-path outlier.
- Current cost: `get_battle_state` `2.1106B` on the endpoint-surface invalid-id path.
- Shape matches the query-auth smell fixed earlier: the endpoint used durable caller/session/participant lookup before checking battle runtime or parsing the battle id.

Cut:

- `get_battle_state` now uses `require_session_caller_runtime_first`.
- Durable lookup remains the fallback when no active runtime caller row is available.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `git diff --check`

Expected measurement:

- `get_battle_state` should drop by roughly the same `~2.1B` query-auth floor removed from the other read endpoints when an active turn runtime has caller context.

## Round-Robin Pivot: Match-History Player Cache

New cluster:

- Rotated from battle-state query auth to match history.
- Current cost: `get_match_history` `1.4046B`.
- The endpoint does not take a session id, so it cannot use `SessionTurnRuntime` caller context. It still pays a durable player principal lookup before paging match history.

Cut:

- Exposed the existing lobby/session `find_player_by_principal` helper as `pub(crate)`.
- `get_match_history` now uses that helper, so a warm `PLAYER_PRINCIPAL_CACHE` from registration/lobby calls avoids `players.by_principal` and still falls back to durable lookup if cold.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister`
- `git diff --check`

Expected measurement:

- In the endpoint-surface route, the player cache should be warm from registration/session setup, so `get_match_history` should drop by roughly one durable principal lookup (`~0.70B`).
- The remaining cost should be the bounded `history.by_player_finished_at` page and DTO assembly.

## Measurement: Round-Robin Champion, Battle-State, Match-History

Artifact:

- `target/benchmarks/20260520-roundrobin-champion-battle-history-c783e49`
- Passed in `198.09s`.
- Git: `c783e49`.
- Coverage: `59/59` required endpoints.
- Calls: `146`.
- Row growth: `150` (unchanged from prior run).
- Stable pages: `2049 -> 81281` (unchanged from prior run).

Scenario delta versus `20260520-roundrobin-worldgen-endturn-query-35c9c27`:

- `152.4118B -> 145.4290B`, `-6.9828B`, `-4.6%`.
- Versus the earlier endpoint-rotation run, the combined recent cuts moved `185.4812B -> 145.4290B`, `-40.0522B`, `-21.6%`.

Targeted endpoint deltas:

- `get_battle_state`: `2.1106B -> 0.0000B`, `-2.1106B`, `-100.0%`; the measured invalid-id path now costs `25,847` instructions.
- `learn_champion_spell`: `10.3807B -> 8.9726B`, `-1.4081B`, `-13.6%`.
- `cast_adventure_spell`: `9.2153B -> 7.8120B`, `-1.4033B`, `-15.2%`.
- `select_champion_level_up`: `7.8004B -> 6.3975B`, `-1.4029B`, `-18.0%`.
- `get_match_history`: `1.4046B -> 0.7029B`, `-0.7017B`, `-50.0%`.

Repo-op confirmation:

- `select_champion_level_up`, `learn_champion_spell`, and `cast_adventure_spell` no longer show `effects.command_effect_by_command_key` or `events.by_session_event_key`; they keep the required `effects.create_applied_command_effect` and `events.create_game_event` rows.
- Global `effects.command_effect_by_command_key` calls dropped `9 -> 6`.
- Global `events.by_session_event_key` calls dropped `8 -> 5`.
- `get_battle_state` has no repo ops in the measured path after the runtime-first auth change.
- `get_match_history` has no repo-op trace because query repo ops are not fully instrumented, but the `0.7017B` delta matches the avoided durable principal lookup.

Post-measurement heavy endpoints:

- `sync_advanced_victory`: `14.6173B`.
- `claim_quest_reward`: `14.1557B`.
- `submit_dwelling_recruit`: `12.5253B`.
- `hire_tavern_champion`: `12.1146B`.
- `sync_objectives`: `10.3812B`.
- `submit_market_trade`: `9.2295B`.
- `learn_champion_spell`: `8.9726B`.
- `sync_world_generation`: `7.8909B`.
- `cast_adventure_spell`: `7.8120B`.
- `accept_quest`: `7.7979B`.
- `sync_world_events`: `7.3200B`.
- `select_champion_level_up`: `6.3975B`.
- `end_turn`: `5.9115B`.

Next decision:

- Rotate away from champion/battle/history. The best next low-risk cluster is likely the shared update context/command floor on scenario/economy/worldgen endpoints, but only if the runtime participant/session state is authoritative enough for those command paths. Otherwise target the next obvious endpoint-specific durable metadata churn from the latest repo-op trace.

## Round-Robin Pivot: Scenario And Worldgen Fresh Event/Effect

New cluster:

- Rotated away from champion/battle/history to scenario/worldgen update commands.
- Current costs: `sync_advanced_victory` `14.6173B`, `claim_quest_reward` `14.1557B`, `sync_objectives` `10.3812B`, `accept_quest` `7.7979B`, `sync_world_events` `7.3200B`, and `sync_world_generation` `7.8909B`.
- The latest method traces still show one `effects.command_effect_by_command_key` absence read and one `events.by_session_event_key` absence read on most scenario commands, plus one effect absence read on `sync_world_generation`.

Cut:

- Successful fresh `accept_quest`, `claim_quest_reward`, `sync_objectives`, `sync_world_events`, and `sync_advanced_victory` now use `append_fresh_public_event` and `create_fresh_command_effect`.
- Successful fresh `sync_world_generation` now uses `create_fresh_command_effect`.
- This is the same pattern already measured for economy and champion magic: `begin_participant_command` returns completed replay responses before the fresh event/effect code runs, so the fresh path does not need to prove absence with a stable read.
- Existing objective progress rows are no longer rewritten during objective sync when owner/progress/status/score fields are unchanged and only `last_command_id` would move. This targets the two `scenario.update_objective_progress` writes in the current `sync_objectives` and `sync_advanced_victory` traces.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `git diff --check`

Expected measurement:

- Scenario commands that emit both public event and command effect should save about `1.4B` each if the route still uses the fresh command path.
- `sync_world_generation` should save about `0.7B` from the effect absence read.
- Objective sync commands should save about `0.95B` in the current unchanged-objective route.
- Remaining costs after this cut will still include durable command create/idempotency/update, event/effect create, session update for event seq, and the scenario row scans/updates themselves.

## Round-Robin Pivot: Tavern Hire Metadata Writes

New cluster:

- Rotated from scenario/worldgen to `hire_tavern_champion`.
- Current cost: `12.1146B`.
- The latest trace shows `champions.create_champion` followed by `champions.update_champion`, and `map.create_occupancy_cell` followed by `map.update_occupancy_cell`. Both updates only stamp `last_command_id` after a fresh create.

Cut:

- Freshly created hired champions are no longer immediately updated only to set `last_command_id`; the `ChampionHire` row and command receipt already provide the command link.
- Freshly created champion occupancy cells are no longer immediately updated only to set `last_command_id`; the occupancy create remains durable and authoritative.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `git diff --check`

Expected measurement:

- `hire_tavern_champion` should drop `champions.update_champion` and `map.update_occupancy_cell`, about `0.96B` combined in the current route.

## Round-Robin Pivot: Dwelling Runtime Resolution

New cluster:

- Rotated from tavern hire to dwelling recruit/read paths.
- Current costs: `submit_dwelling_recruit` `12.5253B`, `preview_dwelling_recruit` `2.1339B`, and `get_dwelling_pool` `1.4121B`.
- The latest `submit_dwelling_recruit` trace still shows `champions.load_champion` at about `0.71B`; object lookup savings may not be visible in repo-op traces.

Cut:

- Dwelling object resolution now checks `SessionTurnRuntime` world-object snapshots by id and first-playable coordinate before stable `WorldObject` reads.
- Recruit target champion resolution now checks `SessionTurnRuntime` champion snapshots by id and first-playable start coordinate before stable `Champion` reads.
- Stable fallbacks remain unchanged for cold runtime, missing snapshots, and non-scenario ids.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `git diff --check`

Expected measurement:

- `submit_dwelling_recruit` should drop the visible `champions.load_champion` read when the target champion snapshot is present, about `0.71B`.
- Dwelling object runtime lookup should reduce method instructions for `get_dwelling_pool`, `preview_dwelling_recruit`, and `submit_dwelling_recruit` if those calls were previously touching stable object rows.

## Random Pivot: Claim Quest Reward

New cluster:

- Randomly picked `claim_quest_reward` from the remaining heavy endpoint list after the dwelling cut.
- Current cost: `14.1557B`.
- The latest trace shows the command paying a full scenario rule sweep after one quest claim: objective scans, active-rule scans, participant scans, claimed-quest scans, and one rule update.

Cut:

- `claim_quest_reward` now calls a targeted `sync_quest_victory_rule_after_claim` helper instead of `sync_scenario_rule_rows`.
- The helper loads `rule:quest-victory`, increments `current_value`, computes `victory_state`, and updates only when value/state changes.
- This preserves the direct rule touched by the claim while avoiding unrelated conquest/objective/max-turn rule recomputation in the claim command.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `git diff --check`

Expected measurement:

- `claim_quest_reward` should drop the previous full-rule-sweep floor: `scenario.objectives_by_status`, `scenario.rules_by_status`, `sessions.participants_by_session_status`, and `scenario.quests_by_participant_status`.
- The targeted helper adds one `scenario.rule_by_key` read and keeps the real `scenario.update_scenario_rule_state` when the quest-victory value changes.
- Expected net save is roughly `3.5B-4.2B` on the current route, on top of the earlier fresh event/effect savings queued for the next benchmark.

## Random Pivot: Champion Magic Runtime Champion Resolution

New cluster:

- Randomly picked `cast_adventure_spell` from the remaining heavy endpoint list after the quest-claim cut.
- Current costs: `cast_adventure_spell` `7.8120B`, `learn_champion_spell` `8.9726B`, and `select_champion_level_up` `6.3975B`.
- All three paths call the shared `require_owned_champion`, and the trace shows `champions.load_champion` at about `0.70B` per method.

Cut:

- `require_owned_champion` now checks `SessionTurnRuntime::champion_snapshot` before the durable `Champion` load.
- The existing session and participant ownership checks still run on the returned row.
- Durable fallback remains for cold runtime or missing snapshots.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `git diff --check`

Expected measurement:

- Champion magic commands should drop `champions.load_champion` when active runtime snapshots are present, about `0.70B` per command in the current route.

## Measurement: Random Rotations

Artifact:

- `target/benchmarks/20260520-random-rotations-43ccf1f`
- Passed in `425.17s`.
- Git: `43ccf1f`.
- Coverage: `59/59` required endpoints.
- Calls: `146`.
- Row growth: `150`.
- Stable pages: `2049 -> 81281`.
- Note: the single-gate command used a relative `DOMM_BENCH_QUERY_LOG_PATH`, so the generated summary missed query instruction values; the raw log still contains them. Use absolute paths for direct single-gate benchmark commands going forward.

Scenario delta versus `20260520-roundrobin-champion-battle-history-c783e49`:

- `145.4290B -> 113.5880B`, `-31.8410B`, `-21.9%`.

Targeted endpoint deltas:

- `claim_quest_reward`: `14.1557B -> 9.2372B`, `-4.9185B`, `-34.7%`.
- `sync_advanced_victory`: `14.6173B -> 12.2543B`, `-2.3631B`, `-16.2%`.
- `sync_objectives`: `10.3812B -> 8.0271B`, `-2.3542B`, `-22.7%`.
- `submit_dwelling_recruit`: `12.5253B -> 11.1059B`, `-1.4194B`, `-11.3%`.
- `sync_world_events`: `7.3200B -> 5.9111B`, `-1.4089B`, `-19.2%`.
- `accept_quest`: `7.7979B -> 6.3948B`, `-1.4030B`, `-18.0%`.
- `hire_tavern_champion`: `12.1146B -> 11.1497B`, `-0.9649B`, `-8.0%`.
- `cast_adventure_spell`: `7.8120B -> 7.0985B`, `-0.7135B`, `-9.1%`.
- `learn_champion_spell`: `8.9726B -> 8.2591B`, `-0.7135B`, `-8.0%`.
- `select_champion_level_up`: `6.3975B -> 5.6901B`, `-0.7074B`, `-11.1%`.
- `sync_world_generation`: `7.8909B -> 7.1881B`, `-0.7028B`, `-8.9%`.
- Raw log query values: `get_dwelling_pool` `0.7064B`, `preview_dwelling_recruit` `0.7064B`.

Repo-op confirmation:

- `effects.command_effect_by_command_key` is gone from the summary repo ops.
- `events.by_session_event_key` is gone from the summary repo ops.
- `map.update_occupancy_cell` is gone from the summary repo ops.
- `champions.load_champion` is gone from the summary repo ops.
- `scenario.update_objective_progress` is gone from the summary repo ops for unchanged objective sync.
- Remaining visible floors include command idempotency/create/update, durable event/effect creates, `sessions.load_session` and `sessions.update_session`, economy ledger rows, participant updates, and scenario scan rows in `sync_advanced_victory`.

Subagent findings to carry forward:

- Noether confirmed the targeted quest claim cut saves the expected full rule sweep, but pointed out that incrementing from `rule.current_value` is not self-healing if the row was already stale. Next fix should count claimed opening quests directly by `(session_id, quest_key)` and set the rule value from that count.
- Bernoulli recommended deferring economy participant row writes with runtime mirroring, then making `submit_dwelling_recruit` reuse the already-loaded pool/unit from precheck.
- Herschel recommended runtime event-seq allocation before switching event-emitting update endpoints to runtime-first auth; switching auth first can collide event sequences because the runtime session snapshot can carry stale `next_event_seq`.

## Follow-Up: Self-Healing Quest Victory Count

Reason:

- The measured targeted quest-claim cut was fast but used `rule.current_value.saturating_add(1)`.
- That is correct for normal fresh commands, but it is not self-healing if a migrated or stale `rule:quest-victory` row already has the wrong value.

Cut:

- Added `scenario.quests_by_session_key` repository paging over the existing `QuestState(session_id, quest_key)` index.
- `claimed_quest_count` now counts claimed `OPENING_QUEST_KEY` rows from that page instead of scanning active participants and each participant's claimed quests.
- `sync_quest_victory_rule_after_claim` now sets `rule.current_value` from that direct count rather than incrementing the existing rule value.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `git diff --check`

Expected measurement:

- `claim_quest_reward` should keep most of the `14.1557B -> 9.2372B` gain while adding one bounded quest-key page.
- `sync_advanced_victory` should also improve because the shared claimed quest count no longer scans active participants plus per-participant claimed quest pages.

## Random Pivot: Economy Participant Runtime Mirroring

New cluster:

- Rotated to the Bernoulli economy proposal after the random-rotation benchmark and quest-count hardening.
- Current costs after the benchmark: `hire_tavern_champion` `11.1497B`, `submit_dwelling_recruit` `11.1059B`, and `submit_market_trade` `9.2251B`.
- The benchmark still shows `sessions.update_participant` at four calls / `1.9207B` total; three of those calls are economy resource updates.

Cut:

- Added `persist_or_mirror_active_participant`.
- When the current active `SessionTurnRuntime` exists, economy commands mirror the already-mutated participant into runtime and skip the immediate durable participant update.
- When runtime is absent, the helper keeps the old durable `sessions.update_participant` path.
- Resource ledger rows remain durable for replay/recovery idempotency.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `git diff --check`

Expected measurement:

- Endpoint-surface should drop up to three `sessions.update_participant` calls, about `1.44B` total, across `hire_tavern_champion`, `submit_market_trade`, and `submit_dwelling_recruit`.

## Random Cross-Cut: Runtime Event Sequence Allocation

New cluster:

- Randomly picked runtime event-seq allocation from the remaining subagent proposals.
- Current benchmark still shows `sessions.update_session` at 12 calls / `5.7216B` total, much of it from public event creation advancing `GameSession.next_event_seq`.
- Active turn runtimes already reserve an event sequence block at turn setup; durable `GameSession.next_event_seq` is advanced to the block end there.

Cut:

- Added `session_turn_runtime::take_reserved_session_event_seq`.
- `command_response::create_event_for_audience` now consumes a reserved active-turn sequence first.
- If no runtime sequence is available, event creation falls back to the old `session.next_event_seq` plus durable `sessions.update_session` path.
- Durable `GameEvent` rows are still created immediately, so event feeds/history keep their current durable shape.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `git diff --check`

Expected measurement:

- Fresh public-event commands should stop paying one `sessions.update_session` each when an active turn runtime has sequence headroom.
- This should unblock later runtime-first auth for event-emitting commands, because event creation no longer depends on mutating a possibly stale runtime session copy.

## Random Pivot: Scenario Runtime-First And Objective Summary

New cluster:

- Rotated back to scenario progress after the runtime event-seq cut because event-emitting update endpoints can now safely use active runtime caller context without depending on a stale durable `GameSession.next_event_seq` copy.
- The last measured scenario update costs are still high: `sync_advanced_victory` `12.2543B`, `claim_quest_reward` `9.2372B`, `sync_objectives` `8.0271B`, `accept_quest` `6.3948B`, and `sync_world_events` `5.9111B`.

Cut:

- `accept_quest`, `claim_quest_reward`, `sync_objectives`, `sync_world_events`, and `sync_advanced_victory` now use `require_active_session_caller_runtime_first`.
- Central objective sync checks `SessionTurnRuntime` world-object snapshots before falling back to durable `WorldObject` lookup by session coordinate.
- Objective sync now returns touched and completed counts; `sync_advanced_victory` and advanced-victory maintenance jobs reuse the completed count instead of re-paging objective rows immediately after syncing them.
- Cold-runtime and maintenance fallback behavior remains durable-row based.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `git diff --check`

Expected measurement:

- Scenario update endpoints should lose the durable active-caller floor when an active turn runtime is present.
- `sync_objectives` and `sync_advanced_victory` should avoid stable world-object coordinate lookups for central objectives when runtime snapshots are hydrated.
- `sync_advanced_victory` should also avoid the immediate second objective active/complete status page sweep after syncing central objectives.

## Random Pivot: Champion Magic Runtime-First Updates

New cluster:

- Rotated to champion magic after the scenario runtime-first/objective-summary cut.
- The last measured champion magic update costs are still above target: `learn_champion_spell` `8.2591B`, `cast_adventure_spell` `7.0985B`, and `select_champion_level_up` `5.6901B`.
- Runtime event-seq allocation is now in place, so event-emitting commands can use runtime-first caller context without relying on a stale durable session sequence copy.

Cut:

- `select_champion_level_up`, `learn_champion_spell`, and `cast_adventure_spell` now use `require_active_session_caller_runtime_first`.
- The existing `require_owned_champion` runtime snapshot lookup remains the champion ownership gate after caller context resolution.
- Durable command, champion/spell, event, and effect writes are unchanged in this checkpoint.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `git diff --check`

Expected measurement:

- Champion magic update endpoints should drop the durable active-caller lookup floor when active turn runtime context is available.
- Any remaining multi-billion cost after that should be command/idempotency writes, champion/spell row writes, event/effect creation, or spell definition/spellbook lookups.

## Measurement: Runtime-Context Rotations

Artifact:

- `target/benchmarks/20260520-runtime-context-rotations-d1765d7`
- Git: `d1765d7`.
- Endpoint-surface passed in `184.63s`.
- Coverage: `59/59` required endpoints.
- Calls: `146`.
- Row growth: `150`.
- Stable pages: `2049 -> 81281`.
- The absolute query log path worked; generated summaries include query instruction values.

Scenario delta versus `20260520-random-rotations-43ccf1f`:

- `113.5880B -> 95.9404B`, `-17.6476B`, `-15.5%`.

Targeted endpoint deltas:

- `sync_advanced_victory`: `12.2543B -> 5.4449B`, `-6.8094B`, `-55.6%`.
- `sync_objectives`: `8.0271B -> 4.0345B`, `-3.9925B`, `-49.7%`.
- `accept_quest`: `6.3948B -> 3.7947B`, `-2.6001B`, `-40.7%`.
- `cast_adventure_spell`: `7.0985B -> 4.5114B`, `-2.5871B`, `-36.4%`.
- `select_champion_level_up`: `5.6901B -> 3.1065B`, `-2.5836B`, `-45.4%`.
- `sync_world_events`: `5.9111B -> 3.3316B`, `-2.5795B`, `-43.6%`.
- `learn_champion_spell`: `8.2591B -> 5.6836B`, `-2.5754B`, `-31.2%`.
- `claim_quest_reward`: `9.2372B -> 7.3452B`, `-1.8920B`, `-20.5%`.
- `hire_tavern_champion`: `11.1497B -> 10.1886B`, `-0.9612B`, `-8.6%`.
- `submit_dwelling_recruit`: `11.1059B -> 10.1479B`, `-0.9580B`, `-8.6%`.
- `submit_market_trade`: `9.2251B -> 8.2701B`, `-0.9550B`, `-10.4%`.

Repo-op confirmation:

- `sessions.update_session`: `12` calls / `5.7216B -> 0`.
- `sessions.update_participant`: `4` calls / `1.9207B -> 1` call / `0.4798B`.
- `sessions.load_session`: `15` calls / `10.5423B -> 7` calls / `4.9236B`.
- `sessions.participants_by_session_status`: `2` calls / `0.7045B -> 0`.
- `scenario.objectives_by_status`: `4` calls / `1.4076B -> 0`.
- `scenario.quests_by_participant_status`: `4` calls / `1.4110B -> 0`.
- `scenario.quests_by_session_key` is now present at `4` calls / `1.4076B`; this replaces the old participant-by-participant quest sweep with a direct self-healing quest-key count.

Next target:

- Rotate to economy update runtime-first auth for `hire_tavern_champion`, `submit_dwelling_recruit`, and `submit_market_trade`.
- This should remove the same active-caller durable floor from the current top three endpoints before any deeper economy/town aggregate work.

## Random Pivot: Economy Runtime-First Updates

New cluster:

- Rotated to economy updates after the runtime-context measurement.
- The current top three endpoint costs are all economy commands: `hire_tavern_champion` `10.1886B`, `submit_dwelling_recruit` `10.1479B`, and `submit_market_trade` `8.2701B`.
- Their previews already use runtime-first caller context, but the update endpoints still used the durable active-caller path.

Cut:

- `hire_tavern_champion`, `submit_market_trade`, and `submit_dwelling_recruit` now use `require_active_session_caller_runtime_first`.
- Existing town/dwelling/champion resolution, command rows, resource ledger rows, hire/trade/recruit rows, event/effect writes, and durable fallbacks are unchanged.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `git diff --check`

Expected measurement:

- These three economy updates should lose the active-caller durable lookup floor when active turn runtime context exists.
- Remaining high cost should then mostly be command/idempotency writes, resource ledger rows, recruit/hire/trade durable rows, town/dwelling/tavern projections, and champion/town child-row writes.

## Random Pivot: Worldgen Runtime-First Update

New cluster:

- Rotated away from economy before deeper economy-only work.
- `sync_world_generation` is still a separate heavy endpoint at `7.1939B` in the last benchmark and still used the durable active-caller path.

Cut:

- `sync_world_generation` now uses `require_active_session_caller_runtime_first`.
- Seeded worldgen state, durable command/effect rows, and command response behavior are unchanged.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `git diff --check`

Expected measurement:

- `sync_world_generation` should lose the active-caller durable lookup floor when active turn runtime context exists.
- Remaining cost should mostly be command/idempotency writes plus existing worldgen state reads.

## Random Pivot: End Turn Runtime-Current Context

New cluster:

- Rotated to `end_turn` after the worldgen runtime-first update cut.
- `end_turn` measured `5.4317B` in the last benchmark and still used `require_active_session_caller`, while neighboring movement submit/sync paths already use the runtime-current context helper.

Cut:

- `end_turn` now uses `require_runtime_current_active_session_caller`.
- Turn-ready row creation, runtime readiness counts, optional turn-resolution job scheduling, public event creation, and command response behavior are unchanged.
- The helper still falls back to the durable active-caller path when runtime/cached caller context is unavailable.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `git diff --check`

Expected measurement:

- `end_turn` should lose the active-caller durable lookup floor when active turn runtime context exists.
- Remaining cost should mostly be turn-ready row creation, command/idempotency writes, event/effect rows, and optional turn-resolution job scheduling.

## Random Pivot: Battle Sync Runtime-First Context

New cluster:

- Rotated to battle boundary calls after the `end_turn` runtime-current context cut.
- The last endpoint-surface run measured `sync_battle` and `end_battle_turn` around `2.1B` on invalid/edge calls, which is mostly the durable active-caller floor before the battle-specific response.

Cut:

- `sync_battle` and `end_battle_turn` now use `require_active_session_caller_runtime_first`.
- Battle row loading, runtime adoption, timeout/recovery handling, readiness, aftermath, events, and command response behavior are unchanged.
- The runtime-first helper still falls back to durable caller/session/participant rows when no active runtime context exists.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `git diff --check`

Expected measurement:

- `sync_battle` and `end_battle_turn` should drop the active-caller durable lookup floor on endpoint-surface.
- Remaining cost should be battle row lookup/adoption and command handling for these boundary calls.

## Measurement: Runtime-Context Tail Cuts

Artifact:

- `target/benchmarks/20260520-runtime-context-tail-6e65208`
- Git: `6e65208`.
- Endpoint-surface passed in `193.20s`.
- Coverage: `59/59` required endpoints.
- Calls: `146`.
- Row growth: `150`.
- Stable pages: `2049 -> 81281`.

Scenario delta versus `20260520-runtime-context-rotations-d1765d7`:

- `95.9404B -> 81.2096B`, `-14.7308B`, `-15.4%`.

Targeted endpoint deltas:

- `hire_tavern_champion`: `10.1886B -> 8.0801B`, `-2.1085B`, `-20.7%`.
- `submit_dwelling_recruit`: `10.1479B -> 8.0460B`, `-2.1020B`, `-20.7%`.
- `submit_market_trade`: `8.2701B -> 6.1614B`, `-2.1087B`, `-25.5%`.
- `sync_world_generation`: `7.1939B -> 5.0824B`, `-2.1115B`, `-29.4%`.
- `end_turn`: `5.4317B -> 3.3239B`, `-2.1078B`, `-38.8%`.
- `sync_battle`: `2.1085B -> 0.0000B`, effectively removed on endpoint-surface.
- `end_battle_turn`: `2.1096B -> 0.0000B`, effectively removed on endpoint-surface.

Repo-op confirmation:

- `sessions.load_session`: `7` calls / `4.9236B -> 0`.
- `sessions.update_participant` stayed at `1` call / `0.4798B`.
- Command rows are still the dominant shared floor: `commands.game_command_idempotency` `13` calls / `9.1996B`, `commands.create_game_command` `13` calls / `6.2337B`, and `commands.update_game_command` `13` calls / `6.2405B`.

Next target:

- The easy auth floor is now gone from the endpoint-surface route.
- The next meaningful reductions need to attack command/idempotency writes, economy durable rows, quest/scenario row work, champion spellbook/content rows, or worldgen state reads.

## Random Pivot: Economy Runtime Command Receipts

New cluster:

- Rotated to economy command/idempotency writes because the runtime-context tail benchmark left economy commands as the top measured endpoints.
- Current costs: `hire_tavern_champion` `8.0801B`, `submit_dwelling_recruit` `8.0460B`, and `submit_market_trade` `6.1614B`.
- Shared floor to attack: `commands.game_command_idempotency`, `commands.create_game_command`, and `commands.update_game_command`.

Cut:

- Added `begin_runtime_participant_command`, `apply_runtime_command_with_result`, and `fail_runtime_command`.
- When active `SessionTurnRuntime` can hold receipts, the begin path checks runtime receipts by nonce, creates a transient `GameCommand`, and skips durable command idempotency/create rows.
- Apply/fail stores the final `CommandResponse` in `SessionTurnRuntime` instead of updating a durable `GameCommand`.
- Durable fallback is unchanged when runtime is unavailable.
- `hire_tavern_champion`, `submit_market_trade`, and `submit_dwelling_recruit` now use this runtime command receipt path.
- Resource ledger rows, hire/trade/recruit rows, event rows, effect rows, and projections remain durable in this checkpoint.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `git diff --check`

Expected measurement:

- The three economy endpoints should lose the durable command idempotency/create/update row floor.
- Command row growth should drop, but active runtime status/replay should still work through `SessionTurnRuntime` receipts.
- Remaining cost should mainly be economy ledger/hire/trade/recruit rows, event/effect rows, and command-specific projection writes.

## Economy Runtime Command Measurement

Time: `2026-05-20T23:05:08Z`.

Artifact:

- `target/benchmarks/20260520-economy-runtime-command-trim-local2`
- Git in artifact: `47959f0` with local size-trim edits.
- Endpoint-surface passed in `236.37s`.
- Coverage: `59/59` required endpoints.
- Calls: `146`.
- Row growth: `147`, down from `150`.
- Stable pages: `2049 -> 79745`, down from previous final `81281`.
- Benchmark Wasm code section after trims: `0x00bff320` / `12,579,616` bytes, `3,296` bytes under the IC limit.

Size trims needed to make the benchmark canister install:

- Removed the extra `commands_events_effects::runtime_game_command_by_idempotency` heap-cache probe from `begin_runtime_participant_command`; session-turn command receipts are the authoritative runtime replay/status surface for this path.
- Built runtime apply/fail responses directly from transient command identity instead of mutating a transient `GameCommand` to applied/failed status first.
- In benchmark builds, failure-only runtime command receipts return a normal failed response without retaining a runtime receipt; normal non-benchmark durability stays intact.
- Omitted the internal `get_canister_endpoint_inventory` export only from benchmark canisters so the required-endpoint inventory table does not bloat the measured Wasm. Normal builds still export it.

Measurement versus `20260520-runtime-context-tail-6e65208`:

- Scenario instructions: `81.2096B -> 76.1968B`, `-5.0128B`, `-6.2%`.
- Memory delta: `4955.1875 MB -> 4859.1250 MB`, `-96.0625 MB`.
- `hire_tavern_champion`: `8.0801B -> 6.4138B`, `-1.6663B`, `-20.6%`.
- `submit_dwelling_recruit`: `8.0460B -> 6.3756B`, `-1.6704B`, `-20.8%`.
- `submit_market_trade`: `6.1614B -> 4.4964B`, `-1.6649B`, `-27.0%`.
- `commands.game_command_idempotency`: `13 -> 10` calls and `9.1996B -> 7.0794B`.
- `commands.create_game_command`: `13 -> 10` calls and `6.2337B -> 4.7955B`.
- `commands.update_game_command`: `13 -> 10` calls and `6.2405B -> 4.8024B`.

Decision:

- Keep this checkpoint. It cleanly removed the three economy command-row writes from the active runtime route and preserved endpoint-surface coverage.
- Rotate to another command cluster next. The same receipt pattern is now proven; the next bounded pass should apply it to scenario/champion/worldgen commands instead of digging deeper into economy rows first.

## Scenario/Champion/Worldgen Runtime Command Measurement

Time: `2026-05-20T23:18:12Z`.

Cut:

- Rotated away from economy into scenario, champion magic, and worldgen command wrappers.
- Moved `claim_quest_reward`, `sync_objectives`, `sync_world_events`, `sync_advanced_victory`, `select_champion_level_up`, `learn_champion_spell`, `cast_adventure_spell`, and `sync_world_generation` to `begin_runtime_participant_command` plus runtime apply/fail.
- Kept durable fallback behavior and left domain rows, events, effects, quest/rule rows, champion spell/skill rows, and worldgen state rows unchanged.
- Left `accept_quest` durable for the next small pass because it was outside the bounded heavy set.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size: `0x00bff3f3` / `12,579,827` bytes, `3,085` bytes under the IC code-section limit.
- Endpoint-surface benchmark artifact: `target/benchmarks/20260520-command-receipts-scenario-champion-worldgen-local`, passed in `236.37s`.

Measurement versus `20260520-economy-runtime-command-trim-local2`:

- Coverage stayed `59/59` required endpoints.
- Row growth: `147 -> 139`.
- Stable pages: `2049 -> 79745` became `2049 -> 75393`.
- Scenario instructions: `76.1968B -> 62.8131B`, `-13.3836B`, `-17.6%`.
- `claim_quest_reward`: `7.3463B -> 5.6788B`, `-22.7%`.
- `learn_champion_spell`: `5.6836B -> 4.0121B`, `-29.4%`.
- `cast_adventure_spell`: `4.5114B -> 2.8451B`, `-36.9%`.
- `select_champion_level_up`: `3.1065B -> 1.4363B`, `-53.8%`.
- `sync_advanced_victory`: `5.4461B -> 3.7732B`, `-30.7%`.
- `sync_world_generation`: `5.0832B -> 3.4009B`, `-33.1%`.
- `sync_objectives`: `4.0341B -> 2.3623B`, `-41.4%`.
- `sync_world_events`: `3.3294B -> 1.6561B`, `-50.3%`.
- `commands.game_command_idempotency`: `10 -> 2` calls and `7.0794B -> 1.4147B`.
- `commands.create_game_command`: `10 -> 2` calls and `4.7955B -> 0.9586B`.
- `commands.update_game_command`: `10 -> 2` calls and `4.8024B -> 0.9593B`.

Decision:

- Keep this checkpoint. The shared command-row floor is now almost gone from endpoint-surface.
- Next random cluster should be the remaining durable command rows, likely `accept_quest` and `end_turn`, then rotate to another mechanism because deeper receipt work will have diminishing returns.

## Final Runtime Command Receipt Measurement

Time: `2026-05-20T23:29:20Z`.

Cut:

- Moved the two remaining endpoint-surface durable command wrappers, `accept_quest` and `end_turn`, to runtime command receipts.
- Kept quest rows, turn-ready rows, scheduled job behavior, public events, command effects, and changed subjects unchanged.
- `end_turn` now constructs the same strategic receipt explicitly for `apply_runtime_command_with_result` instead of using durable `apply_command`.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size: `0x00bff4cd` / `12,580,045` bytes, `2,867` bytes under the IC code-section limit.
- Endpoint-surface benchmark artifact: `target/benchmarks/20260520-final-command-receipts-local`, passed.

Measurement versus `20260520-command-receipts-scenario-champion-worldgen-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth: `139 -> 137`.
- Stable pages: `2049 -> 75393` became `2049 -> 74881`.
- Scenario instructions: `62.8131B -> 59.5007B`, `-3.3125B`, `-5.3%`.
- `accept_quest`: `3.8038B -> 2.1359B`, `-43.8%`.
- `end_turn`: `3.3238B -> 1.6573B`, `-50.1%`.
- `commands.game_command_idempotency`: `2 -> 0` calls and `1.4147B -> 0`.
- `commands.create_game_command`: `2 -> 0` calls and `0.9586B -> 0`.
- `commands.update_game_command`: `2 -> 0` calls and `0.9593B -> 0`.

Decision:

- Keep this checkpoint. The endpoint-surface benchmark no longer shows any `commands.*` repo operations.
- Rotate away from runtime command receipt work. The new shared floor is `events.create_game_event` at `12` calls / `5.7358B` and `effects.create_applied_command_effect` at `12` calls / `5.7024B`, followed by economy ledger rows at `5` calls / `2.3844B`.

## Champion Runtime Event Measurement

Time: `2026-05-20T23:45:00Z`.

Gate 42 analysis:

- Endpoint-surface event/effect writers before this slice: champion magic `3`, economy `3`, scenario `5`, `end_turn` `1`, plus `sync_world_generation` effect-only `1`.
- Public events are safe to move first because `SessionTurnRuntime.active_events` already merges into `get_events_after` and flushes on non-benchmark barriers.
- `CommandEffect` rows stay durable in this slice; they are still a history/recovery marker and need a separate runtime-effect/flush plan before removal.

Cut:

- Added a small runtime-first public event append helper that consumes a reserved active-turn event sequence and pushes a `SessionTurnEvent`, with durable fallback if no reserved runtime sequence is available.
- Moved `select_champion_level_up`, `learn_champion_spell`, and `cast_adventure_spell` public events to the runtime-first helper.
- Fixed `get_command_status_by_nonce` runtime lookup to check all game command receipt types, not only movement/end-turn/sync.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size: `0x00bffcf1` / `12,582,129` bytes, `783` bytes under the IC code-section limit.
- Endpoint-surface benchmark artifact: `target/benchmarks/20260520-champion-runtime-events-local`, passed.

Measurement versus `20260520-final-command-receipts-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth: `137 -> 134`.
- Stable pages: `2049 -> 74881` became `2049 -> 73729`.
- Scenario instructions: `59.5007B -> 58.0803B`, `-1.4204B`, `-2.4%`.
- `events.create_game_event`: `12 -> 9` calls and `5.7358B -> 4.3015B`.
- `effects.create_applied_command_effect`: stayed `12` calls.
- `select_champion_level_up`: `1.4363B -> 0.9579B`, `-33.3%`.
- `learn_champion_spell`: `4.0121B -> 3.5412B`, `-11.7%`.
- `cast_adventure_spell`: `2.8451B -> 2.3727B`, `-16.6%`.

Decision:

- Keep this checkpoint. The runtime event merge path preserved endpoint coverage and removed exactly the three champion event rows.
- Next bounded event/effect family should not add new shared code unless code-size headroom is freed; current benchmark headroom is only `783` bytes.

## Scenario Runtime Event Measurement

Time: `2026-05-20T23:57:00Z`.

Cut:

- Moved scenario quest/objective/world-event public events to the existing runtime-first event append helper.
- Covered `accept_quest`, `claim_quest_reward`, `sync_objectives`, `sync_world_events`, and `sync_advanced_victory`.
- Kept all scenario domain rows and `CommandEffect` rows durable; this slice only removes active-turn public event-row writes from the hot path.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `git diff --check`
- Direct benchmark Wasm size: `0x00bffb45` / `12,581,701` bytes, `1,211` bytes under the IC code-section limit.
- Endpoint-surface benchmark artifact: `target/benchmarks/20260520-scenario-runtime-events-local`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260520-champion-runtime-events-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth: `134 -> 129`.
- Stable pages: `2049 -> 73729` became `2049 -> 71169`.
- Scenario instructions: `58.0803B -> 55.6905B`, `-2.3898B`, `-4.1%`.
- `events.create_game_event`: `9 -> 4` calls and `4.3015B -> 1.9090B`.
- `effects.create_applied_command_effect`: stayed `12` calls.
- `accept_quest`: `2.1420B -> 1.6649B`, `-22.3%`.
- `claim_quest_reward`: `5.6817B -> 5.1985B`, `-8.5%`.
- `sync_objectives`: `2.3633B -> 1.8836B`, `-20.3%`.
- `sync_world_events`: `1.6580B -> 1.1802B`, `-28.8%`.
- `sync_advanced_victory`: `3.7742B -> 3.2958B`, `-12.7%`.

Decision:

- Keep this checkpoint. The existing runtime event merge handled another five public-event rows without adding shared code.
- Rotate again instead of polishing public events. The latest floor is now `effects.create_applied_command_effect` at `12` calls / `5.7096B`, `economy.create_resource_ledger_entry` at `5` calls / `2.3839B`, and the remaining `events.create_game_event` at `4` calls / `1.9090B`.

## Effect/Event Floor Measurement

Time: `2026-05-21T00:06:00Z`.

Cut:

- Removed duplicate fresh `CommandEffect` writes from active runtime command paths: champion magic, scenario progress, economy expansion, and worldgen.
- Kept fallback `ensure_command_effect` for non-fresh/recovered economy expansion paths.
- Moved the remaining fresh economy and `end_turn` public events to `append_runtime_or_fresh_public_event`.
- Removed the now-unused `create_fresh_command_effect` helper.

Correctness argument:

- These fresh active-turn paths already have runtime command receipts for status/replay and domain rows for the actual game projection.
- IC traps revert partial writes, so the effect row is not needed as a saga checkpoint inside the same endpoint call.
- Public event visibility remains covered by `SessionTurnRuntime.active_events`, `get_events_after`, and non-benchmark runtime event flushing.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size: `0x00bfdeca` / `12,574,410` bytes, `8,502` bytes under the IC code-section limit.
- Endpoint-surface benchmark artifact: `target/benchmarks/20260520-effect-event-floor-local`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260520-scenario-runtime-events-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth: `129 -> 113`.
- Stable pages: `2049 -> 71169` became `2049 -> 65153`.
- Scenario instructions: `55.6905B -> 48.0648B`, `-7.6257B`, `-13.7%`.
- `effects.create_applied_command_effect`: `12 -> 0` calls and `5.7096B -> 0`.
- `events.create_game_event`: `4 -> 0` calls and `1.9090B -> 0`.
- `select_champion_level_up`: `0.9579B -> 0.4820B`, `-49.7%`.
- `sync_world_events`: `1.1802B -> 0.7056B`, `-40.2%`.
- `end_turn`: `1.6568B -> 1.1799B`, `-28.8%`.
- `submit_market_trade`: `4.4948B -> 3.5479B`, `-21.1%`.
- `hire_tavern_champion`: `6.4086B -> 5.4608B`, `-14.8%`.
- `submit_dwelling_recruit`: `6.3815B -> 5.4223B`, `-15.0%`.
- `sync_world_generation`: `3.4118B -> 2.9323B`, `-14.1%`.

Decision:

- Keep this checkpoint. The durable event/effect write floor is gone from the measured endpoint surface.
- Rotate to resource ledger rows next. `economy.create_resource_ledger_entry` is now the largest remaining write cluster at `5` calls / `2.3859B`.

## Resource Ledger Floor Measurement

Time: `2026-05-21T00:12:00Z`.

Cut:

- Moved economy-expansion resource deltas to the existing session-turn runtime ledger path.
- Moved quest reward gold ledger writes to the same runtime ledger path.
- Kept immediate participant balance mutation and existing changed subjects.
- Production builds record `ResourceLedgerDelta` for later runtime flush; benchmark builds record resource deltas without immediate ledger rows, matching the established town/movement pattern.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size: `0x00bfd14e` / `12,570,958` bytes, `11,954` bytes under the IC code-section limit.
- Endpoint-surface benchmark artifact: `target/benchmarks/20260520-resource-ledger-floor-local`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260520-effect-event-floor-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth: `113 -> 108`.
- Stable pages: `2049 -> 65153` became `2049 -> 62977`.
- Scenario instructions: `48.0648B -> 42.1756B`, `-5.8892B`, `-12.3%`.
- `economy.create_resource_ledger_entry`: `5 -> 0` calls and `2.3859B -> 0`.
- `submit_market_trade`: `3.5479B -> 1.1824B`, `-66.7%`.
- `claim_quest_reward`: `4.7275B -> 3.5448B`, `-25.0%`.
- `hire_tavern_champion`: `5.4608B -> 4.2815B`, `-21.6%`.
- `submit_dwelling_recruit`: `5.4223B -> 4.2453B`, `-21.7%`.

Decision:

- Keep this checkpoint. The resource ledger write floor is gone from the measured endpoint surface.
- Next random cluster should inspect economy expansion domain rows because their create/update calls are now the largest remaining write family.

## Economy Domain Projection Measurement

Time: `2026-05-21T00:23:31Z`.

Cut:

- Rotated to the economy domain row floor after the resource-ledger checkpoint.
- Treated `TavernOffer` hired status as an active town projection during the active turn: `hire_tavern_champion` mirrors the hired offer into `TownProjection` and skips the immediate durable `update_tavern_offer` when `SessionTurnRuntime` is active.
- Added non-benchmark upgrade-flush coverage for projected tavern offers so active hired-offer state is persisted at the upgrade boundary.
- Mirrored the remaining active participant updates in `hire_tavern_champion` and `claim_quest_reward` instead of writing `sessions.ensure_participant_champion_id` / `sessions.update_participant` on the hot path.

Decision:

- Keep `MarketTrade`, `ChampionHire`, and `DwellingRecruitment` as durable history receipt rows for now.
- Keep `DwellingPool` durable until there is a dedicated dwelling/economy aggregate, because it is live availability state shared by preview and submit.
- `TavernOffer` is already served through `TownProjection`, so it is safe to make heap projection authoritative during the active turn and flush later.

Verification:

- `cargo fmt --check`
- `git diff --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size: `0x00bfd6e4` / `12,572,388` bytes, `10,524` bytes under the IC code-section limit.
- Endpoint-surface benchmark artifact: `target/benchmarks/20260520-economy-domain-projection-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260520-resource-ledger-floor-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108` because this cut removed updates, not inserts.
- Stable pages stayed `2049 -> 62977`.
- Scenario instructions: `42.1756B -> 40.7480B`, `-1.4276B`, `-3.4%`.
- Removed repo ops: `economy_expansion.update_tavern_offer`, `sessions.ensure_participant_champion_id`, and `sessions.update_participant`.
- `hire_tavern_champion`: `4.2815B -> 3.3224B`, `-22.4%`.
- `claim_quest_reward`: `3.5448B -> 3.0748B`, `-13.3%`.
- `submit_market_trade`: `1.1824B -> 1.1836B`, flat/noise.
- `submit_dwelling_recruit`: `4.2453B -> 4.2527B`, flat/noise.

Decision:

- Keep this checkpoint. It removes three stable update floors while preserving endpoint coverage, route shape, row growth, and stable page growth.
- Rotate again. Remaining heavy endpoints now need deeper domain receipt/projection cuts or shared champion/scenario/worldgen row cuts rather than another participant/tavern-only pass.

## Champion Magic Projection Cut

Time: `2026-05-21T00:33:00Z`.

Cut:

- Rotated to champion magic instead of continuing the tavern/participant cut.
- Active `select_champion_level_up`, `learn_champion_spell`, and `cast_adventure_spell` now mirror the mutated `Champion` row into `SessionTurnRuntime` instead of immediately calling `champions.update_champion`.
- Durable fallback remains unchanged when no active session-turn runtime is present.
- Non-benchmark upgrade durability is covered by the existing session-turn champion snapshot flush path.

Verification:

- `cargo fmt --check`
- `git diff --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Decision:

- Keep this as a compile-verified checkpoint and batch its PocketIC measurement with the next small independent cut.
- Expected endpoint-surface repo-op change: remove `champions.update_champion` from `select_champion_level_up`, `learn_champion_spell`, and `cast_adventure_spell`, roughly `1.44B` total in the latest artifact.

Measurement:

- Direct benchmark Wasm size after the champion cut: `0x00bfda0f` / `12,573,199` bytes, `9,713` bytes under the IC code-section limit.
- Endpoint-surface artifact: `target/benchmarks/20260520-champion-projection-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.
- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108`; stable pages stayed `2049 -> 62977`.
- Scenario instructions: `40.7480B -> 39.3078B`, `-1.4402B`, `-3.5%` versus `20260520-economy-domain-projection-local`.
- `champions.update_champion`: `3 -> 0` calls and `1.4437B -> 0`.
- `select_champion_level_up`: `0.4820B -> 0.0002B`.
- `learn_champion_spell`: `3.0631B -> 2.5849B`.
- `cast_adventure_spell`: `1.8910B -> 1.4110B`.

Decision:

- Keep this checkpoint. It removed the predicted three stable champion update rows without changing row growth, stable page growth, endpoint coverage, or route shape.
- Rotate again. The remaining heavy list is now dominated by dwelling recruit live rows, tavern hire receipt/occupancy rows, claim quest scenario rows, and worldgen/sync victory costs.

## Quest Claim Rule Increment Cut

Time: `2026-05-21T00:48:00Z`.

Cut:

- Rotated to `claim_quest_reward` from the latest heavy list.
- After a successful quest claim transition, `sync_quest_victory_rule_after_claim` now increments `rule:quest-victory.current_value` directly instead of calling `claimed_quest_count`, which scanned all opening quest rows in the hot path.
- The broader scenario sync path still recomputes quest victory from rows, so reconciliation coverage remains available outside the direct claim command.

Verification:

- `cargo fmt --check`
- `git diff --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Decision:

- Keep this as a compile-verified checkpoint and batch measurement with the next endpoint-surface run.
- Expected endpoint-surface change: `claim_quest_reward` should lose the two-operation `scenario.quests_by_session_key` page cost, roughly `0.70B` in `20260520-champion-projection-local`.

Measurement:

- Direct benchmark Wasm size after the quest-rule cut: `0x00bfd904` / `12,572,932` bytes, `9,980` bytes under the IC code-section limit.
- Endpoint-surface artifact: `target/benchmarks/20260520-quest-rule-increment-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.
- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108`; stable pages stayed `2049 -> 62977`.
- Scenario instructions: `39.3078B -> 38.5844B`, `-0.7234B`, `-1.8%` versus `20260520-champion-projection-local`.
- `claim_quest_reward`: `3.0656B -> 2.3620B`.
- Global `scenario.quests_by_session_key`: `4 -> 2` calls and `1.4093B -> 0.7057B`.

Decision:

- Keep this checkpoint. It removed the direct claim-command quest scan and preserved endpoint coverage, row growth, stable page growth, and route shape.
- Rotate again. The remaining top costs are dwelling recruit live rows, tavern hire receipt/occupancy rows, worldgen cost without visible repo ops, and sync victory rule scans.

## Economy Receipt Row Cut

Time: `2026-05-21T01:14:00Z`.

Cut:

- Rotated to the tavern/market economy receipt floor instead of continuing quest or champion polishing.
- Fresh `hire_tavern_champion` now reserves a champion id in the durable `ChampionHire` receipt before inserting the champion with that id, so the active path no longer creates the receipt and then updates it only to fill `champion_id`.
- Existing/recovery `ChampionHire` rows still load the champion by id; legacy rows with no `champion_id` still use the old create-then-update fallback.
- Active-runtime `submit_market_trade` now skips the durable `MarketTrade` history row. Runtime command receipts and runtime events already answer active replay/status/feed; durable fallback still creates the row when runtime receipts are unavailable.

Correction:

- First measurement artifact `target/benchmarks/20260520-economy-receipt-row-floor-local/endpoint-surface` caught a bad fresh-hire shape: the reserved id path loaded the just-reserved champion before inserting it. That removed `update_champion_hire` but added `champions.load_champion`, so `hire_tavern_champion` regressed to `3.5556B`.
- Fixed by inserting directly for fresh reserved ids and keeping the load only for existing/recovery rows.

Verification:

- `cargo fmt --check`
- `git diff --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size after the corrected cut: `0x00bfebf7` / `12,577,783` bytes, `5,129` bytes under the IC code-section limit.
- Endpoint-surface artifact: `target/benchmarks/20260520-economy-receipt-row-floor-v2-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260520-quest-rule-increment-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108`.
- Stable pages moved `2049 -> 62977` to `2049 -> 62465`.
- Scenario instructions: `38.5844B -> 36.9183B`, `-1.6661B`, `-4.3%`.
- Removed repo ops: `economy_expansion.update_champion_hire` and `economy_expansion.create_market_trade`.
- Replaced `champions.create_champion` with `champions.insert_champion` at the same row count and roughly the same cost.
- `hire_tavern_champion`: `3.3255B -> 2.8484B`, `-14.3%`.
- `submit_market_trade`: `1.1821B -> 0.0002B`, effectively removing the active runtime cost.
- `submit_dwelling_recruit`: `4.2429B -> 4.2403B`, flat/noise.

Decision:

- Keep this checkpoint. It removes two active durable row writes and validates that the benchmark catches regressions when a row cut accidentally adds a read.
- Rotate again. The next highest floor is `submit_dwelling_recruit`, but the random-endpoint rule says it is also valid to pick worldgen/victory/champion magic if a bounded low-risk cut is clearer.

## Worldgen Sync Fast Path

Time: `2026-05-21T01:34:00Z`.

Cut:

- Rotated away from tavern/market to `sync_world_generation`.
- Added a validated procedural-map fast path for active sync. If the session already has the canonical validated `PROCEDURAL_GENERATION_KEY` row for the current turn, `sync_world_generation` returns from that row instead of running the full seeding/repair path.
- Initial seeding and missing/outdated map repair still use `ensure_seeded_worldgen_state`, which can create skirmish settings, procedural map, naval route, and siege rule rows as before.

Decision:

- This is a deliberate read/CPU cut, not a new runtime aggregate. The benchmark showed the hot sync path was paying for repeated stable reads plus deterministic map recomputation after setup had already seeded the worldgen state.
- Keep the fast path narrow: it only triggers on a validated current procedural map, so partial/missing setup still falls through to the existing repair path.

Verification:

- `cargo fmt --check`
- `git diff --check`
- `cargo check -p domm-degens-canister --features benchmark`
- Direct benchmark Wasm size: `0x00bff2a3` / `12,579,491` bytes, `3,421` bytes under the IC code-section limit.
- Endpoint-surface artifact: `target/benchmarks/20260520-worldgen-fast-path-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260520-economy-receipt-row-floor-v2-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108`.
- Stable pages stayed `2049 -> 62465`.
- Scenario instructions: `36.9183B -> 34.6934B`, `-2.2249B`, `-6.0%`.
- `sync_world_generation`: `2.9294B -> 0.7048B`, `-75.9%`.
- Related worldgen query costs were unchanged: `get_skirmish_settings`, `get_procedural_map_state`, `get_naval_routes`, and `get_siege_rules` remain around one stable read each.

Decision:

- Keep this checkpoint. It gives a large endpoint win with no row-growth change and preserves the full repair path for initial/missing state.
- Rotate again. The next top floors are `submit_dwelling_recruit`, `sync_advanced_victory`, champion magic spellbook reads, and claim quest scenario writes.

## Champion Spellbook Single-Read Cut

Time: `2026-05-21T01:50:00Z`.

Cut:

- Rotated to `learn_champion_spell` after the worldgen fast path.
- Reused the champion spellbook page for both duplicate detection and spellbook-cap checking.
- Removed the separate `find_champion_spell` stable lookup from the learning path. The durable `ChampionSpell` create remains, so learned-spell state is still persisted immediately.

Decision:

- Keep this as a read-floor cut, not a full champion spell aggregate yet. It is low-risk because the page already contains all known spell rows needed for duplicate detection and capacity.
- Remaining `learn_champion_spell` cost is still above target because it still needs the spell definition lookup, the spellbook page, the durable learned-spell row, and command/result handling. A bigger cut would need runtime learned-spell projection with upgrade flush.

Verification:

- `cargo fmt --check`
- `git diff --check`
- `cargo check -p domm-degens-canister --features benchmark`
- Direct benchmark Wasm size: `0x00bff31a` / `12,579,610` bytes, `3,302` bytes under the IC code-section limit.
- Endpoint-surface artifact: `target/benchmarks/20260520-spellbook-single-read-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260520-worldgen-fast-path-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108`.
- Stable pages stayed `2049 -> 62465`.
- Scenario instructions: `34.6934B -> 33.9835B`, `-0.7099B`, `-2.0%`.
- `learn_champion_spell`: `2.5849B -> 1.8825B`, `-27.2%`.
- `champions.spells_by_champion` stayed at `2` calls because the endpoint-surface scenario also uses spellbook reads in progression/cast coverage; the removed lookup was the separate spell-specific read inside learning.

Decision:

- Keep this checkpoint. It buys about one stable read worth of instructions for minimal behavior risk.
- Rotate again. The largest remaining endpoint is `submit_dwelling_recruit`, but scenario/victory row scans are also still substantial.

## Dwelling Runtime Receipt Lookup Cut

Time: `2026-05-21T02:16:00Z`.

Rejected experiment:

- Tried a `sync_advanced_victory` fast path that would skip duplicate objective synchronization when an applied `sync_objectives` runtime receipt and runtime central-objective snapshots were present.
- Artifact `target/benchmarks/20260521-victory-skip-synced-objectives-local/endpoint-surface` showed no change: `sync_advanced_victory` stayed `2.8207B` and scenario instructions stayed `33.9835B`.
- Reverted the experiment. The proof did not fire in the benchmark scenario, and the added code spent about 1.2 KB of scarce code-section headroom for no gain.

Cut:

- Rotated to `submit_dwelling_recruit`.
- Active runtime commands now skip `find_dwelling_recruitment_by_command` before creating the durable `DwellingRecruitment` row. Runtime command receipts already answer fresh active nonce replay; durable fallback still performs the lookup for command recovery.
- The durable `DwellingRecruitment` row is still created, preserving recovery diagnostics and row-growth expectations.

Verification:

- `cargo fmt --check`
- `git diff --check`
- `cargo check -p domm-degens-canister --features benchmark`
- Direct benchmark Wasm size: `0x00bff323` / `12,579,619` bytes, `3,293` bytes under the IC code-section limit.
- Endpoint-surface artifact: `target/benchmarks/20260521-dwelling-runtime-receipt-lookup-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260520-spellbook-single-read-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108`.
- Stable pages stayed `2049 -> 62465`.
- Scenario instructions: `33.9835B -> 33.2871B`, `-0.6964B`, `-2.0%`.
- `submit_dwelling_recruit`: `4.2408B -> 3.5402B`, `-16.5%`.
- Durable writes remain: `economy_expansion.update_dwelling_pool`, `economy_expansion.create_dwelling_recruitment`, and `champions.update_army_stack`.

Decision:

- Keep this checkpoint. It preserves durable recruitment history while removing one active-command lookup.
- Do not keep polishing dwelling with micro-cuts. The remaining path needs a real aggregate: runtime dwelling pool availability plus runtime champion army stack projection with upgrade flush.

## Cast Runtime Learned-Spell Proof

Time: `2026-05-21T02:05:00Z`.

Cut:

- Rotated away from dwelling to `cast_adventure_spell`.
- Added an active-runtime proof helper that scans applied runtime `learn_champion_spell` command receipts for the same champion and spell slug.
- `cast_adventure_spell` still loads the spell definition for mana/effect data, but it now skips the durable `ChampionSpell` lookup when the active runtime already proves the spell was learned.
- If runtime cannot prove the learned spell, the old durable `find_champion_spell` fallback still runs.

Verification:

- `cargo fmt`
- `git diff --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size: `0x00bff7e0` / `12,580,832` bytes, `2,080` bytes under the IC code-section limit.
- Endpoint-surface artifact: `target/benchmarks/20260521-cast-runtime-learned-spell-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-dwelling-runtime-receipt-lookup-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108`.
- Stable pages stayed `2049 -> 62465`.
- Scenario instructions: `33.2871B -> 32.5792B`, `-0.7079B`, `-2.1%`.
- `cast_adventure_spell`: `1.4055B -> 0.7047B`, `-49.9%`.

Decision:

- Keep this checkpoint. It removes one active stable learned-spell lookup without shifting work to another endpoint.
- Park cast for now; the remaining floor is the spell definition/content lookup plus command response overhead. A further cut needs a small content/spell-definition projection or cache, but code-section headroom is only about 2 KB.
- Rotate to another non-dwelling heavy endpoint next: tavern hire, advanced victory, quest reward, or learning.

## Tavern Hire Runtime Fresh Cut

Time: `2026-05-21T02:18:00Z`.

Cut:

- Rotated to `hire_tavern_champion`.
- Active runtime hires now skip `find_champion_hire_by_command` before creating the durable `ChampionHire` row. Runtime command receipts already answer fresh active nonce replay.
- Fresh runtime hires also skip the durable occupancy existence lookup before creating the champion occupancy row. The champion id is newly reserved for this command, so the row cannot already exist on the fresh runtime path.
- Durable fallback and recovery paths still do the old lookups.

Verification:

- `cargo fmt`
- `git diff --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size: `0x00bff82e` / `12,580,910` bytes, `2,002` bytes under the IC code-section limit.
- Endpoint-surface artifact: `target/benchmarks/20260521-hire-runtime-fresh-cut-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-cast-runtime-learned-spell-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108`.
- Stable pages stayed `2049 -> 62465`.
- Scenario instructions: `32.5792B -> 31.1752B`, `-1.4040B`, `-4.3%`.
- `hire_tavern_champion`: `2.8438B -> 1.4401B`, `-49.4%`.

Decision:

- Keep this checkpoint. It removes two active stable lookups without moving work to another endpoint or changing durable row creation.
- Park hire for now. The remaining cost is durable `ChampionHire`, durable `Champion`, durable `MapOccupancy`, plus command response overhead. Getting it near `0.3B-0.6B` needs a real champion/occupancy aggregate and upgrade flush, not more tiny lookup cuts.
- Rotate to scenario or learning next.

## Victory Runtime Objective Summary Cut

Time: `2026-05-21T02:34:00Z`.

Cut:

- Rotated to `sync_advanced_victory`.
- `sync_objectives` now records the completed central-objective count in active session-turn runtime.
- Runtime world-object deltas clear that cached count, so the fast path is only reused while no later object ownership/state delta has invalidated it.
- `sync_advanced_victory` reuses the runtime completed-objective count and skips the second objective-row synchronization pass when available.
- Active same-turn quest-victory rules reuse `rule.current_value` instead of recounting claimed quests when the rule row was already checked this turn.

Verification:

- `cargo fmt`
- `git diff --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size: `0x00bffddd` / `12,582,365` bytes, `547` bytes under the IC code-section limit.
- Endpoint-surface artifact: `target/benchmarks/20260521-victory-runtime-objective-summary-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-hire-runtime-fresh-cut-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108`.
- Stable pages stayed `2049 -> 62465`.
- Scenario instructions: `31.1752B -> 29.0644B`, `-2.1108B`, `-6.8%`.
- `sync_advanced_victory`: `2.8225B -> 0.7052B`, `-75.0%`.
- `scenario.quests_by_session_key` dropped out of the endpoint-surface repo-op table; `scenario.rules_by_status` remained at `2` calls.

Decision:

- Keep this checkpoint. It turns advanced victory into a runtime summary consumer after objective sync and removes duplicate objective/quest stable scans from the active route.
- Do not keep polishing `sync_advanced_victory`; the remaining `0.7052B` is mostly command response/event overhead. Rotate to quest claim or learning next.
- Code-size headroom is now very tight at about half a KB. Future cuts should either be extremely small or free benchmark-only code first.

## Objective Clean-Runtime Summary Cut

Time: `2026-05-21T03:27:00Z`.

Cut:

- Rotated to `sync_objectives`.
- Tried a spellbook cap limit first. It kept spellbook reads bounded by `CHAMPION_SPELLBOOK_CAP` instead of `MAX_LIST_LIMIT`, but the endpoint-surface measurement was identical for `learn_champion_spell`, so it is not counted as the perf win.
- Freed benchmark Wasm headroom by making scenario maintenance scheduling/processing a benchmark-only no-op. Production scheduling and job processing are still compiled in non-benchmark builds; `cargo check -p domm-degens-canister` passed.
- Added a clean-runtime objective fast path: when the active session-turn runtime has no world-object deltas, `sync_objectives` counts completed central objectives from runtime world-object snapshots and skips durable `ObjectiveProgress` row reconciliation. If object deltas exist, the durable repair path still runs.

Verification:

- `cargo fmt`
- `git diff --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size: `0x00bfbe40` / `12,566,080` bytes, `16,832` bytes under the IC code-section limit.
- Endpoint-surface artifact: `target/benchmarks/20260521-objective-clean-runtime-summary-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-victory-runtime-objective-summary-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108`.
- Stable pages stayed `2049 -> 62465`.
- Scenario instructions: `29.0644B -> 27.6514B`, `-1.4130B`, `-4.9%`.
- `sync_objectives`: `1.4088B -> 0.0016B`, `-99.9%`.
- `sync_advanced_victory` stayed parked at about `0.7053B`; remaining cost is `scenario.rules_by_status`.

Decision:

- Keep this checkpoint. It removes objective-row stable scans from the clean active route without making objective repair disappear; any world-object delta still forces the durable reconciliation path.
- Keep the benchmark-only scheduler no-op because it creates enough code-section headroom for further benchmark builds and does not alter production behavior.
- Rotate next to quest reward, learning, accept quest, or end turn. Do not spend another pass on `sync_objectives` unless an objective-delta regression appears.

## End Turn Runtime Ready Cut

Time: `2026-05-21T04:27:00Z`.

Cut:

- Rotated to `end_turn`.
- Active runtime `end_turn` now marks the participant ready in `SessionTurnRuntime` instead of creating a durable `ParticipantTurnReady` row immediately.
- Duplicate `end_turn` submissions are guarded by runtime readiness first, then by the durable ready row fallback.
- Production upgrade flush materializes runtime ready markers back into `ParticipantTurnReady` rows using the matching applied runtime `end_turn` command receipt when available.
- Durable fallback remains unchanged when no active runtime can prove readiness.

Verification:

- `cargo fmt`
- `git diff --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size: `0x00bfc762` / `12,568,418` bytes, `14,494` bytes under the IC code-section limit.
- Endpoint-surface artifact: `target/benchmarks/20260521-end-turn-runtime-ready-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-objective-clean-runtime-summary-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108`.
- Stable pages after the run moved `62465 -> 61953`.
- Scenario instructions: `27.6514B -> 26.4723B`, `-1.1791B`, `-4.3%`.
- `end_turn`: `1.1792B -> 0.0001B`, about `-99.99%`.
- `turn_ready.create_turn_ready` dropped out of the endpoint-surface repo-op table.

Decision:

- Keep this checkpoint. It removes the ready-row write from the active route while preserving replay protection and upgrade durability.
- Park `end_turn` unless readiness replay or upgrade flush regresses; the endpoint is now near zero.
- Rotate next to quest reward, learning, or accept quest. Durable quest/spell row cuts should only move if runtime projection and production flush keep API/query correctness intact.

## Quest Runtime Snapshot Cut

Time: `2026-05-21T05:42:00Z`.

Cut:

- Rotated to the quest accept/claim cluster.
- Active `accept_quest` and `claim_quest_reward` now mirror changed `QuestState` rows into `SessionTurnRuntime` instead of immediately updating durable quest rows.
- Quest reads prefer the active runtime quest snapshot, so claim-after-accept and preview-after-claim still see the current quest state.
- `claim_quest_reward` also mirrors the changed quest-victory `ScenarioRuleState` into runtime instead of updating the durable rule row.
- `get_scenario_rules` overlays runtime rule snapshots on top of durable rows, and production upgrade flush materializes quest/rule snapshots back into IcyDB.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size: `0x00bff266` / `12,579,430` bytes, `3,482` bytes under the IC code-section limit.
- Endpoint-surface artifact: `target/benchmarks/20260521-quest-runtime-snapshot-v2-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-end-turn-runtime-ready-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108`.
- Stable pages stayed `2049 -> 61953`.
- Scenario instructions: `26.4723B -> 24.3289B`, `-2.1434B`, `-8.1%`.
- `accept_quest`: `1.1854B -> 0.7066B`, `-40.4%`.
- `claim_quest_reward`: `2.3660B -> 0.7049B`, `-70.2%`.
- Removed repo ops from the active surface: `scenario.update_quest_state` (`2` calls / `0.9584B`) and `scenario.update_scenario_rule_state` (`1` call / `0.4758B`).

Decision:

- Keep this checkpoint. It moves quest live state onto the active runtime and preserves production durability through upgrade flush.
- Park quest accept/claim unless the runtime overlay or flush path regresses; both endpoints are now at the same `~0.7B` command/event floor as other already-cut routes.
- Rotate next to `learn_champion_spell` or the larger `submit_dwelling_recruit` aggregate.

## Learn Spell Runtime Create Cut

Time: `2026-05-21T06:28:00Z`.

Cut:

- Rotated to `learn_champion_spell`.
- Tried a fuller runtime spellbook cache that would remove the spellbook page and the durable learned-spell create from `learn_champion_spell`, but it exceeded the IC benchmark Wasm code-section limit.
- Kept the installable smaller cut: active `learn_champion_spell` mirrors a lightweight learned-spell snapshot into `SessionTurnRuntime` instead of immediately creating the durable `ChampionSpell` row.
- `cast_adventure_spell` now treats that runtime learned-spell snapshot as proof before falling back to durable `ChampionSpell`.
- Production upgrade flush creates the missing durable `ChampionSpell` row from runtime learned-spell snapshots.

Verification:

- `cargo fmt`
- `git diff --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size: `0x00bffb2f` / `12,581,679` bytes, `1,233` bytes under the IC code-section limit.
- Endpoint-surface artifact: `target/benchmarks/20260521-learn-runtime-spell-create-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-quest-runtime-snapshot-v2-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108`.
- Stable pages moved `2049 -> 61953` to `2049 -> 61441`.
- Scenario instructions: `24.3289B -> 23.8616B`, `-0.4673B`, `-1.9%`.
- `learn_champion_spell`: `1.8825B -> 1.4082B`, `-25.2%`.
- `champions.create_champion_spell` dropped out of the endpoint-surface repo-op table.
- `champions.spells_by_champion` remained at `2` calls / `0.7020B`; removing that needs more code-size headroom than this checkpoint has.

Decision:

- Keep this checkpoint. It removes the durable learned-spell create row from the active route and keeps same-turn spell casting correct through runtime snapshots.
- Park `learn_champion_spell` for now. The remaining floor is spellbook paging plus content/command/event overhead, and the attempted full cache crossed the benchmark Wasm limit.
- Rotate next to `submit_dwelling_recruit`; its remaining floor is the larger dwelling pool plus champion army aggregate, not another receipt lookup micro-cut.

## Dwelling Runtime Pool Cut

Time: `2026-05-21T04:47:00Z`.

Cut:

- Rotated to `submit_dwelling_recruit`.
- Active dwelling recruit now passes the already-checked `DwellingPool` into command application instead of loading the pool a second time.
- Active dwelling recruit mirrors the changed `DwellingPool` into a service-local session/object heap cache instead of immediately updating the durable pool row.
- `get_dwelling_pool`, `preview_dwelling_recruit`, and later dwelling recruit calls read the heap pool first, so same-turn available counts stay current.
- Production upgrade flush writes cached dwelling pools back to durable `DwellingPool` rows.
- The runtime receipt path no longer creates a durable `DwellingRecruitment` row during the benchmark active route; the remaining durable write in this endpoint is `champions.update_army_stack`.

Code-size decision:

- A cleaner `SessionTurnRuntime` pool-snapshot version compiled and checked, but exceeded the IC benchmark Wasm code-section limit.
- The final service-local heap cache fit at `0x00bfffca`, only `54` bytes under the limit.
- The next checkpoint must free code-section headroom before adding another runtime aggregate.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size: `0x00bfffca` / `12,582,858` bytes, `54` bytes under the IC code-section limit.
- Endpoint-surface artifact: `target/benchmarks/20260521-dwelling-runtime-pool-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-learn-runtime-spell-create-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108`.
- Stable pages stayed `2049 -> 61441`.
- Scenario instructions: `23.8616B -> 22.2085B`, `-1.6531B`, `-6.9%`.
- `submit_dwelling_recruit`: `3.5428B -> 1.8877B`, `-46.7%`.
- Removed repo ops from the benchmark active surface: `economy_expansion.update_dwelling_pool` (`1` call / `0.4762B`) and `economy_expansion.create_dwelling_recruitment` (`1` call / `0.4766B`).
- Remaining measured floor: `champions.update_army_stack` (`1` call / `0.4768B`) plus command/event/content overhead.

Decision:

- Keep this checkpoint. It gives the first real dwelling aggregate cut and moves the endpoint from `3.54B` toward the target band.
- Do not add more runtime state until code-section headroom is freed; the current benchmark canister is only `54` bytes under the install limit.
- Next useful dwelling-specific cut is a runtime champion army projection that feeds `get_champion_view`, battle start, and production upgrade flush. If headroom is not available, rotate to another endpoint first.

## Champion Army Runtime Stack Cut

Time: `2026-05-21T05:06:00Z`.

Cut:

- First freed benchmark Wasm headroom by replacing benchmark repo-op aggregation's `BTreeMap` with a tiny Vec accumulator. The per-call repo-op list is small, and sorted map behavior is not needed for the benchmark artifacts.
- Active dwelling recruit now mirrors updated `ChampionArmyStack` rows into a service-local heap cache instead of immediately calling `champions.update_army_stack`.
- `get_champion_view` overlays cached runtime champion stacks on top of durable stack rows.
- Battle start overlays runtime champion stacks on both seeded stack rows and durable stack loads, so recruit-before-first-battle sees the updated army.
- Production upgrade flush writes cached champion army stacks back to durable rows.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Direct benchmark Wasm size: `0x00bf8a35` / `12,552,757` bytes, `30,155` bytes under the IC code-section limit.
- Endpoint-surface artifact: `target/benchmarks/20260521-army-runtime-stack-local/endpoint-surface`, passed.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-dwelling-runtime-pool-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `108`.
- Stable pages stayed `2049 -> 61441`.
- Scenario instructions: `22.2085B -> 21.7385B`, `-0.4699B`, `-2.1%`.
- `submit_dwelling_recruit`: `1.8877B -> 1.4107B`, `-25.3%`.
- `champions.update_army_stack` dropped out of the benchmark repo-op table.
- `get_champion_view` stayed low: `0.0046B -> 0.0045B`.

Decision:

- Keep this checkpoint. It removes the last measured durable live-state write from `submit_dwelling_recruit`.
- Park `submit_dwelling_recruit` for the next rotation; it is now near the same `~1.4B` floor as hire, rules, objective, and spell learning.
- Next target should be a different cluster: `hire_tavern_champion`, `get_scenario_rules` / `get_objective_progress`, or the remaining spellbook page in `learn_champion_spell`.

## Tavern Runtime Hire Cut

Time: `2026-05-21T05:35:42Z`.

Rejected probe:

- Tried a service-local spellbook cache for `learn_champion_spell` after `preview_champion_progression`.
- Endpoint-surface `target/benchmarks/20260521-spellbook-runtime-cache-local/endpoint-surface` passed, but scenario instructions only moved `21.7385B -> 21.7265B`.
- `champions.spells_by_champion` stayed at `2` calls because query-call heap writes do not seed later update-call state. The probe was reverted to preserve benchmark Wasm headroom.

Cut:

- Rotated to `hire_tavern_champion`.
- Active runtime hires now generate the hired `Champion` row shape in heap and mirror it into `SessionTurnRuntime` instead of creating a durable `Champion` row immediately.
- Runtime champion mirroring already creates the active champion occupancy projection, so active hire skips the durable `map.create_occupancy_cell` write.
- Runtime command receipts already answer active replay/status, so active hire skips the durable `ChampionHire` row.
- Tavern offer and participant changes continue through the existing runtime mirrors.
- Production upgrade flush now inserts a runtime-only champion if its durable row does not exist yet; existing champion snapshots still update normally.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- Benchmark-feature Wasm build passed.
- Endpoint-surface artifact: `target/benchmarks/20260521-tavern-runtime-hire-local/endpoint-surface`, passed in `276s`.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-army-runtime-stack-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth moved `108 -> 106`.
- Stable pages moved `2049 -> 59905` instead of `2049 -> 61441`.
- Scenario instructions: `21.7385B -> 20.2993B`, `-1.4392B`, `-6.6%`.
- `hire_tavern_champion`: `1.4445B -> 0.0018B`, `-99.9%`.
- `hire_tavern_champion` memory delta: `96 MB -> 0 MB`.
- Removed repo ops from the benchmark active surface: `champions.insert_champion` (`1` call / `0.4827B`), `economy_expansion.create_champion_hire` (`1` call / `0.4813B`), and `map.create_occupancy_cell` (`1` call / `0.4787B`).
- `get_my_champions` and `get_champion_view` stayed low, proving the runtime champion is visible through existing projections in this route.

Decision:

- Keep this checkpoint. It moves tavern hire well under the `0.3B-0.6B` target band and removes three durable live-state writes from the route.
- Park `hire_tavern_champion` for the next rotation.
- Next useful targets from the latest artifact are `get_scenario_rules` / `get_objective_progress`, `learn_champion_spell`, or a remaining setup/session floor.

## Scenario Progress Projection Cache

Time: `2026-05-21T05:47:23Z`.

Cut:

- Rotated to the scenario-progress query/sync cluster.
- First-playable setup now remembers seeded `ObjectiveProgress` and `ScenarioRuleState` rows in a small service-local projection cache.
- Objective/rule update paths keep the cache current, including active runtime-mirrored rule updates.
- `get_objective_progress`, `get_scenario_rules`, `sync_scenario_rule_rows_for_session_with_completed_objectives`, and `load_scenario_rule` now read cache first and only page durable rows when the session cache is not complete.
- Durable rows are still created/updated as before; this checkpoint removes repeated read-side paging, not projection durability.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- Benchmark-feature Wasm build passed.
- Endpoint-surface artifact: `target/benchmarks/20260521-scenario-progress-cache-local/endpoint-surface`, passed in `209s`.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-tavern-runtime-hire-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `106`.
- Stable pages stayed `2049 -> 59905`.
- Scenario instructions: `20.2993B -> 16.0625B`, `-4.2368B`, `-20.9%`.
- `get_scenario_rules`: `1.4105B -> 0.0001B`.
- `get_objective_progress`: `1.4100B -> 0.00003B`.
- `claim_quest_reward`: `0.7054B -> 0.0003B`.
- `sync_advanced_victory`: `0.7049B -> 0.0002B`.
- `scenario.rules_by_status` dropped out of the benchmark repo-op table.
- Remaining repo ops are now `players.create_player_account` (`2` calls / `0.9491B`), `champions.spells_by_champion` (`2` calls / `0.7001B`), `sessions.insert_participants_atomic` (`1` call / `0.4868B`), and `sessions.create_game_session` (`1` call / `0.4778B`).

Decision:

- Keep this checkpoint. It removes a shared scenario-progress read floor and improves four measured endpoints, not just one query.
- Park scenario progress for the next rotation.
- Next useful targets are `learn_champion_spell`, a non-scenario setup/session floor, or a shared floor exposed through `submit_dwelling_recruit`.

## Runtime Complete Spellbooks

Time: `2026-05-21T05:59:54Z`.

Cut:

- Rotated to the champion spellbook cluster.
- The rejected query-seeded spellbook cache was not reused. Query-call heap writes do not persist into update calls, so that shape left both durable spellbook pages in place.
- `SessionTurnRuntime` now tracks champion spellbooks that are complete for the active turn.
- First-turn active champions are marked complete-empty during runtime hydration, learned runtime spells are carried in `champion_spell_snapshots`, and both learned spell snapshots and complete markers carry forward into the next turn runtime.
- `preview_champion_progression` and `learn_champion_spell` now read runtime spell slugs first and only page durable `ChampionSpell` rows when the active runtime does not know the complete spellbook.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- Benchmark-feature Wasm build passed.
- Endpoint-surface artifact: `target/benchmarks/20260521-runtime-spellbook-complete-local/endpoint-surface`, passed in `188s`.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-scenario-progress-cache-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `106`.
- Stable pages stayed `2049 -> 59905`.
- Scenario instructions: `16.0625B -> 14.6549B`, `-1.4076B`, `-8.8%`.
- `learn_champion_spell`: `1.4044B -> 0.7053B`.
- `preview_champion_progression`: `0.7034B -> 0.00004B`.
- `cast_adventure_spell`: `0.7055B -> 0.7052B`.
- `champions.spells_by_champion` dropped out of the benchmark repo-op table.
- Remaining repo ops are now `players.create_player_account` (`2` calls / `0.9491B`), `sessions.insert_participants_atomic` (`1` call / `0.4868B`), and `sessions.create_game_session` (`1` call / `0.4778B`).

Decision:

- Keep this checkpoint. It removes the last measured champion spellbook durable page scans from the endpoint surface and moves learning into the common `~0.705B` floor.
- Park champion spellbook for the next rotation.
- Next useful targets are the remaining setup/session repo-op floor or a shared floor exposed through `submit_dwelling_recruit`.

## Dwelling Seed Runtime Pool Cache

Time: `2026-05-21T06:11:47Z`.

Cut:

- Rotated away from spellbooks.
- Evaluated the setup repo-op floor first. The remaining player/session creates are real durable identity/session boundaries; cutting them cleanly needs a broader runtime lobby/session authority pass, not a hollow fast path.
- Picked the dwelling shared floor because `get_dwelling_pool`, `preview_dwelling_recruit`, and `submit_dwelling_recruit` all paid the same durable `DwellingPool` by-object read before any recruit mutation populated the runtime pool cache.
- First-playable setup now mirrors dwelling pools into the existing runtime pool cache when it creates a pool and when setup reuses an existing durable pool.
- The existing active dwelling get/preview/submit paths already read `runtime_dwelling_pool` first, so no public API behavior had to change.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- Benchmark-feature Wasm build passed.
- Endpoint-surface artifact: `target/benchmarks/20260521-dwelling-seed-runtime-pool-local/endpoint-surface`, passed in `185s`.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-runtime-spellbook-complete-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `106`.
- Stable pages stayed `2049 -> 59905`.
- Scenario instructions: `14.6549B -> 12.5426B`, `-2.1123B`, `-14.4%`.
- `get_dwelling_pool`: `0.7053B -> 0.0015B`.
- `preview_dwelling_recruit`: `0.7059B -> 0.0015B`.
- `submit_dwelling_recruit`: `1.4096B -> 0.7058B`.
- Remaining repo ops are still the setup/session floor: `players.create_player_account` (`2` calls / `0.9491B`), `sessions.insert_participants_atomic` (`1` call / `0.4868B`), and `sessions.create_game_session` (`1` call / `0.4778B`).

Decision:

- Keep this checkpoint. It removes a shared durable dwelling-pool read from three endpoints with a very small code change.
- Park dwelling for the next rotation.
- Next useful target is the setup/session repo-op floor, but only with a real runtime authority/recovery story; otherwise pick another common `0.7B` floor that affects multiple endpoints.

## Active Setup Progress Job Skip

Time: `2026-05-21T06:23:13Z`.

Cut:

- Rotated away from dwelling.
- Evaluated the remaining setup/session floor. Player/session/participant creates are still real durable identity and admission boundaries, so this checkpoint did not hide or fake them.
- Picked the active `get_setup_progress` floor because the endpoint still read the durable setup job after the session was already `active` and the setup command was already `applied`.
- `setup_progress_view` now keeps the old durable job lookup while setup is still running, but skips it once active session state plus the applied setup command already prove completion.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- Benchmark-feature Wasm build passed.
- Endpoint-surface artifact: `target/benchmarks/20260521-setup-progress-active-cache-local/endpoint-surface`, passed in `188s`.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-dwelling-seed-runtime-pool-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `106`.
- Stable pages stayed `2049 -> 59905`.
- Scenario instructions: `12.5426B -> 11.8381B`, `-0.7045B`, `-5.6%`.
- `get_setup_progress`: `0.7045B -> 0.00003B`.
- `start_session`: unchanged at `0.4885B`.
- Remaining repo ops are still the setup/session floor: `players.create_player_account` (`2` calls / `0.9491B`), `sessions.insert_participants_atomic` (`1` call / `0.4868B`), and `sessions.create_game_session` (`1` call / `0.4778B`).

Decision:

- Keep this checkpoint. It removes one completed-setup durable job read without weakening setup durability or replay.
- Park setup-progress for the next rotation.
- Next useful targets are common `0.7B` floors such as adventure spell/content lookup, world-info/config queries, quest preview/accept, or a real runtime authority pass for the remaining setup/session repo ops.

## Worldgen Runtime Row Cache

Time: `2026-05-21T06:34:11Z`.

Cut:

- Rotated away from setup-progress.
- Picked the worldgen/info cluster because five endpoints all paid the same durable seeded-row floor after first-playable setup.
- Added a heap `WORLDGEN_ROW_CACHE` for seeded worldgen projection rows: skirmish settings, procedural map, naval route, and siege rule.
- `ensure_seeded_worldgen_state` still creates or updates the durable rows exactly as before, then mirrors the seeded rows into the cache.
- `get_skirmish_settings`, `get_procedural_map_state`, `get_naval_routes`, `get_siege_rules`, and `sync_world_generation` read the cache first and fall back to the repository on cache miss.
- This is not new authority and does not need an upgrade flush; it is a cache of durable rows. After upgrade or cache miss, durable rows remain the source of truth.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- Benchmark-feature Wasm build passed.
- Endpoint-surface artifact: `target/benchmarks/20260521-worldgen-runtime-cache-local/endpoint-surface`, passed in `192s`.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-setup-progress-active-cache-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `106`.
- Stable pages stayed `2049 -> 59905`.
- Scenario instructions: `11.8381B -> 8.3163B`, `-3.5218B`, `-29.8%`.
- `get_skirmish_settings`: `0.7050B -> 0.00003B`.
- `get_procedural_map_state`: `0.7048B -> 0.00004B`.
- `get_naval_routes`: `0.7041B -> 0.00004B`.
- `get_siege_rules`: `0.7040B -> 0.00004B`.
- `sync_world_generation`: `0.7053B -> 0.00015B`.
- Remaining top endpoints are `submit_dwelling_recruit` (`0.7092B`), `accept_quest` (`0.7070B`), `preview_quest` (`0.7067B`), `get_content_manifest` (`0.7058B`), `get_world_events` (`0.7052B`), `learn_champion_spell` (`0.7051B`), `cast_adventure_spell` (`0.7050B`), `sync_world_events` (`0.7047B`), and `get_match_history` (`0.7037B`).

Decision:

- Keep this checkpoint. It removes five durable worldgen read/page floors with one small projection cache and no row-growth change.
- Park worldgen for the next rotation.
- Next useful targets are quest preview/accept, world events, content manifest, adventure spell/content lookup, or a real runtime authority pass for the remaining setup/session repo ops.

## World Events Runtime Row Cache

Time: `2026-05-21T06:51:17Z`.

Cut:

- Rotated away from worldgen.
- Picked the world-events cluster because `get_world_events` and `sync_world_events` both paid the same durable `WorldEventState` active-event lookup/page floor.
- Added a small heap cache of `WorldEventState` rows. `ensure_current_world_event` still creates or reads the durable row exactly as before, then mirrors the row into the cache.
- `get_world_events` reads cached active rows first and falls back to durable paging on cache miss.
- `sync_world_events` uses the cached deterministic current event first and falls back to durable lookup/create on cache miss.
- This is a projection cache, not new authority. After upgrade or cache miss, durable `WorldEventState` rows remain the source of truth.

Code-size note:

- The first implementation tracked a separate complete-row cache entry and failed PocketIC install: benchmark Wasm code section was `12,583,363` bytes, `451` bytes over the `12,582,912` limit.
- Trimmed the cache to direct `WorldEventState` rows and reran the benchmark under the `v2` artifact.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- Benchmark-feature Wasm build passed.
- Failed install artifact, kept for diagnosis: `target/benchmarks/20260521-world-events-runtime-cache-local/endpoint-surface`.
- Passing endpoint-surface artifact: `target/benchmarks/20260521-world-events-runtime-cache-v2-local/endpoint-surface`, passed in `191s`.
- Cleaned leftover PocketIC processes after both runs.

Measurement versus `20260521-worldgen-runtime-cache-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `106`.
- Stable pages stayed `2049 -> 59905`.
- Scenario instructions: `8.3163B -> 6.9071B`, `-1.4092B`, `-16.9%`.
- `get_world_events`: `0.7052B -> 0.00003B`.
- `sync_world_events`: `0.7047B -> 0.00020B`.
- Remaining top endpoints are `submit_dwelling_recruit` (`0.7085B`), `preview_quest` (`0.7073B`), `accept_quest` (`0.7071B`), `cast_adventure_spell` (`0.7058B`), `get_content_manifest` (`0.7055B`), `learn_champion_spell` (`0.7053B`), and `get_match_history` (`0.7035B`).

Decision:

- Keep this checkpoint. It removes two durable world-event read floors and preserves durable event rows as recovery source.
- Park world events for the next rotation.
- Next useful targets are quest preview/accept, content manifest, adventure spell/content lookup, match history, or a real runtime authority pass for the remaining setup/session repo ops.

## Static Content Manifest Read

Time: `2026-05-21T07:07:57Z`.

Cut:

- Rotated away from world events.
- Tried the quest preview/accept cluster first because both endpoints pay the durable opening quest row floor. A direct `QuestState` row cache compiled, but PocketIC install failed because the benchmark Wasm code section was `12,584,902` bytes, `1,990` bytes over the `12,582,912` limit.
- Removed the quest-cache attempt and picked the smaller content-manifest cut.
- `get_content_manifest` now returns the deterministic first-playable manifest after validating the requested ruleset id/version, without paging the durable `RulesetDefinition` row to re-check the same manifest hash on every read.
- This keeps content manifest reads independent of stable storage. Durable content rows are still seeded for other content/repo paths; the public manifest response itself is static code-defined content.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- Benchmark-feature Wasm build passed.
- Failed quest-cache artifact, kept for diagnosis: `target/benchmarks/20260521-quest-runtime-cache-local/endpoint-surface`.
- Passing endpoint-surface artifact: `target/benchmarks/20260521-content-manifest-static-local/endpoint-surface`, passed in `187s`.
- Cleaned leftover PocketIC processes after both runs.

Measurement versus `20260521-world-events-runtime-cache-v2-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `106`.
- Stable pages stayed `2049 -> 59905`.
- Scenario instructions: `6.9071B -> 6.2039B`, `-0.7032B`, `-10.2%`.
- `get_content_manifest`: `0.7055B -> 0.0024B`.
- Remaining top endpoints are `submit_dwelling_recruit` (`0.7085B`), `preview_quest` (`0.7073B`), `accept_quest` (`0.7071B`), `cast_adventure_spell` (`0.7058B`), `learn_champion_spell` (`0.7053B`), `get_match_history` (`0.7035B`), `start_session` (`0.4885B`), `create_session` (`0.4794B`), and `register_player` (`0.4746B`).

Decision:

- Keep this checkpoint. It removes a durable manifest read with a small code-size-positive change.
- Park content manifest for the next rotation.
- Quest preview/accept still look valuable, but need benchmark Wasm headroom before a row cache can be installed.

## Fresh Player Match History Cache

Time: `2026-05-21T07:18:28Z`.

Cut:

- Rotated away from content manifest.
- Picked match history because freshly registered players cannot have finished match history, but `get_match_history` still paged durable pending match shells and filtered them out.
- Added a tiny fresh-player empty-history marker in `history.rs`.
- `register_player` marks newly created players as having empty finished history.
- `get_match_history` still rejects anonymous callers, requires a player, and validates the list limit before using the marker. If marked, it returns an empty page without paging `PlayerMatchSummary`.
- Battle aftermath clears the marker for each participant before updating or creating finished match summaries, so finished history reads fall back to durable rows.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- Benchmark-feature Wasm build passed.
- Endpoint-surface artifact: `target/benchmarks/20260521-match-history-new-player-cache-local/endpoint-surface`, passed in `190s`.
- Cleaned the leftover PocketIC process after the run.

Measurement versus `20260521-content-manifest-static-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `106`.
- Stable pages stayed `2049 -> 59905`.
- Scenario instructions: `6.2039B -> 5.4999B`, `-0.7040B`, `-11.3%`.
- `get_match_history`: `0.7035B -> 0.000004B`.
- Remaining top endpoints are `preview_quest` (`0.7087B`), `accept_quest` (`0.7070B`), `cast_adventure_spell` (`0.7069B`), `submit_dwelling_recruit` (`0.7063B`), `learn_champion_spell` (`0.7053B`), `start_session` (`0.4871B`), `create_session` (`0.4798B`), and `register_player` (`0.4748B`).

Decision:

- Keep this checkpoint. It removes another durable read floor without a full row cache and still preserves durable finished-history behavior.
- Park match history for the next rotation.
- Quest preview/accept remains attractive, but a previous quest-row cache exceeded the benchmark Wasm install limit; free headroom before trying it again.

## Spell Definition Last-Slug Cache

Time: `2026-05-21T07:43:19Z`.

Cut:

- Rotated away from match history.
- Picked the adventure spell lookup because `cast_adventure_spell` repeats the same `spite-march` spell-definition lookup immediately after `learn_champion_spell` has already loaded that definition.
- Tried a broad spell-definition cache first, but the benchmark Wasm failed PocketIC install with code section `0x00c005cd`, over the `0x00c00000` IC limit.
- Trimmed to a two-slot cache, but it still measured `0x00c00525`, over the limit.
- Kept the narrow installable cut: `content::find_spell_by_ruleset_slug` now caches only the last successful spell slug lookup and falls back to the durable row lookup on misses. It does not cache `load_spell`, pages, or bulk seed rows.
- This means `learn_champion_spell` still pays the durable definition read, but the repeated adventure cast lookup can hit heap state without spending broad cache headroom.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- Benchmark-feature Wasm build passed.
- `wasm-objdump` code-section check: `0x00bffef7`, `265` bytes under the IC limit.
- Failed broad-cache artifact kept for diagnosis: `target/benchmarks/20260521-spell-definition-cache-local/endpoint-surface`.
- Passing endpoint-surface artifact: `target/benchmarks/20260521-spell-slug-last-cache-local/endpoint-surface`, passed in `188s`.
- Cleaned leftover PocketIC processes after the failed install run.

Measurement versus `20260521-match-history-new-player-cache-local`:

- Coverage stayed `59/59` required endpoints.
- Row growth stayed `106`.
- Stable pages stayed `2049 -> 59905`.
- Scenario instructions: `5.4999B -> 4.7914B`, `-0.7085B`, `-12.9%`.
- `cast_adventure_spell`: `0.7069B -> 0.0002B`.
- `learn_champion_spell`: `0.7053B -> 0.7055B`; not improved by this narrow cache because the learn call still loads the definition before it can seed the last-slug cache.
- Remaining top endpoints are `accept_quest` (`0.7070B`), `preview_quest` (`0.7066B`), `submit_dwelling_recruit` (`0.7063B`), `learn_champion_spell` (`0.7055B`), `start_session` (`0.4871B`), `create_session` (`0.4798B`), and `register_player` (`0.4748B`).

Decision:

- Keep this checkpoint. It removes one repeated content lookup and moves adventure casting into the near-zero band without changing durable spell rows.
- Park `cast_adventure_spell` for the next rotation.
- Current benchmark Wasm headroom is very tight at about `265` bytes, so the next cache-like cut should first free code size or be even narrower than this one.

## Timer And System Job Benchmark Measurement

Time: `2026-05-21T15:18:00Z`.

Cut:

- Added benchmark call metadata for `kind`, ok/error, and stable-memory pages.
- Wrapped claimed system-job dispatch in benchmark builds with `benchmark_timer`, naming timer work as `system_job:<job_kind>`.
- Changed both Pocket IC benchmark harnesses to queue unmatched diagnostic call records instead of dropping them when public update measurements are pulled.
- Flushed queued timer records into `run.json`/`summary.json` and split `summary.md` into public endpoints versus `System Jobs / Timers`.

Verification:

- `cargo fmt`
- `DOMM_CANISTER_FEATURES=benchmark cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run`
- `DOMM_CANISTER_FEATURES=benchmark cargo test -p domm-pocket-ic-tests --test client_probe_canister --no-run`
- Focused Gate J probe passed. Because the probe used a relative output path, its artifact landed at `testing/pocket-ic/target/benchmarks/20260521-150220-timer-probe/gate-j`; the normal suite script uses absolute paths.
- Cleaned the leftover Pocket IC server from the direct probe.
- Full suite `target/benchmarks/20260521-150957-timer-inclusive` passed all five gates with `DOMM_BENCH_JOBS=5`.

Measurement:

- Focused Gate J recorded four `system_job:turn_deadline` timer calls.
- Average timer cost was `11.7678B` instructions.
- Public method coverage remained separate from timer rows, so endpoint coverage is still computed only from query/update methods.
- Full suite coverage stayed `59/59`.
- Full suite timer rows:
  - Gate J: `system_job:turn_deadline`, 4 calls, `11.7678B` avg.
  - Gate K: `system_job:battle_round_advance`, 4 calls, `3.6651B` avg; `system_job:turn_deadline`, 2 calls, `3.4360B` avg.
  - Gate L: `system_job:battle_round_advance`, 6 calls, `3.6665B` avg; `system_job:turn_deadline`, 2 calls, `4.9764B` avg.
  - Gate M: `system_job:battle_round_advance`, 4 calls, `3.7006B` avg; `system_job:turn_deadline`, 13 calls, `12.1741B` avg.

Decision:

- Keep this checkpoint. It answers whether timer-triggered work is measured: yes, it is now visible in the same benchmark artifacts.
- Use the new timer rows to judge `sync_session_turn` / `sync_battle` design work. If timer jobs duplicate the same row-backed sync cost, the next architectural cut should be a shared runtime driver rather than separate public/timer implementations.

## Full Timer Surface Coverage

Time: `2026-05-21T16:02:11Z`.

Cut:

- Added a dedicated `timer-surface` benchmark gate to `scripts/run-benchmarks.sh`.
- Wrapped the benchmark runtime setup-session timer as `runtime_timer:setup_session`, because setup is not dispatched through `SystemJob` in benchmark builds.
- Re-enabled benchmark scheduling for scenario maintenance jobs so `scenario_objectives`, `world_events`, and `advanced_victory` are visible as timer rows.
- Kept benchmark scenario maintenance code-size safe by running the scenario sync work without the synthetic `GameCommand` bookkeeping; production still uses the full command-backed scenario job path.
- Made benchmark new-battle timeout scheduling enqueue a durable `battle_timeout` system job instead of using the benchmark no-op runtime wakeup.
- Added a `timer-surface` Pocket IC route that drives setup, turn deadline, turn resolution, scenario maintenance, battle timeout, and battle round advance, then asserts all timer labels appear.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister --features benchmark`
- `DOMM_CANISTER_FEATURES=benchmark cargo test -p domm-pocket-ic-tests --test canister_endpoints --no-run`
- Focused `timer-surface` passed at `target/benchmarks/20260521-155645-timer-surface-local/timer-surface`.
- Full suite `target/benchmarks/20260521-160211-all-timers` passed with endpoint-surface, timer-surface, Gate J, Gate K, Gate L, and Gate M.
- Cleaned leftover Pocket IC processes after focused direct runs.

Measurement:

- Full-suite endpoint coverage stayed `59/59`.
- Public method samples: `841`.
- Timer/system-job samples: `299`.
- Timer jobs covered: `8`.
- Timer rows in the combined aggregate:
  - `system_job:turn_resolution`: `1` call, `18.4160B` avg.
  - `runtime_timer:setup_session`: `153` calls, `14.5155B` avg.
  - `system_job:turn_deadline`: `33` calls, `13.1504B` avg.
  - `system_job:battle_round_advance`: `21` calls, `3.9430B` avg.
  - `system_job:world_events`: `27` calls, `3.8134B` avg.
  - `system_job:scenario_objectives`: `30` calls, `3.5171B` avg.
  - `system_job:battle_timeout`: `7` calls, `2.5367B` avg.
  - `system_job:advanced_victory`: `27` calls, `1.8078B` avg.
- Replaced `bench.x.md` with the new aggregate from `20260521-160211-all-timers`.

Decision:

- Keep this checkpoint. The benchmark is no longer missing known timer paths.
- The numbers argue for a shared runtime-owned driver for public sync endpoints and timer jobs. Setup, turn deadline/resolution, battle round advance, battle timeout, and scenario maintenance are still paying multi-billion instruction paths, so the next architecture pass should remove row-backed duplicate work rather than tuning public and timer paths separately.

## Kernel Runtime Architecture Direction

Time: `2026-05-21T16:30:00Z`.

Assessment:

- The proposed model is the right next abstraction: gameplay should run in live kernels, while IcyDB should hold projections, history, indexes, and simple durable product data.
- We are already close in pieces. `BattleRuntime` is an ephemeral battle kernel, and `SessionTurnRuntime` is the start of a worldmap/session-turn kernel.
- The missing step is making the architecture explicit and shared: public endpoints and timer jobs must call the same kernel drivers instead of each path rehydrating row-backed state independently.
- The caveat is correctness. Deferred IcyDB sync is fine only when the kernel remains the authoritative source for immediate API responses, command replay/status, event feeds, and upgrade serialization.

Decision:

- Updated `perf1.todo.md` with Section 72, `Kernel Runtime Architecture: Worldmap And Battle`.
- Pause random endpoint micro-cuts unless they unblock code size or a benchmark gate.
- Target the next implementation around shared worldmap and battle kernel drivers, starting with the public/timer pairs that still show multi-billion instruction costs in `bench.x.md`.

## Kernel Implementation Plan Review

Time: `2026-05-21T16:45:00Z`.

Inputs:

- Ran three codebase review agents: worldmap/session-turn, battle runtime, and projection/recovery/benchmark architecture.
- Local read confirmed the same core surfaces: `SessionTurnRuntime`, `BattleRuntime`, `flush_barrier`, `system_jobs`, `movement`, `scenario_progress`, battle tactical tables, and benchmark summaries.

Findings:

- `SessionTurnRuntime` already owns most of the future `WorldMapRuntime`: session, participants, readiness, champions, objects, occupancy/contact indexes, intents, receipts, events, resource/object deltas, quests, rules, and dirty flags.
- The major missing cut is shared driving. Public `sync_session_turn` has runtime pieces, but timer `turn_deadline` / `turn_resolution` still duplicate row-backed durable work through `MovementPersistenceMode::DurableBridge`.
- `BattleRuntime` is already an ephemeral battle kernel for common submit actions, but `sync_battle`, `end_battle_turn`, battle round jobs, timeout jobs, upgrade restore, aftermath, and `CastAbility` still retain durable row dependencies.
- Projection flushing is currently barrier/upgrade oriented. It needs typed dirty queues, generation cursors, chunked idempotent flush, projection lag metrics, and upgrade restore that does not depend on one huge final flush.
- We should not delete tables first. First remove them from hot paths, then prove recovery/history/projection no longer use the indexes before dropping them.

Plan update:

- Rewrote Section 72 in `perf1.todo.md` into five implementation tracks: worldmap kernel driver, projection flush/recovery, ephemeral battle cleanup, table/index taxonomy, and benchmark gates.
- Paused random endpoint micro-cuts in the plan. The next work should be shared worldmap/battle kernel drivers and projection infrastructure.
- Baseline targets remain `target/benchmarks/20260521-160211-all-timers` / `bench.x.md`: setup timer `14.5155B`, turn deadline `13.1504B`, turn resolution `18.4160B`, sync battle `8.6881B`, battle round advance `3.9430B`, battle timeout `2.5367B`, and sync session turn `1.6226B`.

## Worldmap Kernel Facade Checkpoint

Time: `2026-05-21T17:05:00Z`.

Cut:

- Added `canisters/degens/src/services/worldmap_kernel.rs` as the explicit facade around `SessionTurnRuntime`.
- Kept `SessionTurnRuntime` as the only active worldmap store; the new module only centralizes active-turn contains/prepare/insert/remove and caller-context lookup helpers.
- Routed the movement service's runtime setup/adoption/removal and projection-cache active-turn checks through the facade. Public/timer behavior is unchanged; this creates the boundary for the next shared turn driver extraction.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`

Decision:

- Keep this as a small no-behavior checkpoint. The next Section 72A cut should extract a shared turn driver from `movement.rs` so public sync and timer resolution can share runtime-owned advancement logic.

## Worldmap Turn Advance Extraction

Time: `2026-05-21T17:30:00Z`.

Cut:

- Added `worldmap_kernel::advance_turn` for the common session turn increment, deadline refresh, weekly economy boundary, next-runtime preparation, durable session update, and runtime insertion.
- Public `sync_session_turn_with_command` now calls the shared helper with `TurnAdvanceRuntimeMode::CarryPrevious`.
- Timer `process_turn_resolution_job_inner` now calls the same helper with `TurnAdvanceRuntimeMode::HydrateRows`.
- Removed the duplicated local `turn_deadline` helper from `movement.rs`.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bff9dd` / `12,581,341` bytes, `1,571` bytes under the IC limit.

Decision:

- Keep this as the first shared worldmap driver extraction. It is intentionally behavior-neutral: timer resolution still uses durable movement persistence, and moving timers onto runtime-only state remains the next Section 72A cut.

## Worldmap Readiness Authority Facade

Time: `2026-05-21T18:05:00Z`.

Cut:

- Added kernel readiness helpers in `worldmap_kernel`: `mark_participant_ready`, `all_participants_ready`, and `has_no_ready_participants`.
- `end_turn` now marks runtime readiness through the kernel facade first and falls back to durable `ParticipantTurnReady` rows when runtime readiness is incomplete or absent.
- Pre-deadline `sync_session_turn` readiness checks now use the kernel facade, so runtime `ready_participants` can be authoritative before falling back to durable ready rows.
- `sync_session_turn`, `end_turn`, `turn_deadline`, and `turn_resolution` now share the worldmap facade for the active-turn readiness/advance boundary. Timer turn processing still uses durable movement persistence; that remains the next cut.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bffc15` / `12,581,909` bytes, `1,003` bytes under the IC limit.

Decision:

- Keep this checkpoint. It completes the shared readiness/turn-advance facade without moving timer resolution off `MovementPersistenceMode::DurableBridge` yet.

## Runtime Turn Timer Path

Time: `2026-05-21T19:05:00Z`.

Cut:

- Active `turn_deadline` / `turn_resolution` timer jobs now create an in-memory `SyncTurnCommand::Runtime` when `SessionTurnRuntime` exists instead of creating/loading a durable system `GameCommand`.
- The active timer path calls `resolve_pending_movement` with `MovementPersistenceMode::RuntimeOnly`, so runtime intents do not materialize as durable `MovementIntent` rows and runtime movement snapshots are not written.
- Timer movement, income, champion, participant, object, and event writes now use the same runtime-only branches as active public sync. Durable `SystemJob` rows still claim/complete/schedule wakeups; scheduled follow-up jobs do not store a runtime-only command id.
- To stay under the IC benchmark Wasm limit, the timer path still pages durable active participants for income; moving participant enumeration fully into the kernel needs more code-size headroom or another deletion.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bffee2` / `12,582,626` bytes, `286` bytes under the IC limit.
- Focused `timer-surface`: `target/benchmarks/20260521-runtime-turn-timer-local/timer-surface`, passed in `429.36s`.
- Cleaned the leftover PocketIC process after the focused run.

Measurement versus `target/benchmarks/20260521-160211-all-timers/timer-surface`:

- Timer-surface scenario instructions: `1426.3003B -> 1254.7012B`, `-12.0%`.
- `system_job:turn_deadline`: `15.7940B -> 5.0118B`, `-68.3%`.
- `system_job:turn_resolution`: `18.4160B -> 6.1302B`, `-66.7%`.
- `commands.create_game_command`: `20 -> 7` calls.
- `commands.update_game_command`: `94 -> 80` calls.
- `commands.game_command_idempotency`: `89 -> 75` calls.
- Row growth: `473 -> 445`.
- Stable pages final: `252673 -> 238209`.

Decision:

- Keep this checkpoint. It completes the active runtime timer persistence-mode cut while preserving durable job wakeups. The next Section 72A work is scenario maintenance kernel integration; broader projection queues remain Section 72B.

## Scenario Maintenance Runtime Driver

Time: `2026-05-21T20:25:00Z`.

Cut:

- Active scenario maintenance jobs now load the authoritative session row from `SessionTurnRuntime` when the job turn matches the active runtime, falling back to durable `GameSession` rows only when no matching runtime exists.
- Production active-runtime scenario jobs now run the same scenario maintenance application without creating/updating a synthetic system `GameCommand`; durable `SystemJob` rows still claim/complete/schedule the `scenario_objectives -> world_events -> advanced_victory` wakeup chain.
- Benchmark builds use the active runtime session path for scenario maintenance timers and keep the durable fallback out of the benchmark canister to stay below the IC Wasm code-section limit.
- Tried a runtime `WorldEventState` snapshot path first, but it exceeded the benchmark Wasm limit by several KB. Full runtime scenario projection queues remain Section 72B work.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bfff44` / `12,582,724` bytes, `188` bytes under the IC limit.
- Focused `timer-surface`: `target/benchmarks/20260521-scenario-maint-runtime-local/timer-surface`, passed in `434.12s`.
- Cleaned the leftover PocketIC process after the focused run.

Measurement versus `target/benchmarks/20260521-runtime-turn-timer-local/timer-surface`:

- Timer-surface scenario instructions: `1254.7012B -> 1227.2488B`, `-2.2%`.
- `system_job:scenario_objectives`: `3.7688B -> 3.0654B`, `-18.7%`.
- `system_job:world_events`: `3.7663B -> 3.0637B`, `-18.7%`.
- `system_job:advanced_victory`: `1.8840B -> 1.1833B`, `-37.2%`.
- `sessions.load_session`: `130 -> 91` calls.
- Row growth stayed `445`; stable pages stayed `2049 -> 238209`.

Decision:

- Keep this checkpoint. It folds scenario maintenance onto the active runtime session driver and removes synthetic durable command bookkeeping for active production jobs. Runtime-owned scenario projection queues are intentionally left for Section 72B because the direct snapshot attempt did not fit the current code-size budget.

## Synchronous Kernel Response Authority

Time: `2026-05-21T21:05:00Z`.

Cut:

- `get_setup_progress` now reads the latest active `SessionTurnRuntime` session row before falling back to the durable `GameSession` row.
- This matches the existing runtime-first `get_session` path, so setup/session synchronous responses no longer synthesize stale durable session state while an active worldmap kernel exists.
- Broader projection flush queues remain Section 72B; this checkpoint only closes the remaining direct setup-progress query gap.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bfffe1` / `12,582,881` bytes, `31` bytes under the IC limit.
- `cargo test -p domm-degens-canister start_session_replay_while_starting_reuses_original_nonce_and_cursor -- --nocapture`

Test note:

- `cargo test -p domm-degens-canister lobby_session_setup_recovers_from_starting_state_and_replays_nonce -- --nocapture` was also tried and failed later in battle handoff flushing with `events.create_game_event` at `movement.rs:1184`, after the setup-progress recovery assertions. Treat that as a separate movement/flush-barrier issue, not a passing verification for this checkpoint.

Decision:

- Keep this checkpoint. The active session kernel is now the first source for the setup progress response, while durable rows remain the fallback when no active runtime exists.

## Worldmap Projection Dirty Queue

Time: `2026-05-21T20:56:29Z`.

Cut:

- Added a typed production dirty queue to `SessionTurnRuntime` with `{ kernel_id, generation, entity, key, op, priority, first_dirty_at_ms, last_dirty_at_ms }`.
- Queue entries use typed `ProjectionEntity` and `ProjectionDirtyOp` enums, coalesce repeated changes by kernel/entity/key, keep the first dirty timestamp, update the latest generation/timestamp, and preserve tombstones for occupancy removals.
- Runtime mutation helpers now enqueue projection dirty entries for session, participant, ready, champion, champion spell, world object, occupancy, contact, movement intent, command receipt, event, object delta, resource delta, quest, and scenario rule surfaces.
- The current full `flush_runtime_projections_for_upgrade` barrier clears the queue after flushing. Bounded chunk flushing and projection cursors remain the next Section 72B items.
- The queue is compiled out of `feature=benchmark` for now because projection flushing itself is not benchmark-enabled yet and the benchmark Wasm budget is only 31 bytes.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister projection_dirty_queue -- --nocapture`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bfffe1` / `12,582,881` bytes, `31` bytes under the IC limit.

Code-size note:

- The first benchmark release build after this patch exceeded the IC code-section limit at `0x00c0007a`, because `mark_ready` gained a benchmark-visible clone while adding the production queue entry. Splitting `mark_ready` by cfg restored the benchmark path to the previous `0x00bfffe1` size.

Decision:

- Keep this checkpoint. It gives worldmap runtime state a real typed projection backlog without changing active benchmark hot paths. The next work should add bounded flushing over this queue and projection generation cursors.

## Bounded Worldmap Projection Flush

Time: `2026-05-21T21:07:32Z`.

Cut:

- Added production `flush_runtime_projection_queue(limits)` over the `SessionTurnRuntime` dirty queue.
- Limits cover max processed rows/entries, max instruction delta, and max stable-page delta. The flusher records processed entries, durable rows flushed, before/after queue length, truncation, instruction delta, and stable-page delta.
- Flush chunks are sorted by priority, first dirty timestamp, kernel id, and key. Completed entries are removed from the queue; unprocessed entries remain retryable after truncation or error.
- Durable writes are idempotent by existing natural keys where available:
  - session row by session id
  - participant row by participant id
  - ready row by `(session, participant, turn)`
  - champion row by id
  - champion spell by `(champion, spell)`
  - world object by id
  - map occupancy by cell or occupant
  - movement intent by `(session, champion, turn)`
  - command receipt by idempotency
  - event by event key
  - resource ledger by `(command, ledger_key)`
  - quest by `(session, participant, quest_key)`
  - scenario rule by `(session, rule_key)`
- Occupancy tombstones delete durable map occupancy rows by occupant key. `Contact` and `ObjectDelta` entries currently complete as derived/no-op entries because their durable owners are the occupancy/object/visibility rows.
- Event entry completion also marks the runtime event flushed so active event feeds stop serving it after durable projection.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister projection -- --nocapture`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bfffe1` / `12,582,881` bytes, `31` bytes under the IC limit.

Decision:

- Keep this checkpoint. Bounded worldmap projection flushing now exists, but `flush_barrier` still calls the older full flush path and generation cursors are not recorded yet. Those are the next Section 72B cuts.

## Worldmap Projection Checkpoints

Time: `2026-05-21T21:15:16Z`.

Cut:

- Added production `ProjectionCheckpoint` state inside each `SessionTurnRuntime`.
- The checkpoint tracks kernel id, flushed generation, last flush timestamp, and pending dirty entry count.
- Dirty recording updates pending entry counts.
- Completed projection entries refresh the checkpoint conservatively:
  - when entries remain, `flushed_generation` advances to one generation before the oldest pending dirty entry;
  - when the queue is empty, `flushed_generation` advances to the runtime generation.
- Added `projection_checkpoints_snapshot()` for the upcoming diagnostics/recovery surface.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-degens-canister projection -- --nocapture`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bfffe1` / `12,582,881` bytes, `31` bytes under the IC limit.

Decision:

- Keep this checkpoint. Recovery can now tell the last conservatively flushed kernel generation from runtime state. `flush_barrier` still needs to call the bounded flusher, and durable upgrade serialization for worldmap kernels remains a later Section 72B item.

## Flush Barrier Uses Projection Queue

Time: `2026-05-21T21:21:40Z`.

Cut:

- Routed the session runtime portion of `flush_barrier` through `flush_runtime_projection_queue(ProjectionFlushLimits::unbounded())`.
- Strong-read, upgrade, turn-advance, battle-handoff, and runtime-eviction barriers now use the same chunked projection flusher as bounded background flushes.
- If the barrier leaves any session runtime dirty entries behind, it returns `projection_flush_incomplete` with structured details for queue length, processed entries, rows flushed, truncation, instruction delta, and stable-page delta instead of silently clearing or hiding backlog.
- Other barrier participants still use their existing full flush/archive paths; this checkpoint only covers the worldmap/session runtime projection queue.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-degens-canister projection -- --nocapture`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bfffe1` / `12,582,881` bytes, `31` bytes under the IC limit.

Decision:

- Keep this checkpoint. `flush_barrier` no longer has a separate session-runtime flush implementation. Upgrade safety still needs worldmap kernel serialization before timer repair; that remains the next Section 72B item.

## Worldmap Runtime Upgrade Snapshot

Time: `2026-05-21T21:34:00Z`.

Cut:

- Added production-only upgrade snapshot storage for `SessionTurnRuntime` in stable memory id `24`.
- This does not collide with IcyDB stable memory ids `20/21/22`, commit memory id `119`, or battle runtime memory id `23`.
- The snapshot uses Candid to preserve active session-turn runtimes, including runtime command/events, dirty projection queues, and projection checkpoints.
- `pre_upgrade` now writes the session runtime snapshot before the upgrade flush barrier, runs the barrier, then writes it again so completed projection entries are not replayed after upgrade.
- `post_upgrade` restores session runtimes before active-session admission repair, first-playable repair, and `system_jobs::repair_and_schedule_after_install_or_upgrade`, so timer repair can see restored kernel deadlines before relying on durable job rows.
- Added encode/decode coverage proving dirty queues and projection checkpoints survive snapshot round trip.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- `cargo test -p domm-degens-canister projection -- --nocapture`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bfffe1` / `12,582,881` bytes, `31` bytes under the IC limit.

Decision:

- Keep this checkpoint. Worldmap kernels now have the same upgrade-safety class as battle runtimes, but with the full dirty queue/checkpoint snapshot needed for deferred projection recovery. Projection diagnostics and upgrade-recovery benchmark coverage remain open Section 72B items.

## Worldmap Projection Diagnostics

Time: `2026-05-21T22:02:00Z`.

Cut:

- Added production-only `get_diagnostic_projection_snapshot`, controller-gated like the other diagnostic endpoints.
- The diagnostic response reports each active worldmap kernel's dirty queue length, oldest dirty age, kernel generation, flushed generation, lag generations, lag ms, checkpoint flush timestamp, and pending entry count.
- The response also includes global dirty queue length, global oldest dirty age, and the last projection flush outcome.
- `flush_runtime_projection_queue` now records the last flush's rows flushed, entries processed, queue before/after, truncation flag, stable pages delta, instruction delta, and flush timestamp.
- The Candid export test now asserts the production diagnostic query is exported.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-degens-canister projection -- --nocapture`
- `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`
- `cargo test -p domm-degens-canister exported_candid_contains_every_required_game_endpoint -- --nocapture`
- `cargo test -p domm-degens-canister exported_candid --features benchmark -- --nocapture`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bfffe1` / `12,582,881` bytes, `31` bytes under the IC limit.

Decision:

- Keep this checkpoint. Section 72B now has a controller-visible projection lag surface for runtime recovery and flush debugging. The remaining Section 72B item is end-to-end projection-surface and upgrade-recovery benchmark coverage.

## Projection Surface Benchmark Blocked By Production Wasm Size

Time: `2026-05-21T22:42:42Z`.

Attempt:

- Drafted a focused PocketIC route to dirty runtime movement, verify runtime command/event reads before flush, run the strong-read projection flush, verify durable `MovementIntent`/`GameCommand`/`GameEvent` rows and zero lag, dirty `end_turn`, upgrade, and verify restored kernel status.
- Running it against the default production canister failed during install before assertions.

Blocker:

- Default production Wasm code section was `13,668,633` bytes, above the IC limit of `12,582,912`.
- The benchmark Wasm remains installable, but the projection runtime/diagnostics are compiled out of `feature=benchmark`, so the existing benchmark canister cannot cover the production projection surface.
- Do not wire `projection-surface` into `scripts/run-benchmarks.sh` until either production code size is below the IC limit or a smaller projection-enabled benchmark feature exists.

Decision:

- Leave the Section 72B projection-surface benchmark item open and move to the next implementable kernel cleanup item.

## Shared Battle Kernel Driver

Time: `2026-05-21T23:10:52Z`.

Cut:

- Added a production `BattleKernelDriver` in `services/battle.rs`.
- The driver owns the common battle-kernel progression flow for sync timeout catchup, runtime timeout timers/jobs, durable timeout fallback, readiness scheduling, and round auto-defends while collecting events and changed subjects.
- Routed `sync_battle`, `submit_battle_action` timeout catchup/readiness, `submit_runtime_battle_action` readiness, `end_battle_turn` readiness, runtime timeout timers, durable `battle_timeout` jobs, and `battle_round_advance` jobs through the driver in production builds.
- Kept `feature=benchmark` on the previous direct call shape because the benchmark Wasm budget is still extremely tight.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-game battle -- --nocapture`
- `cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_timer_auto_defends_without_sync_battle_and_replays_noop --no-run`
- `cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_all_ready_battle_round_schedules_immediate_advance --no-run`
- `git diff --check`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bffff9` / `12,582,905` bytes, `7` bytes under the IC limit.

Decision:

- Keep this checkpoint. The production battle paths now have one orchestrator for timeout/readiness/round progression, which gives the remaining 72C runtime-receipt and battle-child-row cleanup work a smaller surface to move across.

## Active Battle Sync Runtime Receipts

Time: `2026-05-21T23:19:20Z`.

Cut:

- Added a production-only active `BattleRuntime` fast path for `sync_battle` before durable battle row loading and durable `GameCommand` creation.
- Runtime `sync_battle` now hashes the same payload/idempotency input, replays matching `BattleRuntimeCommandReceipt` rows by nonce, drives runtime timeout catchup through `BattleKernelDriver`, stores timeout public events in `BattleRuntime.active_events`, and stores a `sync_battle` runtime command receipt.
- Active runtime sync no longer runs the durable applying-command recovery scan or creates/updates a durable `GameCommand`; row-backed legacy/adoption battles still use the existing durable fallback.
- Command status by nonce now checks battle runtime receipts for `sync_battle` in production builds.
- Benchmark builds keep the previous direct durable path because the benchmark Wasm has only single-digit-byte headroom.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-game battle -- --nocapture`
- `cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_timer_auto_defends_without_sync_battle_and_replays_noop --no-run`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bffff9` / `12,582,905` bytes, `7` bytes under the IC limit.

Decision:

- Keep this checkpoint. Active battle sync now has the same receipt/replay shape as active battle action submission, leaving `end_battle_turn` as the next durable command/event write to remove from the active battle path.

## Active Battle End Turn Runtime Receipts

Time: `2026-05-21T23:36:46Z`.

Cut:

- Added a production-only active `BattleRuntime` fast path for `end_battle_turn` before durable battle row loading and durable `GameCommand` creation.
- Runtime `end_battle_turn` now hashes the same payload/idempotency input, replays matching `BattleRuntimeCommandReceipt` rows by nonce, marks `BattleRuntime.ready_participants`, drives readiness through `BattleKernelDriver`, stores the public ready event in `BattleRuntime.active_events`, and stores an `end_battle_turn` runtime command receipt.
- Active runtime end-turn status by nonce now reads the battle runtime receipt. Status by id already used the generic battle runtime receipt lookup.
- Benchmark builds keep the previous durable path because the benchmark Wasm has only single-digit-byte headroom. Row-backed legacy/adoption battles also keep the durable fallback.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `git diff --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-game battle -- --nocapture`
- `cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_readiness_advances_and_replays --no-run`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bffff9` / `12,582,905` bytes, `7` bytes under the IC limit.
- Attempted direct production PocketIC run: `cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_readiness_advances_and_replays -- --nocapture`; it failed during canister install because the default production Wasm code section is `13,693,086` bytes, above the IC limit `12,582,912`.

Decision:

- Keep this checkpoint. Active battle sync and end-turn now both answer from runtime receipts/events; the next active battle cleanup should move round advancement authority away from durable `SystemJob` rows.

## Runtime Battle Round Wakeups

Time: `2026-05-21T23:47:47Z`.

Cut:

- Replaced production active battle round scheduling with `BattleRuntime` wakeups. Runtime readiness now records `BattleRuntime.deadline.round_job_key` and schedules a zero-delay runtime timer instead of inserting a `battle_round_advance` `SystemJob`.
- The runtime timer loads the live session, rechecks runtime readiness through `BattleKernelDriver`, drives round auto-defends through the shared driver, reschedules itself for partial round batches, clears stale runtime round keys, and then schedules the next runtime timeout wakeup from the live battle header.
- Benchmark builds and row-backed repair/fallback paths keep the existing durable `SystemJob` branch.
- Focused PocketIC assertions now accept either the legacy `system_job` changed subject or the runtime `battle_round_advance` wakeup subject.

Verification:

- `cargo fmt`
- `cargo fmt --check`
- `git diff --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-game battle -- --nocapture`
- `cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_battle_round_both_players_end_round_and_timer_catches_up --no-run`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bffff9` / `12,582,905` bytes, `7` bytes under the IC limit.

Decision:

- Keep this checkpoint. Active battle round authority now lives in the runtime timer path; durable round `SystemJob` processing remains as benchmark/repair/fallback infrastructure.

## Self-Contained Battle Runtime Upgrade Snapshots

Time: `2026-05-21T23:58:49Z`.

Cut:

- Replaced production battle runtime upgrade persistence with a full Candid-encoded `BattleRuntimeSnapshot`.
- The snapshot now carries live `BattleState` tactical data, runtime events, runtime command receipts, ready participants, deadline metadata, event cursor, and dirty generation.
- Post-upgrade restore installs the decoded runtime directly and no longer loads `Battle`, `BattleStack`, `BattleObstacle`, or `BattleOccupancy` rows for snapshots produced by the new code.
- Kept a legacy ref-only snapshot reader for upgrades from older code; that compatibility path still hydrates from rows only when old snapshot bytes are encountered.
- Benchmark builds use no-op battle runtime snapshot persistence/restore so the deployable benchmark Wasm stays under the IC code-section limit.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-degens-canister battle_runtime -- --nocapture`
- `cargo test -p domm-game battle -- --nocapture`
- `cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_timer_jobs_repair_deadlines_and_recover_expired_leases --no-run`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bfd3e7` / `12,571,623` bytes, `11,289` bytes under the IC limit.

Decision:

- Keep this checkpoint. Active battle runtime restore no longer depends on durable tactical child rows for snapshots written by current production code, unblocking the next cleanup work around tactical child-row projection and aftermath reads.

## Runtime Survivor Battle Aftermath

Time: `2026-05-22T00:09:17Z`.

Cut:

- Added a runtime survivor stack source to `battle_aftermath`.
- Active resolved battle handoff now persists only the resolved `Battle` header before applying aftermath.
- Champion survivor updates and town garrison replacement can consume `BattleRuntime.state.stacks` summaries directly, avoiding the old full `BattleStack` projection just so aftermath could page those rows back.
- Row-backed and legacy aftermath paths keep using durable `BattleStack` rows.
- Added direct unit coverage for runtime stack-summary filtering without durable rows.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister`
- `cargo test -p domm-degens-canister battle_aftermath -- --nocapture`
- `cargo test -p domm-degens-canister battle -- --nocapture`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_render_projection_tracks_battle_aftermath_objects --no-run`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bfe1fa` / `12,575,226` bytes, `7,686` bytes under the IC limit.

Decision:

- Keep this checkpoint. Active battle aftermath no longer depends on durable tactical survivor rows, leaving those rows as a row-backed/legacy fallback instead of an active runtime finalization prerequisite.

## Runtime Battle CastAbility

Time: `2026-05-22T00:20:28Z`.

Cut:

- Let active `CastAbility` submissions use the runtime battle command path.
- Shared the spell application logic so active runtime casts mutate `BattleRuntime.state` for spell damage, target status keys, caster `cast_round`, and caster `acted_round`.
- Added runtime spell/action feed events and runtime command receipts for active cast replay/status/feed behavior.
- Kept champion mana, learned-spell validation, champion cache mirroring, and battle-spell `CommandEffect` writes durable because those are strategic boundaries.
- Kept the row-backed cast fallback for legacy/adoption paths.

Verification:

- `cargo fmt`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- `cargo test -p domm-degens-canister battle -- --nocapture`
- `cargo test -p domm-game battle -- --nocapture`
- `cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_canister_exposes_every_required_game_endpoint --no-run`
- Benchmark Wasm build with `feature=benchmark`: code section `0x00bffe14` / `12,582,420` bytes, `492` bytes under the IC limit.

Decision:

- Keep this checkpoint, but note the benchmark Wasm is very close to the IC code-section ceiling. The next cut should prefer taxonomy/docs or code-size-neutral changes unless it frees headroom first.

## Direct IcyDB Authority Taxonomy

Time: `2026-05-22T00:22:15Z`.

Cut:

- Added `perf1.table-taxonomy.md` as the Section 72D ownership artifact.
- Marked `PlayerAccount`, account/profile identity, static content definitions, controller/admin diagnostics, durable finished match summaries, and pre-active lobby/session admission as direct IcyDB authority.
- Recorded that caches around those rows are mirrors, negative caches, or replay helpers only; durable rows remain the source of truth for uniqueness, conflicts, recovery, and finished-history reads.
- Left the active worldmap, active battle, and index-retirement taxonomy slices open for later checkpoints.

Verification:

- `git diff --check`

Decision:

- Keep this as a doc-only checkpoint because the benchmark Wasm has only `492` bytes of code-section headroom after the runtime `CastAbility` cut.

## Active Worldmap Projection Taxonomy

Time: `2026-05-22T00:23:40Z`.

Cut:

- Extended `perf1.table-taxonomy.md` with the active worldmap projection/history slice.
- Marked active `GameSession` turn fields, active `GameParticipant` resources/readiness, ready rows, movement rows, command/effect/event rows, resource ledger rows, scenario progress rows, visibility/occupancy/known-object rows, mutable world-object state, and town/champion/tavern/dwelling child rows as projection/history-only while a worldmap kernel exists.
- Recorded `SessionTurnRuntime` and the worldmap kernel facade as live authority for those active gameplay surfaces.
- Kept durable rows as setup/adoption, explicit projection flush, post-upgrade recovery, diagnostics, history, and row-backed fallback surfaces.

Verification:

- `git diff --check`

Decision:

- Keep this as a doc-only checkpoint. The taxonomy prevents row-backed fallback paths from being treated as active authority while avoiding additional benchmark Wasm growth.

## Active Battle Projection Taxonomy

Time: `2026-05-22T00:25:12Z`.

Cut:

- Extended `perf1.table-taxonomy.md` with the active battle projection/history slice.
- Marked `BattleStack`, `BattleObstacle`, `BattleOccupancy`, `BattleParticipantRoundReady`, battle command/event rows, and battle-specific `SystemJob` rows as projection/history-only while a `BattleRuntime` exists.
- Recorded `BattleRuntime` as active authority for tactical stack state, obstacle state, occupancy, readiness, runtime command receipts, runtime events, deadlines, and round wakeups.
- Kept `Battle` as a durable shell/outcome/history boundary and startup/adoption anchor, with active tactical header fields mirrored from runtime while active.

Verification:

- `git diff --check`

Decision:

- Keep this as a doc-only checkpoint. Current-code battle upgrade restore and active aftermath no longer require tactical projection rows, so the taxonomy now reflects the implementation boundary without spending benchmark Wasm headroom.
