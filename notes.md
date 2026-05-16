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

## 2026-05-16 - Checkpoint 19A Canister Endpoint Contract

Area: canister API, IcyDB integration
Severity: medium
Status: inventory resolved; behavior pending later gates

Observation:

Checkpoint 19A added a canonical inventory for 28 required public game endpoints, a canister inventory query, typed Candid endpoint shells in `domm-degens-canister`, Candid export coverage, documentation for deferred `leave_session`, `cancel_session`, `surrender`, `retreat`, and `request_rematch`, and a Pocket-IC endpoint-presence test that installs the canister and calls every required method. The endpoints currently return `ApiError { code: "icydb_repository_not_implemented", retryable: true }` until repository/service wiring starts in 19C/19D.

Impact:

The project now has a real canister API surface and a Pocket-IC proof that missing public methods fail the test. This is endpoint-contract coverage only, not IcyDB-backed gameplay e2e. Public DTOs use `domm-game` Candid types and do not expose raw IcyDB rows. The audit found no missing endpoint names against spec section 15 and `FixtureApiBackend`; argument shapes intentionally follow the current fixture/client DTOs where they have evolved beyond the older spec examples, and final time-source cleanup should happen while wiring real services.

Suggested follow-up:

Checkpoint 19B should keep splitting canister code by API/service/repository domains before endpoint bodies grow. Checkpoint 19C/19D must replace the placeholder errors with typed IcyDB repository calls and derive canister time consistently instead of preserving fixture-only time inputs if the final public API is tightened.

## 2026-05-16 - Pocket-IC Wasm Build Ergonomics

Area: Pocket-IC tests
Severity: low
Status: mitigated

Observation:

The Pocket-IC endpoint-presence test needs to build `domm-degens-canister` for `wasm32-unknown-unknown`. Debug wasm was about 111 MiB and exceeded Pocket-IC's 100 MiB wasm chunk-store limit, while release wasm was about 3.7 MiB. Cross-compiling also hit the host rustup/Nix `ld.lld` wrapper issue for build scripts, because stable Cargo does not apply the existing host linker workaround cleanly to cross-build host artifacts.

Impact:

The test helper now builds release wasm and creates a local `cc` wrapper under the test target directory that appends `-fuse-ld=bfd` for the nested canister build. This keeps the workaround local to the Pocket-IC test.

Suggested follow-up:

If the environment linker is repaired, remove the wrapper helper. If canister wasm size grows, add an explicit size check or release profile before Pocket-IC install starts failing again.

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

## 2026-05-15 - Checkpoint 9 Champion State Audit

Area: champions, armies, artifacts
Severity: low
Status: resolved

Observation:

Checkpoint 9 added a split pure champion module with champion rows, army stack rows, artifact instance/equipment rows, movement state, status transitions, deterministic artifact capture, v1 experience/level progression, and visibility-facing champion DTOs. Movement uses `movement_turn` plus `movement_remaining` for lazy reset. Artifact equipment enforces uniqueness through equipment rows, not nullable cache fields. Skill-tree selection is explicitly deferred with a typed progression status.

Impact:

Champion state now supports the strategic lifecycle needed by later movement and battle checkpoints: active/in-battle/garrisoned/defeated statuses, battle aftermath artifact transfer, stack cap enforcement, champion defeat tracking, active-champion victory checks, and hidden-enemy DTO redaction. Battle resolution, movement occupancy updates, and real artifact pickup commands remain future saga work.

Suggested follow-up:

Checkpoint 9A should validate implemented effect hooks against content `effect_key` and `ability_keys`. Checkpoint 10 should consume champion movement fields and update map occupancy through command/effect recovery rather than mutating champion coordinates directly.

## 2026-05-15 - Checkpoint 9A Effects Audit

Area: effects, abilities, disabled systems
Severity: low
Status: resolved

Observation:

Checkpoint 9A added a bounded effect dispatch module covering first-playable ability keys, artifact effects, building effects, and object interaction keys. Unsupported spellbook, skill-tree, morale, luck, and complex status systems return typed disabled reasons. `CastAbility` is never returned enabled for v1 content. Chance effects use deterministic `RollKey` audit data, and status keys are capped at 8.

Impact:

All current first-playable `ability_keys`, building `effect_key`s, artifact effects, and map-object interaction keys are accounted for by tests. Since the manifest currently has no spells or champion spell rows, spell dispatch remains explicitly deferred instead of silently enabled. Future battle, artifact, and object systems can call one dispatcher rather than hard-coding effect support checks.

Suggested follow-up:

Checkpoint 10 and later battle/effect systems should thread `EffectResolution` and `RollAudit` into command results/events when an effect changes gameplay. New content keys should fail coverage tests until the dispatcher has an explicit supported or deferred handler.

## 2026-05-15 - Checkpoint 10 Movement Audit

Area: movement intents and turn sync
Severity: low
Status: resolved

Observation:

Checkpoint 10 added a split pure movement module for path previews, replaceable intents, turn-final movement resolution, deterministic microsteps, snapshots, object and battle stop drafts, partial cursors, and bounded sync budgets. Queries expose `MovementTimeView` without finalizing movement. Recovery tests cover partial cursor resume and simulated traps after partial apply.

Impact:

The first playable champion can submit a movement intent, cross multiple turn windows, stop on object tiles, and later stop on neutral contact without private state mutation from queries. Movement still runs in the pure fixture layer; durable system command rows and public canister entrypoints should preserve the same recovery ordering when wired into generated repositories.

Suggested follow-up:

Checkpoint 11 should consume movement object stops through command/effect-backed world-object interactions. Battle checkpoints should turn battle stop drafts into durable battle rows instead of resolving combat inside movement.

## 2026-05-15 - Checkpoint 11 World Object Audit

Area: world objects, pickups, mines, captures
Severity: low
Status: resolved

Observation:

Checkpoint 11 added world-object visits, deterministic visit keys, once-only and refreshable visits, resource rewards, mine ownership cutover, central objective scoring, and movement-object stop application. Object rewards reuse `EconomyState` ledger idempotency, and object reads continue through the existing map visibility/redaction contract.

Impact:

The first playable strategic path can pick up nearby resources, capture income-producing map objects, and update scoring without duplicating visits or bypassing resource recovery. Guarded-object interactions now fail closed until the guard is defeated. Durable IcyDB wiring is still future work, but the pure command shape matches the required idempotent saga ordering.

Suggested follow-up:

Checkpoint 11A should add neutral guard rows and encounter starts so guarded-object and neutral-contact stops can produce battle triggers. Checkpoint 14 should consume object ownership and scoring state during capture/victory aftermath.

## 2026-05-15 - Checkpoint 11A Neutral Army Audit

Area: neutral armies and encounter starts
Severity: low
Status: resolved

Observation:

Checkpoint 11A added neutral army rows, stack rows, strength labels, visible/scouting/redacted neutral DTOs, v1 behavior policy flags, movement-contact encounter creation, guarded-object encounter creation, and defeat cleanup. Growth is implemented as an explicit v1 no-op rather than silent background behavior, and roaming/join/bribe behavior is disabled through policy fields.

Impact:

Neutral armies now block occupancy until defeated, can be inspected with visibility-safe stack detail, and can create idempotent pending battle keys from strategic movement or guarded-object contact. This supports the first playable battle trigger without requiring Part 1 neutral AI expansion.

Suggested follow-up:

Checkpoint 12 should materialize these encounter records into battle state. Later expansion work can promote roaming, join, or bribe behavior only after Part 2 adds bounded command, DTO, randomness, and cleanup rules.

## 2026-05-15 - Checkpoint 11B Strategic Gate Audit

Area: strategic headless playable gate
Severity: low
Status: resolved

Observation:

Checkpoint 11B added a split `strategic` module with a public backend trait, fixture backend, headless driver, Candid-facing DTOs, and Gate C tests. The fixture starts a match, inspects the map, moves to a pickup, applies the object reward, syncs income, builds `freehold-training-yard`, syncs to turn 8, recruits four `mudhook-levy` into the town garrison, moves into the west neutral guard, and applies the neutral encounter trigger.

Impact:

Gate C now runs through public command/query methods only and asserts visible state after each step. Current deterministic metrics are 22 commands, 36 fixture event units, 15 public strategic/lobby queries, and 4992 approximate max query bytes. No blocking slow path was found in the pure fixture loop. The metrics are not IC cycle/storage measurements; canister-level instrumentation is still needed when public entrypoints are wired.

Suggested follow-up:

Checkpoint 12 can start from the pending battle key produced by the strategic gate. Checkpoint 16 should preserve this public DTO and command/query shape when replacing the fixture backend with generated IcyDB-backed canister methods.

## 2026-05-15 - Checkpoint 12 Battle Engine Audit

Area: battle engine baseline
Severity: low
Status: resolved

Observation:

Checkpoint 12 added a split pure `battle` module for row-shaped battle records, stack snapshots, tactical occupancy, obstacles, round-based initiative, legal action generation, deterministic damage, DTO assembly, and a first-playable neutral battle smoke fixture. The fixture snapshots the west champion army against `neutral:west-mine` as one active battle with 3 stacks, 2 obstacles, 3 occupancy rows, a 12x10 grid, readiness fixed at 0, and a deterministic active stack selected by initiative, speed, then seeded tie-break hash.

Impact:

The tactical baseline can now expose `BattleView`, legal move/melee/ranged/defend/wait/deferred-action DTOs, apply ranged/melee damage with unit attack/defense plus champion Might/Guard modifiers, remove defeated stack occupancy, and repair cached stack coordinates from authoritative `BattleOccupancy`. V1 morale and luck are explicitly disabled through the effect dispatcher with `morale_disabled_v1` and `luck_disabled_v1`, and battle stack status keys remain capped at 8.

Suggested follow-up:

Checkpoint 13 should wrap these pure rules in command/recovery/timeout flows, add active-stack action progression, and emit battle events. When generated IcyDB repositories are wired, preserve `BattleOccupancy` as the authoritative tactical position table and keep `BattleStack.battle_x/y` as repairable DTO cache fields.

## 2026-05-15 - Checkpoint 13 Battle Command Audit

Area: battle commands, timeouts, and recovery
Severity: low
Status: resolved

Observation:

Checkpoint 13 added a pure battle command layer around the tactical rules: `submit_battle_action`, `sync_battle`, command records, event records, idempotent client nonce replay, applying-command recovery, active-stack action deadlines, deterministic timeout `AutoDefend` system commands, bounded timeout processing, and event sequencing. The command path recovers applying commands and resolves due timeout commands before validating the caller action.

Impact:

Battle actions now have the recovery/idempotency shape needed for generated IcyDB-backed commands. Tests cover duplicate payload mismatch, action-after-timeout races, player action just before timeout, auto-defend idempotency, timeout budget slicing with `battle_sync_incomplete`, recovery before validation, and ordered battle events. This is still an in-memory command journal; checkpoint 16 must map the same semantics onto public canister methods and generated repositories.

Suggested follow-up:

Checkpoint 14 should consume applied battle events and stack survivor state to implement battle aftermath, neutral defeat, town capture, champion defeat, and victory finalization through idempotent commands.

## 2026-05-15 - Checkpoint 14 Battle Aftermath Audit

Area: battle aftermath, capture, defeat, and victory
Severity: low
Status: resolved

Observation:

Checkpoint 14 added a split `aftermath` module that consumes resolved battle rows and applies neutral defeat cleanup, town capture, income cutover, garrison survivor placement, champion strategic repositioning, champion defeat, deterministic artifact capture, match finish summaries, match history, and local aftermath events. Retreat and surrender are explicit disabled v1 paths with typed reasons instead of partially implemented behavior.

Impact:

The first playable pure harness can resolve the neutral guard battle, capture the east town, defeat the enemy champion, and finalize the match. Victory checks ignore elimination while any battle remains active. Max-turn scoring is bounded to the first playable participant set and uses towns, mines, army combat power, and a seeded tie-break. Town capture now applies the two-turn unrest window and reuses economy income cutover materialization before changing ownership.

Suggested follow-up:

Checkpoint 14A should drive the same path through public backend commands and queries from lobby start to victory. Checkpoint 16 should preserve these aftermath command, event, and idempotency semantics when generated IcyDB repositories replace the in-memory fixture state.

## 2026-05-15 - Checkpoint 14A Backend Gate Audit

Area: backend-only first playable gate
Severity: low
Status: resolved

Observation:

Checkpoint 14A added a split `playable` gate module that runs the existing public strategic backend from player registration and lobby start through the neutral blocker, then continues through public battle, sync, aftermath, town-capture, champion-defeat, match-inspection, and event-refresh calls. The gate includes an exact battle-action retry with the same nonce, a turn sync, a battle sync, resource pickup/income/build/recruit mutations, the neutral movement blocker, and a final event page refresh.

Impact:

Gate D now completes the first playable backend path to victory in the normal regression suite. Current deterministic metrics are 32 command/update calls, 42 events, 19 queries, 190 estimated storage rows, and a 5072-byte max query estimate. No slow-query or row-growth concern crossed the current gate thresholds.

Suggested follow-up:

Checkpoint 15 can add deterministic AI on top of the same public command surfaces. Checkpoint 16 still needs to wire the pure backend contracts into generated IcyDB repositories and canister-facing DTOs without weakening the Gate D idempotency and query-only boundaries.

## 2026-05-15 - Checkpoint 15 AI Audit

Area: deterministic canister-safe AI
Severity: low
Status: resolved

Observation:

Checkpoint 15 added a split pure `ai` module for bounded actor inputs, small actor state, deterministic command drafts, candidate caps, per-update command caps, no-op fallback, battle defend decisions, and strategic build/recruit/sync/move candidates based on public DTOs. AI command nonces and tie-breaks use the existing keyed `RollKey`/`hash64` helper with explicit session seed, turn, actor, and candidate inputs.

Impact:

The AI layer emits command drafts only; it does not mutate gameplay state directly. The normal command/recovery/event paths remain responsible for applying AI actions. Focused tests cover same-state same-command determinism, legal battle defend fallback, actor/command caps, no-available no-op behavior, fail-closed zero budget, and unsupported actor rejection.

Suggested follow-up:

Checkpoint 16 should expose these AI command drafts through canister update APIs and generated IcyDB command rows using `actor_kind = "ai"`. Full bot opponents remain deferred; the current v1 surface covers neutral battle behavior and optional autopilot-style command generation.

## Checkpoint 16: API DTOs And Client Contract

Added a `domm_game::api` module split across DTO types, view assembly, response/hash helpers, backend wrapper, and contract tests. The fixture API now exposes the public update/query surface from `spec.md`: lobby/session commands, strategic sync and command submission, battle sync/action submission, render-ready game views, map/object/champion/town/battle queries, content manifests, command status, event feeds, match history, and previews.

The API command envelopes now carry `command_id`, `command_type`, `client_nonce`, `payload_hash`, status/phase, effective/durable turns, changed subjects, typed results, emitted events, and structured `ApiError` values. Retry semantics are pinned by tests: same actor+nonce+payload replays the same response, while same actor+nonce with a different payload returns `duplicate_nonce_payload_mismatch`.

The `GameView` DTO is render-ready and includes session/participant summaries, viewport chunks, objects, champion/town/battle views, content hash, event page data, action affordances, and render-time sync metadata. The checkpoint 6A thin client probe now loads this final DTO shape instead of stitching together older lobby/map/event reads.

Audit notes:

- The public API layer is still a deterministic fixture wrapper over `StrategicFixtureBackend`; it does not introduce an IcyDB dependency into `crates/domm-game`.
- Query methods assemble persisted fixture projections and time metadata only. They do not advance turns, recover commands, apply battle timeouts, mutate resources, or finalize victory.
- Event feeds merge lifecycle events with API-envelope events. Private API command payloads are redacted for other participants while public lifecycle events remain readable.
- Battle query/update methods use the first-playable battle fixture until checkpoint 18+ web/client work drives active battle selection from a live match path.
- Focused coverage now includes representative DTO contract checks, Candid roundtrips for `GameView`, `CommandResponse`, and `ApiError`, cursor behavior, event audience redaction, hidden champion errors, command retry/mismatch behavior, content manifest reads, previews, and the client probe render path.

Performance and size notes:

- The API contract tests keep the first visible map payload at four chunk DTOs for the opening viewport.
- API metrics expose response/event counts and the underlying strategic command/event/query counters for checkpoint 17 limit audits.
- Largest new API file is `api/backend.rs` at roughly 1.2k LOC after splitting response/hash helpers into `api/codec.rs`; the DTO, view, and test files remain separate by domain.

Suggested follow-up:

Checkpoint 17 should enforce hard request limits and payload-size caps on this API layer, then checkpoint 18 can consume the DTOs from the web client. When canister entrypoints are wired, this API fixture should be replaced with generated IcyDB repository reads/writes while preserving the same response contracts.

## Checkpoint 17: Cleanup, Compaction, And Storage Limits

Added a dedicated `domm_game::cleanup` module for finished-session compaction rather than growing aftermath/playable files. The cleanup saga writes retained `GameEventTurnSummaryRecord` and `ResourceLedgerTurnSummaryRecord` rows before deleting raw event and ledger rows, preserves `PlayerMatchSummary` and match-history rows, removes map occupancy through the generic occupant key path, removes visibility/known-object rows, and compacts resolved battle operational rows in dependency order.

The cleanup path is explicitly bounded by `CleanupBudget` with v1 defaults of 100 rows and one finished session per update. Repeated bounded calls resume idempotently: summaries are written once, partial deletes continue from the next remaining rows, and each report exposes rows compacted plus operation ordering. Cleanup refuses active sessions and sessions with pending/applying battle commands or unapplied resource ledger rows so recovery data is not destroyed.

Retention and cap behavior now has shared constants for the v1 limits: 7-day raw finished-log retention, 100 retained raw finished sessions, 100 active sessions, 100 cleanup rows per update, and one finished session cleaned per update. `LifecycleBackend::create_session` now refuses the 101st live lobby/starting/active session with the existing active-session limit error path.

Audit notes:

- Cleanup does not physically delete the session record or retained summaries; the test fixture verifies player summaries, match history, event summaries, and ledger summaries survive compaction.
- Raw finished-session logs are compacted when older than the retention window or when the retained raw finished-session rank exceeds 100.
- Battle stacks/occupancy/obstacles/battle rows are deleted after summaries and map occupancy cleanup. Battle commands/events are treated as raw logs and are deleted only when raw-log retention requires compaction.
- Active sessions and active recovery rows fail closed with no writes, preserving replay/recovery safety.
- The final cleanup state leaves no map occupancy, visibility chunks, known-object rows, or resolved battle operational rows for the compacted finished session.

Focused tests cover summary correctness, cleanup order, bounded retry behavior, weak/generic occupancy cleanup, active recovery blocking, raw-log retention decisions, the active-session cap helper, lifecycle enforcement of the 100 active-session cap, and zero finished-session budget fail-closed behavior.

Suggested follow-up:

Checkpoint 17A should enforce the remaining hot-path payload and query limits against this cleanup/API surface, including command/event/ledger active-session retention caps and measured response sizes.

## Checkpoint 17A: Performance Budgets And Query Contracts

Added a split `domm_game::limits` module for v1 hard-limit constants and the first-playable measurement helper, keeping budget definitions out of the gameplay domains. Battle, cleanup, movement, AI, command journals, economy ledger writes, API view pagination, and public query defaults now reference the shared limits rather than local numeric caps.

Enforcement added in this checkpoint:

- Command journals reject oversized command payload JSON, result JSON, event payload JSON, command/effect payload JSON, command retention overflow, event retention overflow, and more than 100 events in one turn.
- Resource ledger writes reject the 3001st retained active-session ledger row while preserving idempotent retries of already-applied rows.
- API game-view and direct viewport/object queries fail closed on zero or over-limit page sizes. Game-view recent events cap at 50, generic event feeds clamp to the 200-row list cap, and command payloads over 4096 bytes return `payload_too_large`.
- Movement submit enforces the unresolved movement-intent cap while still allowing a champion to replace its own pending intent.
- AI actor/candidate/path/chunk/command caps are exposed through shared limit constants, with the existing AI decision code still bounded by actor and candidate caps.

Measurement output from the checkpoint test:

- commands: 32
- events: 42
- queries: 19
- estimated storage rows: 190
- max query bytes: 5072
- estimated response bytes: 5072

Audit notes:

- Hot public query payloads are bounded by request limits before backend reads. Opening map chunks are capped at 9, visible objects at the generic 200 list cap, recent game-view events at 50, and event feeds at 200.
- Event pagination now uses the monotonic event sequence cursor and takes only `limit + 1` rows after the cursor before sorting merged lifecycle/API event slices.
- Map chunks are physically capped at 9 for the v1 scenario, dynamic map/object surfaces are capped at 200, active sessions are capped at 100, and command/event/ledger vectors are capped before append. The current pure fixture backend still uses in-memory vectors, but no user-facing update or query path is left with unbounded growth beyond the v1 active-session limits.
- The eventual generated IcyDB repository layer should preserve these contracts with indexes on session/cursor/owner fields rather than depending on vector scans.

Focused tests cover hard-limit constants, command payload/result/event caps, command retention, event per-turn caps, ledger retention, API viewport/event/list caps, API payload failure responses, first-playable measurement output, and limit validation before unauthorized object query work. The full workspace test suite passes after this checkpoint.

## Checkpoint 17B: Schema Evolution And Migration Safety

Added schema/macro regression coverage for the section 19 migration policy: stable memory IDs, append-only hot entity field prefixes, generated insert defaults versus persisted database defaults, composite index ordinals, weak retained-history relations, strong child deletion ordering, and fail-closed unsupported drift classifications.

Cleanup regression coverage now exercises the operational deletion order against the same policy. The finished-session compactor writes event and ledger summaries first, deletes generic `MapOccupancy` rows by occupant kind/key before target cleanup, drains battle occupancy/obstacles/stacks before battle rows, removes known-object rows before visibility chunks, and deletes raw battle/aftermath events plus ledger rows before command markers. Player match summaries, match history, event summaries, ledger summaries, and the session row remain retained.

Audit notes:

- Section 19 remains append-only for the current schema surface. New required primitive fields need persisted defaults; literal defaults are encoded as database defaults by the macro, while generated/function defaults such as ULIDs and timestamps remain insert-time construction behavior only.
- The schema tests keep history/replay references weak where finished cleanup may retain rows after targets are compacted: event command links, resource ledger command links, battle participant refs, `last_command_id` fields, match summaries, ledger summaries, command actor refs, and artifact ownership.
- Strong child order is now pinned for towns, champions, artifacts, neutral armies, battles, map chunks, visibility, object visits/known objects, commands, effects, events, summaries, movement intents, resource ledger rows, pending effects, AI actor state, and session children.
- Physical `GameSession` deletion is still deferred. The implemented cleanup path only compacts finished-session operational rows after summaries are written, which is consistent with the spec requirement that sessions are not physically removed until commands, effects, battles, occupancy, visibility, and child rows have been compacted or deleted.

IcyDB ergonomics notes:

- Macro metadata distinguishes persisted defaults from generated insert values, but the distinction is easy to miss when reading the entity declaration because literal `default = ...` can imply a database default while function defaults do not.
- Generated model metadata can include macro-added bookkeeping fields after declared fields. Append-only tests use declared prefixes instead of exact field lists so future safe appends remain possible while renames/reorders still fail loudly.
- `MapOccupancy` uses generic `occupant_kind + occupant_id_text` rather than typed relations, so deletion safety must be tested at the gameplay cleanup layer as well as the schema layer.

## IcyDB Ergonomics Notes

Checkpoint 17B captured the current schema-evolution ergonomics: literal defaults can act as persisted defaults, generated/function defaults do not backfill existing rows, generated model metadata may include macro-added bookkeeping fields, and generic occupant keys require gameplay-layer cleanup tests.

## Checkpoint 18: Playable Web Client

Added a split `domm_client_probe::web` layer that models the first playable web client against the public API DTO/update contract. The client is separated into a backend service adapter, durable client state, browser-facing view model, and walkthrough controller rather than extending the original viewport probe into a single large file.

The Gate E walkthrough now drives the first playable path from the client side: lobby creation/join/ready/start, map load, first resource pickup movement, exact command retry, sync retry, build preview/build, turn syncs, recruit preview/recruit, neutral battle trigger, battle panel load, battle defend retry, battle sync, match result, rematch affordance, and basic history/win-loss panel. The UI-level test asserts map rows, town/champion/battle panels, resources, events, command status, checklist completion, replayed idempotent commands, match result, rematch availability, and history rows.

Backend/client contract fix:

- `FixtureApiBackend::sync_session_turn` now also applies movement object interactions, neutral encounters, and lazy income materialization after movement sync. This matches the section 15 client contract that sync can materialize bounded lazy state before the client refreshes visible state.
- Exact retry semantics include every payload field used by the backend hash. The web client preserves the original timestamp as well as nonce and typed payload when retrying idempotent commands.

Audit notes:

- The web client uses only public DTO/update methods for lobby, map, resources, movement, sync, build, recruit, battle state, battle action, command status, events, match history, previews, and content manifest.
- The match-result panel reuses the existing first-playable backend gate report to represent the finished aftermath/victory state in this fixture layer. The canister-facing API still needs the real checkpoint 19 end-to-end fixture to unify the live web route and final aftermath route.
- No additional IcyDB dependency was introduced into `domm-client-probe`; it remains a public contract/client test layer over `domm-game`.

## Checkpoint 19: End-To-End First Playable

Added a split `domm_game::e2e` module for the final first-playable checkpoint fixture. The fixture composes the Gate D public backend victory route with a separate deterministic movement-conflict probe so the automated path covers exploration, pickup, building, recruitment, movement conflict, battle, town capture, and victory without growing the existing `playable` backend driver.

Measurement output from the checkpoint fixture:

- commands: 32
- events: 42
- queries: 19
- estimated storage rows: 190
- max query bytes: 5072
- estimated response bytes: 5072

Spec audit notes:

- The Part 2 first-playable surface is marked implemented across schema, command/event/recovery, deterministic RNG, content, lifecycle, map/visibility, client DTOs, economy, towns, champions, effects, movement, objects, neutrals, battles, aftermath/victory, AI, cleanup, limits, migration safety, and the web-client route.
- No missing required first-playable behavior was found in checkpoint 19.
- Campaign, large procedural maps, naval movement, complex siege, full spellbooks, skill-tree choices, quests, markets, taverns, external dwellings, ranked, guild, diplomacy, and broader meta systems remain deferred to checkpoints 21-27 until bounded Part 2 specs are added.

Manual smoke command added:

```text
make smoke-e2e
```

## Performance And Storage Notes

None yet.

## Spec Ambiguities

None yet.

## Test Gaps

None yet.

## Decisions And Tradeoffs

None yet.
