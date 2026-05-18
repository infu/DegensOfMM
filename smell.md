# submit_battle_action Performance Smell

## Summary

`submit_battle_action` is measuring around 25B to 29B instructions for normal battle actions. The latest full benchmark run shows this is consistent, not a single outlier:

| scenario | calls | avg instructions | max instructions | avg memory delta | avg wall time |
| --- | ---: | ---: | ---: | ---: | ---: |
| `aftermath_victory` | 45 | 27.7169B | 29.2661B | 172.20 MB | 3.15s |
| `guarded_mine_battle` | 6 | 21.5041B | 27.3657B | 110.76 MB | 2.46s |

One replay/idempotency call was cheap at 3.5254B instructions and 0 MB memory delta, which strongly suggests the heavy cost is in the full apply path, not endpoint overhead alone.

## Main Smell

One player battle action is doing much more than applying one tactical command. The path currently:

1. Authenticates caller and loads session/participant context.
2. Loads the battle row.
3. Checks command idempotency and battle round guards.
4. Scans applying commands for recovery.
5. Optionally checks/applies battle timeouts.
6. Loads the full battle state.
7. Computes legal actions for validation.
8. Applies the battle command in memory.
9. Persists the battle by diffing full stack and occupancy sets.
10. Fans out events to participants and public feed.
11. Reloads much of the battle state for readiness.
12. Recomputes round readiness and schedules jobs.
13. Reloads session and marks the command applied.

That means the endpoint is closer to a mini transaction processor than a single tactical update.

## Tables And Indexes Touched

Exact per-call repo-op counts are not currently captured by the benchmark tooling. The counts below are static estimates from the normal non-cast, non-timeout path.

| area | tables/indexes | typical count |
| --- | --- | ---: |
| caller/session auth | `PlayerAccount.players.by_principal`, `GameSession` PK, `GameParticipant.sessions.participant_by_session_player` | 3 reads |
| command begin | `GameCommand.commands.game_command_idempotency`, `GameCommand` create | 1 read, 1 write |
| round guard | `BattleParticipantRoundReady.by_battle_participant_round`, `SystemJob.system_jobs.by_job_key` | 2 reads |
| recovery no-op | `GameCommand.commands.game_command_by_session_status` | 1 page read |
| timeout no-op | `Battle` PK | usually 1 read |
| action state load | `Battle` PK, `BattleStack.battles.stacks_by_battle`, `BattleObstacle.battles.obstacles_by_battle`, `BattleOccupancy.battles.occupancy_by_battle` | 4 reads |
| command apply | in-memory `domm_game` legal action and battle command logic | CPU only |
| state persist | `Battle` PK/update, full `BattleStack` list, changed stack updates, full `BattleOccupancy` list, changed occupancy updates/deletes | 3 reads, 2-5 writes |
| event fanout | `GameEvent.events.by_session_event_key`, `GameEvent` create, `GameSession` update | per audience/event |
| readiness | `GameSession` PK, full stack/obstacle/occupancy reload, ready lookup/page, optional job upsert | 5-8 reads, 0-3 writes |
| timeout scheduling | `SystemJob.system_jobs.by_job_key`, `SystemJob` create/update | 1 read, 1 write |
| command finish | `GameSession` PK, `GameCommand` update | 1 read, 1 write |

For a normal two-player action with one generated battle event, this is roughly 30 to 33 reads/page queries and 12 to 17 writes. The big repeated cost is the battle child rows:

| row group | why it repeats | typical list passes |
| --- | --- | ---: |
| `BattleStack` | load action state, diff/persist stacks, reload for readiness | 3 |
| `BattleObstacle` | load action state, reload for readiness | 2 |
| `BattleOccupancy` | load action state, diff/persist occupancy, reload for readiness | 3 |

## Likely Cost Drivers

### 1. Full battle state is materialized multiple times

`battle_rows::load_battle_state` loads `Battle`, all `BattleStack`, all `BattleObstacle`, and all `BattleOccupancy` rows. A single submit then later reloads stack/obstacle/occupancy again for readiness.

Relevant code:

- `canisters/degens/src/services/battle.rs:540`
- `canisters/degens/src/services/battle_rows.rs:36`
- `canisters/degens/src/services/battle.rs:1103`

### 2. Persistence diffs whole child tables

`persist_battle_state` reloads the battle, then lists every stack and every occupancy row so it can diff the in-memory state against durable rows. Most actions only change active stack, target stack, maybe one occupancy row, and the battle header, but the persistence path still scans the whole child set.

Relevant code:

- `canisters/degens/src/services/battle_rows.rs:73`
- `canisters/degens/src/services/battle_rows.rs:152`
- `canisters/degens/src/services/battle_rows.rs:208`

### 3. Event fanout writes amplify quickly

Each battle event is fanned out to every involved participant plus public. For each audience, the code performs an event-key lookup, creates the event if absent, increments `session.next_event_seq`, and updates `GameSession`.

For one battle event with two participants, that is commonly:

| operation | count |
| --- | ---: |
| `GameEvent.events.by_session_event_key` lookups | 3 |
| `GameEvent` creates | up to 3 |
| `GameSession` updates | up to 3 |

Relevant code:

- `canisters/degens/src/services/battle.rs:2318`
- `canisters/degens/src/services/battle.rs:2373`
- `canisters/degens/src/services/command_response.rs:353`

### 4. Legal action validation computes more than needed

`validate_player_action` calls `legal_actions_for_stack`, and that computes reachable move tiles even when validating attacks, defend, or wait. The reachability code does a BFS over the battle grid and repeatedly checks occupancy/obstacles in memory.

Relevant code:

- `canisters/degens/src/services/battle.rs:2109`
- `crates/domm-game/src/battle/actions.rs:14`
- `crates/domm-game/src/battle/actions.rs:130`

### 5. Readiness recomputation is another broad pass

After the action is persisted, readiness reloads battle child rows and checks each living participant. For participants that are not already ready, it may call `legal_actions_for_stack` again to decide whether auto-ready applies.

Relevant code:

- `canisters/degens/src/services/battle.rs:1103`
- `canisters/degens/src/services/battle.rs:1177`
- `canisters/degens/src/services/battle.rs:1264`

### 6. Recovery and timeout checks run on every submit

Even when they are no-ops, every submit checks for applying commands and may check battle timeout state. These are not likely the whole 25B by themselves, but they add fixed cost to every battle action.

Relevant code:

- `canisters/degens/src/services/battle.rs:1620`
- `canisters/degens/src/services/battle.rs:1823`

## What We Still Cannot Prove Exactly

The current benchmark captures endpoint-level instructions, memory delta, cycles, response bytes, and wall time. It does not yet capture:

- exact repo operation count per endpoint call
- exact table/index hit count per endpoint call
- row counts returned per repo query
- per-phase instruction split inside `submit_battle_action`
- number of generated internal battle events per action

So the table/index counts above are code-path estimates. They are good enough to identify the smell, but not good enough to rank every sub-cost precisely.

## Recommended Next Instrumentation

Add benchmark-only repo operation tracing in `repos/foundation.rs`.

For every foundation operation, record:

| field | purpose |
| --- | --- |
| benchmark call sequence | tie repo ops to one public endpoint call |
| endpoint method | group by `submit_battle_action`, `sync_battle`, etc. |
| operation name | e.g. `battles.stacks_by_battle` |
| entity/table | e.g. `BattleStack` |
| operation kind | load, query, page, create, update, delete |
| count | how many times it ran |
| rows returned/affected | separate cheap lookup from full child-row scan |
| limit/cursor used | identify accidental broad pages |

Also add phase markers inside `submit_battle_action`:

| phase | expected value |
| --- | --- |
| auth/context | should be small fixed cost |
| command begin/guard | should be small fixed cost |
| recovery | should be near zero in normal path |
| timeout sync | should be near zero in normal path |
| load/apply/persist | likely largest hot area |
| event fanout | likely high write amplification |
| readiness/schedule | likely second full-state hot area |
| final response | should be small fixed cost |

## Optimization Order

1. Reuse the already-loaded battle state for readiness instead of reloading stacks/obstacles/occupancy after persistence.
2. Change battle persistence to write only touched rows: battle header, active stack, target stack, and changed occupancy.
3. Batch event fanout so `GameSession.next_event_seq` is updated once per command, not once per audience event.
4. Add action-specific validation paths so attack/defend/wait do not compute full move reachability.
5. Cache involved participant IDs once per battle event append, instead of loading champion/town ownership repeatedly per event.
6. Make recovery/timeout guards cheaper in the known-normal path, or move them behind narrower predicates.

## Working Hypothesis

The 25B instruction cost is mostly caused by repeated IcyDB table/index work and row serialization around full battle state materialization, full child-row diff persistence, event fanout, and readiness recomputation. The in-memory combat math is probably not the primary cost by itself, but `legal_actions_for_stack` and reachability BFS are still worth optimizing after the repo-op tracing confirms the shape.
