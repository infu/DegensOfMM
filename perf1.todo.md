# perf1 Todo: Aggregate Runtime Performance Rewrite

Companion notes and decision log: `perf1.notes.md`.

Implementation plan: `perf1.impl.md`.

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

## Current Strategy Update: Whole-Game Fast Path

`submit_battle_action` proved the architectural direction. The recorded Gate K/L average moved from `26.9860B` to `0.2789B` instructions once active battle submit stopped using normalized IcyDB rows as the live mutation model.

That win is not enough by itself. Real tests and real gameplay must still pass through setup, session/world views, movement, turn sync, towns, champions, economy, jobs, and projections before battle. Optimizing only the battle endpoint now gives diminishing returns because the rest of the game path still spends many billions of instructions moving through row-backed live state.

The new perf1 goal is therefore scenario-level speed, not one endpoint vanity. Use the battle runtime as the proof pattern and modernize the next highest-impact systems quickly.

Priority order:

| priority | target | why now | first target |
| --- | --- | --- | --- |
| 1 | session-turn/champion-movement runtime | Gate J still has `submit_move_intent` around `13.75B` and `sync_session_turn` around `14.73B`; every battle route must pass through movement/turn state | get both below `5B`, then push toward sub-`1B` |
| 2 | setup/session/view projections | tests and client routes repeatedly touch session, participant, champion, visible map/object, and game view reads before gameplay reaches battle | reduce scenario totals and test wall time |
| 3 | town/economy aggregate | town build/recruit commands are still around `16B-21B` and are early gameplay actions | command-side town aggregate with durable projection |
| 4 | champion aggregate | champion state spans army, spells, artifacts, movement, aftermath, map occupancy, mana/status, and town interactions | one command-side champion aggregate/projection contract |
| 5 | remaining battle boundary work | runtime archive flush, `CastAbility`, and final boundary projection still need cleanup | finish after code-size headroom or split/debug surface exists |

Modernization rule:

| smell | default fix |
| --- | --- |
| A hot command reads/writes several IcyDB child rows as its live object model | move the command-side state into a heap aggregate |
| A command writes durable command/event/effect rows only to answer immediate replay/status/feed during active gameplay | answer from runtime receipt/event buffers first, durable-flush later |
| A job row is updated every action to model live state | make the job a wakeup hint; runtime owns the authority |
| A query rehydrates many rows that were just mutated by an active aggregate | merge/read runtime projection first, durable fallback second |
| A micro-cut saves less than the cost of a real aggregate rewrite and does not unblock code size | skip it unless it is already in hand and low risk |

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

## Active Runtime Merge Contract

While a battle is active, `BattleRuntime` is the command-side authority. Durable rows remain shells, indexes, history, projections, or post-resolution state. Public APIs must merge runtime data before falling back to durable rows so the client never sees stale row-backed battle state.

Runtime identity and adoption:

| area | contract |
| --- | --- |
| key | runtime store is keyed by durable `Battle.id` |
| adoption | if an active battle row exists but runtime is missing, hydrate runtime from current `Battle`, `BattleStack`, `BattleObstacle`, and `BattleOccupancy` rows on first access |
| upgrade | `pre_upgrade` serializes active runtimes; `post_upgrade` restores them before job repair/scheduling |
| removal | runtime is removed only after resolution/finalization has projected required durable state and history |

Endpoint merge rules:

| endpoint/path | active-runtime behavior | fallback |
| --- | --- | --- |
| `get_battle_state` | build the `BattleView` from runtime `BattleState` and runtime metadata; include current active stack/round/deadline/readiness from runtime | row-backed `Battle` plus child rows when no runtime exists |
| `submit_battle_action` | validate idempotency against runtime receipts first, mutate runtime, append runtime events, update runtime readiness/deadline/round state, and return from runtime receipt on replay | hydrate runtime from rows if missing, then use runtime path |
| `sync_battle` | apply due runtime timeouts/auto-round actions, finalize if resolved, and return runtime-derived changed subjects/events | hydrate runtime if the active row exists but runtime is missing |
| `end_battle_turn` | mark runtime readiness for the participant/round and advance runtime round when ready; do not make `BattleParticipantRoundReady` rows authoritative for active battles | hydrate runtime if needed; resolved/missing battle uses durable error/fallback path |
| `get_events_after` | merge durable `GameEvent` rows with runtime active events whose `event_seq` is greater than `after_event_seq`; sort by sequence and avoid duplicates after flush | durable events only when no active runtime events exist for the session/audience |
| `get_command_status` | check active runtime command receipts by command id before durable `GameCommand` rows | durable command rows |
| `get_command_status_by_nonce` | check active runtime command receipts by participant/session/client nonce before durable idempotency rows | durable command/lobby rows |
| timeout jobs | job wakeups load/adopt runtime and ask runtime deadline state what work is due; job rows are wakeup hints, not active battle authority | legacy row-backed timeout only for active rows that cannot hydrate |
| round jobs | job wakeups load/adopt runtime and advance runtime auto-ready/auto-defend state; readiness rows are not active battle authority | legacy row-backed round advance only for active rows that cannot hydrate |
| aftermath | finalization must either project runtime survivor stack rows before calling existing aftermath or call a rewritten aftermath function that consumes runtime survivors directly | durable survivor rows after runtime finalization |

Event and command sequencing:

| topic | contract |
| --- | --- |
| event sequence | runtime owns a session event sequence cursor for active battle events; durable `GameSession.next_event_seq` is advanced once per flush/finalization batch, not once per active event |
| active event IDs | runtime events carry deterministic keys derived from battle id, command id, audience key, and runtime event sequence |
| command receipt | runtime receipt stores command id, nonce, actor participant, status, result/error, retryability, changed subjects, event references, and payload hash |
| replay | replay with the same nonce/payload returns the runtime receipt without remutating state; same nonce/different payload returns the existing idempotency error |
| flush | flushing command/event history to durable rows must be idempotent and must not duplicate events already visible through runtime merge |

Safety rules:

| rule | implication |
| --- | --- |
| active runtime wins | active API reads must never synthesize state from stale child rows when runtime exists |
| traps are rollback | per-action heap changes can rely on IC atomic rollback; durable writes during the same update must still be ordered so a trap cannot leave projected state ahead of runtime state |
| projection is explicit | tactical child rows are projections/snapshots after Gate 1, not the active mutation model |
| missing runtime is recoverable | hydrate from rows first, and only fail if both runtime and durable active rows are inconsistent |
| benchmark gates decide durability | if command/event/job durable writes still dominate after Gate 1, Gate 2/3 move those responsibilities into runtime too |

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

## Traced Baseline: submit_battle_action

Traced source:

```text
run id: 20260519-002234-3dfd9a4
suite: target/benchmarks/20260519-002234-3dfd9a4/suite-summary.md
gate-k: target/benchmarks/20260519-002234-3dfd9a4/gate-k/summary.json
gate-l: target/benchmarks/20260519-002234-3dfd9a4/gate-l/summary.json
git sha: 3dfd9a4
```

The traced average is effectively unchanged from the pinned baseline, so the attribution hooks are good enough for Gate 1 decisions.

| source | calls | avg instructions | p95 instructions | avg memory delta | avg cycles |
| --- | ---: | ---: | ---: | ---: | ---: |
| Gate K | 25 | 27.4636B | 28.7972B | 173.88 MB | 0.0275T |
| Gate L | 26 | 26.5264B | 28.8148B | 156.41 MB | 0.0265T |
| combined | 51 | 26.9858B | 28.8148B | 164.97 MB | 0.0270T |

Top combined `submit_battle_action` phases:

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

Trace conclusion: the next cut should remove repeated active battle row loads/persistence and readiness/job/event fanout from the per-action path. `apply_rules` and `validate_action` are tiny in this baseline, so the first win is storage shape, not pure battle CPU.

## Testing Policy

Roll fast here. Use focused tests during implementation and full benchmark/regression runs only at meaningful checkpoints. The goal is not to run every slow PocketIC route after every edit; the goal is to use small compile checks and focused gates while architecture is moving, then spend the full-suite cost only when a broad checkpoint is worth measuring.

| checkpoint type | minimum testing |
| --- | --- |
| doc/planning only | no tests required |
| compile-affecting refactor | `cargo check` or targeted package test |
| battle aggregate checkpoint | Gate K or Gate L targeted PocketIC test |
| movement/session-turn aggregate checkpoint | Gate J focused PocketIC test first |
| town/economy aggregate checkpoint | endpoint-surface or focused route that exercises build/recruit |
| query/projection checkpoint | endpoint-surface plus the smallest route that reads the affected projection |
| benchmark claim | full `scripts/run-benchmarks.sh` |
| broad aggregate pattern change | Gate J/K/L, and Gate M only if API/client behavior may change |

Parallel policy:

| test group | parallelization rule |
| --- | --- |
| independent PocketIC gates | run with separate lock namespaces and output dirs when measuring a stable checkpoint |
| Rust package checks/tests | run independent package checks in parallel when they do not share target-dir contention badly |
| full benchmark suite | use `DOMM_BENCH_JOBS` and keep it for meaningful gates, not every micro-edit |
| slow recovery regressions | keep as focused checkpoint tests, not part of every inner loop |

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
- [x] Run a fresh traced baseline and record the run ID plus `submit_battle_action` phase/repo counts in this file and `perf1.notes.md`.
- [x] Document the active runtime merge contract for `get_battle_state`, `sync_battle`, `end_battle_turn`, `get_events_after`, `get_command_status`, `get_command_status_by_nonce`, timeout jobs, round jobs, and aftermath.

### 1. Active Battle Runtime Authority

- [x] Define the first `BattleRuntime` as `domm_game::BattleState` plus canister runtime metadata: session id, battle id, participant audience keys, command receipts, active event buffer, readiness set, deadline state, and session event sequence cursor.
- [x] Add a heap active battle store keyed by battle id.
- [x] Add `pre_upgrade`/`post_upgrade` serialization for active battles using a dedicated memory slot that does not collide with IcyDB memory IDs.
- [x] Create/adopt active runtime during battle start paths before the first player action, with compatibility hydration from existing row-backed active battles.
- [x] Implement loader compatibility so existing row-backed battles can be converted into `BattleRuntime` during migration or first access.
- [x] Make `get_battle_state` read active runtime directly and fall back to legacy rows only when runtime is absent.
- [x] Decide whether active runtime events use a heap session event sequence overlay or a stable batch reservation; prevent collisions with stable `GameSession.next_event_seq`.
- [x] Implement battle finalization that projects resolved runtime state into durable strategic rows/history/events needed after the battle is over.
- [x] Ensure finalization either projects survivor `BattleStack` rows before calling existing aftermath or rewrites aftermath to read survivors from runtime.

### 2. Gate 1: Remove Tactical Child-Row Hot Writes

- [x] Change non-spell active `submit_battle_action` to load/mutate/commit one heap `BattleRuntime` aggregate instead of loading/diffing `BattleStack`, `BattleObstacle`, and `BattleOccupancy` rows repeatedly.
- [x] Stop persisting active tactical changes to `BattleStack`, `BattleObstacle`, and `BattleOccupancy` rows per non-spell player action, timeout auto-defend, and round auto-defend.
- [x] Resolve `auto:enemy` target normalization from active runtime instead of loading full row-backed state.
- [x] Reuse the already-mutated aggregate for round readiness instead of reloading battle child state.
- [x] Make battle validation action-specific so attack/defend/wait do not compute full move reachability unless needed.
- [x] Keep current durable command/event behavior only long enough to measure the Gate 1 delta.
- [x] Move `CastAbility` off the row-backed tactical persist path or explicitly benchmark/document it as a rare fallback. Decision: keep it as a documented rare fallback for now because the full suite does not exercise `CastAbility`, the client probe avoids it, and the spell path also mutates champion mana/spell/effect rows.
- [x] Run focused Gate K or a shorter battle benchmark and decide whether Gate 1 cleared `<10B`. Gate K now passes and the active runtime event archive checkpoint brought `submit_battle_action` to 9.3621B avg, so the first `<10B` target is cleared. The remaining blocker is stable command/header/readiness/auth work, not tactical child-row mutation.

### 3. Gate 2: Remove Per-Action Battle Job/Readiness/Header Writes

- [x] Move submit-time auto-ready bookkeeping to `BattleRuntime.ready_participants` and use runtime readiness for active submit/round-job checks, while preserving row-backed `end_battle_turn` compatibility for now.
- [x] Route submit-time timeout checks through active runtime and remove the redundant submit response session reload.
- [x] Try the active `BattleRuntime` submit path before loading the durable `Battle` row, falling back to row load/adoption only when runtime is absent or the action needs legacy handling.
- [x] Move battle readiness state into `BattleRuntime` for active battles; stop writing `BattleParticipantRoundReady` rows per battle action.
- [x] Move active battle deadlines/timeouts into `BattleRuntime`; stop upserting `SystemJob` rows on every battle action.
- [x] Stop projecting the durable `Battle` row header for active non-spell player submissions; runtime is authoritative for active round/stack/deadline until sync/finalization projection.
- [x] Stop updating the `Battle` row header for active round/active stack/deadline changes; update the durable `Battle` row at start/end or explicit projection points. Active timeout sync, timeout timer jobs, and round-advance jobs now read active round/stack/deadline from `BattleRuntime`; durable header projection is left for row-backed fallback and finalization/checkpoint paths.
- [x] Replace readiness recompute with runtime alive/acted tracking; avoid `legal_actions_for_stack` during active runtime readiness except for a measured edge case. The row-backed fallback still uses the legacy legal-action scan.
- [x] Move `end_battle_turn` onto runtime readiness so manual readiness does not keep `BattleParticipantRoundReady` hot. Active runtime battles now mark manual ready state in `BattleRuntime.ready_participants`; durable `GameCommand`/public event behavior stays for this checkpoint because the full runtime receipt/event version exceeded the IC Wasm code-section limit.
- [x] Keep enough scheduling behavior to make timeout/round advancement work from heap state.
- [x] Run focused Gate K/L and decide whether Gate 2 cleared `<3B`. Focused Gate K `20260519-055234-gate-k-runtime-timeout-hints` passed with `submit_battle_action` at 2.2481B avg, so Gate 2 is cleared.

### 4. Gate 3: Move Active Battle Commands And Events Out Of Per-Action Stable Writes

- [x] Add active battle command receipt/idempotency storage inside `BattleRuntime` for battle actions.
- [x] Make replay of an active battle action return from runtime command receipts without durable `GameCommand` lookup/create/update.
- [x] Make `get_command_status` and `get_command_status_by_nonce` merge active runtime command receipts before falling back to durable `GameCommand` rows.
- [x] Store active battle events in runtime and make `get_events_after` merge active runtime events with durable `GameEvent` rows.
- [x] Precompute/use participant audience keys in runtime so active event fanout does not load champion/town owners per event.
- [x] Keep resolved-battle runtime events visible through an in-memory session archive after runtime removal; defer bulk durable event flushing because a one-shot finalization flush exceeded the 40B single-message limit.
- [ ] Flush or project runtime command/event data to durable rows in bounded batches at battle resolution, explicit checkpoint, or upgrade as needed for history/debugging. Blocked for now by the IC Wasm code-section limit: the full command/event job, a slimmer command/event checkpoint, and an event-only checkpoint all exceeded the limit. Keep runtime command receipts/events archived in heap and upgrade snapshot until code size is reduced or this moves to a split/debug canister.
- [x] Batch or avoid `GameSession.next_event_seq` durable updates during active battle commands by reserving active event sequence blocks.
- [x] Run focused Gate K/L and decide whether Gate 3 cleared `<1B`. The command-receipt checkpoint alone did not clear Gate 3, but the later two-slot active auth cache run `20260519-062456-gate-k-two-slot-auth-cache` measured `submit_battle_action` at 0.2846B avg, so Gate 3 is now cleared.

### 5. Gate 4: CPU And Runtime Data-Structure Pass

- [x] Add runtime indexes for occupancy by cell and stack id if scans remain visible in phase timings. Decision: no runtime indexes needed yet; focused Gate K `20260519-071345-5515805-gate-k-runtime-readiness` still shows `load_battle_state` at 0.0002B and `apply_rules` at 0.0003B avg.
- [x] Avoid full legal action generation for simple validation paths.
- [x] Avoid reachability BFS unless the submitted action is `Move` or a read API actually needs move paths.
- [x] Add a narrow two-slot active session caller cache for active runtime battle submits so auth/session lookup is not a stable-read tax on every action.
- [x] Remove or defer any remaining per-action serialization/checkpointing visible in traces. Focused Gate K `20260519-071345-5515805-gate-k-runtime-readiness` shows `persist_battle_state` at 0B avg and active submit memory delta at about 0.0025 MB.
- [x] Run focused Gate K/L and decide whether Gate 4 reached around `0.3B`. Focused Gate K `20260519-062456-gate-k-two-slot-auth-cache` measured `submit_battle_action` at 0.2846B avg.

### 6. Benchmark And Regression Discipline

- [x] Perf Gate 0: record traced baseline with repo-operation and phase attribution.
- [x] Perf Gate 1: get `submit_battle_action` below 10B average instructions or document the measured blocker and change direction. The measured blocker is now durable command/event/session/aftermath work after tactical row writes were reduced; continue into Gate 2/3 cuts.
- [x] Perf Gate 2: get `submit_battle_action` below 3B average instructions or document the measured blocker and change direction. Gate K `20260519-055234-gate-k-runtime-timeout-hints` measured 2.2481B avg.
- [x] Perf Gate 3: get `submit_battle_action` below 1B average instructions or document the measured blocker and change direction. Gate K `20260519-062456-gate-k-two-slot-auth-cache` measured 0.2846B avg.
- [x] Perf Gate 4: get `submit_battle_action` around 0.3B average instructions. Gate K `20260519-062456-gate-k-two-slot-auth-cache` measured 0.2846B avg.
- [x] Record before/after method summaries for `submit_battle_action`, `sync_battle`, `get_battle_state`, `get_game_view`, `get_events_after`, and any active runtime event/status APIs.
- [x] Confirm no missing required endpoints and no benchmark instruction deltas show `n/a` for update methods. Added an `endpoint-surface` benchmark gate that records all 59 required public endpoints; suite `20260519-081911-fe84689` covered `59/59` endpoints with no update or query method missing `avg_instruction_delta`. `Inst change` can still be `n/a` on a first comparable run, which is expected.
- [x] Confirm no leftover PocketIC processes after focused benchmark runs.
- [x] Run full benchmark suite only after a meaningful gate is reached or a broad API behavior change lands. Suite `20260519-081911-fe84689` passed endpoint-surface plus Gate J/K/L/M in parallel with `DOMM_BENCH_JOBS=5`.

### 7. Whole-Game Fast-Path Strategy

- [x] Change perf1 strategy from `submit_battle_action`-only optimization to whole-game path modernization after the battle runtime reached the `0.3B` target.
- [x] Treat the battle runtime result as architectural proof: command-side heap aggregates for hot live state, IcyDB for projections/history/fallback/boundaries.
- [x] Rank aggregate work by scenario-level cost and route criticality before each major checkpoint, not by which endpoint has the nicest isolated number. Current order is session-turn/champion-movement runtime, setup/session/view projections, town/economy aggregate, champion aggregate, then remaining battle boundary work; see `perf1.impl.md`.
- [x] Prefer broad aggregate rewrites over more row-level movement/town/champion micro-cuts unless a micro-cut is trivial, already in hand, or frees code-size headroom. `perf1.impl.md` treats the in-progress `movement_intent` effect removal as the last optional row-level movement micro-cut before runtime work.
- [x] Keep full benchmark runs for major architecture gates; use focused gates and compile checks for the fast edit loop. `perf1.impl.md` records the focused Gate J and targeted regression matrix.
- [x] Review town command paths for the same live-row smell: buildings, recruit pools, and garrison. Town build/recruit still reads and writes `TownBuilding`, `TownRecruitPool`, and `TownGarrisonStack` rows directly; `get_town_view` is now real-row-backed, so the API is truthful but the command path remains an aggregate candidate.
- [x] Review champion command paths for the same live-row smell: army stacks, spells, artifacts, cooldowns, and map position. Champion state is spread across `Champion`, `ChampionArmyStack`, `ChampionSpell`, artifact/equipment rows, map occupancy, battle aftermath, and movement updates.
- [x] Review session/world turn sync paths for aggregate or shard opportunities. `submit_move_intent` and `sync_session_turn` remain high-cost, high-frequency row workflows around movement intents, participants, champions, occupancy, turn-ready rows, system jobs, income, and session updates.
- [x] Pick the next aggregate after battle based on benchmark cost and code complexity. Next target should be a session-turn/champion-movement aggregate because full-suite Gate K/L show `submit_move_intent` around 15.4-15.6B and `sync_session_turn` around 19.3B, both more frequent than town commands and now far above active battle submit.
- [x] Define a setup/query projection pass if Gate J/K/L still spend too much time before reaching the core gameplay endpoint being optimized. `perf1.impl.md` lists runtime overlays for `get_game_view`, events/status, participant/champion/object/map views, and `preview_move_path`.
- [x] Define a town/economy aggregate once session-turn movement is no longer the dominant route cost, or sooner if endpoint-surface/full-suite shows town dominates setup. `perf1.impl.md` sequences `EconomyRuntime` before `TownRuntime` after champion overlay work.
- [x] Define a champion aggregate/projection contract that movement, aftermath, town, spells, and views can share. `perf1.impl.md` defines current `SessionTurnRuntime` champion deltas and defers full champion ownership to `ChampionOverlay`/`ChampionRuntime`.
- [ ] Repeat the same benchmark discipline before and after each aggregate migration.

### 8. Gate 5: Session-Turn / Champion-Movement Runtime

- [x] Anchor the next aggregate baseline with the current full-suite and focused Gate J numbers. Baseline suite `20260519-081911-fe84689` measured Gate J `submit_move_intent` at 15.363B avg and `sync_session_turn` at 18.4665B avg; Gate K/L measured `sync_session_turn` around 19.3B avg.
- [x] Apply a first code-size-safe movement checkpoint: serialize submit movement path text once per `submit_move_intent`, and skip the caller participant reload on partial `sync_session_turn` responses that return before income/turn advancement.
- [x] Run focused Gate J after the checkpoint and compare against baseline. Run `20260519-085429-movement-small-gate-j` passed; scenario instructions moved 404.0368B -> 399.1517B, `sync_session_turn` moved 18.4665B -> 18.0194B avg, and `sessions.load_participant` repo calls moved 15 -> 8. `submit_move_intent` stayed essentially flat at 15.3598B avg.
- [x] Record the immediate code-size blockers before larger movement/runtime work. Benchmark phase attribution failed to install at code section 12,603,857 bytes, 20,945 over the IC limit; the indexed pending-intent lookup checkpoint failed at 12,598,630 bytes, 15,718 over the limit.
- [x] Free at least about 20 KB of benchmark Wasm code section, or move enough benchmark/debug surface out of the main canister, before adding new movement runtime/query instantiations. The benchmark canister now omits diagnostic system-job control endpoints that the benchmark gates do not use; measured code section is 12,547,075 bytes with 35,837 bytes of headroom, freeing 34,338 bytes versus the prior benchmark build.
- [x] Re-attempt indexed pending movement loading, or replace active turn movement with a heap session-turn runtime, once the code-size headroom exists. Indexed pending movement loading now fits and Gate J `20260519-090940-movement-index-gate-j` passed; `sync_session_turn` moved 18.0194B -> 17.5063B avg versus the previous small checkpoint and the route exposed `movement.intents_by_session_turn_status` as a measured repo op. Benchmark Wasm code section is 12,564,294 bytes with 18,618 bytes of headroom.
- [x] Apply the first fresh-submit command/effect shortcut for movement intent commands. Fresh `submit_move_intent` commands now create their `CommandEffect` row directly instead of first proving absence through `effects.command_effect_by_command_key`; replay/recovery still uses the idempotent path. Focused Gate J `20260519-095656-movement-effect-fresh-gate-j` passed with query instruction logging restored; versus indexed baseline `20260519-090940-movement-index-gate-j`, `submit_move_intent` moved 15.3572B -> 14.6615B avg (-4.5%), scenario instructions moved 393.4643B -> 391.3873B (-0.5%), and effect lookup calls moved 18 -> 15. An attempted active-session caller cache for movement was rejected after Gate J exposed stale `GameSession.next_event_seq` risk. Benchmark Wasm code section is 12,569,998 bytes with 12,914 bytes of headroom.
- [x] Apply the fresh movement event append shortcut for brand-new movement intents. Fresh `submit_move_intent` calls now skip the `events.by_session_event_key` absence read only when both the command and champion-turn `MovementIntent` row were just created; replays, replacements, and recovery still use the idempotent event lookup path. Focused Gate J `20260519-100606-movement-new-event-gate-j` passed; versus `20260519-095656-movement-effect-fresh-gate-j`, `submit_move_intent` moved 14.6615B -> 14.1924B avg (-3.2%), scenario instructions moved 391.3873B -> 389.8865B (-0.4%), and event lookup calls moved 22 -> 20. Benchmark Wasm code section is 12,570,668 bytes with 12,244 bytes of headroom.
- [x] Move early `sync_session_turn` `turn_not_due` responses before durable command creation. Focused Gate J `20260519-101910-movement-sync-precheck-gate-j` passed; versus `20260519-100606-movement-new-event-gate-j`, `sync_session_turn` moved 17.5045B -> 17.0512B avg (-2.6%), scenario instructions moved 389.8865B -> 384.8650B (-1.3%), early not-due sync calls moved about 5.19B -> 3.52B each, and total stable memory moved 6,462.4 MB -> 6,358.3 MB (-1.6%). Stale-turn regression `pocket_ic_end_turn_closes_turn_and_blocks_stale_actions` passed. Benchmark Wasm code section is 12,571,456 bytes with 11,456 bytes of headroom.
- [x] Apply the fresh `sync_session_turn` command/effect shortcut. Fresh manual sync commands now create the turn-resolution effect directly; seeded/recovered pending sync commands and system-job sync commands still use the idempotent effect lookup path. Focused Gate J `20260519-102824-movement-sync-effect-gate-j` passed; versus `20260519-101910-movement-sync-precheck-gate-j`, `sync_session_turn` moved 17.0512B -> 16.5509B avg (-2.9%), scenario instructions moved 384.8650B -> 379.3951B (-1.4%), and total `effects.command_effect_by_command_key` calls moved 15 -> 7. Service regression `lobby_session_setup_recovers_from_starting_state_and_replays_nonce` passed. Benchmark Wasm code section is 12,572,384 bytes with 10,528 bytes of headroom.
- [x] Replace applied-sync nearest-job scans with direct timer refresh for rescheduled current-turn jobs, and skip the redundant nearest-job scan when completing the current turn before scheduling next-turn jobs. Focused Gate J `20260519-103739-movement-job-reschedule-gate-j` passed; versus `20260519-102824-movement-sync-effect-gate-j`, `sync_session_turn` moved 16.5509B -> 15.3974B avg (-7.0%), scenario instructions moved 379.3951B -> 366.6428B (-3.4%), and global `system_jobs.by_status_due`/`system_jobs.by_status_lease` calls moved 11/11 -> 2/2. Stale-turn regression `pocket_ic_end_turn_closes_turn_and_blocks_stale_actions` passed. Benchmark Wasm code section is 12,575,951 bytes with 6,961 bytes of headroom.
- [x] Narrow the pre-deadline map-turn command guard to scheduled session jobs only. Before the turn deadline, new movement/town/economy commands now avoid the redundant running-job scan while still blocking due scheduled turn-resolution/deadline jobs; at or after the deadline the old running+scheduled guard remains. Focused Gate J `20260519-110341-movement-scheduled-guard-gate-j` passed; versus `20260519-103739-movement-job-reschedule-gate-j`, `submit_move_intent` moved 14.2054B -> 13.7361B avg (-3.3%), scenario instructions moved 366.6428B -> 365.2604B (-0.4%), and `system_jobs.by_session_status_due` calls moved 52 -> 48. Service regression `lobby_session_setup_recovers_from_starting_state_and_replays_nonce` passed. Benchmark Wasm code section is 12,576,504 bytes with 6,408 bytes of headroom.
- [x] Free benchmark Wasm headroom before the next movement cuts by turning benchmark phase markers into pass-through wrappers while keeping public method and repo-operation metrics. Phase summaries were useful for the original battle investigation, but the current movement work is driven by method and repo-op metrics. Benchmark Wasm code section is now 12,564,045 bytes with 18,867 bytes of headroom, freeing 12,459 bytes versus the scheduled guard checkpoint.
- [x] Remove redundant `CommandEffect` projection writes from movement snapshots. `MovementSnapshot` rows already provide the durable projection and idempotent `(command_id, intent_id, step_index)` replay guard, so `record_movement_snapshot` no longer writes a duplicate `movement_snapshot` command effect. Focused Gate J `20260519-112144-movement-snapshot-effect-gate-j` passed; versus `20260519-110341-movement-scheduled-guard-gate-j`, scenario instructions moved 365.2604B -> 361.7712B (-1.0%), `sync_session_turn` moved 15.3986B -> 15.0796B avg (-2.1%), `effects.command_effect_by_command_key` calls moved 7 -> 4, and `effects.create_applied_command_effect` calls moved 18 -> 15. Service regression `lobby_session_setup_recovers_from_starting_state_and_replays_nonce` passed.
- [x] Remove the top-level `turn_resolution` `CommandEffect` from `resolve_pending_movement`. Command idempotency, movement snapshot idempotency, event keys, and intent status already guard replay/recovery, so the extra effect row was redundant. Focused Gate J `20260519-113052-movement-turn-effect-gate-j` passed; versus `20260519-112144-movement-snapshot-effect-gate-j`, scenario instructions moved 361.7712B -> 357.9811B (-1.0%), `sync_session_turn` moved 15.0796B -> 14.7341B avg (-2.3%), scenario memory moved 6,326.1875 MB -> 6,190.125 MB (-2.2%), and `effects.create_applied_command_effect` calls moved 15 -> 7. Service regression `lobby_session_setup_recovers_from_starting_state_and_replays_nonce` passed. Benchmark Wasm code section is 12,560,886 bytes with 22,026 bytes of headroom.
- [x] Decide whether to keep, finish, or abandon the in-progress `movement_intent` command-effect deletion as the last row-level movement micro-cut before the runtime rewrite. Decision: keep and finish it. `submit_move_intent` no longer writes the redundant `movement_intent` `CommandEffect`, and the now-unused direct create helper was removed. Focused Gate J `20260519-123422-movement-intent-effect-gate-j` passed; versus `20260519-113052-movement-turn-effect-gate-j`, `submit_move_intent` moved 13.7537B -> 13.2749B avg instructions (-3.5%) and 53.4375 MB -> 21.4375 MB avg memory (-59.9%). `sync_session_turn` was unchanged at 14.7336B avg. The direct run did not persist query instruction logs, so no query/scenario instruction claim is made from this artifact.
- [x] Design `SessionTurnRuntime`: active movement intents, participant readiness, champion deltas, occupancy deltas, event buffer, command receipts, deadline/job hints, query merge, upgrade/adoption, and durable boundary projection. Consolidated implementation plan is in `perf1.impl.md`.
- [x] Add the inert `SessionTurnRuntime` heap module with runtime keys, active-turn store helpers, ready state, movement intents, command receipts, event buffers, delta containers, snapshot/restore helpers, and unit tests. This is behavior-neutral scaffolding for the runtime rewrite; verified with `cargo fmt --check`, `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`, and `cargo check -p domm-degens-canister --features benchmark`.
- [x] Reuse the already-paged active participant rows while loading pending movement intents for `sync_session_turn`, removing the per-intent `sessions.load_participant` stable read. This is a direct low-hanging row-read cut; verified with `cargo fmt --check`, `cargo check -p domm-degens-canister --features benchmark`, and `cargo test -p domm-degens-canister -- --nocapture`. Measurement is deferred to the next batched Gate J run because the last direct focused Gate J took `689.81s`.
- [x] Wire session-turn runtime event and command-status overlays into `get_events_after`, `get_command_status`, and `get_command_status_by_nonce` while the runtime is still empty. This is behavior-neutral API plumbing so the next movement runtime patch can expose receipts/events without durable rows; verified with `cargo fmt --check`, `cargo check -p domm-degens-canister --features benchmark`, and `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`.
- [x] Make active `submit_move_intent` use runtime receipts/events first while keeping the durable `MovementIntent` projection bounded for current `sync_session_turn` compatibility. Runtime receipts now handle nonce replay/status, runtime events feed `get_events_after`, and event sequence allocation uses a 4,096-slot stable reservation. Focused Gate J `20260519-132615-movement-runtime-submit-gate-j` passed; versus `20260519-123422-movement-intent-effect-gate-j`, `submit_move_intent` moved 13.2749B -> 11.0412B avg instructions (-16.8%) and 21.4375 MB -> 10.8333 MB avg memory (-49.5%). Scenario instructions moved 287.2952B -> 275.6421B (-4.1%), and `sync_session_turn` moved 14.7336B -> 14.2859B avg (-3.0%). Benchmark-only diagnostic row counting was trimmed to keep the benchmark Wasm installable after two failed code-size attempts.
- [x] Make row-backed `sync_session_turn` source pending movement intents from `SessionTurnRuntime` when the active runtime has submit-populated or hydrated intents, and mirror partial/resolved intent updates back into the runtime. This is a low-hanging bridge cut before full runtime sync; it removes many repeated `movement.intents_by_session_turn_status` scans for active runtime turns while one-time runtime creation hydrates pre-existing durable pending intents for compatibility. Verified with `cargo fmt --check`, `cargo check -p domm-degens-canister --features benchmark`, `cargo check -p domm-degens-canister`, `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`, native PocketIC `pocket_ic_timer_jobs_deadline_resolves_multistep_movement_without_sync` in `193.27s`, and focused Gate J `20260519-135856-movement-runtime-pending-gate-j`. Versus `20260519-132615-movement-runtime-submit-gate-j`, `movement.intents_by_session_turn_status` calls moved 16 -> 6 (-62.5%), total instructions moved 5.6276B -> 2.1084B (-62.5%), `sync_session_turn` moved 14.2859B -> 13.8411B avg (-3.1%), and scenario instructions moved 275.6421B -> 272.1560B (-1.3%). Submit moved 11.0412B -> 11.5097B avg (+4.2%) from one-time runtime hydration, so the next cut should avoid submit-side hydration cost or make sync fully runtime-owned.
- [x] Reject the broad fresh `sync_session_turn` event shortcut and restore idempotent event appends. Focused Gate J `20260519-142130-fresh-sync-events-gate-j` failed after `617.83s` because fresh sync commands can still emit an existing `movement_sync_incomplete` business event key, so skipping `events.by_session_event_key` made `events.create_game_event` hit the unique event constraint. Keep event-key absence reads on this path until the full runtime sync/event buffer owns event identity.
- [x] Evaluate and reject a runtime ready-set cache for pre-deadline `sync_session_turn` checks. The eager version passed Gate J `20260519-144234-runtime-ready-shortcut-gate-j` and cut `sync_session_turn` 13.8411B -> 13.5809B avg, but raised `submit_move_intent` 11.5097B -> 11.9766B by moving ready hydration onto submit. The lazy version passed Gate J `20260519-145905-runtime-ready-lazy-gate-j` but was effectively flat versus baseline: scenario 272.1560B -> 272.1154B and repo counts unchanged. Do not keep this micro-cut; use a real active turn aggregate instead.
- [x] Collapse current-turn job reschedule/complete scans to one session job page plus Rust-side status/kind/turn filtering. Native PocketIC `pocket_ic_timer_jobs_deadline_resolves_multistep_movement_without_sync` passed in `187.76s`; focused Gate J `20260519-151917-current-turn-job-page-gate-j` passed in `684.00s`. Versus `20260519-135856-movement-runtime-pending-gate-j`, `system_jobs.by_session_status_due` calls moved 48 -> 32 (-33.3%), total job-scan instructions moved 16.8968B -> 11.2665B (-33.3%), `sync_session_turn` moved 13.8411B -> 13.3267B avg (-3.7%), and scenario instructions moved 272.1560B -> 266.5048B (-2.1%).
- [x] Collapse the post-deadline map-turn command guard to one session job page while preserving the same running/scheduled closure-job blocking rules. Native PocketIC `pocket_ic_end_turn_closes_turn_and_blocks_stale_actions` passed in `189.92s`; focused Gate J rerun `20260519-153948-map-turn-guard-page-gate-j-rerun` passed in `567.83s` after one PocketIC instance flake. Versus `20260519-151917-current-turn-job-page-gate-j`, `system_jobs.by_session_status_due` calls moved 32 -> 26 (-18.8%), total job-scan instructions moved 11.2665B -> 9.1537B (-18.8%), `submit_move_intent` moved 11.5099B -> 11.2709B avg (-2.1%), and scenario instructions moved 266.5048B -> 264.4034B (-0.8%).
- [x] Hydrate runtime pending movement entries with the active champion and participant rows, and make `sync_session_turn` build `PendingMovement` directly from runtime when the active turn runtime is complete. This keeps the durable fallback for incomplete/runtime-missing cases but avoids repeated participant scans and champion loads during active runtime sync. Verified with `cargo fmt --check`, `cargo check -p domm-degens-canister --features benchmark`, `cargo check -p domm-degens-canister`, `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`, native PocketIC `pocket_ic_timer_jobs_deadline_resolves_multistep_movement_without_sync` in `187.19s`, and focused Gate J `20260519-160042-runtime-hydrated-pending-gate-j` in `681.74s`. Versus `20260519-153948-map-turn-guard-page-gate-j-rerun`, `sessions.participants_by_session_status` calls moved 36 -> 22 (-38.9%), `champions.load_champion` calls moved 10 -> 3 (-70.0%), `sync_session_turn` moved 13.3287B -> 12.4249B avg (-6.8%), and scenario instructions moved 264.4034B -> 254.4790B (-3.8%). `submit_move_intent` stayed flat at 11.2709B -> 11.2759B.
- [x] Add a pre-deadline runtime empty-ready shortcut for `sync_session_turn`, with `end_turn` mirroring participant ready state into the active runtime. This avoids durable participant/ready scans when the active runtime proves no one has ended turn yet, while falling back to the durable all-ready check as soon as any ready participant exists or no runtime exists. Verified with `cargo fmt --check`, `cargo check -p domm-degens-canister --features benchmark`, `cargo check -p domm-degens-canister`, `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`, native PocketIC `pocket_ic_end_turn_closes_turn_and_blocks_stale_actions` in `184.32s`, and focused Gate J `20260519-162235-runtime-empty-ready-gate-j` in `230.20s`. Versus `20260519-160042-runtime-hydrated-pending-gate-j`, update-only Gate J instructions moved 254.4790B -> 251.6616B (-1.1%), `sync_session_turn` moved 12.4249B -> 12.1704B avg (-2.0%), `sessions.participants_by_session_status` calls moved 22 -> 18 (-18.2%), and `turn_ready.by_session_turn` calls moved 6 -> 2 (-66.6%). Full scenario totals are not compared here because the new run restored query instruction capture with absolute benchmark paths, while the previous direct run had query totals recorded as zero.
- [x] Use the active turn runtime's hydrated champion for `submit_move_intent` when it can prove the champion id, session, owner, and `active` status, otherwise fall back to the durable champion load. This keeps stale battle/aftermath cases on the durable path while avoiding one hot same-turn submit load in Gate J. Verified with `cargo fmt --check`, `cargo check -p domm-degens-canister --features benchmark`, `cargo check -p domm-degens-canister`, `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`, and focused Gate J `20260519-163253-runtime-champion-submit-gate-j` in `229.75s`. Versus `20260519-162235-runtime-empty-ready-gate-j`, `champions.load_champion` calls moved 3 -> 2 (-33.3%), `submit_move_intent` moved 11.2728B -> 11.0394B avg (-2.1%), and full scenario instructions moved 320.8642B -> 320.1759B (-0.2%).
- [x] Revert the runtime champion submit lookup after `lobby_session_setup_recovers_from_starting_state_and_replays_nonce` exposed stale cached champion state: a spell/skill update made after movement was not reflected when a later movement intent started a battle, so `CastAbility` disappeared from legal actions. Correctness wins over the small Gate J submit gain; active turn runtime may cache champions again only with an invalidation/version contract or full champion overlay. Verified with `cargo fmt --check`, `cargo check -p domm-degens-canister --features benchmark`, the recovery regression in `241.60s`, and focused Gate J `20260519-town-events-champion-revert-gate-j` in `232.32s`. Current Gate J versus `20260519-164028-final-sync-new-event-gate-j`: scenario instructions still moved 319.4200B -> 317.2806B (-0.7%) from the town event cut, while `submit_move_intent` returned to 11.2751B avg (+2.1%) because the durable champion load is back.
- [x] Skip the final `session_turn_synced` event-key absence read on successful fresh turn advancement by creating the deterministic event first and only falling back to lookup on conflict. This is scoped to the final sync event after `current_turn` is incremented; partial movement sync events keep the idempotent lookup path. Verified with `cargo fmt`, `cargo check -p domm-degens-canister --features benchmark`, `cargo fmt && cargo check -p domm-degens-canister && cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`, and focused Gate J `20260519-164028-final-sync-new-event-gate-j` in `230.87s`. Versus `20260519-163253-runtime-champion-submit-gate-j`, `events.by_session_event_key` calls moved 19 -> 18 (-5.3% total instruction cost), `sync_session_turn` moved 12.1716B -> 12.1032B avg (-0.6%), and scenario instructions moved 320.1759B -> 319.4200B (-0.2%).
- [x] Evaluate and reject a pre-deadline runtime-open map-turn guard shortcut. The first version incorrectly returned before the duplicate `end_turn` ready-row check; stale-turn regression caught it. The fixed version passed stale-turn regression in `186.94s` and focused Gate J `20260519-runtime-open-guard-gate-j` in `232.09s`, but measured no delta versus `20260519-164028-final-sync-new-event-gate-j`: `system_jobs.by_session_status_due` stayed 26 calls, `submit_move_intent` stayed 11.0396B avg, and scenario instructions stayed 319.4200B. Reason: the measured commands start fresh turns where no active turn runtime exists yet, so this proof cannot fire without moving runtime creation earlier.
- [x] Skip fresh town build/recruit event-key absence reads by using create-first event append for the deterministic private/public town command events, while preserving lookup fallback on conflict. Verified with `cargo fmt --check`, `cargo check -p domm-degens-canister --features benchmark`, and focused Gate J `20260519-town-new-events-gate-j` in `238.69s`. Versus `20260519-164028-final-sync-new-event-gate-j`, `events.by_session_event_key` calls moved 18 -> 14 (-22.2% total lookup cost), `submit_build_town_structure` moved 20.0597B -> 18.6468B avg (-7.0%), `submit_recruit_units` moved 15.5931B -> 14.1707B avg (-9.1%), and scenario instructions moved 319.4200B -> 316.5882B (-0.9%).
- [x] Remove redundant `town_build` and `town_recruit` `CommandEffect` rows from town build/recruit commands. The real town rows, resource ledger entries, command rows, and events remain the replay/status/projection surface, so these effect rows were duplicate durable writes. Verified with `cargo fmt --check`, `cargo check -p domm-degens-canister --features benchmark`, and focused Gate J `20260519-town-no-effects-gate-j` in `230.50s`. Versus `20260519-town-events-champion-revert-gate-j`, `submit_build_town_structure` moved 18.6468B -> 17.4740B avg (-6.3%), `submit_recruit_units` moved 14.1707B -> 12.9995B avg (-8.3%), `effects.command_effect_by_command_key` and `effects.create_applied_command_effect` calls both moved 4 -> 2, and scenario instructions moved 317.2806B -> 315.0101B (-0.7%).
- [x] Remove redundant partial movement `movement_cursor` and `visibility_refresh` `CommandEffect` rows. Movement snapshots, updated intent path/hash, command nonce replay, and event keys already provide the durable replay/projection surface for partial syncs. Verified with `cargo fmt --check`, `cargo check -p domm-degens-canister --features benchmark`, and focused Gate J `20260519-movement-partial-no-effects-gate-j` in `231.98s`. Versus `20260519-town-no-effects-gate-j`, `effects.command_effect_by_command_key` and `effects.create_applied_command_effect` calls both moved 2 -> 1, `sync_session_turn` moved 12.1079B -> 12.0140B avg (-0.8%), and scenario instructions moved 315.0101B -> 313.9172B (-0.3%). The remaining sync effect is the neutral battle-start session guard, so it stays until battle-start ownership moves into runtime.
- [x] Skip fresh lobby setup event-key absence reads by creating session-created, participant-joined, and participant-ready events first with lookup fallback on conflict. Nonce replay still returns from the lobby command before event append. Verified with `cargo fmt`, `cargo check -p domm-degens-canister --features benchmark`, and focused Gate J `20260519-lobby-new-events-gate-j` in `231.56s`. Versus `20260519-movement-partial-no-effects-gate-j`, `events.by_session_event_key` calls moved 14 -> 10, `create_session` moved 12.7030B -> 11.9966B avg (-5.6%), `join_session` moved 8.0180B -> 7.3116B avg (-8.8%), `mark_ready` moved 6.6117B -> 5.9041B avg (-10.7%), and scenario instructions moved 313.9172B -> 310.8836B (-1.0%).
- [x] Batch the two town command events so town build/recruit update `GameSession.next_event_seq` once per command instead of once per private/public event. Verified with `cargo fmt --check`, `cargo check -p domm-degens-canister --features benchmark`, and focused Gate J `20260519-town-batch-events-gate-j` in `232.87s`. Versus `20260519-lobby-new-events-gate-j`, `sessions.update_session` calls moved 18 -> 16, `submit_build_town_structure` moved 17.4653B -> 16.9903B avg (-2.7%), `submit_recruit_units` moved 12.9939B -> 12.5197B avg (-3.6%), and scenario instructions moved 310.8836B -> 309.9352B (-0.3%).
- [x] Add a cache-only fast path for immediate pre-deadline `sync_session_turn` retries when the cached active session context exists and the active turn runtime proves no participant is ready. All other sync cases still reload durable session state. Verified with `cargo fmt`, `cargo check -p domm-degens-canister --features benchmark`, and focused Gate J `20260519-sync-not-due-cache-gate-j` in `227.73s`. Versus `20260519-town-batch-events-gate-j`, `sessions.load_session` calls moved 20 -> 18, two not-due sync calls moved to about 0.0001B instructions, `sync_session_turn` moved 12.0021B -> 11.6196B avg (-3.2%), and scenario instructions moved 309.9352B -> 305.7282B (-1.4%).
- [x] Remove the redundant successful-sync participant reload and reserve the final `session_turn_synced` event sequence inside the same session update that advances the turn. This keeps conflict fallback for recovered final events but removes one normal `sessions.update_session` write and one `sessions.load_participant` read from the successful turn-advance path. Verified with `cargo fmt --check`, `cargo check -p domm-degens-canister --features benchmark`, `cargo check -p domm-degens-canister`, `cargo test -p domm-degens-canister session_turn_runtime -- --nocapture`, and focused Gate J `20260519-sync-final-reserved-event-gate-j` in `696.40s`. Versus `20260519-sync-not-due-cache-gate-j`, `sync_session_turn` moved 11.6196B -> 11.5131B avg (-0.9%), update-only Gate J instructions moved 236.5752B -> 235.4024B (-0.5%), the successful turn-advance sync call moved 23.1070B -> 21.9392B (-5.1%), `sessions.update_session` calls moved 16 -> 15, and `sessions.load_participant` calls moved 1 -> 0. Full scenario instruction totals are not compared for this direct run because the query instruction fields were not populated in the generated artifact.
- [ ] Make active `sync_session_turn` resolve from runtime state and apply champion/occupancy/resource deltas without repeated row hydration.
- [ ] Make `get_game_view`, `get_events_after`, `get_champion_view`, and object/map views merge active turn runtime state before durable fallback.
- [ ] Treat turn deadline/resolution `SystemJob` rows as wakeup hints while runtime owns active turn authority.
- [ ] Flush/checkpoint active turn projections at turn boundary, battle start handoff, explicit checkpoint, upgrade, or finalization.
- [ ] Drive `sync_session_turn` below 5B average instructions, then below 1B if the heap turn aggregate behaves like the battle runtime.
- [ ] Drive `submit_move_intent` below 5B average instructions by moving active turn intent/idempotency/event state out of per-submit stable writes.

## Expected Outcome

The first successful battle aggregate checkpoint already reduced `submit_battle_action` by removing repeated stable row/index work. The broader expected outcome is to apply the same command-side aggregate model across the route that tests and players actually traverse.

For battle, the wins came from eliminating:

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

For the rest of perf1, the likely wins should come from eliminating the same shape in movement, session-turn, town, champion, and view code:

- per-command durable intent/command/event/effect rows when runtime receipts and event buffers can answer active gameplay
- repeated champion/participant/map row hydration during turn sync
- repeated town child-row loads and diffs during build/recruit
- query routes that rebuild the same active state from stable rows instead of reading runtime projections
- job rows that model live gameplay authority instead of acting as wakeup hints

The benchmark should prove whether each aggregate migration is actually better. If a runtime aggregate does not move scenario totals enough, repo-op/method summaries should tell us whether the remaining cost is event feed writes, command/idempotency writes, job/readiness writes, query projection, serialization, or rule CPU. Continue removing the largest measured cost until the whole game route is fast, not just one endpoint.
