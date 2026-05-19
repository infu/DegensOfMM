# perf1 Todo: Aggregate Runtime Performance Rewrite

Companion notes and decision log: `perf1.notes.md`.

## Problem

The current gameplay hot paths use normalized IcyDB rows as the live mutation model. That means a single player action often hydrates many rows, mutates an in-memory shape, diffs child rows, updates several indexes, emits events, reloads related rows, and schedules background work.

`submit_battle_action` is the clearest smell. The latest benchmark run measured:

| scenario | calls | avg instructions | max instructions | avg memory delta | avg wall time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `aftermath_victory` | 45 | 27.7169B | 29.2661B | 172.20 MB | 3.15s |
| `guarded_mine_battle` | 6 | 21.5041B | 27.3657B | 110.76 MB | 2.46s |

The problem is bigger than one missing index. Stable storage and IcyDB row/index maintenance amplify the cost, but the deeper issue is the data shape: battle/town/champion/session state is being treated as many live database rows when gameplay commands naturally mutate one aggregate.

## Motivation

Gameplay commands should feel like:

1. Load one live object.
2. Validate the command.
3. Mutate the object.
4. Save/checkpoint the object.
5. Append command/event receipts.
6. Update small projections only when needed.

Instead, some paths currently behave like small database transactions over dozens of rows and indexes. This adds instruction cost, memory growth, code complexity, and more chances for state/view drift.

The target is a command-side aggregate model with query-side projections:

| area | current smell | target |
| --- | --- | --- |
| battle | `Battle`, `BattleStack`, `BattleObstacle`, `BattleOccupancy` live-row mutation | one active `BattleRuntime` aggregate |
| town | buildings, recruit pools, garrison split across rows | one town aggregate for command-side state |
| champion | army, artifacts, spells, cooldowns spread across rows | one champion aggregate for command-side state |
| session/world | commands touch many small durable rows | bounded aggregate/shard mutations |
| APIs | read through repeated row loads | read aggregate or rebuilt projections |
| history/events | mixed with live mutation details | durable append/projection output |

IcyDB remains useful for command idempotency, event feeds, lookup rows, history, diagnostics, and projections. It should not be the main live object model for every tactical field update.

## Measurement Plan

Use the benchmark suite added under `scripts/run-benchmarks.sh` as the main performance yardstick.

Primary command:

```bash
DOMM_BENCH_JOBS=4 scripts/run-benchmarks.sh
```

Primary artifacts:

```text
target/benchmarks/<run-id>/suite-summary.md
target/benchmarks/<run-id>/suite-summary.json
target/benchmarks/<run-id>/gate-k/summary.json
target/benchmarks/<run-id>/gate-l/summary.json
target/benchmarks/<run-id>/gate-k/run.json
target/benchmarks/<run-id>/gate-l/run.json
```

Primary method to watch:

| method | current baseline | first target | good target | normal target |
| --- | ---: | ---: | ---: | ---: |
| `submit_battle_action` | ~26B-28B avg | under 10B avg | under 1B avg | around 0.3B avg |

The plan is intentionally fluid. The implementation details can change aggressively if the benchmark does not move enough. What matters is reducing `submit_battle_action` toward the normal target, not preserving any particular intermediate design.

Performance gates:

| gate | target | likely work | decision rule |
| --- | ---: | --- | --- |
| Perf Gate 0 | measured baseline | endpoint repo/phase tracing | establish exact cost attribution |
| Perf Gate 1 | under 10B avg | eliminate repeated child-row hydrate/diff/reload in active battles | if not hit, skip smaller cleanup and remove more stable writes |
| Perf Gate 2 | under 3B avg | heap-resident active battle state, no per-action tactical child-row persistence | if not hit, inspect event/command writes and battle rule CPU |
| Perf Gate 3 | under 1B avg | batch event/session writes, avoid readiness rows where possible, action-specific validation | if not hit, profile command/idempotency and Candid/serialization cost |
| Perf Gate 4 | around 0.3B avg | keep only essential durable writes on submit, defer/project everything else | this is the normal target |

If a gate is missed, do not treat the current design as fixed. Re-open the architecture and remove the next largest measured cost.

## Smarter Strategy

The first version of this plan was correct about the smell, but too conservative about the path to `0.3B`. A heap battle aggregate alone may not be enough if every submit still creates/updates durable commands, events, readiness rows, battle timeout jobs, battle headers, or Candid snapshots.

The smarter plan is to make active battle execution progressively less durable on every single action. Durability comes from IC atomicity plus heap state during normal execution, and from upgrade serialization during upgrades. Stable/IcyDB writes should happen at battle start, battle end, explicit flush points, or for small public projections only when measurement proves they are affordable.

Guiding rules:

| rule | implication |
| --- | --- |
| Do not optimize around a bad write model | If per-action stable writes dominate, remove them rather than making them prettier. |
| Measure before broad refactors | Add enough phase/repo tracing to rank costs; do not spend days instrumenting every repo before cutting obvious hot writes. |
| Active battle state is authoritative while active | `BattleRuntime` owns stacks, occupancy, round, deadline, readiness, transient command receipts, and active battle events. |
| Durable rows are projections | `Battle`, `GameCommand`, `GameEvent`, `SystemJob`, and tactical child rows should not be assumed authoritative for active battle internals. |
| Public behavior must survive | If data moves to heap, public query APIs must read or merge heap state while active, then read durable rows after flush/finalization. |
| Gate failures drive architecture changes | Missing a perf gate means removing the next largest measured cost, even if that means changing idempotency/event/job design. |

Expected cuts by gate:

| gate | primary cut | secondary cut |
| --- | --- | --- |
| Gate 1 | no active tactical child-row hydrate/diff/persist | `get_battle_state` reads runtime |
| Gate 2 | no per-action `Battle` header, `SystemJob`, or readiness durable writes | battle deadlines/readiness live in runtime |
| Gate 3 | active battle command receipts and battle events live in runtime, with durable flush/merge APIs | batch `GameSession.next_event_seq` or move active sequence into runtime |
| Gate 4 | battle rule CPU/data-structure pass | action-specific validation, indexed runtime occupancy, no unnecessary reachability BFS |

## Codebase Reality Check

Re-reading the canister code confirms the direction, but tightens the implementation shape:

| code path | current behavior | plan impact |
| --- | --- | --- |
| `submit_battle_action` | loads the `Battle` row, creates/updates `GameCommand`, scans applying commands, applies due timeouts, loads full tactical rows, persists tactical diffs, fans out events, reloads session, recomputes readiness, schedules jobs, updates command complete | phase tracing should wrap these exact blocks; do not spend time on generic tracing before these are visible |
| `battle_rows::load_battle_state*` / `persist_battle_state` | repeatedly converts row sets to `domm_game::BattleState`, then updates `Battle`, changed `BattleStack`s, and occupancy diffs | Gate 1 is valid and should remove this from the active submit path |
| `recompute_battle_round_readiness_and_schedule` | reloads full battle state, reads readiness rows, writes auto-ready rows, and may call full legal-action generation per participant | Gate 2 must replace readiness with runtime state, not just move rows |
| event fanout | writes public and participant `GameEvent`s and updates `GameSession.next_event_seq` once per audience event | Gate 3 needs a heap event sequence allocator or batched reservation to avoid collisions |
| command status | `get_command_status` and nonce lookup read durable `GameCommand` rows | Gate 3 must merge active runtime command receipts, not only `get_events_after` |
| aftermath | `apply_resolved_battle_aftermath` reads `Battle` and survivor `BattleStack` rows | runtime finalization must project survivor rows before calling existing aftermath or rewrite aftermath to consume runtime survivors directly |

Therefore the first runtime cannot be only `domm_game::BattleState`. It should wrap `BattleState` with the active metadata that the canister currently stores in several durable tables: command receipts, active events, participant audiences, readiness, deadlines, and a session event sequence cursor.

Secondary methods to watch:

| method | why |
| --- | --- |
| `get_battle_state` | should not regress badly if active battle reads move to aggregate/projection |
| `sync_battle` | shares timeout, aftermath, and event paths |
| `get_game_view` | catches projection or visibility regressions |
| `get_events_after` | catches event fanout/projection regressions |
| `get_town_view` | important when the same aggregate pattern reaches towns |
| `get_champion_view` | important when the same aggregate pattern reaches champions |

Benchmark comparison checklist:

- Compare `avg_instruction_delta`, `p95_instruction_delta`, `avg_memory_delta_bytes`, `avg_cycle_cost`, and wall time.
- Compare total stable memory growth for Gate K and Gate L.
- Compare row growth for `Battle`, `BattleStack`, `BattleObstacle`, `BattleOccupancy`, `GameCommand`, `GameEvent`, and future aggregate/snapshot rows.
- Compare scenario totals for `guarded_mine_battle` and `aftermath_victory`.
- Keep before/after run IDs in commit messages or todo notes when a checkpoint claims performance progress.

## Saved Baseline: submit_battle_action

Baseline source:

```text
run id: 20260518-231144-9f17dcb
suite: target/benchmarks/20260518-231144-9f17dcb/suite-summary.md
gate-k: target/benchmarks/20260518-231144-9f17dcb/gate-k/summary.json
gate-l: target/benchmarks/20260518-231144-9f17dcb/gate-l/summary.json
```

This baseline was not rerun after creating `smell.md` and `perf1.todo.md` because those commits are documentation-only. There are no code/test/script changes between benchmarked git sha `9f17dcb` and this checkpoint for `Cargo.toml`, `Cargo.lock`, `canisters`, `crates`, `schema`, `scripts`, `testing`, `dfx.json`, or `Makefile`.

Saved method summary:

| source | kind | calls | avg instructions | p95 instructions | avg memory delta | avg cycles |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Gate K | update | 25 | 27.4632B | 28.7961B | 173.88 MB | 0.0275T |
| Gate L | update | 26 | 26.5272B | 28.8134B | 156.41 MB | 0.0265T |
| combined | update | 51 | 26.9860B | 28.8344B | 164.97 MB | 0.0270T |

Saved scenario split:

| scenario | calls | avg instructions | min instructions | max instructions | avg memory delta | avg cycles | avg wall time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `aftermath_victory` | 45 | 27.7169B | 20.7519B | 29.2661B | 172.20 MB | 0.0277T | 3.15s |
| `guarded_mine_battle` | 6 | 21.5041B | 3.5254B | 27.3657B | 110.76 MB | 0.0215T | 2.46s |

Improvement will be measured against the combined line unless a checkpoint specifically optimizes one scenario. The first target is under 10B average instructions for `submit_battle_action`; the normal target is around 0.3B. If an intermediate design cannot plausibly reach that range, replace it rather than polishing it.

## Testing Policy

Roll fast here. Use focused tests during implementation and full benchmark/regression runs only at meaningful checkpoints.

| checkpoint type | minimum testing |
| --- | --- |
| doc/planning only | no tests required |
| compile-affecting refactor | `cargo check` or targeted package test |
| battle aggregate checkpoint | Gate K or Gate L targeted PocketIC test |
| benchmark claim | full `scripts/run-benchmarks.sh` |
| broad aggregate pattern change | Gate J/K/L, and Gate M if API/client behavior may change |

When a todo item is completed:

1. Change its checkbox from `[ ]` to `[x]`.
2. Run the minimum useful test for that checkpoint.
3. Commit with a message that explains what changed and what was verified.

## Todo

### 0. Plan And Baseline

- [x] Write the perf1 problem statement, motivation, aggregate rewrite direction, benchmark measurement plan, and checkbox/commit workflow in `perf1.todo.md`.
- [x] Save the current `submit_battle_action` benchmark baseline in `perf1.todo.md` so aggregate-runtime work has a fixed comparison point.
- [x] Set explicit performance gates in `perf1.todo.md` from the 26.9860B baseline toward the 0.3B normal target.
- [x] Create `perf1.notes.md` as the running notes and decision log for this work.
- [x] Re-evaluate the whole plan and update it so it can plausibly reach the 0.3B target instead of stopping at a partial aggregate rewrite.
- [x] Re-evaluate the updated plan against the actual battle, event, command-status, readiness, and aftermath code paths.
- [x] Add targeted benchmark-only phase markers around `submit_battle_action`: auth/context, command begin, recovery, timeout, load/apply/persist, event fanout, readiness/schedule, final response.
- [x] Add benchmark-only repo operation tracing for central wrappers and battle hot repos first; do not block Gate 1 on converting every repo module.
- [ ] Run a fresh traced baseline and record the run ID plus `submit_battle_action` phase/repo counts in this file and `perf1.notes.md`.
- [ ] Document the active runtime merge contract for `get_battle_state`, `sync_battle`, `end_battle_turn`, `get_events_after`, `get_command_status`, `get_command_status_by_nonce`, timeout jobs, round jobs, and aftermath.

### 1. Active Battle Runtime Authority

- [ ] Define the first `BattleRuntime` as `domm_game::BattleState` plus canister runtime metadata: session id, battle id, participant audience keys, command receipts, active event buffer, readiness set, deadline state, and session event sequence cursor.
- [ ] Add a heap active battle store keyed by battle id.
- [ ] Add `pre_upgrade`/`post_upgrade` serialization for active battles using a dedicated memory slot that does not collide with IcyDB memory IDs.
- [ ] Create/adopt active runtime during battle start paths before the first player action, with compatibility hydration from existing row-backed active battles.
- [ ] Implement loader compatibility so existing row-backed battles can be converted into `BattleRuntime` during migration or first access.
- [ ] Make `get_battle_state` read active runtime directly and fall back to legacy rows only when runtime is absent.
- [ ] Decide whether active runtime events use a heap session event sequence overlay or a stable batch reservation; prevent collisions with stable `GameSession.next_event_seq`.
- [ ] Implement battle finalization that projects resolved runtime state into durable strategic rows/history/events needed after the battle is over.
- [ ] Ensure finalization either projects survivor `BattleStack` rows before calling existing aftermath or rewrites aftermath to read survivors from runtime.

### 2. Gate 1: Remove Tactical Child-Row Hot Writes

- [ ] Change `submit_battle_action` to load/mutate/commit one heap `BattleRuntime` aggregate instead of loading/diffing `BattleStack`, `BattleObstacle`, and `BattleOccupancy` rows repeatedly.
- [ ] Stop persisting active tactical changes to `BattleStack`, `BattleObstacle`, and `BattleOccupancy` rows per action.
- [ ] Resolve `auto:enemy` target normalization from active runtime instead of loading full row-backed state.
- [ ] Reuse the already-mutated aggregate for round readiness instead of reloading battle child state.
- [ ] Make battle validation action-specific so attack/defend/wait do not compute full move reachability unless needed.
- [ ] Keep current durable command/event behavior only long enough to measure the Gate 1 delta.
- [ ] Run focused Gate K or a shorter battle benchmark and decide whether Gate 1 cleared `<10B`.

### 3. Gate 2: Remove Per-Action Battle Job/Readiness/Header Writes

- [ ] Move battle readiness state into `BattleRuntime` for active battles; stop writing `BattleParticipantRoundReady` rows per battle action.
- [ ] Move active battle deadlines/timeouts into `BattleRuntime`; stop upserting `SystemJob` rows on every battle action.
- [ ] Stop updating the `Battle` row header for active round/active stack/deadline changes; update the durable `Battle` row at start/end or explicit projection points.
- [ ] Replace readiness recompute with runtime alive/acted tracking; avoid `legal_actions_for_stack` during readiness except for a measured edge case.
- [ ] Move `end_battle_turn` onto runtime readiness so manual readiness does not keep `BattleParticipantRoundReady` hot.
- [ ] Keep enough scheduling behavior to make timeout/round advancement work from heap state.
- [ ] Run focused Gate K/L and decide whether Gate 2 cleared `<3B`.

### 4. Gate 3: Move Active Battle Commands And Events Out Of Per-Action Stable Writes

- [ ] Add active battle command receipt/idempotency storage inside `BattleRuntime` for battle actions.
- [ ] Make replay of an active battle action return from runtime command receipts without durable `GameCommand` lookup/create/update.
- [ ] Make `get_command_status` and `get_command_status_by_nonce` merge active runtime command receipts before falling back to durable `GameCommand` rows.
- [ ] Store active battle events in runtime and make `get_events_after` merge active runtime events with durable `GameEvent` rows.
- [ ] Precompute participant audience keys in runtime so event fanout does not load champion/town owners per event.
- [ ] Flush or project runtime command/event data to durable rows at battle resolution, explicit checkpoint, or upgrade as needed for history/debugging.
- [ ] Batch or avoid `GameSession.next_event_seq` durable updates during active battle commands.
- [ ] Run focused Gate K/L and decide whether Gate 3 cleared `<1B`.

### 5. Gate 4: CPU And Runtime Data-Structure Pass

- [ ] Add runtime indexes for occupancy by cell and stack id if scans remain visible in phase timings.
- [ ] Avoid full legal action generation for simple validation paths.
- [ ] Avoid reachability BFS unless the submitted action is `Move` or a read API actually needs move paths.
- [ ] Remove or defer any remaining per-action serialization/checkpointing visible in traces.
- [ ] Run focused Gate K/L and decide whether Gate 4 reached around `0.3B`.

### 6. Benchmark And Regression Discipline

- [ ] Perf Gate 0: record traced baseline with repo-operation and phase attribution.
- [ ] Perf Gate 1: get `submit_battle_action` below 10B average instructions or document the measured blocker and change direction.
- [ ] Perf Gate 2: get `submit_battle_action` below 3B average instructions or document the measured blocker and change direction.
- [ ] Perf Gate 3: get `submit_battle_action` below 1B average instructions or document the measured blocker and change direction.
- [ ] Perf Gate 4: get `submit_battle_action` around 0.3B average instructions.
- [ ] Record before/after method summaries for `submit_battle_action`, `sync_battle`, `get_battle_state`, `get_game_view`, `get_events_after`, and any active runtime event/status APIs.
- [ ] Confirm no missing required endpoints and no benchmark instruction deltas show `n/a` for update methods.
- [ ] Confirm no leftover PocketIC processes after full benchmark runs.
- [ ] Run full benchmark suite only after a meaningful gate is reached or a broad API behavior change lands.

### 7. Broader Aggregate Pattern

- [ ] Review town command paths for the same live-row smell: buildings, recruit pools, and garrison.
- [ ] Review champion command paths for the same live-row smell: army stacks, spells, artifacts, cooldowns, and map position.
- [ ] Review session/world turn sync paths for aggregate or shard opportunities.
- [ ] Pick the next aggregate after battle based on benchmark cost and code complexity.
- [ ] Repeat the same benchmark discipline before and after each aggregate migration.

## Expected Outcome

The first successful battle aggregate checkpoint should reduce `submit_battle_action` by removing repeated stable row/index work. The deeper target requires eliminating almost all per-action stable writes from active battle submit. The likely wins should come from eliminating:

- repeated `BattleStack` list reads
- repeated `BattleObstacle` list reads
- repeated `BattleOccupancy` list reads
- full child-row diff persistence
- redundant battle state reload for readiness
- excessive event/session write amplification
- per-action battle timeout job upserts
- per-action readiness rows
- per-action durable command create/update when runtime receipts can answer active replays/status
- per-action durable event fanout when runtime events can answer active feeds and flush later

The benchmark should prove whether each cut is actually better. If the active aggregate model does not move `submit_battle_action` below 10B average instructions, repo-op and phase tracing should tell us whether the remaining cost is event feed writes, command/idempotency writes, job/readiness writes, serialization, or battle rule CPU. Continue removing the largest measured cost until the endpoint is near `0.3B`.
