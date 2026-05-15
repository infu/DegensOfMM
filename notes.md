# Degens of Misery & Mayhem Implementation Notes

Use this file during implementation. Keep notes short, concrete, and useful for future game and IcyDB work.

Add an entry whenever you find:

- A bug or surprising behavior.
- A limitation or design compromise.
- A blocker.
- A performance, cycle, memory, query-size, or storage concern.
- An IcyDB ergonomics issue or suggested improvement.
- A spec ambiguity that slowed implementation.
- A test gap or fixture weakness.

Preferred entry format:

```text
## YYYY-MM-DD - Short Title

Area:
Severity:
Status:

Observation:

Impact:

Suggested follow-up:
```

## Open Bugs

None yet.

## Blockers

None yet.

## 2026-05-15 - Standalone Repo Initialization Needed Escalation

Area: repo setup
Severity: low
Status: resolved

Observation:

Initializing `DoMM/` as its own git repo failed under the default sandbox with a read-only `.git` path error, then succeeded when rerun with elevated filesystem permissions.

Impact:

Future agents may see git metadata writes fail from the parent workspace even though normal file edits work.

Suggested follow-up:

Run game development commands from `/srv/shared/icydb/DoMM` and record any repeated git lock or read-only metadata errors.

## 2026-05-15 - Checkpoint 0 Harness Audit

Area: project harness
Severity: low
Status: resolved

Observation:

Checkpoint 0 added a standalone Cargo workspace, IcyDB schema/canister skeleton, deterministic first-playable fixture data, pure headless driver, schema/macro test crate, generated-session test crate, and Pocket-IC test scaffold.

Impact:

The harness can now test deterministic command flow sequencing, nonce retry and payload-mismatch recovery behavior, and Candid DTO serialization without deploying a canister. Generated-session and Pocket-IC tests are scaffolded until checkpoint 1 adds durable entities and checkpoint 5 adds public lobby/session APIs.

Suggested follow-up:

Extend the generated-session crate with real `db().create/update/query` tests in checkpoint 1 and replace the Pocket-IC scaffold with public canister call tests as APIs land.

## 2026-05-15 - Host Linker Wrapper Workaround

Area: test environment
Severity: medium
Status: mitigated

Observation:

The default host Rust linker path invoked the rustup `gcc-ld` wrapper, which referenced a missing Nix `ld-wrapper.sh`. Plain `rustc` and `cargo test` failed before project code linked.

Impact:

Regression commands could not run reliably from a fresh checkout in this environment without overriding linker behavior.

Suggested follow-up:

`.cargo/config.toml` now sets an x86_64-only Rust flag to use the system C linker with `bfd`. Remove that workaround if the host rustup/Nix linker installation is repaired.

## 2026-05-15 - IcyDB Native Test Feature Assumption

Area: IcyDB ergonomics
Severity: low
Status: open

Observation:

Building the schema/test surface with `icydb` default features disabled exposed SQL-gated imports inside the current IcyDB dependency. The DoMM workspace uses IcyDB default features for native schema tests while `icydb.toml` keeps generated SQL readonly and DDL endpoints disabled for the `degens` canister.

Impact:

For now, endpoint exposure is controlled by generated build options/config rather than treating the crate `sql` feature as the public API gate.

Suggested follow-up:

Revisit after IcyDB supports a cleaner no-SQL native build, or keep the generated SQL feature compiled but config-disabled for controller/test-only surfaces.

## 2026-05-15 - Checkpoint 1 Schema Baseline Audit

Area: schema
Severity: low
Status: resolved

Observation:

Checkpoint 1 implemented the 45 Part 2 IcyDB entities from `spec.md`, including content definitions, sessions, participants, map/visibility/occupancy, towns, champions, artifacts, neutral armies, battles, commands, events, and pending effects. Macro tests now assert key unique indexes and selected strong/weak relation cleanup assumptions.

Impact:

The schema can compile and register generated model metadata for the full first-playable durable state surface before command systems are implemented.

Suggested follow-up:

Checkpoint 2 should add command/effect/event repository code and convert metadata-only command invariant tests into write-path idempotency tests.

## 2026-05-15 - Reserved Principal Field Name

Area: schema
Severity: low
Status: resolved

Observation:

IcyDB rejects a field named `principal` because `principal` is reserved by Candid. `PlayerAccount.principal` from `spec.md` was implemented as `account_principal` with the same unique lookup semantics.

Impact:

This is a small schema naming deviation from the text spec, but it preserves the intended principal-to-player uniqueness invariant and keeps generated Candid/Rust code valid.

Suggested follow-up:

Use `account_principal` consistently in checkpoint 5 account/lobby APIs, or update `spec.md` to match the valid IcyDB field name.

## 2026-05-15 - Checkpoint 2 Command/Event/Recovery Audit

Area: command core
Severity: low
Status: resolved

Observation:

Checkpoint 2 added a pure command journal that mirrors the durable `GameCommand`, `LobbyCommand`, `CommandEffect`, `PendingEffect`, and `GameEvent` row contracts. It covers actor kinds, lifecycle/status views, SHA-256 payload hashes, nonce idempotency, retryable failures, bounded recovery, event-key idempotency, numeric event cursors, per-audience redaction, and turn summaries.

Impact:

There are no real gameplay mutations yet beyond the harness, so the audit target is the reusable mutation surface rather than lobby/movement/economy write paths. Query-style reads in this core return status/event DTOs without performing recovery writes; later gameplay systems must route every mutation through this command/effect/event layer before marking their checkpoint complete.

Suggested follow-up:

When checkpoint 5 introduces public lobby/session APIs, add repository-backed integration tests that prove the pure command journal semantics are preserved through generated IcyDB create/query/update calls.

## 2026-05-15 - IcyDB Created At Field Is Implicit

Area: IcyDB ergonomics
Severity: low
Status: open

Observation:

`GameCommand` and `LobbyCommand` indexes can reference `created_at` even though the field is not declared in the handwritten entity field list. Adding an explicit `created_at` field caused duplicate-field macro errors, which indicates IcyDB supplies this audit field implicitly.

Impact:

The schema matches the command recovery lookup intent, but future agents should not add explicit `created_at` fields just because only the indexes mention them.

Suggested follow-up:

Document implicit entity fields in the local schema notes or upstream IcyDB docs if this remains surprising during repository-wrapper work.

## 2026-05-15 - Checkpoint 3 Deterministic RNG Audit

Area: deterministic pseudo-randomness
Severity: low
Status: resolved

Observation:

Checkpoint 3 added a pure `RollKey` helper that implements the Part 2 `hash64(session.seed, domain_key, turn_number, command_id_or_system_key, actor_id_text, target_id_text, roll_index)` rule with SHA-256, bounded roll helpers, and compact audit DTOs for future command results and event payloads. Fixture tests pin the first playable combat roll digest and bounded damage value.

Impact:

The current gameplay surface has no systems that consume randomness yet, so there were no call sites to migrate. A scan of executable game/canister/test code found no raw randomness APIs, wall-clock branching, ULID-order decisions, or mutable RNG cursors. The remaining `event_seq` hits are command/event replay cursor logic, not RNG input.

Suggested follow-up:

As checkpoints 4, 9A, 11A, 12, 15, and later systems add content generation, combat, artifacts, neutral armies, effects, or AI tie-breaks, require them to call `RollKey` and include `RollAudit` data in command results or event DTOs.

## 2026-05-15 - Checkpoint 4 Content Audit

Area: content and first playable scenario
Severity: low
Status: resolved

Observation:

Checkpoint 4 added a pure first-playable content manifest and scenario fixture: two factions, two champion classes, six terrain definitions, nine units including neutral units, eight buildings, one v1-safe artifact definition, four map-object definitions, sorted asset keys, stable manifest/scenario hashes, and a hand-authored 48x48 map layout with roads, terrain patches, starts, mines, resource piles, central objectives, neutral armies, and walkthrough targets.

Impact:

The content is sufficient for the planned first playable loop without requiring deferred Part 1 systems. The spell list is intentionally empty because full spellbooks are deferred; artifact placement is also deferred even though one artifact definition exists for later pickup/equipment work. Runtime seeding into IcyDB rows still belongs to the lobby/session setup checkpoint.

Suggested follow-up:

Checkpoint 5 should consume this manifest/scenario fixture during setup and persist the corresponding ruleset, content, session, participant, town, champion, and initial event rows through command/effect phases.

## 2026-05-15 - Checkpoint 5 Lifecycle Audit

Area: lobby and session lifecycle
Severity: low
Status: resolved

Observation:

Checkpoint 5 added a pure lifecycle backend that implements registration, duplicate principal behavior, create/join/ready/start, leave/cancel helpers, session and participant queries, match-history shell reads, active-session/player-cap enforcement, and Gate A through the headless public driver. Setup now runs as idempotent `GameCommand`/`CommandEffect` phases and emits one deterministic setup event.

Impact:

The setup projection prevents a session from becoming `active` until content rows, participants, towns, champions, map chunks, occupancy, visibility chunks, and the setup event are all represented. An interrupted setup remains `starting` and recovery resumes without duplicating effects or events. This is still an in-memory lifecycle backend; durable generated IcyDB API methods are deferred to the API/client-contract checkpoint.

Suggested follow-up:

Checkpoint 6 should consume the setup counts by adding real map chunks, occupancy, visibility rows, and opening viewport DTOs. Checkpoint 16 should wire the lifecycle backend semantics into public canister entrypoints backed by generated IcyDB repositories.

## 2026-05-15 - Checkpoint 6 Map Visibility Audit

Area: map, terrain, occupancy, visibility
Severity: low
Status: resolved

Observation:

Checkpoint 6 added a split `domm-game::map` module with focused files for public DTO/types, bitset helpers, first-playable map building, viewport/redaction reads, occupancy helpers, snapshot hashing, and tests. The first playable fixture now produces nine 16x16 map chunks, terrain/movement/flag blobs, per-participant discovered/visible bitsets, known-object rows, occupancy rows for towns/champions/world objects/neutrals, paged viewport chunk/object reads, hidden-subject `not_visible` results, and a pinned opening viewport snapshot hash.

Impact:

The frontend can render the opening visible map from public `MapChunkView` plus `ObjectView` DTOs: chunks include terrain, movement, flags, discovered, and visible blobs; object lists omit hidden rows and direct hidden lookups return `NotVisible` without raw payloads. No frontend path needs private occupancy rows, world-object storage rows, participant-known rows, or speculative local query state to draw the visible map. The implementation is still pure harness state, not generated IcyDB repository wiring.

Suggested follow-up:

Checkpoint 6A should consume only these public DTOs in the thin client/probe and record any missing fields. Durable map seeding and public canister entrypoints should reuse the same DTO/redaction contract when generated IcyDB repositories are wired in later checkpoints.

## 2026-05-15 - Checkpoint 6A Thin Client Probe Audit

Area: client probe and public DTO contract
Severity: low
Status: resolved

Observation:

Checkpoint 6A added `testing/client-probe` as a separate workspace crate with split backend, render, and DTO modules. The probe starts the first playable fixture through the public headless lobby flow, loads the active match, participant, paged map chunks, paged objects, events, and sync-required state, then renders a 24x24 ASCII opening viewport from public DTOs only. Gate B now asserts visible champion, town, resource, neutral, event, and sync state render without exposing the hidden east champion.

Impact:

The probe found two public contract gaps before it could stay outside backend internals: setup events needed a paged public `EventPage`, and opening viewport constants needed root-level exports from `domm-game`. Both were fixed. The probe backend still uses fixture-backed pure state internally because generated IcyDB read APIs are not wired yet, but the client-facing `ThinClientProbe` talks only to public query-style methods and DTOs.

Suggested follow-up:

When canister entrypoints land, mirror the probe backend trait with real query calls and keep the Gate B test as an external contract test. Later checkpoints should add DTO fields before client code needs private rows, especially for resources, town actions, movement previews, and battle state.

## 2026-05-15 - Checkpoint 7 Economy Audit

Area: resources, ledger, lazy income
Severity: low
Status: resolved

Observation:

Checkpoint 7 added a split pure economy module for participant balances, income sources, resource piles, resource ledger entries, turn summaries, lazy income materialization, cap handling, and first-playable economy smoke coverage. Resource mutations flow through deterministic ledger keys and idempotent `command_id + ledger_key` rows; replay skips applied rows, resumes after bounded partial application, and fails closed on balance mismatches. Lazy income is bounded to 14 turns, setup starts `last_income_turn` at turn 1, player rewards fail over cap, and system income can saturate at the numeric cap.

Impact:

The first playable economy path can collect a resource pile, capture an income source, materialize income, and summarize ledger rows without double-applying rewards or income. Ownership cutover materializes old-owner and new-owner income before changing the source owner, then delays new source income until subsequent turns. This remains pure harness state; durable IcyDB repository writes and public canister command endpoints are still future work.

Suggested follow-up:

Checkpoint 8 should consume `EconomyState` through the same ledger protocol before build/recruit spends. When generated repositories are wired, preserve the same recovery order: command/effect row, deterministic resource ledger rows, participant balance update, then event/output row.

## 2026-05-15 - Checkpoint 8 Town And Recruitment Audit

Area: towns, buildings, recruitment
Severity: low
Status: resolved

Observation:

Checkpoint 8 added a split pure town module for town rows, authoritative building rows, recruit pools, garrison/champion stacks, champion-location checks, build/recruit previews, build/recruit commands, cache repair, lazy recruit growth, and the first-playable build/recruit smoke path. Build and recruit spends go through `EconomyState` resource ledger rows before town mutations. Retry paths detect already-applied spend ledger rows and finish missing building/stack mutations without charging again.

Impact:

The first playable town loop can build `freehold-training-yard`, create a `mudhook-levy` recruit pool, grow it lazily by week, and recruit into a town garrison. Tests cover missing prerequisites, duplicate buildings, affordability, lazy growth caps, champion-at-town validation, full or incompatible stack targets, cache repair from building rows, and recovery after build/recruit spend interruptions. No durable occupancy updates or public canister command rows are wired yet; this remains a pure rules layer.

Suggested follow-up:

Checkpoint 9 should consume the same army stack shape for champion strategic state. When movement and real canister commands land, town/recruit commands need to update map occupancy/events through the command/effect saga after the resource ledger and stack mutations complete.

## IcyDB Ergonomics Notes

None yet.

## Performance And Storage Notes

None yet.

## Spec Ambiguities

None yet.

## Test Gaps

None yet.

## Decisions And Tradeoffs

None yet.
