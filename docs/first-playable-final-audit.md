# First Playable Final Audit

Checkpoint 26 audits the v1 release scope against `spec.md`, `todo.md`, and the
V2 backlog split in `spec.v2.md`. Checkpoint 20 remains the historical Gate N
first-playable audit; Gate O is the final docs/spec/regression agreement pass.

## Gate 14 Closure

Status: pass as of 2026-05-18.

The active `spec.md` v1.1 contract has implementation and test evidence:

| Area | Closure evidence |
| --- | --- |
| `spec.md` Part 1 | Design background only under the redaction note. Anything without the bounded Part 2 model, endpoint, recovery path, deterministic key, caps, and tests remains deferred. |
| `spec.md` Part 2 database, architecture, turn, command, API, lazy, budget, evolution, and first-playable sections | Covered by typed IcyDB repositories, public Candid endpoints, setup/turn/battle system jobs, command/effect/event recovery, bounded render endpoints, diagnostics, and the first-playable route tests. |
| `spec.1.1.md` Topics 0-16 | Closed through the Gate 1-14 checklist; remaining expansion mechanics are in `spec.v2.md`. |
| Historical `spec.missing.md` findings | Classified as fixed, deferred, or historical evidence in the 2026-05-18 status note at the top of that file. No blocking v1.1 item remains open there. |
| Public read endpoints versus IcyDB diagnostics | Gate L reads public state through `get_session`, `get_visible_objects`, `get_champion_view`, `get_town_view`, `get_events_after`, and `get_match_history`, then verifies the expected typed IcyDB diagnostic row counts for the same route. Local blast evidence also records `icydb_snapshot` with zero corrupted entries/keys and small-batch diagnostic row counts. |
| Local new-developer route | `dfx.json`, `make build-wasm`, `make dfx-deploy-local`, and `docs/local-deploy-blast.md` provide the committed local deploy path. The local checklist uses one `start_session` call, polls setup state, plays through public endpoints, and verifies IcyDB diagnostics in batches. |

## Gate O Result

Status: pass. `make regression` completed successfully for checkpoint 26 on
2026-05-16.

Baseline before checkpoint-26 docs edits: `make regression` passed from the
DoMM repo root on 2026-05-16.

Post-edit verification: `make regression` passed again after the v1 coverage
table, V2 split, and movement-contract documentation updates. This regression
includes the pure rule tests, canister service/repository tests, schema and
macro tests, generated-session harness, Pocket-IC endpoint gates, Gate L full
canister e2e, Gate M canister-backed client probe, and doc tests.

## V1 Coverage Table

| Area | V1 audit result |
| --- | --- |
| IcyDB schema and repositories | Implemented for the v1 row surface with typed domain repositories, indexed hot-path tests, relation/deletion tests, and generic SQL limited to controller-gated diagnostics. |
| Public canister endpoints | Implemented and inventoried in Candid. Pocket-IC coverage calls the account, lobby, setup, content, map, visibility, event, command-status, preview, strategic, battle, history, champion progression/magic, expanded economy, scenario progress, and world-generation boundary endpoints. |
| Lobby/session/setup | Implemented with registration, create/join/ready/start, setup phases, setup recovery, participant rows, active-session caps, and match-history shell rows. |
| Content and first playable map | Implemented for the hand-authored compact 1v1 map, content manifest, factions, units, buildings, objects, terrain, towns, champions, neutrals, and walkthrough targets. |
| Map, visibility, and rendering | Implemented through map chunks, terrain/movement/flag blobs, visibility chunks, known objects, occupancy, redaction, and bounded viewport/list queries. |
| Movement and turn sync | Implemented with replaceable movement intents, persisted turn-sync slices, movement snapshots, object stops, conflict/blocker handling, visibility refreshes, and IcyDB command/effect/event rows. |
| Late movement-intent hard rejection | Intentionally deferred from v1. V1 keeps sync-driven turn closure through `sync_required` metadata and `sync_session_turn`; hard canister-side rejection of late new `submit_move_intent` calls is documented in `spec.v2.md` as non-blocking contract cleanup. |
| Resources and economy | Implemented for pickups, ledger-backed spends/rewards, income materialization, mine ownership cutover, marketplace trading, tavern hiring, and one external dwelling slice. Broader economy variety is V2. |
| Towns and recruitment | Implemented for town ownership, buildings, build previews/commands, recruit pools, garrison/champion targets, direct recruitment, and recovery-safe resource spends. |
| Champions, armies, artifacts, magic | Implemented for v1 champion state, army stacks, status, XP/level caps, bounded level-up/spell learning/adventure spell/battle spell slice, artifact ownership/equipment/capture, and explicit unsupported-effect responses. Larger spell/skill/artifact-set expansion is V2. |
| Objects, quests, objectives, scenario rules | Implemented for pickups, mines, central objective progress, one opening quest slice, deterministic weekly events, and disabled rows for advanced victory variants. Richer scenario systems are V2. |
| Neutral armies | Implemented for guard behavior, strength labels, visibility redaction, encounter starts, battle handoff, and defeat cleanup. Roaming/join/bribe behavior is disabled or V2. |
| Battle, aftermath, and victory | Implemented with battle rows, stack/occupancy/obstacle rows, legal actions, timeout sync, action idempotency, aftermath, town capture, champion defeat, victory finalization, summaries, and match history. |
| AI | Implemented only as bounded deterministic canister-safe command generation and neutral/battle support needed by v1. Full bot opponents/general planners are V2. |
| Cleanup, retention, budgets, migration | Implemented with bounded cleanup, summaries, active-session protection, retention caps, schema-evolution tests, deletion-order tests, payload/path/query caps, and performance budget coverage. |
| Client/probe | Implemented for fixture-backed and canister-backed web-client probes. Gate E and Gate M cover the first-playable route through public DTOs. |
| World-generation/skirmish boundary | Implemented only as skirmish settings, deterministic preview metadata, and explicit disabled boundary rows. Active siege, naval movement, and larger/procedural map gameplay are V2. |
| Removed from v1 scope | Sequential turns, hotseat-only backend flow, monolithic `GameState` persistence, generic SQL gameplay APIs, active siege/naval gameplay, large procedural maps, durable rematch, ranked/guild/diplomacy/meta systems, large content packs, and full bot opponents. |

## Gate N History

Status: pass. `make regression` completed successfully for checkpoint 20.

The first playable production backend is `canisters/degens`. Public gameplay
uses typed Candid endpoints backed by IcyDB rows in `schema/degens`; the pure
`domm-game` fixture remains the deterministic rules, DTO, and fixture contract
layer.

## Implemented Scope

| Area | Audit result |
| --- | --- |
| IcyDB schema and repositories | Implemented with typed domain repositories, indexed hot-path plan tests, relation/deletion tests, and generic SQL scans limited to diagnostics. |
| Public canister endpoints | 28 required 19A endpoints were exported in Candid, inventoried, and called by Pocket-IC tests at Gate N. Checkpoints 22-25 expand the live inventory to 54 with champion progression, magic, tavern, market, external dwelling, objective, quest, world-event, scenario-rule, skirmish, procedural-map, naval-route, and siege-rule endpoints. |
| Lobby/session/setup | IcyDB-backed registration, create/join/ready/start, setup phases, setup recovery, participant rows, and match-history shells are implemented. |
| Content/opening views | First-playable content, map, visibility, objects, champions, towns, neutrals, economy, and opening viewport projections are persisted and queryable. |
| Strategic gameplay | Movement intents, sync slicing, resource pickup, mine capture/income, building, recruitment, object visits, visibility, and battle handoff persist IcyDB rows. |
| Battle/aftermath/victory | Battle views/actions/sync, timeout commands, neutral defeat, champion defeat, town capture, victory, summaries, and history persist IcyDB rows. |
| Web/client contract | Fixture-backed and canister-backed client probes complete the first-match route through result/history panels. |
| Idempotency and recovery | Command nonce replay, payload mismatch, setup recovery, movement sync recovery, battle action retry, and timeout sync recovery are covered. |
| Query contracts | Render/query endpoints enforce limits and remain projection/read surfaces; update endpoints own materialization and recovery. The canister `get_game_view` is a lightweight session shell; clients compose map/object/town/champion/battle detail through dedicated endpoints to stay below IC query budgets. |
| Determinism | Gameplay pseudo-randomness uses explicit keyed helpers; canister public time-sensitive endpoints derive time at the boundary rather than trusting caller time. |
| Performance and storage | Pocket-IC gates record update/query counts, response sizes, selected row growth, and stable-memory observations. |

## Audit Fixes In Checkpoint 20

- Removed public `now_ms` Candid arguments from movement and battle endpoints.
- Switched Pocket-IC tests to advance Pocket-IC time for turn and battle deadlines.
- Removed server time from time-sensitive command idempotency payloads.
- Added native guard tests for caller-controlled time arguments, fixture/placeholder backend calls, and `now_ms` payload drift.
- Removed the stale canister service placeholder file.

## Deferred Scope

Expansion scope outside the v1 first playable is now tracked in `spec.v2.md`.
Checkpoint 21 records the historical classification in
`docs/full-spec-expansion-triage.md`; checkpoint 25 adds persisted
world-generation boundary rows and explicit disabled states rather than active
naval/siege/larger-map gameplay.

## Known Follow-Up

Hard rejection of late new `submit_move_intent` calls is explicitly deferred to
`spec.v2.md` as non-blocking contract cleanup. The public API no longer trusts
caller time, and the current playable route uses the existing sync-driven turn
submission behavior with `sync_required` plus `sync_session_turn`.
