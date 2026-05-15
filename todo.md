# Degens of Misery & Mayhem Implementation Todo

This file accompanies `spec.md`. It should not restate the spec. Its job is to drive implementation in small, auditable checkpoints until the game is playable and the Part 2 spec is fully executed.

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
- Treat every audit as a work generator: record pass/missing/deferred findings, add missing tasks immediately, then implement them before advancing.
- Keep commit messages tied to checkpoint numbers, for example `DoMM checkpoint 6: map visibility`.
- If git is unavailable, record the attempted commit and reason in `DoMM/notes.md` before advancing.
- Never mark a checkpoint complete with failing tests, unknown deterministic behavior, or an unresolved blocker that affects playability.

## Playability Gates

- [ ] Gate A after checkpoint 5: a headless test can create, join, start, and inspect an active match.
- [ ] Gate B after checkpoint 6A: a minimal client/probe can render the first playable map from public DTOs.
- [ ] Gate C after checkpoint 11A: a headless strategic loop can move, pick up resources, earn income, build, recruit, and trigger a battle.
- [ ] Gate D after checkpoint 14A: a backend-only match can proceed through battle, aftermath, town capture, and victory.
- [ ] Gate E after checkpoint 18: the web client can play the first playable match path end to end.
- [ ] Gate F after checkpoint 20: the implementation, tests, notes, and spec audit agree with the full required first playable scope.

## 0. Project Harness

- [ ] Verify `DoMM/` is an independent git repo, `git status` works from this directory, and the branch/remote policy is documented.
- [ ] Create the canister/backend project layout for the game modules.
- [ ] Add a test harness that can run pure unit tests without deploying a canister.
- [ ] Add deterministic fixture support for scenario seeds, principals, timestamps, command nonces, and IDs.
- [ ] Add documented smoke and regression commands for future agents.
- [ ] Add a headless game-driver test utility that can call public command/query functions in sequence.
- [ ] Notes requirement: create the first entries in `DoMM/notes.md` for any setup friction or IcyDB ergonomics issues.
- [ ] Audit: confirm the harness can test deterministic command flows, recovery flows, and DTO serialization.
- [ ] Commit after this checkpoint.

## 1. IcyDB Schema Baseline

- [ ] Implement the Part 2 entity schema in IcyDB.
- [ ] Keep indexes within IcyDB limits and match the spec's intended lookup paths.
- [ ] Model strong and weak relations according to ownership and cleanup requirements.
- [ ] Add generated or hand-written repository access wrappers where needed.
- [ ] Unit test important schema invariants: unique command keys, event sequence uniqueness, occupancy uniqueness, participant/session uniqueness, and relation cleanup assumptions.
- [ ] Audit: compare every implemented entity, relation, unique index, status field, and default against `spec.md`.
- [ ] Commit after this checkpoint.

## 2. Command, Event, And Recovery Core

- [ ] Implement `GameCommand`, `LobbyCommand`, `CommandEffect`, `PendingEffect`, `GameEvent`, and event sequence allocation.
- [ ] Implement command lifecycle states, idempotency keys, payload hashes, actor model, retryable errors, and command status reads.
- [ ] Implement bounded recovery for pending/applying commands before turn advancement.
- [ ] Unit test command dedupe, payload mismatch rejection, retry recovery, event key idempotency, event sequence gaps, and budget exhaustion.
- [ ] Audit: verify every gameplay mutation has a command/effect/event recovery surface and that query methods do not perform recovery writes.
- [ ] Commit after this checkpoint.

## 3. Deterministic Pseudo-Random Module

- [ ] Implement the keyed pseudo-random helper used by gameplay.
- [ ] Use only explicit inputs: session seed, domain key, turn, command/system key, actor id text, target id text, and roll index.
- [ ] Ban IC raw randomness, host entropy, wall-clock elapsed time, ULID order, event sequence, and mutable RNG cursors from gameplay decisions.
- [ ] Unit test reproducibility, domain separation, roll-index separation, and fixture stability.
- [ ] Audit: scan gameplay code for any direct randomness or time-derived branching.
- [ ] Commit after this checkpoint.

## 4. Ruleset And First Playable Content

- [ ] Seed the first playable ruleset, factions, unit definitions, champion classes, terrain, buildings, objects, battle rules, and one hand-authored 1v1 map.
- [ ] Define the intended first playable walkthrough as fixture data: opening positions, nearby pickup, build/recruit path, neutral fight, town capture, and victory path.
- [ ] Keep deferred features as data omissions, not half-wired runtime behavior.
- [ ] Unit test content loading, definition lookup, numeric caps, and first playable fixture validity.
- [ ] Audit: confirm all content needed for a complete match exists and no deferred Part 1 feature is required for play.
- [ ] Commit after this checkpoint.

## 5. Lobby And Session Lifecycle

- [ ] Implement create session, join session, leave/cancel where needed, and start session.
- [ ] Implement setup as idempotent command/effect phases.
- [ ] Enforce player caps, principal ownership, active session limits, and deterministic setup events.
- [ ] Extend the headless game driver so it can create, join, start, and fetch an active match summary.
- [ ] Unit test duplicate lobby commands, setup recovery, session state transitions, and invalid caller rejection.
- [ ] Audit: confirm setup only marks a session active after all required rows, occupancy, visibility seeds, and setup events exist.
- [ ] Gate A: run the headless create/join/start/inspect smoke path and fix all failures.
- [ ] Commit after this checkpoint.

## 6. Map, Terrain, Occupancy, And Visibility

- [ ] Implement map chunks, terrain blobs, movement-cost blobs, flags, discovered/visible bitsets, known objects, and occupancy rows.
- [ ] Implement viewport reads with limits, cursors, visibility redaction, and `not_visible` behavior.
- [ ] Add a stable fixture snapshot for the opening viewport of the first playable map.
- [ ] Unit test chunk encoding, bitset encoding, movement costs, single-tile and multi-tile occupancy, cleanup by occupant key, and hidden-object redaction.
- [ ] Audit: confirm the frontend can render the visible map without needing backend-private rows or speculative query state.
- [ ] Commit after this checkpoint.

## 6A. Thin Client Skeleton And API Probe

- [ ] Build the smallest possible client/probe that can connect to the backend or fixture, load a match, and render map chunks, visible champions, towns, resources, events, and sync-required state.
- [ ] Use only public APIs and DTOs; do not read repository internals or test-only state.
- [ ] Add a minimal component or integration test for loading the opening viewport.
- [ ] Run backend regression tests after adding the probe.
- [ ] Audit: record every missing DTO field, ambiguous API behavior, or inefficient read pattern in `DoMM/notes.md`; fix backend contract gaps before advancing.
- [ ] Gate B: verify the first playable map renders from public DTOs.
- [ ] Commit after this checkpoint.

## 7. Resources, Economy, And Lazy State

- [ ] Implement participant balances, resource ledger entries, income sources, turn summaries, and bounded lazy materialization.
- [ ] Enforce resource caps and saturating math.
- [ ] Implement ownership cutover rules for income-producing sources.
- [ ] Extend the headless smoke path with a resource pickup and one income materialization.
- [ ] Unit test idempotent ledger writes, partial recovery, income catch-up caps, capture cutover, cap rejection, and summary generation.
- [ ] Audit: confirm every resource mutation can recover safely after a trap and cannot double-apply.
- [ ] Commit after this checkpoint.

## 8. Towns, Buildings, And Recruitment

- [ ] Implement town ownership, buildings, recruit pools, build commands, recruit commands, target selection, and garrison/champion stack merge rules.
- [ ] Keep derived town caches repairable from authoritative rows.
- [ ] Extend the headless smoke path with one build and one recruit action from the first playable walkthrough.
- [ ] Unit test build prerequisites, duplicate builds, recruit pool growth, champion-at-town checks, full target errors, stack compatibility, and resource spend rollback/recovery.
- [ ] Audit: confirm town state, recruit state, resources, events, and occupancy remain coherent through command recovery.
- [ ] Commit after this checkpoint.

## 9. Champions, Armies, Artifacts, And Strategic State

- [ ] Implement champions, army stacks, artifact instances, equipment, statuses, movement points, ownership, and visibility-facing champion DTOs.
- [ ] Implement deterministic artifact capture rules needed by the first playable version.
- [ ] Unit test stack caps, champion ownership, status transitions, artifact ownership changes, and DTO redaction.
- [ ] Audit: confirm champion state supports movement, battle entry, battle aftermath, defeat, garrisoning, and victory checks.
- [ ] Commit after this checkpoint.

## 10. Movement Intents And Turn Sync

- [ ] Implement replaceable movement intents, submit-time validation, movement snapshots, turn-final movement resolution, partial cursors, and `sync_session_turn`.
- [ ] Resolve simultaneous movement in deterministic microsteps.
- [ ] Use idempotent system commands for turn resolution.
- [ ] Extend the headless smoke path with movement across at least two turn windows.
- [ ] Unit test intent replacement, hidden blockers, tile conflicts, crossing conflicts, object interaction stops, partial sync, budget exhaustion, and recovery after a trap.
- [ ] Audit: confirm update calls recover pending/applying commands before advancing the turn and queries never finalize movement.
- [ ] Commit after this checkpoint.

## 11. World Objects, Pickups, Mines, And Captures

- [ ] Implement object visits, visit keys, pickups, mines, central objectives, scoring fields, and ownership changes.
- [ ] Implement command/effect-backed object interaction during movement.
- [ ] Extend the headless smoke path through the first playable pickup/mine/object sequence.
- [ ] Unit test once-only visits, refreshable visits, resource rewards, mine income start turn, object redaction, and duplicate interaction recovery.
- [ ] Audit: confirm object interactions do not bypass command idempotency, visibility rules, resource ledger rules, or occupancy cleanup.
- [ ] Commit after this checkpoint.

## 11A. Strategic Headless Playable Gate

- [ ] Create or update a single headless strategic fixture that starts a match and plays the non-battle loop: inspect map, move, pick up resources, receive income, build, recruit, interact with an object, and reach a battle trigger.
- [ ] Run this fixture through public command/query functions only.
- [ ] Add assertions for visible state after each step so regressions are localized.
- [ ] Measure command count, event count, query sizes, and any obvious slow path; record concerns in `DoMM/notes.md`.
- [ ] Audit: compare the strategic loop against `spec.md`; any missing required behavior becomes immediate work before battle implementation continues.
- [ ] Gate C: the strategic loop is playable headlessly and remains part of the regression suite.
- [ ] Commit after this checkpoint.

## 12. Battle Engine Baseline

- [ ] Implement battle rows, battle stacks, battle occupancy, obstacles, legal action generation, active stack selection, and battle views.
- [ ] Keep the tactical engine pure where possible so it can be unit tested without IcyDB.
- [ ] Add a deterministic battle fixture from the first playable walkthrough.
- [ ] Unit test initiative order, seeded tie-breaks, occupancy uniqueness, legal moves, legal attacks, damage fixtures, deaths, morale/default actions if present, and battle DTOs.
- [ ] Audit: confirm `BattleOccupancy` is authoritative and cached stack coordinates can be repaired.
- [ ] Commit after this checkpoint.

## 13. Battle Commands, Timeouts, And Recovery

- [ ] Implement `submit_battle_action`, `sync_battle`, action deadlines, deterministic auto-defend, timeout system commands, and battle event emission.
- [ ] Enforce bounded timeout processing and `battle_sync_incomplete`.
- [ ] Unit test duplicate actions, action after timeout, player action racing timeout, auto-defend idempotency, battle recovery, and event ordering.
- [ ] Audit: confirm battle updates recover applying commands and due timeout commands before validating the caller command.
- [ ] Commit after this checkpoint.

## 14. Aftermath, Town Capture, Defeat, And Victory

- [ ] Implement battle aftermath, strategic position updates, town capture, garrison survivor placement, champion defeat, surrender/retreat handling, and victory finalization.
- [ ] Implement stalemate scoring and bounded winner checks.
- [ ] Extend the headless smoke path through battle resolution, capture, and victory.
- [ ] Unit test town battles, neutral battles, champion defeats, artifact capture, no-elimination-while-battle-active, capture income cutover, and max-turn stalemate.
- [ ] Audit: confirm victory, defeat, scoring, events, resources, occupancy, and visibility are all updated by idempotent commands/effects.
- [ ] Commit after this checkpoint.

## 14A. Backend Match Playable Gate

- [ ] Create a backend-only first playable fixture that starts from lobby creation and ends in victory using public command/query functions.
- [ ] Include at least one recovery retry, one turn sync, one battle sync, one resource mutation, one recruit, one movement conflict or blocker case, and one event refresh.
- [ ] Keep the fixture deterministic and fast enough to run in the normal regression suite.
- [ ] Measure command costs, event volume, storage row growth, and slow queries; record concerns in `DoMM/notes.md`.
- [ ] Audit: read the first playable requirements in `spec.md` and fix any missing backend behavior before starting AI or full client work.
- [ ] Gate D: the backend can complete a real match path without private test hooks.
- [ ] Commit after this checkpoint.

## 15. AI Player

- [ ] Implement deterministic canister-safe AI command generation.
- [ ] Keep AI decisions bounded, fast, and based only on visible/persisted state plus deterministic pseudo-random keyed rolls.
- [ ] Enforce per-turn AI command caps.
- [ ] Unit test same-state same-command behavior, command caps, no available action behavior, and fail-closed budget behavior.
- [ ] Audit: scan AI code for IC randomness, wall-clock branching, unbounded search, hidden-information reads, and direct gameplay writes.
- [ ] Commit after this checkpoint.

## 16. API DTOs And Client Contract

- [ ] Implement command responses, lobby responses, typed command results, errors, event views, game views, object views, town/champion/battle views, and pagination.
- [ ] Ensure responses include enough information for a web client to render and recover without private backend assumptions.
- [ ] Update the thin client/probe from checkpoint 6A to use the final DTO shapes.
- [ ] Add contract tests that compare representative DTO fixtures to the client expectations.
- [ ] Unit test candid/serialization compatibility, error mapping, cursor behavior, event audience filtering, redaction, and retry/sync contract.
- [ ] Audit: compare every public method and DTO against `spec.md`; add missing fields before moving on.
- [ ] Commit after this checkpoint.

## 17. Cleanup, Compaction, And Storage Limits

- [ ] Implement finished-session cleanup, event summaries, ledger summaries, battle cleanup, occupancy cleanup, and retained summaries.
- [ ] Enforce raw log retention and active session caps.
- [ ] Unit test cleanup ordering, weak relation cleanup, summary correctness, no deletion of active recovery data, and bounded cleanup budgets.
- [ ] Audit: confirm cleanup cannot break replay/recovery for active sessions and does not leave orphaned occupancy or visibility rows.
- [ ] Commit after this checkpoint.

## 18. Playable Web Client

- [ ] Expand the thin client/probe into the first playable web client against the public API contract.
- [ ] Render map, towns, champions, resources, events, movement intents, battle state, legal battle actions, command status, and sync-required states.
- [ ] Implement retry behavior for idempotent commands and sync flows.
- [ ] Run the first playable walkthrough manually through the web client.
- [ ] Add UI-level tests for key flows where practical.
- [ ] Run backend regression tests after client integration changes.
- [ ] Audit: confirm no client flow requires an endpoint, DTO field, or event that the backend does not provide.
- [ ] Gate E: the first playable match path can be played end to end in the web client.
- [ ] Commit after this checkpoint.

## 19. End-To-End First Playable

- [ ] Create an automated 1v1 fixture that starts a match and plays through exploration, pickup, building, recruitment, movement conflict, battle, town capture, and victory.
- [ ] Add manual smoke instructions for running the game locally.
- [ ] Measure command costs, query sizes, event volume, and storage growth for the fixture.
- [ ] Unit/regression test the full fixture and all smaller modules touched by fixes.
- [ ] Audit: read `spec.md` from Part 2 start to end and mark every required first-playable behavior implemented, deferred, or missing.
- [ ] If anything is missing, add new todo entries above this checkpoint, implement them, retest, and commit before continuing.
- [ ] Commit after this checkpoint.

## 20. Final Spec Audit

- [ ] Perform a full implementation audit against `spec.md`.
- [ ] Re-run all playability gates from Gate A through Gate E.
- [ ] Verify all command paths are idempotent and recoverable.
- [ ] Verify all query paths are read-only and do not materialize gameplay state.
- [ ] Verify deterministic pseudo-randomness is used everywhere gameplay needs randomness.
- [ ] Verify all important modules have focused unit tests.
- [ ] Verify the full regression suite passes from a clean checkout.
- [ ] Verify `DoMM/notes.md` has useful implementation notes for IcyDB maintainers and no unresolved blocker that invalidates the playable game.
- [ ] Fix every audit finding or explicitly update `spec.md` and this todo if the intended behavior changed.
- [ ] Gate F: the implementation, tests, notes, and spec audit agree with the full required first playable scope.
- [ ] Commit the final audit fixes.
