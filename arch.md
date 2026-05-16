# DoMM Architecture

This document summarizes the current architecture of Degens of Misery & Mayhem
as implemented in this repository.

## Workspace Shape

DoMM is an independent Rust workspace under `/srv/shared/icydb/DoMM`. The
adjacent `../icydb` workspace is used as a path dependency for IcyDB.

Main workspace members:

- `crates/domm-game`: pure deterministic game rules, DTOs, fixtures, and
  headless drivers.
- `schema/degens`: IcyDB schema root and durable entity definitions.
- `canisters/degens`: production Internet Computer canister.
- `testing/client-probe`: thin public DTO client and playable walkthrough probe.
- `testing/generated-session`, `testing/macro-tests`, `testing/pocket-ic`: test
  layers around generated schema/session behavior and the real canister.

## Layering

The production path is:

```text
public Candid endpoint
  -> canisters/degens/src/api/*
  -> canisters/degens/src/services/*
  -> canisters/degens/src/repos/*
  -> typed IcyDB entities from schema/degens
```

The `api` modules are intentionally thin. They pull canister-boundary values
such as `msg_caller()` and server time, then delegate to service modules.

The `services` modules own domain orchestration: authorization, command
idempotency, session checks, gameplay validation, recovery, event creation, and
projection assembly. They may call pure rule/DTO code from `domm-game`; durable
reads and writes still go through repositories.

The `repos` modules own typed IcyDB access. They are grouped by durable row
ownership rather than by endpoint. Repository helpers start from a fresh
generated `db()` session for each operation.

## Pure Game Crate

`crates/domm-game` has no IcyDB dependency. It owns:

- public Candid DTO types reused by the canister and client probe;
- first-playable content, deterministic fixture data, and semantic IDs;
- pure movement, battle, economy, town, champion, map, scenario, AI, cleanup,
  and aftermath rule code;
- deterministic pseudo-random helpers;
- headless fixture and backend drivers used by tests.

This crate is the rules and contract layer. It lets gameplay logic be tested
without the canister or persistent storage.

## Schema And Persistence

`schema/degens` defines the normalized IcyDB storage model. The architecture
explicitly avoids a monolithic serialized `GameState` row.

Important durable row groups:

- lifecycle: `PlayerAccount`, `GameSession`, `GameParticipant`,
  `LobbyCommand`;
- content: rulesets, factions, units, buildings, spells, artifacts, map-object
  definitions, and terrain definitions;
- strategic map: `MapChunk`, `VisibilityChunk`, `ParticipantKnownObject`,
  `MapOccupancy`, `WorldObject`;
- actors and settlements: `Champion`, `ChampionArmyStack`, `ChampionSpell`,
  `Town`, `TownBuilding`, `TownRecruitPool`, `TownGarrisonStack`;
- economy and progress: resource ledger rows, tavern/market/dwelling rows,
  objectives, quests, scenario rules, and world events;
- battle: `Battle`, `BattleStack`, `BattleObstacle`, `BattleOccupancy`;
- audit and recovery: `GameCommand`, `CommandEffect`, `PendingEffect`,
  `GameEvent`, `GameEventTurnSummary`;
- history and cleanup: match summaries and retained finished-session data.

`MapOccupancy` is the authoritative strategic occupancy table. `BattleOccupancy`
is the authoritative tactical occupancy table.

## IcyDB Usage

DoMM uses IcyDB as the canister-local durable store, not as a generic SQL layer.
The schema crate declares the canister/store with IcyDB macros:

- `DegensCanister` uses memory range `20..120` and commit memory id `119`.
- `DegensStore` uses data memory id `20`, index memory id `21`, and schema
  memory id `22`.
- Entity definitions live in `schema/degens/src/schema/entities.rs` and use
  IcyDB `#[entity(...)]` declarations with primary keys, relation fields,
  defaults, generated ids, and composite indexes.

The canister build script loads the schema type and calls
`icydb::build_with_options!` with options from `icydb.toml`. The root
configuration keeps generated SQL surfaces disabled by default:

```toml
[canisters.degens.sql]
readonly = false
ddl = false
```

At runtime, `canisters/degens/src/lib.rs` calls `icydb::start!()`. That macro
brings in the generated actor/storage code and exposes the generated `db()`
entrypoint used by repositories.

Repository code uses typed/fluent IcyDB APIs:

- `db().create(...)` for normal row creation with generated ids/defaults.
- `db().insert(...)` for deliberate full-row writes.
- `db().insert_many_atomic(...)` for bounded same-entity fixture/setup batches.
- `db().load::<E>().by_id(...).try_entity()` for primary-key reads.
- `db().load::<E>().filter(FieldRef::new(...).eq(...))` for indexed lookups.
- `FluentLoadQuery::page()` for cursor pagination.
- `db().update(...)` for full typed row updates.
- `db().delete::<E>().by_id(...).count()` for typed deletes.

Relation fields are stored as primitive keys in schema rows. Service/repository
code passes `Id<T>` at the Rust boundary, then writes or filters with
`some_id.key()`.

`repos/foundation.rs` centralizes the common IcyDB wrappers. It validates public
list limits, maps raw IcyDB errors into stable `ApiError` values, and prevents
storage internals from leaking through public gameplay responses.

Hot lookups are documented with `IndexedQueryPlan` values in repository files.
Native repository tests call generated IcyDB `explain()` output for these paths
and fail on full scans or missing bounded limits. Covered paths include player
principal lookup, ruleset/content lookup, participant/session lookup, command
idempotency, event feeds, map chunks, visibility, occupancy, town/champion
ownership, movement intents, battles, scenario progress, and match history.

Gameplay repositories deliberately avoid generic SQL and `core_db()`. A native
scan test checks repository sources for forbidden SQL/internal surfaces.
Controller-gated diagnostics may count selected entity rows, but public
gameplay APIs stay typed Candid plus typed IcyDB repositories.

IcyDB's lack of broad multi-entity transactions shapes the command architecture.
Mutations that touch several entities are modeled as recoverable sagas with
`GameCommand`, `LobbyCommand`, `CommandEffect`, target-row markers such as
`last_command_id`, and append-only `GameEvent` rows.

## Canister

`canisters/degens` is the production backend. The canister entrypoint calls
`icydb::start!()`, which includes generated actor/storage code and provides
`db()`.

Public gameplay APIs are typed Candid endpoints. Generic SQL and SQL DDL are
disabled by default in `icydb.toml` and are not used for normal gameplay paths.
Diagnostics are separate and controller-gated.

The public endpoint inventory is declared in `canisters/degens/src/contract.rs`.
It currently covers account/lobby/session, render reads, movement, town,
battle, content, events, history, champion magic, expanded economy, and scenario
progress endpoints.

## Command And Recovery Model

Gameplay mutations are saga-style commands because IcyDB does not provide broad
multi-entity transactions.

The normal update pattern is:

1. Find or create a `GameCommand` or `LobbyCommand` using the command
   idempotency key.
2. Compare the stored payload hash for nonce replay safety.
3. Validate the action against current durable state.
4. Write bounded target rows and, for durable side effects, `CommandEffect`
   markers.
5. Append `GameEvent` rows.
6. Mark the command applied or failed.
7. Return a typed command response with events and changed subjects.

Exact nonce retries are idempotent: they find the stored command and return its
stored status/result path instead of applying effects again. Reusing a nonce
with a different payload returns `duplicate_nonce_payload_mismatch`.

Partial application is recovered through durable command/effect rows and
domain-specific markers such as `last_command_id`.

## Turn Model

DoMM uses simultaneous timed turns rather than sequential player turns.

During a turn, players submit intents and commands. On a later update,
`sync_session_turn` materializes due work: movement resolution, object
interactions, income, battle handoffs, events, and turn advancement.

Queries may report time metadata such as whether sync is required, but they do
not persist turn advancement or speculative gameplay state. Update endpoints
own durable materialization and recovery.

Time-sensitive public canister endpoints derive server time at the canister
boundary instead of accepting caller-controlled `now_ms`.

## Render And Client Contract

Render-facing queries return DTOs from `domm-game`, not raw IcyDB rows.

`get_game_view` is a bounded aggregate projection for session, participant, map
chunks, visible objects, a small event projection, render-time metadata, and
action affordances. Full event feeds and heavier or high-cost details are split
into dedicated endpoints such as:

- `get_visible_map_chunks`
- `get_visible_objects`
- `get_my_champions`
- `get_champion_view`
- `get_town_view`
- `get_battle_state`
- `get_events_after`
- `get_command_status`

The client probe composes these public endpoints into a playable walkthrough.

## First-Playable Scope

The implemented first-playable backend supports:

- player registration and lobby/session setup;
- deterministic first-playable content and map seeding;
- visible map/object/champion/town projections;
- movement intents and bounded turn sync;
- resource pickup, mine capture, income, building, and recruitment;
- tavern, marketplace, and external dwelling economy slices;
- champion progression and magic slices;
- objective, quest, world-event, and advanced-victory slices;
- battle state, battle actions, battle sync, aftermath, victory, and match
  history;
- command idempotency, event feeds, and recovery-oriented row markers.

Deferred systems include broader procedural maps, naval movement, complex siege,
full bot opponents, diplomacy, rematches, ranked/guild/social systems, campaign
persistence, and unbounded content expansion. New systems are expected to land
as bounded Part 2 specs before runtime implementation.

## Testing Architecture

The test stack mirrors the layering:

- pure tests exercise `domm-game` deterministic rules and fixtures;
- schema/macro tests validate IcyDB schema surfaces;
- generated-session tests verify the schema canister type and fixture sharing;
- Pocket-IC tests install the real canister and call public Candid methods;
- the client probe runs the first-playable route through the public DTO
  contract.

The expected command loop is documented in `TESTING.md`, with `make regression`
covering the full workspace.

## Plain-Language Runtime Flow

The canister is the authority. Players do not mutate local game state and then
upload it. They call typed canister endpoints, and the canister reads/writes
IcyDB rows.

For an immediate command, the flow is:

```text
player calls update endpoint
  -> canister reads needed DB rows
  -> service checks auth, ownership, session state, resources, map/battle state
  -> service applies deterministic rules
  -> repository writes changed DB rows
  -> command/effect/event rows record what happened
  -> response tells the client the command status and changed subjects
```

Examples of immediate commands are building a town structure, recruiting units,
market trades, tavern hiring, quest accept/claim, and battle actions. Those
commands read the current durable state, validate the action, and write the
result during that update call.

Movement is different. Submitting movement does not usually move the champion
right away. A player submits an intent:

```text
Player 1 submit_move_intent
  -> writes GameCommand
  -> writes or replaces MovementIntent
  -> writes CommandEffect and GameEvent
  -> champion position usually stays unchanged
```

Player 2 can submit their own movement intent later in the same turn:

```text
Player 2 submit_move_intent
  -> writes that participant's command/intent/event rows
```

Canister update calls are processed one at a time, so Player 1 and Player 2 do
not write in parallel. Each call sees the committed DB state from previous
calls.

The turn is time-based, but there is no background timer that wakes up and
settles the turn automatically. The session stores:

```text
GameSession.current_turn
GameSession.turn_started_at
GameSession.turn_deadline_at
GameSession.turn_duration_ms
```

When the deadline has passed, query endpoints can report that sync is required,
but queries do not advance the game. The DB stays on the old turn until an
update call settles it.

Turn settlement happens when someone calls:

```text
sync_session_turn(session_id, client_nonce)
```

That update call reads all durable state needed for settlement: the session,
participants, pending movement intents, champions, map/terrain data, occupancy,
visibility, world objects, neutral armies, towns, economy rows, and relevant
command/effect markers.

Then game logic resolves the turn deterministically:

```text
load pending movement intents
  -> resolve movement microsteps
  -> handle tile/crossing/blocker conflicts
  -> update Champion rows
  -> update MapOccupancy
  -> write MovementSnapshot rows
  -> apply resource pickups, mine capture, object visits, visibility changes
  -> start Battle rows when movement reaches enemies/guards/towns
  -> materialize income if due
  -> append GameEvent rows
  -> update GameSession.current_turn when the turn is complete
```

The settlement is bounded. If too much movement/work exists for one update,
`sync_session_turn` can persist partial progress and return. A later sync call
continues from the DB rows and command/effect markers instead of starting over.

In short:

```text
players submit commands and intents during the turn window
deadline passes
nothing happens by itself
the next sync/update call reads DB state, applies rules, writes results
the session advances when settlement is complete
```
