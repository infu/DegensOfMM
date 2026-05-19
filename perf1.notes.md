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
