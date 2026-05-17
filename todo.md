# Degens of Misery & Mayhem Implementation Todo

This file accompanies `spec.md`. It should not restate the spec. Its job is to
drive the v1 first-playable implementation through final release hardening.
Expansion backlog belongs in `spec.v2.md`, not in the v1 todo.

`DoMM/` is the game repository. Run game implementation, tests, audits, and commits from this repo root. The surrounding `icydb` workspace is dependency/context, not the game repo.

Every checkpoint must use the same loop:

```text
0. Before starting new work, run the existing regression suite from the previous checkpoint.
1. Run focused unit tests for the modules changed in this checkpoint.
2. Run the full regression test suite that exists at that point.
3. Audit the implementation against spec.md for the checkpoint scope.
4. If the audit finds missing work, add the work immediately and repeat tests.
5. Update DoMM/notes.md with bugs, blockers, limitations, performance issues, and IcyDB ergonomics notes.
6. Commit the completed checkpoint in the DoMM git repo before starting the next one.
```

Do not advance to a later checkpoint with known spec drift in the current checkpoint. If the spec audit exposes missing schema, missing recovery paths, missing idempotency, unsafe query behavior, or a non-deterministic gameplay result, fix it in the same checkpoint.

## Operating Rules

- Prefer vertical slices over broad unfinished layers. Each checkpoint should preserve or improve the latest playable smoke path.
- Build only what the first playable map needs unless `spec.md` marks the behavior as mandatory infrastructure.
- Keep all game code, tests, client code, specs, todos, and implementation notes in the DoMM repo unless a separate IcyDB library change is explicitly required.
- Keep pure rule modules separate from IcyDB persistence when practical, so deterministic gameplay can be tested cheaply.
- `domm-game` may remain the deterministic rules/DTO engine, but it is not the final backend. The required production backend is `canisters/degens` with typed public canister endpoints backed by IcyDB rows in `schema/degens`.
- Public gameplay must use typed IcyDB create/load/update paths through domain repository modules. Generic SQL/DDL may exist only for controller-gated diagnostics or test fixture loading, never as the normal game API.
- Do not call any fixture, pure driver, or in-memory backend "complete e2e" unless it is explicitly labeled fixture e2e. The accepted end-to-end proof is Pocket-IC driving `domm-degens-canister` through public Candid endpoints and verifying IcyDB-backed persisted state.
- Treat every audit as a work generator: record pass/missing/deferred findings, add missing tasks immediately, then implement them before advancing.
- If a system exists only in Part 1, do not implement it blindly. First add or update a bounded Part 2 design covering schema, indexes, command path, recovery path, deterministic pseudo-random keys, caps, DTOs, tests, and cleanup.
- Keep commit messages tied to checkpoint numbers, for example `DoMM checkpoint 6: map visibility`.
- If git is unavailable, record the attempted commit and reason in `DoMM/notes.md` before advancing.
- Never mark a checkpoint complete with failing tests, unknown deterministic behavior, or an unresolved blocker that affects playability.

## Playability Gates

- [x] Gate A after checkpoint 5: a headless test can create, join, start, and inspect an active match.
- [x] Gate B after checkpoint 6A: a minimal client/probe can render the first playable map from public DTOs.
- [x] Gate C after checkpoint 11B: a headless strategic loop can move, pick up resources, earn income, build, recruit, interact with neutral armies, and trigger a battle.
- [x] Gate D after checkpoint 14A: a backend-only match can proceed through battle, aftermath, town capture, and victory.
- [x] Gate E after checkpoint 18: the web client can play the fixture-backed first playable match path end to end.
- [x] Gate F after checkpoint 19A: every required game endpoint is inventoried, named, typed, and mapped to a canister method plus existing fixture behavior.
- [x] Gate G after checkpoint 19B: canister code is split by API/service/repository domains and `domm-degens-canister` exposes every endpoint in Candid.
- [x] Gate H after checkpoint 19C: IcyDB repository modules can create, read, update, page, and clean up the first-playable durable row surface without generic SQL gameplay paths.
- [x] Gate I after checkpoint 19F: Pocket-IC can drive lobby, setup, content, map, visibility, account, event, command-status, and preview endpoints against the real canister.
- [x] Gate J after checkpoint 19H: Pocket-IC can drive the strategic loop against IcyDB-backed canister endpoints through pickup, income, build, recruit, movement, object interaction, and neutral encounter.
- [x] Gate K after checkpoint 19I: Pocket-IC can drive battle, aftermath, town capture, champion defeat, victory, match summary, and match history against IcyDB-backed canister endpoints.
- [x] Gate L after checkpoint 19J: Pocket-IC can play the complete first-playable 1v1 route from registration through victory using only public canister endpoints and IcyDB state.
- [x] Gate M after checkpoint 19K: the web/client probe can run against a real canister adapter, not only `FixtureApiBackend`.
- [x] Gate N after checkpoint 20: the implementation, tests, notes, and spec audit agree with the full required first playable canister/IcyDB scope.
- [x] Gate O after checkpoint 26: the v1 release audit passes and V2-only backlog is isolated in `spec.v2.md`.

## 0. Project Harness

- [x] Verify `DoMM/` is an independent git repo, `git status` works from this directory, and the branch/remote policy is documented.
- [x] Create the canister/backend project layout for the game modules.
- [x] Add a test harness that can run pure unit tests without deploying a canister.
- [x] Add the test layers required by `spec.md`: pure Rust unit tests, schema/macro tests, generated-session tests, and Pocket-IC canister tests.
- [x] Add deterministic fixture support for scenario seeds, principals, timestamps, command nonces, and IDs.
- [x] Add documented smoke and regression commands for future agents.
- [x] Add a headless game-driver test utility that can call public command/query functions in sequence.
- [x] Notes requirement: create the first entries in `DoMM/notes.md` for any setup friction or IcyDB ergonomics issues.
- [x] Audit: confirm the harness can test deterministic command flows, recovery flows, and DTO serialization.
- [x] Commit after this checkpoint.

## 1. IcyDB Schema Baseline

- [x] Implement the Part 2 entity schema in IcyDB.
- [x] Keep indexes within IcyDB limits and match the spec's intended lookup paths.
- [x] Model strong and weak relations according to ownership and cleanup requirements.
- [x] Add generated or hand-written repository access wrappers where needed.
- [x] Keep generic SQL/DDL disabled for public gameplay APIs; allow diagnostics or fixture loading only behind controller/test-only gates.
- [x] Unit test important schema invariants: unique command keys, event sequence uniqueness, occupancy uniqueness, participant/session uniqueness, and relation cleanup assumptions.
- [x] Audit: compare every implemented entity, relation, unique index, status field, and default against `spec.md`.
- [x] Commit after this checkpoint.

## 2. Command, Event, And Recovery Core

- [x] Implement `GameCommand`, `LobbyCommand`, `CommandEffect`, `PendingEffect`, `GameEvent`, and event sequence allocation.
- [x] Implement command lifecycle states, idempotency keys, payload hashes, actor model, retryable errors, and command status reads.
- [x] Implement bounded recovery for pending/applying commands before turn advancement.
- [x] Implement the durable event log as the replay/audit surface with audience redaction, numeric cursors, event summaries, and event-key idempotency.
- [x] Unit test command dedupe, payload mismatch rejection, retry recovery, event key idempotency, event sequence gaps, and budget exhaustion.
- [x] Audit: verify every gameplay mutation has a command/effect/event recovery surface and that query methods do not perform recovery writes.
- [x] Commit after this checkpoint.

## 3. Deterministic Pseudo-Random Module

- [x] Implement the keyed pseudo-random helper used by gameplay.
- [x] Use only explicit inputs: session seed, domain key, turn, command/system key, actor id text, target id text, and roll index.
- [x] Ban IC raw randomness, host entropy, wall-clock elapsed time, ULID order, event sequence, and mutable RNG cursors from gameplay decisions.
- [x] Unit test reproducibility, domain separation, roll-index separation, and fixture stability.
- [x] Audit: scan gameplay code for any direct randomness or time-derived branching.
- [x] Commit after this checkpoint.

## 4. Ruleset And First Playable Content

- [x] Seed the first playable ruleset, factions, unit definitions, champion classes, terrain, buildings, objects, battle rules, and one hand-authored 1v1 map.
- [x] Define the intended first playable walkthrough as fixture data: opening positions, nearby pickup, build/recruit path, neutral fight, town capture, and victory path.
- [x] Implement `get_content_manifest` data for factions, champion classes, terrain, units, buildings, spells, artifacts, map objects, asset keys, and ruleset hash/version, even if some lists are empty in v1.
- [x] Keep deferred features as data omissions, not half-wired runtime behavior.
- [x] Unit test content loading, definition lookup, numeric caps, and first playable fixture validity.
- [x] Audit: confirm all content needed for a complete match exists and no deferred Part 1 feature is required for play.
- [x] Commit after this checkpoint.

## 5. Lobby And Session Lifecycle

- [x] Implement create session, join session, leave/cancel where needed, and start session.
- [x] Implement player account basics: `register_player`, `get_my_player`, display names, principal mapping, and duplicate registration behavior.
- [x] Implement `mark_ready`, `get_session`, `get_my_participant`, and the match-history shell needed by the frontend.
- [x] Implement setup as idempotent command/effect phases.
- [x] Enforce player caps, principal ownership, active session limits, and deterministic setup events.
- [x] Extend the headless game driver so it can create, join, start, and fetch an active match summary.
- [x] Unit test duplicate lobby commands, setup recovery, session state transitions, and invalid caller rejection.
- [x] Audit: confirm setup only marks a session active after all required rows, occupancy, visibility seeds, and setup events exist.
- [x] Gate A: run the headless create/join/start/inspect smoke path and fix all failures.
- [x] Commit after this checkpoint.

## 6. Map, Terrain, Occupancy, And Visibility

- [x] Implement map chunks, terrain blobs, movement-cost blobs, flags, discovered/visible bitsets, known objects, and occupancy rows.
- [x] Implement viewport reads with limits, cursors, visibility redaction, and `not_visible` behavior.
- [x] Add a stable fixture snapshot for the opening viewport of the first playable map.
- [x] Unit test chunk encoding, bitset encoding, movement costs, single-tile and multi-tile occupancy, cleanup by occupant key, and hidden-object redaction.
- [x] Audit: confirm the frontend can render the visible map without needing backend-private rows or speculative query state.
- [x] Commit after this checkpoint.

## 6A. Thin Client Skeleton And API Probe

- [x] Build the smallest possible client/probe that can connect to the backend or fixture, load a match, and render map chunks, visible champions, towns, resources, events, and sync-required state.
- [x] Use only public APIs and DTOs; do not read repository internals or test-only state.
- [x] Add a minimal component or integration test for loading the opening viewport.
- [x] Run backend regression tests after adding the probe.
- [x] Audit: record every missing DTO field, ambiguous API behavior, or inefficient read pattern in `DoMM/notes.md`; fix backend contract gaps before advancing.
- [x] Gate B: verify the first playable map renders from public DTOs.
- [x] Commit after this checkpoint.

## 7. Resources, Economy, And Lazy State

- [x] Implement participant balances, resource ledger entries, income sources, turn summaries, and bounded lazy materialization.
- [x] Enforce resource caps and saturating math.
- [x] Implement ownership cutover rules for income-producing sources.
- [x] Extend the headless smoke path with a resource pickup and one income materialization.
- [x] Unit test idempotent ledger writes, partial recovery, income catch-up caps, capture cutover, cap rejection, and summary generation.
- [x] Audit: confirm every resource mutation can recover safely after a trap and cannot double-apply.
- [x] Commit after this checkpoint.

## 8. Towns, Buildings, And Recruitment

- [x] Implement town ownership, buildings, recruit pools, build commands, recruit commands, target selection, and garrison/champion stack merge rules.
- [x] Keep derived town caches repairable from authoritative rows.
- [x] Implement `preview_build_town_structure` and `preview_recruit_units` with the same validation and affordability rules but without writes.
- [x] Extend the headless smoke path with one build and one recruit action from the first playable walkthrough.
- [x] Unit test build prerequisites, duplicate builds, recruit pool growth, champion-at-town checks, full target errors, stack compatibility, and resource spend rollback/recovery.
- [x] Audit: confirm town state, recruit state, resources, events, and occupancy remain coherent through command recovery.
- [x] Commit after this checkpoint.

## 9. Champions, Armies, Artifacts, And Strategic State

- [x] Implement champions, army stacks, artifact instances, equipment, statuses, movement points, ownership, and visibility-facing champion DTOs.
- [x] Implement deterministic artifact capture rules needed by the first playable version.
- [x] Implement the v1-safe champion progression foundation: experience field updates, level cap enforcement, reward hooks, and explicit no-op/deferred behavior for skill-tree choices.
- [x] Unit test stack caps, champion ownership, status transitions, experience caps, artifact ownership changes, equipment uniqueness, and DTO redaction.
- [x] Audit: confirm champion state supports movement, battle entry, battle aftermath, defeat, garrisoning, and victory checks.
- [x] Commit after this checkpoint.

## 9A. Effects, Abilities, Spells, And Status Hooks

- [x] Implement a bounded effect-key dispatch layer for unit abilities, artifact effects, building effects, object rewards, and spell effects.
- [x] Support the v1 required effects and return typed disabled reasons for unsupported advanced effect behavior.
- [x] Ensure `CastAbility` is either fully supported for the v1 content that uses it or never returned as an enabled legal action.
- [x] Unit test effect dispatch, unsupported effect rejection, deterministic pseudo-random effect rolls, status key caps, and DTO disabled reasons.
- [x] Audit: compare all `effect_key`, `ability_keys`, `SpellDefinition`, `ChampionSpell`, artifact, building, and object effect usage in the first playable content against implemented handlers.
- [x] Commit after this checkpoint.

## 10. Movement Intents And Turn Sync

- [x] Implement replaceable movement intents, submit-time validation, movement snapshots, turn-final movement resolution, partial cursors, and `sync_session_turn`.
- [x] Resolve simultaneous movement in deterministic microsteps.
- [x] Use idempotent system commands for turn resolution.
- [x] Implement `preview_move_path` with the same deterministic validation rules but without writes.
- [x] Extend the headless smoke path with movement across at least two turn windows.
- [x] Unit test intent replacement, hidden blockers, tile conflicts, crossing conflicts, object interaction stops, partial sync, budget exhaustion, and recovery after a trap.
- [x] Audit: confirm update calls recover pending/applying commands before advancing the turn and queries never finalize movement.
- [x] Commit after this checkpoint.

## 11. World Objects, Pickups, Mines, And Captures

- [x] Implement object visits, visit keys, pickups, mines, central objectives, scoring fields, and ownership changes.
- [x] Implement command/effect-backed object interaction during movement.
- [x] Extend the headless smoke path through the first playable pickup/mine/object sequence.
- [x] Unit test once-only visits, refreshable visits, resource rewards, mine income start turn, object redaction, and duplicate interaction recovery.
- [x] Audit: confirm object interactions do not bypass command idempotency, visibility rules, resource ledger rules, or occupancy cleanup.
- [x] Commit after this checkpoint.

## 11A. Neutral Armies And Encounter Starts

- [x] Implement neutral army rows, stacks, strength labels, visibility redaction, occupancy, and growth fields.
- [x] Implement neutral-army encounter triggers from strategic movement and object interaction.
- [x] Implement v1 neutral behavior: guard aggression, no roaming unless promoted by a later spec update, optional join logic disabled unless bounded by Part 2.
- [x] Unit test strength-label calculation, growth materialization or explicit no-op, occupancy blocking, battle creation from neutral contact, and cleanup after defeat.
- [x] Audit: confirm neutral-army behavior supports the first playable map and matches the canister AI limits for neutral battle behavior.
- [x] Commit after this checkpoint.

## 11B. Strategic Headless Playable Gate

- [x] Create or update a single headless strategic fixture that starts a match and plays the non-battle loop: inspect map, move, pick up resources, receive income, build, recruit, interact with an object, and reach a battle trigger.
- [x] Run this fixture through public command/query functions only.
- [x] Add assertions for visible state after each step so regressions are localized.
- [x] Measure command count, event count, query sizes, and any obvious slow path; record concerns in `DoMM/notes.md`.
- [x] Audit: compare the strategic loop against `spec.md`; any missing required behavior becomes immediate work before battle implementation continues.
- [x] Gate C: the strategic loop is playable headlessly and remains part of the regression suite.
- [x] Commit after this checkpoint.

## 12. Battle Engine Baseline

- [x] Implement battle rows, battle stacks, battle occupancy, obstacles, legal action generation, active stack selection, and battle views.
- [x] Keep the tactical engine pure where possible so it can be unit tested without IcyDB.
- [x] Add a deterministic battle fixture from the first playable walkthrough.
- [x] Implement v1 morale/luck policy exactly: disabled or capped to the spec's allowed equivalent effect, with explicit DTO/event explanation.
- [x] Unit test initiative order, seeded tie-breaks, occupancy uniqueness, legal moves, legal attacks, damage fixtures, deaths, morale/luck policy, status caps, and battle DTOs.
- [x] Audit: confirm `BattleOccupancy` is authoritative and cached stack coordinates can be repaired.
- [x] Commit after this checkpoint.

## 13. Battle Commands, Timeouts, And Recovery

- [x] Implement `submit_battle_action`, `sync_battle`, action deadlines, deterministic auto-defend, timeout system commands, and battle event emission.
- [x] Enforce bounded timeout processing and `battle_sync_incomplete`.
- [x] Unit test duplicate actions, action after timeout, player action racing timeout, auto-defend idempotency, battle recovery, and event ordering.
- [x] Audit: confirm battle updates recover applying commands and due timeout commands before validating the caller command.
- [x] Commit after this checkpoint.

## 14. Aftermath, Town Capture, Defeat, And Victory

- [x] Implement battle aftermath, strategic position updates, town capture, garrison survivor placement, champion defeat, surrender/retreat handling, and victory finalization.
- [x] Implement stalemate scoring and bounded winner checks.
- [x] Write `PlayerMatchSummary` and match-history rows when the match finishes.
- [x] Extend the headless smoke path through battle resolution, capture, and victory.
- [x] Unit test town battles, neutral battles, champion defeats, artifact capture, no-elimination-while-battle-active, capture income cutover, and max-turn stalemate.
- [x] Audit: confirm victory, defeat, scoring, events, resources, occupancy, and visibility are all updated by idempotent commands/effects.
- [x] Commit after this checkpoint.

## 14A. Backend Match Playable Gate

- [x] Create a backend-only first playable fixture that starts from lobby creation and ends in victory using public command/query functions.
- [x] Include at least one recovery retry, one turn sync, one battle sync, one resource mutation, one recruit, one movement conflict or blocker case, and one event refresh.
- [x] Keep the fixture deterministic and fast enough to run in the normal regression suite.
- [x] Measure command costs, event volume, storage row growth, and slow queries; record concerns in `DoMM/notes.md`.
- [x] Audit: read the first playable requirements in `spec.md` and fix any missing backend behavior before starting AI or full client work.
- [x] Gate D: the backend can complete a real match path without private test hooks.
- [x] Commit after this checkpoint.

## 15. AI Player

- [x] Implement deterministic canister-safe AI command generation.
- [x] Keep AI decisions bounded, fast, and based only on visible/persisted state plus deterministic pseudo-random keyed rolls.
- [x] Enforce per-turn AI command caps.
- [x] Unit test same-state same-command behavior, command caps, no available action behavior, and fail-closed budget behavior.
- [x] Audit: scan AI code for IC randomness, wall-clock branching, unbounded search, hidden-information reads, and direct gameplay writes.
- [x] Commit after this checkpoint.

## 16. API DTOs And Client Contract

- [x] Implement all public update/query methods in `spec.md`, including registration, session, sync, command submission, view queries, content manifest, command status, event feed, match history, and preview endpoints.
- [x] Implement command responses, lobby responses, typed command results, errors, event views, game views, object views, town/champion/battle views, and pagination.
- [x] Ensure responses include enough information for a web client to render and recover without private backend assumptions.
- [x] Update the thin client/probe from checkpoint 6A to use the final DTO shapes.
- [x] Add contract tests that compare representative DTO fixtures to the client expectations.
- [x] Unit test candid/serialization compatibility, error mapping, cursor behavior, event audience filtering, redaction, and retry/sync contract.
- [x] Audit: compare every public method and DTO against `spec.md`; add missing fields before moving on.
- [x] Commit after this checkpoint.

## 17. Cleanup, Compaction, And Storage Limits

- [x] Implement finished-session cleanup, event summaries, ledger summaries, battle cleanup, occupancy cleanup, and retained summaries.
- [x] Enforce raw log retention and active session caps.
- [x] Unit test cleanup ordering, weak relation cleanup, summary correctness, no deletion of active recovery data, and bounded cleanup budgets.
- [x] Audit: confirm cleanup cannot break replay/recovery for active sessions and does not leave orphaned occupancy or visibility rows.
- [x] Commit after this checkpoint.

## 17A. Performance Budgets And Query Contracts

- [x] Enforce every v1 hard limit in `spec.md`: active sessions, participants, champions, towns, map chunks, dynamic objects, active battles, battle rounds, AI caps, command/event retention, movement sync caps, battle timeout caps, cleanup caps, and payload size caps.
- [x] Add regression tests for viewport limits, pagination, path caps, payload caps, active-session caps, recovery budgets, movement slicing, battle-timeout slicing, and cleanup slicing.
- [x] Add lightweight measurement output for the headless first playable fixture: command counts, row counts, event counts, response sizes, and slow-path notes.
- [x] Audit: confirm no update or query scans all commands, events, champions, towns, chunks, or dynamic objects for a session.
- [x] Commit after this checkpoint.

## 17B. Schema Evolution And Migration Safety

- [x] Add schema evolution tests for append-only fields, `db_default`, relation strength, index declarations, default-vs-db-default behavior, and unsupported drift fail-closed behavior.
- [x] Add deletion-order tests covering strong child rows, weak history rows, map occupancy cleanup by generic occupant key, battle cleanup, artifacts, known objects, visibility, commands, effects, events, and summaries.
- [x] Document the migration policy and any IcyDB ergonomics issues in `DoMM/notes.md`.
- [x] Audit: compare schema evolution, retention, and deletion behavior against `spec.md` section 19.
- [x] Commit after this checkpoint.

## 18. Playable Web Client

- [x] Expand the thin client/probe into the first playable web client against the public API contract.
- [x] Render map, towns, champions, resources, events, movement intents, battle state, legal battle actions, command status, and sync-required states.
- [x] Implement retry behavior for idempotent commands and sync flows.
- [x] Run the first playable walkthrough manually through the web client.
- [x] Include first-match checklist, match result, and basic match history/win-loss display from the spec's frontend scope.
- [x] Add UI-level tests for key flows where practical.
- [x] Run backend regression tests after client integration changes.
- [x] Audit: confirm no client flow requires an endpoint, DTO field, or event that the backend does not provide.
- [x] Gate E: the fixture-backed first playable match path can be played end to end in the web client.
- [x] Commit after this checkpoint.

## 19. Fixture End-To-End First Playable

- [x] Create an automated 1v1 fixture that starts a match and plays through exploration, pickup, building, recruitment, movement conflict, battle, town capture, and victory.
- [x] Add manual smoke instructions for running the game locally.
- [x] Measure command costs, query sizes, event volume, and storage growth for the fixture.
- [x] Unit/regression test the full fixture and all smaller modules touched by fixes.
- [x] Audit: read `spec.md` from Part 2 start to end and mark every fixture-backed first-playable behavior implemented, deferred, or missing.
- [x] Record that this is fixture e2e only, not canister/IcyDB e2e; add the mandatory canister, IcyDB, and Pocket-IC gates below before final first-playable audit.
- [x] Commit after this checkpoint.

## 19A. Canister Endpoint Inventory And Contract Gate

- [x] Create a canonical endpoint inventory for every public game method required by `spec.md`, `FixtureApiBackend`, and the web/client probe.
- [x] Account/lobby/session endpoint inventory must include: `register_player`, `get_my_player`, `create_session`, `join_session`, `mark_ready`, `start_session`, `get_session`, `get_my_participant`, and `get_match_history`.
- [x] Render/query endpoint inventory must include: `get_game_view`, `get_visible_map_chunks`, `get_visible_objects`, `get_my_champions`, `get_champion_view`, `get_town_view`, `get_battle_state`, `get_content_manifest`, `get_events_after`, and `get_command_status`.
- [x] Preview/update endpoint inventory must include: `preview_move_path`, `preview_build_town_structure`, `preview_recruit_units`, `submit_move_intent`, `sync_session_turn`, `submit_build_town_structure`, `submit_recruit_units`, `sync_battle`, and `submit_battle_action`.
- [x] Document whether out-of-route session actions are implemented now or explicitly deferred with typed disabled responses and matching client behavior.
- [x] Define Candid input/output DTOs for every endpoint using the same semantics as `domm-game` public DTOs; no endpoint may expose raw IcyDB rows as the public UI contract.
- [x] Add an endpoint contract test that fails if any required method is missing from the canister Candid export.
- [x] Add a Pocket-IC endpoint-presence test that calls every required method at least once and proves missing methods fail the test as method-not-found/trap rather than being silently skipped.
- [x] Audit: compare the endpoint list against `spec.md` section 15 and the current `FixtureApiBackend`; every mismatch must become immediate todo work.
- [x] Gate F: every required game endpoint is inventoried, named, typed, and mapped to a canister method plus existing fixture behavior.
- [x] Commit after this checkpoint.

## 19B. Canister API, Service, And Repository Layout

- [x] Split `canisters/degens/src` into domain modules before adding large endpoint bodies: `api/`, `services/`, `repos/`, `dto/`, `auth/`, `errors/`, and `metrics/`.
- [x] Keep endpoint files grouped by domain: account/lobby/session, game view/map, movement, economy/town/recruitment, battle, events/command status, content, history, cleanup, diagnostics.
- [x] Keep repository files grouped by durable row ownership: players, sessions, commands/events/effects, content, map/visibility/occupancy, economy, towns, champions/artifacts, movement, neutrals, battles, aftermath/history, cleanup.
- [x] Implement public canister endpoint shells for the full 19A inventory with typed arguments and typed errors; no endpoint may call `FixtureApiBackend`.
- [x] Wire Candid export tests so CI catches endpoint renames, missing methods, or DTO drift.
- [x] Keep generated SQL/DDL disabled for public gameplay builds; diagnostics must be controller-gated and never used by game endpoints.
- [x] Run `cargo check -p domm-degens-canister` and the Candid endpoint inventory test.
- [x] Gate G: canister code is split by API/service/repository domains and `domm-degens-canister` exposes every endpoint in Candid.
- [x] Commit after this checkpoint.

## 19C. IcyDB Repository Foundation

- [x] Implement typed IcyDB repository helpers around `db().load`, `db().create`, `db().insert`, `db().update`, `insert_many_atomic`, and paged query flows.
- [x] Prohibit generic SQL from gameplay repositories. SQL may be used only in controller-gated diagnostics, fixture loading, or test-only tooling.
- [x] Add repository error mapping from IcyDB errors into stable gameplay/API errors without leaking storage internals to clients.
- [x] Implement indexed lookup helpers for principal/account, session/participant, command idempotency, event feed cursors, map chunk windows, occupancy, visibility, town/champion/battle ownership, and match history.
- [x] Add native repository tests for create/read/update/page/delete behavior against generated schema types where possible.
- [x] Add tests proving repository queries use bounded limits and indexed lookup fields for every hot path.
- [x] Audit: compare every repository helper against the schema indexes and Part 2 lookup paths.
- [x] Gate H: IcyDB repository modules can create, read, update, page, and clean up the first-playable durable row surface without generic SQL gameplay paths.
- [x] Commit after this checkpoint.

## 19D. IcyDB-Backed Lobby, Session, Commands, And Setup

- [x] Implement `register_player`, `get_my_player`, `create_session`, `join_session`, `mark_ready`, `start_session`, `get_session`, and `get_my_participant` against IcyDB rows.
- [x] Persist lobby commands, game commands, command effects, pending effects, setup events, participants, session rows, player rows, and match-history shell rows in IcyDB.
- [x] Implement setup recovery so interrupted `start_session` resumes from IcyDB command/effect rows and cannot mark a session active before required durable rows exist.
- [x] Enforce player caps, session caps, principal ownership, active-session limits, duplicate nonce replay, payload mismatch rejection, and typed authorization failures through canister endpoints.
- [x] Add Pocket-IC tests for account/lobby/session endpoints and duplicate/retry behavior through the real canister.
- [x] Audit: compare canister lobby/session behavior against Gate A and the pure lifecycle tests; fix drift immediately.
- [x] Commit after this checkpoint.

## 19E. IcyDB-Backed Content, Map, Visibility, Town, Champion, And Opening Views

- [x] Seed first-playable ruleset/content definition rows into IcyDB during setup or a controller-gated fixture loader that the canister tests can call.
- [x] Persist map chunks, terrain/movement/flag blobs, visibility chunks, known objects, occupancy rows, towns, recruit pools when present, champions, champion stacks, artifacts, opening neutrals, opening occupancy, and initial economy rows in IcyDB.
- [x] Implement `get_content_manifest`, `get_game_view`, `get_visible_map_chunks`, `get_visible_objects`, `get_my_champions`, `get_champion_view`, and `get_town_view` from IcyDB rows. 19E keeps heavy champion/town detail on the dedicated endpoints because embedding those rows in `get_game_view` exceeds the Pocket-IC single-query instruction cap.
- [x] Preserve visibility redaction and hidden-object behavior exactly at the canister boundary.
- [x] Add Pocket-IC tests that render the opening viewport from canister calls only and verify the same public DTO facts as Gate B.
- [x] Audit: confirm queries are read-only and do not materialize movement, income, visibility writes, recovery, or events.
- [x] Commit after this checkpoint.

## 19F. Pocket-IC Endpoint Completeness Gate

- [x] Build a Pocket-IC endpoint coverage harness that installs `domm-degens-canister` and calls every endpoint listed in 19A.
- [x] For each endpoint, assert one valid or deliberately invalid typed response; method-not-found, decode failure, unexpected trap, or untyped string error fails the test.
- [x] Verify Candid argument and response compatibility for all command responses, query DTOs, pages, previews, content manifests, battle views, event views, command status views, and match-history pages.
- [x] Verify anonymous/unauthorized calls fail with typed authorization errors without bypassing query limits.
- [x] Verify all list/query endpoints enforce cursor/limit contracts at the canister boundary.
- [x] Gate I: Pocket-IC can drive lobby, setup, content, map, visibility, account, event, command-status, and preview endpoints against the real canister.
- [x] Commit after this checkpoint.

## 19G. IcyDB-Backed Strategic Gameplay

19G status: the first IcyDB-backed strategic canister slice is implemented and covered by native and Pocket-IC tests. The implemented slice covers movement intent submission, bounded turn-sync movement application, persisted sync cursor slicing for long movement intents, crossing-conflict resolution through cursor slices, stationary enemy blocker resolution, terrain-cost validation, first-class movement snapshot rows plus command effects, final and partial champion occupancy updates, owner visibility refresh writes, resource pickup, guarded mine contact without premature capture, guarded neutral battle handoff rows, unguarded mine capture/income, town building, recruitment into town garrison, command replay, payload-mismatch rejection, and movement sync recovery from a pending command row. 19I owns full battle view/action/sync and aftermath.

- [x] Wire `submit_move_intent`, `sync_session_turn`, `submit_build_town_structure`, and `submit_recruit_units` to real IcyDB service implementations instead of placeholder errors.
- [x] Persist command/effect/event rows, movement intents, movement snapshot rows/effects, participant object visits, resource ledger rows, participant balances, resource turn summaries, town buildings, recruit pools, garrison stacks, world-object updates, champion occupancy updates, visibility chunk updates, and mine ownership/income state for the first strategic slice.
- [x] Add Pocket-IC coverage for resource pickup, long-intent movement cursor slices, crossing movement conflict, unguarded mine income, guarded neutral contact, build, recruit, exact command retry, and nonce payload mismatch through public canister endpoints.
- [x] Add schema/macro and native service coverage proving `MovementSnapshot` is a first-class IcyDB entity with indexed lookup and rows written by `sync_session_turn`.
- [x] Persist partial movement progress by writing champion/occupancy/visibility state, trimming the pending `MovementIntent.path_json`, recording a partial `MovementSnapshot`, and emitting `movement_sync_incomplete` without advancing the turn.
- [x] Persist guarded neutral battle handoff rows on movement contact: `Battle`, initial `BattleStack`, `BattleOccupancy`, obstacle rows, champion `in_battle_id`, neutral `in_battle` state, command effect, and event payload `battle_id`.
- [x] Reconcile movement blocker parity for stationary enemy champion blockers and normalize seeded scenario-key champion occupancy rows to persisted champion IDs during movement updates.
- [x] Implement `submit_move_intent`, `sync_session_turn`, `preview_move_path`, object interactions, resource pickup, lazy income materialization, mine/object ownership changes, `submit_build_town_structure`, `preview_build_town_structure`, `submit_recruit_units`, and `preview_recruit_units` against IcyDB rows.
- [x] Persist movement intents, movement snapshots, sync command/effect rows, object visits, object command effects, resource ledger entries, turn summaries, balances, income ownership state, town buildings, recruit pools, garrison stacks, and related events.
- [x] Preserve idempotent command replay, payload mismatch rejection, object stop behavior, and sync budget slicing for the implemented strategic slice.
- [x] Preserve/reconcile movement recovery from pending/applying command rows against the pure movement coverage.
- [x] Add Pocket-IC coverage for strategic blocker parity cases not covered by the crossing-conflict test.
- [x] Add recovery coverage through native seeded-pending movement sync plus Pocket-IC exact command retry and cursor retry flows.
- [x] Audit: compare canister strategic behavior against Gate C and the pure movement/economy/town/world-object/neutral tests.
- [x] Commit after this checkpoint.

## 19H. Pocket-IC Strategic First-Playable Gate

- [x] Create a Pocket-IC 1v1 strategic fixture that starts from canister player registration and reaches the first neutral battle trigger using only public canister endpoints.
- [x] Assert persisted IcyDB row state after each milestone: session active, opening viewport visible, pickup visited, resources updated, income summarized, building built, recruit pool decremented, garrison updated, movement snapshots written, neutral encounter pending.
- [x] Measure canister command count, event count, query count, response sizes, IcyDB row growth, stable-memory growth where available, and any slow update/query path.
- [x] Gate J: Pocket-IC can drive the strategic loop against IcyDB-backed canister endpoints through pickup, income, build, recruit, movement, object interaction, and neutral encounter.
- [x] Commit after this checkpoint.

## 19I. IcyDB-Backed Battle, Aftermath, Victory, And History

- [x] Implement `get_battle_state`, `submit_battle_action`, `sync_battle`, battle timeout recovery, battle event feeds, battle aftermath, neutral defeat, town capture, champion defeat, victory finalization, match summaries, and match history against IcyDB rows.
- [x] Persist battle rows, battle stacks, battle occupancy, obstacles, battle commands, battle events, aftermath command/effect rows, town/garrison/occupancy updates, champion/artifact updates, victory events, player summaries, and history rows.
- [x] Preserve battle action idempotency, timeout auto-defend determinism, battle sync budget slicing, no-elimination-while-battle-active, capture income cutover, artifact capture, and final winner scoring.
- [x] Add Pocket-IC tests for battle view, legal action submission, retry, timeout sync, neutral aftermath, town capture, champion defeat, victory, event feed, command status, and match history.
- [x] Audit: compare canister battle/aftermath behavior against Gate D and pure battle/aftermath tests.
- [x] Gate K: Pocket-IC can drive battle, aftermath, town capture, champion defeat, victory, match summary, and match history against IcyDB-backed canister endpoints.
- [x] Commit after this checkpoint.

## 19J. Pocket-IC Full First-Playable Canister E2E

- [x] Create the real first-playable e2e test: install `domm-degens-canister` in Pocket-IC and play the full 1v1 path from registration through victory using only public canister Candid endpoints.
- [x] Cover exploration, pickup, income, building, recruitment, movement conflict or blocker, neutral battle, battle action retry, battle sync, neutral aftermath, town capture, champion defeat, victory, event refresh, command status polling, and match-history read.
- [x] Verify no step calls `FixtureApiBackend`, pure in-memory backend helpers, test-only private state, or generic SQL gameplay paths.
- [x] Verify final IcyDB persisted rows: finished session, winner participant, defeated neutral, captured town owner, defeated enemy champion, summary rows, history rows, event feed, command rows, effects, movement snapshots, battle cleanup/retention state, and visibility/occupancy coherence.
- [x] Record real canister metrics and storage observations in `DoMM/notes.md`.
- [x] Gate L: Pocket-IC can play the complete first-playable 1v1 route from registration through victory using only public canister endpoints and IcyDB state.
- [x] Commit after this checkpoint.

## 19K. Client Against Real Canister And Canister Performance Gate

- [x] Add a canister-backed client/probe adapter that implements the same web-client backend trait used by the fixture client.
- [x] Run the Gate E web client walkthrough against Pocket-IC canister endpoints, not `FixtureApiBackend`.
- [x] Verify client retry, sync-required, event refresh, battle panel, result panel, and match-history behavior through the canister adapter.
- [x] Compare fixture DTOs and canister DTOs for representative states to catch drift between pure rules and persisted projections.
- [x] Measure real canister update/query response sizes, IcyDB row growth, stable-memory growth, command/event retention, and cleanup behavior for the first playable path.
- [x] Add manual smoke instructions for running canister-backed tests locally.
- [x] Gate M: the web/client probe can run against a real canister adapter, not only `FixtureApiBackend`.
- [x] Commit after this checkpoint.

## 20. First Playable Final Spec Audit

- [x] Perform a full implementation audit against `spec.md`.
- [x] Re-run all playability gates from Gate A through Gate M.
- [x] Verify every 19A endpoint exists in canister Candid and has Pocket-IC coverage.
- [x] Verify every public gameplay endpoint uses typed IcyDB repositories and no generic SQL gameplay path.
- [x] Verify all command paths are idempotent and recoverable.
- [x] Verify all query paths are read-only and do not materialize gameplay state.
- [x] Verify deterministic pseudo-randomness is used everywhere gameplay needs randomness.
- [x] Verify all important modules have focused unit tests.
- [x] Verify the full regression suite passes from a clean checkout.
- [x] Verify `DoMM/notes.md` has useful implementation notes for IcyDB maintainers and no unresolved blocker that invalidates the playable game.
- [x] Fix every audit finding or explicitly update `spec.md` and this todo if the intended behavior changed.
- [x] Gate N: the implementation, tests, notes, and spec audit agree with the full required first playable canister/IcyDB scope.
- [x] Commit the final audit fixes.

## 21. Full Spec Expansion Triage

- [x] Review every Part 1 system deferred by Part 2 and classify it as implement-now, promote-to-Part-2-first, or still-deferred.
- [x] For each promote-to-Part-2-first item, update `spec.md` with bounded schema, indexes, command paths, recovery paths, deterministic pseudo-random keys, caps, DTOs, tests, cleanup, and frontend requirements before implementation starts.
- [x] Add concrete implementation checkpoints below for any newly promoted system.
- [x] Audit: confirm no Part 1 system is silently implemented without a bounded Part 2 design.
- [x] Commit after this checkpoint.

## 22. Champion Progression, Skills, Magic, And Spellbook Expansion

- [x] Promote and implement champion skill trees, level-up choices, spell learning, battle spellcasting, adventure spellcasting, mana reset rules, and advanced status effects only after Part 2 is expanded for them.
- [x] Add or update entities if `ChampionSpell`, `SpellDefinition`, `BattleStack.status_keys`, and effect keys are insufficient.
- [x] Unit test XP rewards, level-up choices, skill prerequisites, spell targeting, mana costs, status duration, dispel/stacking rules, deterministic rolls, and DTO/legal-action behavior.
- [x] Add Candid inventory and Pocket-IC e2e coverage for every new canister endpoint before marking the bucket complete.
- [x] Audit: confirm progression and magic do not break v1 battle determinism, command recovery, or frontend action affordances.
- [x] Commit after this checkpoint.

## 23. Expanded Economy, Taverns, Marketplace, And External Dwellings

- [x] Promote and implement the bounded checkpoint 23 slice: tavern champion hiring, marketplace trading, one external dwelling, direct recruitment, and dwelling growth; keep defeated champion reappearance, advanced economy buildings, and broader resource-source variety deferred until a later bounded spec.
- [x] Add command/API/DTO support for trade, hire, dwelling recruitment, and any new lazy growth or income rules.
- [x] Unit test affordability, market rates, tavern candidate determinism, external dwelling ownership, direct recruitment, growth routing, resource ledger recovery, and frontend affordances.
- [x] Add Candid inventory and Pocket-IC e2e coverage for every new canister endpoint before marking the bucket complete.
- [x] Audit: confirm these systems cannot double-spend, double-reward, bypass visibility, or create unbounded scans.
- [x] Commit after this checkpoint.

## 24. Quests, Objectives, Advanced Victory, And Scenario Rules

- [x] Promote and implement the bounded checkpoint 24 slice: central-objective tracking, one opening scenario quest, deterministic weekly world events, quest reward claim, active conquest/objective/quest/max-turn rule rows, and explicit disabled rows for artifact victory, king-of-the-hill, survival, and scenario-specific defeat.
- [x] Add bounded entities or effect rows for quest state, objective ownership, rewards, progress visibility, and victory checks.
- [x] Unit test objective progress, reward idempotency, visibility/redaction, bounded victory scoring, max-turn interactions, and event history.
- [x] Add Candid inventory and Pocket-IC e2e coverage for every new canister endpoint before marking the bucket complete.
- [x] Audit: confirm every advanced victory path is indexed, bounded, recoverable, and visible to the client.
- [x] Commit after this checkpoint.

## 25. World-Generation Boundary Rows And Skirmish Settings

- [x] Promote and implement the bounded checkpoint 25 slice: skirmish settings, deterministic first-playable generated preview metadata, and explicit disabled IcyDB rows for V2 world-expansion affordances.
- [x] Add deterministic generation fixtures and cap checks before allowing larger maps or extra movement layers. Larger-map materialization remains disabled until a future spec is written.
- [x] Unit test seeded preview stability, generation caps, disabled route/rule validation, and skirmish disabled flags.
- [x] Add Candid inventory and Pocket-IC e2e coverage for every new canister endpoint before marking the bucket complete.
- [x] Audit: confirm generated or larger content still satisfies canister performance budgets and query contracts.
- [x] Commit after this checkpoint.

## 26. V1 Final Audit

- [x] Re-read `spec.md` end to end and produce a v1 implementation coverage table: implemented, intentionally disabled in v1, or removed from v1 scope.
- [x] Verify V2-only features live in `spec.v2.md` and are not listed as active v1 implementation tasks.
- [x] Document the late `submit_move_intent` contract cleanup: fix it for v1 or explicitly document it as a v2/non-blocking contract cleanup.
- [x] Re-run the full regression suite, all playability gates, all schema/migration tests, and all client contract tests.
- [x] Verify `DoMM/notes.md` contains actionable IcyDB ergonomics, blocker, performance, and limitation notes discovered during the full implementation.
- [x] Fix every v1 audit finding or update `spec.md` and this todo to make the intended scope explicit.
- [x] Gate O: the v1 release audit passes and V2-only backlog is isolated in `spec.v2.md`.
- [x] Commit the full-spec audit fixes.

## 27. Spec 1.1 Decision Lock-In

- [x] Enforce the locked late-submit contract: once a durable turn-resolution
  job is accepted for the current turn, old-turn commands fail before command
  creation with `backend_work_pending` or `turn_expired`; exact retries replay
  stored status.
- [x] Keep `get_session` as a lobby/setup shell and make active gameplay
  metadata come from `get_game_view`, `get_content_manifest`, and dedicated
  bounded render endpoints.
- [x] Add `GameView.omitted_fields`; omitted collections must report
  `has_more = false` and `next_cursor = None`.
- [x] Treat static map terrain/movement/flags as public surveyed-base-map data
  while keeping dynamic objects, owners, occupants, battle details, and events
  visibility-gated.
- [x] Reject `RecruitTarget::Champion` for town recruitment before command
  creation, resource spending, pool decrement, or stack mutation; reserve it
  for v2.
- [x] Document and test remote direct dwelling recruitment into owned active
  world-map champions; reject inactive, defeated, garrisoned, in-battle, or
  enemy champions before mutation.
- [x] Implement week-two tavern offers and recruit growth
  projection/materialization; keep town hall income, unrest/pacification,
  recruit-pool halving, and desperation income deferred to v2.
- [x] Make neutral battle tactical detail private to involved participants.
- [x] Make hidden town build/recruit events audience-scoped or redacted.
- [x] Keep battle spellcasting for learned v1.1 spells; expose retreat and
  surrender only as disabled/deferred action metadata.
- [x] Keep disabled/future systems out of enabled action affordances and prevent
  disabled-only sync/update paths from appending public gameplay events.
- [x] Return a claimed-specific non-retryable error for `accept_quest` on
  claimed quests.

## 28. Local DFX Deploy And Agent-Run `blast` Gates

- [x] Add the local deployment design first: generate the canister Candid from
  the Rust canister export, configure local DFX deploy so the installed wasm has
  public `candid:service` metadata, and document the exact fresh-replica deploy
  command.
- [x] Put a hard gate before any `blast` gameplay testing: deploy a fresh local
  canister with DFX, then run `blast scan <canister_id> --host
  http://127.0.0.1:$(dfx info webserver-port)` directly from the agent shell
  and compare it with the generated Candid plus
  `get_canister_endpoint_inventory`.
- [x] Use direct `blast call` commands with multiple identities, not committed
  scripts: at minimum `--id 1`, `--id 2`, and a third identity when validating
  3-player turn behavior.
- [x] Record the direct command lines, canister id, principals, endpoint scan
  result, session ids, and IcyDB diagnostic outputs in `spec.missing.md` during
  playability audits.
- [ ] Only after the deploy and direct `blast scan` gates pass, run deeper
  agent-driven gameplay checks for register/create/join/ready/start, active
  gameplay inspection, map movement, battles, economy, events, and
  `icydb_snapshot`/`icydb_metrics` corruption evidence.
  - Earlier 2026-05-17 direct `blast` runs passed fresh deploy/scan, lobby/setup,
    movement preview, pickup/render projection, build/recruit economy, battle
    trigger, event feed, snapshot, metrics, and small diagnostic batches.
    At that point this item stayed open because `get_battle_state` still
    returned IC0522 on the fresh local canister after the guarded-mine battle
    was created.
  - Follow-up work removed internal battle-state pagination, added all-ready
    direct turn sync, and added delayed partial-job retries. The fresh local
    route now proves all-ready pickup sync and build/recruit, but the local
    replica still terminates before the final guarded-object battle trigger can
    return; keep this item open.
  - Follow-up manual-sync hardening now delays overlapping current-turn jobs
    after partial manual sync, completes them after manual turn advancement,
    schedules the next deadline, and prevents turn advancement while pending
    movement intents remain. Fresh DFX evidence still leaves the gate open:
    scan/setup/pickup/build/recruit/guarded-intent all pass, but local timer
    processing still terminates PocketIC before the guarded battle id can be
    read through `get_battle_state`.
  - Follow-up delayed partial retries removed the final-player `end_turn` race:
    scan/setup/pickup/build/recruit/guarded intent and both turn-ending calls now
    apply through public `blast`. The guarded route still stays open because the
    final guarded manual sync hits the local 50B update instruction limit before
    returning `neutral_encounter_pending` or a battle id.
  - Follow-up guarded battle phase slicing clears the trigger/read blocker for
    the direct local route: fresh scan exposed 63 methods, setup reached
    `active` at `start:9`, guarded preview returned cost `30` with
    `guarded_object`, sync slices `0..8` returned
    `movement_sync_incomplete`, sync `9` returned `neutral_encounter_pending`
    plus `session_turn_synced`, `get_battle_state` returned an `active` neutral
    battle with enabled legal actions, and `icydb_snapshot` reported
    `corrupted_entries=0` and `corrupted_keys=0`. Keep this item open for guard
    defeat, mine capture, later income, full diagnostics, one-call setup, and
    regression/PocketIC evidence.
  - Follow-up guarded battle/capture/income smoke on direct local session
    `01KRTT8MHY0000000000000008` resolved battle
    `01KRTTH6XV0000000000000004`, emitted `mine_captured`,
    `neutral_defeated`, `battle_aftermath_applied`, and later
    `income_materialized` with `{"gold":250}`, rendered the mine as
    owned/captured with the defeated neutral absent, and kept
    `icydb_snapshot` corruption counts at zero. Keep this item open for the
    full collect/build/recruit walkthrough, diagnostics/metrics batches, and
    automated PocketIC/regression evidence.
  - Follow-up PocketIC work added and passed guarded-route assertions for
    capture, aftermath idempotency, stale neutral render absence, captured mine
    owner/state, income, champion battle, town capture, victory, and final
    diagnostics. The passing command was
    `cargo test -p domm-pocket-ic-tests --test canister_endpoints
    pocket_ic_gate_l_first_playable_canister_e2e_uses_public_endpoints_and_icydb_state`.
    Keep this broader local-DFX item open for the remaining full direct
    collect/build/recruit walkthrough and diagnostics/metrics batches, not for
    the automated Gate L guarded-route assertion blocker.
- [x] Do not add committed `blast` scripts or blast-based automated tests unless
  explicitly requested later; PocketIC remains the automated IC e2e test layer.
