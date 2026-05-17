# Degens of Misery & Mayhem V2 Spec Backlog

This file is the holding area for systems that are not required for the v1
first playable. Nothing in this file is approved for runtime implementation
until it is promoted into a bounded implementation subsection with schema,
indexes, endpoint contracts, command recovery, deterministic keys, caps, tests,
cleanup, and Pocket-IC e2e coverage.

`spec.md` is the v1 implementation contract. V2 work must not weaken the v1
canister/IcyDB playability gates.

## Promotion Gates

Before a V2 bucket can start implementation, add a concrete subsection covering:

```text
IcyDB schema:
  entities, fields, relation strength, defaults, append-only migration behavior
Indexes:
  every hot lookup, unique/idempotency key, pagination order, cleanup lookup, and visibility/recovery lookup
Commands and endpoints:
  public update/query method names, typed Candid DTOs, preview endpoints, disabled responses, and ownership checks
Recovery and idempotency:
  GameCommand, CommandEffect, PendingEffect, event keys, retry behavior, partial-application resume order, and budget exhaustion behavior
Deterministic pseudo-random keys:
  explicit domain keys and input fields; no IC raw randomness, wall-clock elapsed time, row order, or mutable RNG cursors
Numeric caps:
  per-session, per-turn, per-participant, per-object, per-query, and per-update caps with fail-closed errors
DTOs and frontend:
  render-ready public views, legal action affordances, disabled reasons, pagination/cursor contracts, redaction rules, and retry/sync expectations
Tests:
  pure unit tests, schema/macro tests, generated-session or repository tests, endpoint inventory tests, and Pocket-IC e2e coverage for every public endpoint in the bucket
Cleanup and retention:
  strong/weak relation cleanup order, summary rows, raw log retention, active-session protection, and bounded retry behavior
```

Any implementation that does not satisfy these gates is out of scope, even if a
similar Part 1 design note exists.

## Canister Contract Cleanup

V1 keeps the playable sync-driven turn contract: clients use render-time
`sync_required` metadata and call `sync_session_turn` before trusting
turn-sensitive state. A stricter hard rejection for `submit_move_intent` calls
that arrive after `turn_deadline_at` is V2 contract cleanup because it needs a
bounded compatibility pass for command replay, payload mismatch behavior,
pre-submit sync affordances, client recovery copy, and Pocket-IC coverage.

Before promoting this cleanup back to `spec.md`, define:

```text
exact retry behavior for an already-applied movement intent after the deadline
whether validation happens before or after command idempotency lookup
typed Candid error code and retryability for new late submissions
client recovery flow when a submit races with turn expiration
command-status behavior for failed late submissions
Pocket-IC tests for on-time submit, late submit, exact late retry, and late nonce mismatch
```

## World Generation, Siege, And Naval

V1 has only persisted boundary rows for this bucket:

```text
SkirmishSettingsState
ProceduralMapState
NavalRouteState disabled with checkpoint_25_schema_only
SiegeRuleState disabled with checkpoint_25_schema_only
```

The following are V2-only until a bounded subsection replaces the disabled rows
with active gameplay semantics.

Siege engines:

```text
Define:
  engine ownership, placement, transport, ammunition, durability, targeting, destruction
  battle action DTOs for engine fire, wall/gate damage, repair, breach, and disabled reasons
  indexes by session, battle, owner participant, engine key, target fortification, command, and status
  deterministic damage/miss/critical keys
  caps for engines per battle, shots per round, affected tiles, and repair/breach work per update
```

Fortifications:

```text
Define:
  authoritative wall segment, gate, tower, breach, and fortification-state rows
  cleanup and repair rules for town capture, battle aftermath, surrender, and abandoned sessions
  battle and town view projections that stay below IC query budgets
```

Naval movement:

```text
Define:
  boat ownership and occupancy rows
  shipyard or embark-structure rules
  embark/disembark commands
  water terrain costs and route validation
  visibility/redaction behavior for boats and water routes
  indexes by session, participant, boat, occupant, water route, coordinate, command, and status
  deterministic encounter/current/weather keys only after their rules are specified
```

Procedural and larger maps:

```text
Define:
  generation job rows and chunk materialization rows
  content manifests and seed/version migration rules
  scenario hash contracts
  bounded generation batches
  visibility fan-out caps
  paging contracts
  Pocket-IC install/query/update budgets
```

## Multiplayer Meta And Long-Term Product Systems

V2 candidates:

```text
durable rematch creation
campaign persistence and carryover
ranked leaderboard
guilds
diplomacy
broader match history
social/meta systems
```

Before implementation, define:

```text
privacy and authorization boundaries
retention and cleanup policy
pagination and cursor contracts
abuse-resistance and rate/cap rules
summary rows versus raw history rows
whether any meta write can touch hot gameplay command paths
Pocket-IC e2e coverage for every public endpoint
```

## Scenario Expansion

V2 candidates:

```text
quest huts
quest chains
monthly world events
artifact victory
king-of-the-hill
survival
scenario-specific defeat
richer scenario rules beyond the disabled row contract
```

Before implementation, define scenario-specific row ownership, visibility and
redaction, victory/defeat scoring order, reward idempotency, event keys, caps,
and public endpoint contracts.

## Progression, Magic, Artifacts, And Content Expansion

V1 includes only the bounded progression, magic, artifact, and content slices
already promoted into `spec.md`. Broader variants remain V2.

V2 candidates:

```text
full spellbook expansion
large spell trees
additional champion skill-tree branches
neutral negotiation skill lines
fortified-town combat skill lines
artifact sets
additional factions beyond the first-playable roster
large content packs
```

Before implementation, define content manifests, versioning, skill/spell/status
indexes, artifact ownership/equipment/effect keys, caps for options and active
effects, migration behavior, DTO affordances, cleanup, and Pocket-IC budget
coverage.

## Economy Expansion

V2 candidates:

```text
defeated champion reappearance
advanced economy buildings
broader resource-source variety
additional tavern/market/dwelling variants
champion-target town recruitment
local/same-tile external dwelling recruitment variants
town hall income
captured-town unrest penalties and pacification
desperation income
```

Before implementation, define resource ledger effects, no-double-spend recovery,
weekly/monthly growth keys, ownership cutover rules, caps, and query budgets.

## AI And Content Packs

Full bot opponents and general strategic planners are V2-only. V1 keeps neutral
battle behavior and bounded autopilot-style command generation only.

Unbounded content packs are V2-only. A content-pack spec must define caps,
migration policy, content hashes, manifest/version contracts, and canister
budget tests before code starts.

## Removed Scope

These are not V2 backlog items unless the product direction changes:

```text
sequential player turns
hotseat-only backend flow
monolithic GameState persistence
generic SQL gameplay APIs
```
