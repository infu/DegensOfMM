# What I Liked In This Backend

This project failed as a full backend shape, but not everything in it is bad.
These are the parts I would carry into the remake.

## 1. The pure rules crate was the right instinct

`crates/domm-game` being separate from the canister and IcyDB was good.

The next backend should keep a pure game kernel that can run without:

- canister APIs
- stable memory
- IcyDB rows
- Candid
- timers
- async work

That pure kernel should become the real game engine, not a helper library around
DB rows. The live game state should be a fast in-memory structure, probably a
chunked layered tilemap plus actor/object records.

## 2. Typed public endpoints were good

The canister endpoint inventory in `canisters/degens/src/contract.rs` was useful.

I liked having:

- explicit endpoint names
- query/update classification
- endpoint grouping
- tests proving required endpoints exist
- no generic SQL gameplay API

The remake should keep a typed Candid/API contract from the start.

## 3. Thin API modules were good

The `api/*` modules mostly did the right thing: collect caller/time at the
boundary, then delegate.

That separation is worth keeping:

```text
public endpoint
  -> thin boundary adapter
  -> game/application service
  -> pure kernel
  -> persistence adapter
```

The mistake was letting services become the engine.

## 4. Command idempotency was worth keeping

The nonce + payload hash model was good.

Things to keep:

- exact retry returns the same result
- same nonce with different payload fails
- command responses have stable status/error shapes
- clients can ask for command status by id or nonce

That is especially important for IC clients where users refresh, retry, or lose
connection mid-action.

## 5. Event feeds and changed subjects were good

The event model was useful for UI and recovery:

- append visible events
- return changed subjects from commands
- let the client refresh only relevant panels
- expose event paging

The remake should keep this concept, but events should come out of the kernel
after a single action resolves, not from deferred jobs.

## 6. Repository boundaries were good

The repository layer had a useful discipline:

- typed row access only
- bounded pagination
- sanitized storage errors
- hot-path index plans
- tests against generic SQL/core DB use

This should survive, but the repositories should persist snapshots/deltas from
the kernel instead of being queried constantly by gameplay logic.

## 7. Indexed-query audits were a good practice

The `IndexedQueryPlan` pattern was one of the better engineering practices in
the codebase.

It made important lookups explicit and testable. The remake should keep an
equivalent idea for any DB access that remains on a hot path.

## 8. Deterministic first-playable content helped

The first-playable map/content path was valuable because it forced real game
flows to exist:

- players
- sessions
- towns
- champions
- movement
- objects
- battles
- victory/history

The remake should start with an even smaller deterministic scenario, but keep
semantic ids and reproducible fixture content.

## 9. Headless walkthrough tests were good

The headless driver/client-probe idea was worth keeping.

A remake should have tests that play the game through public APIs, not only
unit tests of individual functions.

Minimum future test shape:

- start game
- move champion
- capture object
- enter town
- build/recruit
- fight battle
- end game
- replay/retry commands

## 10. Shared DTOs were helpful

Having DTOs in the shared game crate helped keep backend/client/test contracts
aligned.

The remake should keep shared typed DTOs for:

- command responses
- map chunks
- visible objects
- champion views
- town views
- battle views
- events
- errors

## 11. Diagnostics were useful

The benchmark/diagnostic surfaces were useful even though they exposed the wrong
architecture getting slow.

Worth keeping:

- per-endpoint timing/instruction measurements
- repo operation counts
- row counts
- memory growth snapshots
- smoke scripts that write results to files

The next version should add kernel metrics too: loaded chunks, dirty chunks,
tile lookups, path checks, entities touched, and commit cost.

## 12. Local deploy path mattered

The `Makefile`, `dfx.json`, and local-deploy docs were worth having.

The remake should be runnable with one command:

```text
make deploy-local
```

And the client should be able to point at that local canister without hand
editing multiple files.

## 13. The map chunk idea was close

`MapChunk` with terrain/movement/flags blobs was the closest part to the right
game model.

For the remake, push that idea further:

```text
chunk:
  terrain/art layer
  walkable/collision layer
  structure occupancy layer
  champion occupancy layer
  visibility/discovery layer
```

The key change: gameplay must read/write these layers directly in memory.
Durable rows should be persistence, not the engine.

## 14. Runtime projections pointed in the right direction

`session_turn_runtime` and `battle_runtime` were trying to solve the right
problem: stop hitting durable rows for every gameplay step.

The lesson is to make runtime state the primary game state from day one, not a
late optimization layered over a row-first backend.

The remake should start with:

```text
active game runtime state
  -> kernel mutates it
  -> dirty chunks/entities/events
  -> persistence commit
```

## 15. Things not to repeat

These are not "liked", but they are important lessons:

- do not make DB rows the gameplay engine
- do not split one player action across jobs/timers/manual sync calls
- do not maintain separate durable/runtime versions of the same rules
- do not let services grow into giant orchestration files
- do not optimize around a bad state model
- do not make tests encode backend machinery users should never see
- do not make setup or turn resolution depend on deferred work

## Carry Forward Checklist

For the remake, keep:

- pure deterministic game kernel
- typed public API contract
- command nonce/hash idempotency
- stable command/event/error DTOs
- headless playable walkthrough tests
- indexed/bounded persistence access
- diagnostics and benchmark reports
- one-command local deploy
- deterministic first-playable content

Replace:

- row-first gameplay with chunked layered runtime state
- deferred jobs with one-call action resolution
- scattered services with a smaller kernel/application/persistence split
- piecemeal runtime caches with a real active-game state model

