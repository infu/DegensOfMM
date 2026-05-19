# perf1 Implementation Plan: Whole-Game Runtime Aggregates

Source: subagent research on movement/session-turn flow, query merge requirements, testing strategy, and future aggregate boundaries. No tests were run to create this plan.

## Goal

Use the `BattleRuntime` result as the implementation pattern for the rest of the game route. The target is not just a fast `submit_battle_action`; it is faster scenario execution through setup, map, movement, turn sync, towns, champions, economy, views, and battle.

The next implementation target is `SessionTurnRuntime`.

## Core Rule

Hot command-side state lives in heap runtime while active. IcyDB remains durable projection, history, recovery, fallback, and boundary storage.

Do not remove durable writes before reads and status paths can see runtime state. The safe order is:

1. Add runtime/projection plumbing.
2. Mirror current durable behavior into runtime.
3. Make runtime authoritative for fresh active commands.
4. Move sync/resolution state into runtime.
5. Flush/project at boundaries.
6. Remove redundant durable hot writes after tests prove replay, status, events, projections, timers, and handoffs.

## Current Movement Hot Path

`submit_move_intent` currently:

- reads active session/participant/player context;
- reads champion ownership and current champion row;
- validates path bounds, adjacency, movement cost, blockers;
- checks command idempotency and map-turn guards;
- creates/updates `GameCommand`;
- creates/updates `MovementIntent`;
- writes public `GameEvent` and advances session event sequence;
- returns `CommandResponse`.

`end_turn` currently:

- creates/replays a durable command;
- writes `ParticipantTurnReady`;
- reads active participants and ready rows;
- may schedule `turn_resolution:{session}:{turn}`;
- writes public event.

`sync_session_turn` currently:

- sometimes returns early without command if the turn is not due and not all ready;
- otherwise creates/replays a durable command;
- loads pending `MovementIntent`s;
- loads/mutates champions, participants, occupancy, visibility, world objects, towns, neutrals, battles, economy rows, jobs, session;
- writes movement snapshots, events, effects, resource ledgers, occupancy, visibility, battle-start rows, turn jobs, session turn advancement.

This is the same row-first smell that battle had before `BattleRuntime`.

## Runtime Ownership

`SessionTurnRuntime` should be a turn orchestration aggregate. It should not become the first owner of all town/champion/economy canonical state.

Own now:

| runtime-owned active state | purpose |
| --- | --- |
| session id, turn number, started/deadline/duration | active turn authority |
| participant ready set | avoid `ParticipantTurnReady` row as live authority |
| movement intents by champion | one active intent per `(session, champion, turn)` |
| command receipts by command id and nonce | replay/status without durable command rows |
| runtime events and event-key index | active feed without durable event fanout |
| reserved event sequence blocks | avoid event-seq collisions with durable rows |
| champion movement deltas | position, movement remaining, movement turn, temporary status/in-battle |
| occupancy deltas | champion occupancy changes without immediate stable writes |
| visibility and known-object deltas | active render overlay |
| movement cursor/partial sync state | one authoritative partial cursor |
| job/deadline hints | durable jobs wake runtime, runtime owns authority |
| dirty sets/version stamps | safe boundary flush and stale-write detection |

Defer to later runtimes:

| future runtime | defer ownership |
| --- | --- |
| `ChampionRuntime` | progression, mana, spells, armies, artifacts, base champion ownership/status |
| `EconomyRuntime` | durable balances, ledgers, market trades, town spend, hire/recruit spend |
| `TownRuntime` | buildings, recruit pools, garrison, tavern offers, growth |
| `BattleRuntime` bridge | combat internals and resolved battle projection |

## Proposed Struct Shape

Initial module: `canisters/degens/src/services/session_turn_runtime.rs`.

Keep the first version small and serializable:

```rust
pub(crate) struct SessionTurnRuntime {
    pub session_id: String,
    pub turn_number: u32,
    pub turn_started_at_ms: u64,
    pub turn_deadline_at_ms: u64,
    pub turn_duration_ms: u64,
    pub closing: bool,
    pub generation: u64,
    pub participants: Vec<SessionTurnParticipant>,
    pub ready_participants: BTreeSet<String>,
    pub intents: Vec<RuntimeMovementIntent>,
    pub command_receipts: Vec<SessionTurnCommandReceipt>,
    pub active_events: Vec<SessionTurnEvent>,
    pub event_seq_block: Option<SessionTurnEventSeqBlock>,
    pub champion_deltas: Vec<ChampionTurnDelta>,
    pub occupancy_deltas: Vec<OccupancyTurnDelta>,
    pub visibility_deltas: Vec<VisibilityTurnDelta>,
    pub object_deltas: Vec<ObjectTurnDelta>,
    pub resource_deltas: Vec<ResourceTurnDelta>,
    pub partial_cursor: Option<MovementCursor>,
    pub dirty: SessionTurnDirtySets,
}
```

Use `Vec` plus small linear scans first unless benchmark evidence says otherwise. `BattleRuntime` already showed that smaller code can matter more than theoretical map lookup speed at current active set sizes.

## Runtime APIs

Add APIs in this order:

| API | purpose |
| --- | --- |
| `runtime_key(session_id, turn_number)` | deterministic key |
| `with_runtime`, `with_runtime_mut`, `insert_runtime`, `remove_runtime` | heap store |
| `adopt_active_turn_from_rows(session)` | hydrate runtime from durable rows |
| `snapshot_for_upgrade`, `restore_from_upgrade` | upgrade survival |
| `begin_runtime_command` | nonce/payload replay and mismatch logic |
| `insert_command_receipt` | runtime command status |
| `active_events_after` | runtime event feed overlay |
| `command_receipt_by_id`, `command_receipt_by_nonce` | status overlay |
| `submit_move_intent_runtime` | active movement intent path |
| `mark_turn_ready_runtime` | active ready path |
| `sync_turn_runtime` | active turn resolution |
| `flush_barrier(reason)` | project durable state at boundary |

Flush barrier reasons:

| reason | when |
| --- | --- |
| `TurnAdvance` | before removing runtime and advancing durable turn |
| `BattleHandoff` | before row-backed battle start needs champion/town/object state |
| `Upgrade` | `pre_upgrade` if full runtime snapshot is too large or risky |
| `RuntimeEviction` | explicit cleanup/checkpoint |
| `StrongRead` | only for rare reads that cannot merge runtime state |

## Query And Status Merge Requirements

Runtime reads must be wired before durable hot writes are removed.

| endpoint/path | runtime overlay required |
| --- | --- |
| `get_events_after` | merge session-turn runtime events with durable and battle runtime events, sort/dedupe by event seq/key |
| `get_command_status` | check session-turn runtime receipts before durable commands |
| `get_command_status_by_nonce` | check runtime receipts by actor/session/nonce before durable commands |
| `get_game_view` | runtime turn/deadline/sync-required/ready/event overlay |
| `get_my_participant` | runtime current-turn ready and resource deltas |
| `get_my_champions` | runtime champion position, movement, status, in-battle |
| `get_champion_view` | same champion overlay |
| `get_visible_map_chunks` | runtime visibility/discovered blob overlay |
| `get_visible_objects` | runtime known objects, object deltas, champion/object positions, redaction |
| `get_object_view` | runtime object/champion/town state before durable fallback |
| `preview_move_path` | runtime champion position and occupancy once runtime owns movement state |

Copy the `BattleRuntime` pattern: runtime-first builder, durable fallback/adoption, and shared event/status merge.

## Checkpoint Sequence

### Checkpoint 0: Finish Or Abandon Current Micro-Cut

There is an in-progress local edit removing the `movement_intent` command effect from `submit_move_intent`.

Decision before runtime work:

- either finish it as the final row-level micro-cut, remove dead helper code, run compile + focused Gate J, update notes/todo, commit;
- or revert only this local edit if it distracts from runtime work.

Do not leave it mixed into runtime commits.

### Checkpoint 1: Inert Runtime Module

Add `session_turn_runtime.rs` with heap store, key, structs, basic receipt/event buffers, and small unit tests.

No behavior change.

Verification:

- `cargo fmt --check`
- `cargo check -p domm-degens-canister`
- `cargo check -p domm-degens-canister --features benchmark`
- native unit test if added

### Checkpoint 2: Read Overlay Plumbing, Still Unused By Writes

Wire events/status/query helpers so they can ask `SessionTurnRuntime`, but return no overlay when runtime is absent.

Touch likely files:

- `services/mod.rs`
- `services/events.rs`
- `services/game_view.rs`
- `services/render_projection.rs`
- `services/account_lobby_session.rs`
- `services/movement.rs` for preview hook, if needed

No durable write removal yet.

Verification:

- compile checks;
- focused projection tests if affected code is nontrivial.

### Checkpoint 3: Adopt And Mirror

Create/adopt runtime for active sessions/turns while keeping all durable writes.

Mirror into runtime:

- `submit_move_intent` intent/receipt/event;
- `end_turn` readiness/receipt/event;
- maybe `sync_session_turn` command receipt/event metadata.

At this stage durable rows remain authoritative, but runtime mirrors them. Add debug/test assertions where cheap: runtime intent equals durable `MovementIntent`; runtime ready set matches ready rows.

Verification:

- compile checks;
- Gate J focused benchmark/test;
- command status/event replay probe if added.

### Checkpoint 4: Runtime-Authoritative `submit_move_intent`

For active turn runtime:

- begin command through runtime receipt;
- keep one movement intent per champion in runtime;
- append runtime event;
- return runtime `CommandResponse`;
- use durable fallback if runtime absent/closing;
- preserve same nonce/payload replay and mismatch behavior.

Do not move sync resolver yet. If the existing resolver needs durable intents, add a projection flush before sync/job resolution as an intermediate compatibility bridge.

Expected win: `submit_move_intent` should drop sharply, but scenario total may not until sync is moved.

Verification:

- command id/nonce replay and mismatch;
- `get_events_after` after submit;
- focused Gate J.

### Checkpoint 5: Runtime-Authoritative `end_turn`

For active turn runtime:

- mark ready in runtime;
- use runtime ready counts/all-ready;
- schedule only a wakeup hint job if needed;
- keep durable fallback;
- make `get_my_participant` and `get_game_view` show current-turn readiness from runtime.

Verification:

- `pocket_ic_end_turn_closes_turn_and_blocks_stale_actions`;
- `get_game_view` ready/sync-required probe;
- focused Gate J if behavior is stable.

### Checkpoint 6: Runtime Turn Guard And Job Authority

Replace hot map-turn guard scans with runtime state when runtime exists:

- if runtime closing or deadline due, reject/route to sync;
- durable job scans remain fallback;
- `turn_deadline` and `turn_resolution` jobs wake/adopt runtime and ask runtime what is due;
- stale durable jobs no-op.

Verification:

- stale action block test;
- timer repair/deadline tests;
- compile checks.

### Checkpoint 7: Movement Resolver State-Access Layer

Refactor current resolver behind a small access layer before changing behavior:

- load pending intents;
- load/update champion;
- find/update occupancy;
- update visibility;
- mark object/resource interactions;
- create movement snapshots;
- append events.

First implementation can delegate to existing row functions. Then swap the implementation to runtime overlays.

This is the bug-prevention checkpoint. Avoid a giant direct rewrite of `resolve_pending_movement`.

Verification:

- focused movement resolver tests;
- crossing conflict;
- stationary blocker battle handoff;
- compile checks.

### Checkpoint 8: Runtime Resolver And Partial Cursor

Move active resolution into runtime:

- pending intents from runtime;
- deterministic order by champion id;
- champion deltas in runtime;
- occupancy deltas in runtime;
- visibility/object/resource deltas buffered;
- movement cursor authoritative in runtime;
- movement snapshots buffered or flushed at safe partial checkpoints.

Flush only at:

- partial checkpoint if recovery tests require it;
- battle handoff;
- turn advance;
- upgrade/checkpoint.

Verification:

- crossing conflict;
- timer-driven multistep movement;
- stationary enemy blocker starts battle;
- event/status after partial sync;
- focused Gate J.

### Checkpoint 9: Boundary Projection And Removal

Implement `flush_barrier`:

- project runtime intents/snapshots;
- champion row updates;
- occupancy/visibility/object updates;
- resource ledger and participant deltas;
- events/command archive if required;
- ready rows if still needed for diagnostics;
- session turn advance;
- job completion/reschedule;
- remove/archive runtime.

Battle handoff must be a strong boundary: involved champion/town/object deltas must be visible to row-backed battle start or explicitly passed to `BattleRuntime`.

Verification:

- movement-triggered battle;
- resource pickup/capture;
- income/materialization;
- seeded recovery regression;
- focused Gate J.

### Checkpoint 10: Remove Redundant Durable Hot Writes

Only after checkpoints 4-9 pass:

- remove per-submit durable `MovementIntent` creation;
- remove per-submit durable `GameCommand`/event writes for runtime-owned active turn commands;
- remove ready-row live authority;
- reduce job rows to wakeup hints;
- remove stable snapshot/effect writes from the hot path where runtime/projection covers them.

Verification:

- focused Gate J benchmark and comparison;
- selected bug tests based on touched paths;
- full suite only after a meaningful performance gate is reached.

## Invariants

| invariant | why |
| --- | --- |
| one active movement intent per `(session, champion, turn)` | preserves current replacement semantics |
| same nonce + same payload returns same receipt | idempotent client retries |
| same nonce + different payload fails | prevents command ambiguity |
| no new map-turn command after runtime closing starts | stale turn safety |
| one monotonic session event sequence across durable, battle runtime, and session-turn runtime | event feed correctness |
| movement order remains deterministic by champion id | replay and tests |
| one authoritative partial cursor | prevents skip/double-apply |
| battle handoff is a flush/pass boundary | row-backed battle start must not see stale champion/town/object state |
| resource deltas cannot double-spend with town/economy commands | future economy runtime boundary |
| runtime events/receipts are archived before runtime removal | status/feed remains visible |

## Highest-Risk Bugs And Tests

| risk | fast test/probe |
| --- | --- |
| event sequence collision or missing runtime event | submit move/end/sync events, query `get_events_after`, flush, query again |
| command status disappears | status by id/nonce before and after projection |
| nonce mismatch accepted | replay same nonce with different payload |
| stale champion position in API | partial sync then `get_champion_view`, `get_my_champions`, visible objects |
| stale map visibility/object redaction | partial move into visibility and query chunks/objects |
| double movement after partial sync | crossing-conflict and multistep deadline tests |
| all-ready/deadline race | `pocket_ic_end_turn_closes_turn_and_blocks_stale_actions` |
| timer job stale/rerun bug | timer repair/deadline tests |
| duplicate battle/resource side effects | stationary blocker, guarded mine/resource pickup probes |
| lost heap state on upgrade | unflushed intent/ready/partial movement across upgrade |

## Test Matrix

Use this cadence to stay fast.

| checkpoint | minimum run |
| --- | --- |
| inert module / compile refactor | `cargo fmt --check`; `cargo check -p domm-degens-canister`; `cargo check -p domm-degens-canister --features benchmark` |
| runtime store/snapshot/adoption | native unit tests plus focused Gate J |
| `submit_move_intent` runtime receipts/intents/events | focused Gate J; command status/nonce probe |
| `end_turn` runtime readiness | `pocket_ic_end_turn_closes_turn_and_blocks_stale_actions` |
| query merge | `pocket_ic_query_budget_keeps_preview_submit_and_render_bounded`; `pocket_ic_render_projection_tracks_live_objects_and_fog` |
| resolver/partial cursor | crossing-conflict, stationary blocker, timer-driven multistep movement |
| durable boundary flush | Gate J plus seeded recovery regression |
| benchmark claim | full `scripts/run-benchmarks.sh` |

Focused Gate J command:

```bash
run_id="$(date +%Y%m%d-%H%M%S)-session-turn-gate-j"
out="target/benchmarks/$run_id"
DOMM_CANISTER_FEATURES=benchmark \
DOMM_BENCH_OUTPUT_DIR="$out" \
DOMM_BENCH_QUERY_LOG_PATH="$out/test-output.log" \
CANIC_POCKET_IC_LOCK_NAMESPACE="domm-bench-$run_id-gate-j" \
cargo test -p domm-pocket-ic-tests --test canister_endpoints \
  pocket_ic_gate_j_strategic_loop_persists_icydb_rows -- --nocapture
```

Full benchmark suite:

```bash
DOMM_BENCH_JOBS=4 scripts/run-benchmarks.sh
```

Do not run full suite for every micro-step. Use it when runtime authority, durable boundary behavior, or public API behavior has a meaningful checkpoint.

## Future Aggregate Order

After `SessionTurnRuntime`:

1. `ChampionOverlay` / `ChampionRuntime`: position/status is already in session-turn runtime, but spells, mana, armies, artifacts, progression, aftermath, and render need one shared contract.
2. `EconomyRuntime`: participant balances/resource deltas/ledger flushing, so movement, town, hire/recruit, market, and income stop duplicating balance writes.
3. `TownRuntime`: buildings, recruit pools, garrison, tavern offers, town growth/capture.
4. Refactor `battle_aftermath` to write through champion/town/economy sinks instead of owning every row mutation directly.
5. Setup/render projection pass for any remaining route cost.

Design hooks to add now without overbuilding:

- `get_champion_snapshot`
- `apply_champion_turn_delta`
- `get_town_contact_snapshot`
- `apply_resource_delta_intent`
- `get_object_snapshot`
- `flush_barrier(reason)`
- runtime projection hooks for events/status/render
- generation/version stamps on active runtime deltas
