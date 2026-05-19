# perf1 Todo: Aggregate Runtime Performance Rewrite

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
- [ ] Add benchmark-only repo operation tracing so each public endpoint call can report table/index operation counts, row counts returned/affected, and operation names.
- [ ] Add benchmark-only phase markers around `submit_battle_action`: auth/context, command begin, recovery, timeout, load/apply/persist, event fanout, readiness/schedule, final response.
- [ ] Run a fresh baseline benchmark after repo-op tracing lands and record the run ID plus `submit_battle_action` table/index counts in this file.

### 1. Battle Aggregate Runtime

- [ ] Define `BattleRuntime` as the active command-side aggregate containing the battle header, stacks, obstacles, occupancy, round/deadline state, and any runtime metadata needed to apply commands.
- [ ] Choose and implement serialization for `BattleRuntime` snapshots. Candid is acceptable unless measurement shows it is too expensive.
- [ ] Add an active battle store keyed by battle id. Prefer heap-resident active state with pre-upgrade/post-upgrade serialization.
- [ ] Add a compact durable checkpoint/snapshot path for active battles. This can be per command, every N commands, or explicit milestone-based if the heap model is enough for the first pass.
- [ ] Implement loader compatibility so existing row-backed battles can be converted into `BattleRuntime` during migration or first access.
- [ ] Implement finalization that projects resolved battle state into durable rows/history/events needed after the battle is over.

### 2. Battle Command Hot Path

- [ ] Change `submit_battle_action` to load/mutate/save one `BattleRuntime` aggregate instead of loading/diffing `BattleStack`, `BattleObstacle`, and `BattleOccupancy` rows repeatedly.
- [ ] Replace full child-row diff persistence with aggregate save/checkpoint semantics.
- [ ] Reuse the already-mutated aggregate for round readiness instead of reloading battle child state.
- [ ] Make battle validation action-specific so attack/defend/wait do not compute full move reachability unless needed.
- [ ] Keep command idempotency and command status durable so replay behavior remains stable.
- [ ] Keep battle event emission durable, but decouple it from child-row persistence.

### 3. Event And Projection Cleanup

- [ ] Batch event fanout so `GameSession.next_event_seq` is updated once per command, not once per audience event.
- [ ] Cache involved battle participants for event fanout instead of reloading champion/town owners once per generated battle event.
- [ ] Decide which battle read APIs should decode the active aggregate directly and which should read a projection.
- [ ] Add or update projections needed by `get_battle_state`, `get_game_view`, and client probes.
- [ ] Ensure public event feed and command status APIs keep their existing behavior.

### 4. Benchmarks And Regression Gates

- [ ] Perf Gate 0: record traced baseline with repo-operation and phase attribution.
- [ ] Perf Gate 1: get `submit_battle_action` below 10B average instructions or document the measured blocker and change direction.
- [ ] Perf Gate 2: get `submit_battle_action` below 3B average instructions or document the measured blocker and change direction.
- [ ] Perf Gate 3: get `submit_battle_action` below 1B average instructions or document the measured blocker and change direction.
- [ ] Perf Gate 4: get `submit_battle_action` around 0.3B average instructions.
- [ ] Run focused Gate K after the first battle aggregate submit path works.
- [ ] Run focused Gate L after first-playable battle flow works through public endpoints.
- [ ] Run full benchmark suite and compare against the baseline run.
- [ ] Record before/after method summaries for `submit_battle_action`, `sync_battle`, `get_battle_state`, `get_game_view`, and `get_events_after`.
- [ ] Confirm no missing required endpoints and no benchmark instruction deltas show `n/a` for update methods.
- [ ] Confirm no leftover PocketIC processes after full benchmark runs.

### 5. Remove Row-Normalized Battle Hot State

- [ ] Stop writing live tactical changes to `BattleStack`, `BattleObstacle`, and `BattleOccupancy` rows during active battle actions unless a projection explicitly requires it.
- [ ] Delete or isolate obsolete row-diff helpers once all active battle callers use `BattleRuntime`.
- [ ] Keep migration or compatibility helpers only where needed for old snapshots/tests.
- [ ] Update tests that asserted row-level live battle internals to assert aggregate/projection behavior instead.

### 6. Broader Aggregate Pattern

- [ ] Review town command paths for the same live-row smell: buildings, recruit pools, and garrison.
- [ ] Review champion command paths for the same live-row smell: army stacks, spells, artifacts, cooldowns, and map position.
- [ ] Review session/world turn sync paths for aggregate or shard opportunities.
- [ ] Pick the next aggregate after battle based on benchmark cost and code complexity.
- [ ] Repeat the same benchmark discipline before and after each aggregate migration.

## Expected Outcome

The first successful battle aggregate checkpoint should reduce `submit_battle_action` by removing repeated stable row/index work. The likely win should come from eliminating:

- repeated `BattleStack` list reads
- repeated `BattleObstacle` list reads
- repeated `BattleOccupancy` list reads
- full child-row diff persistence
- redundant battle state reload for readiness
- excessive event/session write amplification

The benchmark should prove whether the new shape is actually better. If the aggregate model does not move `submit_battle_action` below 10B average instructions, repo-op and phase tracing should tell us whether the remaining cost is event feed writes, command/idempotency writes, Candid serialization, or battle rule CPU.
