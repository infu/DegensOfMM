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
