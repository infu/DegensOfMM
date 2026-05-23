# Degens of Misery & Mayhem Missing V1 Playability Work

Generated from three read-only code/spec audits and a local `blast` canister
play audit on 2026-05-16. This file is for work that appears missing or
under-proven for a truly playable v1 game. It does not list V2 backlog such as
active siege, naval movement, large procedural maps, ranked, guilds, diplomacy,
durable rematch, or full bot opponents.

## Current Status - 2026-05-22

Status: no blocking v1.1 item remains open in this file. The first playable
backend and canister-backed client contract are green for UI implementation:
`DOMM_BENCH_JOBS=5 scripts/run-benchmarks.sh` passed in
`target/benchmarks/20260522-164948-5cfd001`, required endpoint coverage is
`59/59`, Gate M passed, and the hard-target audit reported zero violations.
The older sections below are retained as audit history; their active outcomes
are classified here.

| Historical finding | Current classification |
| --- | --- |
| Local DFX deploy path, public Candid metadata, and blast identity/controller setup | Fixed by committed `dfx.json`, `make build-wasm`, `make dfx-deploy-local`, README/TESTING notes, and `docs/local-deploy-blast.md`. |
| Repeated `start_session` setup driving | Fixed for the v1.1 contract: clients call `start_session` once, then poll `get_session` or `get_setup_progress` while canister-owned setup jobs continue. |
| Real 60-second local DFX turn waits and manual sync friction | Documented as a local-smoke constraint; PocketIC tests use time advancement for fast regression. A configurable short local turn duration remains V2/dev-tooling cleanup, not a v1.1 blocker. |
| Public render/projection and battle read budget gaps | Fixed by dedicated bounded render endpoints, live-row render projection, battle start/read slicing, and Gate L/Gate M regression evidence. |
| Guarded mine, battle aftermath, capture, income, victory, and history route | Fixed and covered by Gate L, Gate K, render-projection, battle-round, command-recovery, and local blast evidence. |
| Champion-target town recruitment preview/submit mismatch and mutation risk | Fixed. Valid same-tile owned active champion targets are supported; invalid champion targets fail before resource spend, pool mutation, stack mutation, or pending-command leakage. |
| Diagnostics drift and large local diagnostic snapshots | Fixed for v1.1 by controller-gated small-batch diagnostics and endpoint inventory coverage; large all-entity local diagnostic snapshots stay intentionally unsupported. |
| Auth/redaction gaps | Fixed by visibility-redaction coverage plus the endpoint-auth matrix added on 2026-05-18. |
| Expansion mechanics such as active siege/naval gameplay, richer economy variants, full bot AI, broader scenarios, and local/same-tile dwelling recruitment | Deferred to `spec.v2.md`. |

## Historical Summary - 2026-05-16

The following summary is preserved from the original audit and is superseded by
the current status table above. Historical details below intentionally preserve
now-obsolete observations, including endpoint count `54` and unsupported
champion-target town recruitment; use the current status table above as the
authoritative source for UI/client work.
Later `Status`, `Needed`, and `Evidence` entries are historical unless a
`Current outcome` paragraph explicitly updates them.

The canister and pure test coverage are broad, and `make regression` passed at
checkpoint 26. A local `blast` run also proved that two identities can register,
start an active session, submit movement, sync turns, persist resource pickups,
build, recruit, resolve a guarded-mine battle, capture the mine, and receive
mine income in IcyDB. The remaining risk is that the current project proves
scripted/backend happy paths better than it proves a public canister play
route. The highest-priority missing work is a documented local `blast` path,
dynamic exploration/rendering, explicit endpoint composition over dedicated
canister views, and closing gaps where fixture or synthetic flows hide missing
gameplay integration.

## Local Blast Deployment Audit - 2026-05-16

This section records what happened when the canister was deployed to a local
DFX replica and driven through the `blast` CLI with two separate identities. It
is intentionally concrete so later todo work can be derived from observed
runtime behavior, not only source inspection.

### Local deployment facts

- Local replica host: `http://127.0.0.1:4943`.
- Local canister id used for the audit:
  `uqqxf-5h777-77774-qaaaa-cai`.
- The repo does not currently include a root `dfx.json` or committed canister
  `.did`, so the local deployment used a temporary DFX project under
  `/tmp/domm-local-dfx`.
- The initial `blast scan` failed because the built wasm did not expose public
  Candid metadata. The service DID was extracted with `candid-extractor` and
  manually appended to the wasm as public Candid custom sections before `blast`
  could scan/call endpoints.
- `blast scan` then exposed the expected gameplay endpoints, including account,
  lobby/session, map/object/champion/town, movement/sync, battle, economy,
  scenario/objective/worldgen, diagnostics, and IcyDB snapshot/metrics calls.
- The canister was installed with anonymous DFX controller first, then the
  `blast --id 1` principal was added as a controller to query controller-gated
  diagnostics.

Missing work from deployment:

- Add a committed local deployment path for DoMM: `dfx.json`, generated or
  checked-in DID flow, and documented commands to build/install the canister.
- Ensure release/local wasm artifacts include public Candid metadata, or add a
  reproducible packaging step that does it. `blast` should not require manual
  wasm surgery.
- Document how `blast` identities map to controllers and players for local
  testing.

### Live play path that worked

Two `blast` principals were used:

- Player one principal:
  `azbl5-a6gw2-6vetv-yoybw-cgbl5-7umx2-oim6f-rysnw-27g3i-cswt4-bae`.
- Player two principal:
  `gcese-ge3kk-5f54t-7bl4t-y2vxf-2fga7-cnanr-a5pgw-bwr33-pb4ch-vqe`.

The local game flow succeeded through:

- `register_player` for both identities.
- `create_session` by player one.
- `join_session` by player two with `faction:ashen-ledger`.
- `mark_ready` by both players.
- `start_session` by player one until session activation.
- `get_session`, `get_my_participant`, `get_my_champions`,
  `get_visible_map_chunks`, `get_visible_objects`, and `get_town_view`.
- `submit_move_intent` and `sync_session_turn` for both players.
- `get_events_after` with `audience_key = "public"`.
- `icydb_snapshot`, `icydb_metrics`, and controller-gated
  `get_diagnostic_storage_snapshot`.

Concrete runtime state:

- Session id: `01KRRZS5Z80000000000000008`.
- Player one participant:
  `01KRRZS5Z8000000000000000A`, faction `gutterborn-freehold`.
- Player two participant:
  `01KRRZTF5P0000000000000002`, faction `ashen-ledger`.
- Player one champion:
  `01KRRZW02C0000000000000002`, Mara of the Toll.
- Player two champion:
  `01KRRZW02C0000000000000009`, Korrin of Receipts.
- Player one moved from `(8,24)` to `(9,23)` and picked up
  `pile:west-wood-1`; resources changed from wood `10` to `15`.
- Player two moved from `(39,24)` to `(38,25)` and picked up
  `pile:east-wood-1`; resources changed from wood `10` to `15`.
- The public event feed contained the expected sequence:
  session created, participant joined, both ready, session started, movement
  submitted, partial movement sync, resource picked up, turn synced, and the
  same movement/pickup sequence for player two.

### Setup saga behavior

Historical only. The early local path required repeated `start_session` calls
with fresh nonces to drive setup phases.

Current UI contract: the host calls `start_session` once, then polls
`get_setup_progress`, `get_session`, and/or events until `state == "active"`.
Do not call `start_session` with fresh nonces to advance setup phases.

### Turn clock behavior under local DFX

Observed behavior:

- `sync_session_turn` enforces the 60-second wall-clock turn deadline on the
  local DFX replica.
- An immediate player-two sync after submitting movement failed with:
  `turn_not_due`.
- After waiting through the local turn window, the same flow could sync and
  resolve movement normally.
- Each two-step movement path required two sync passes because the first pass
  applied one microstep and parked the remaining intent; the second pass
  resolved the object interaction and advanced the turn.

Missing work:

- Document the real-time turn clock for manual local play.
- Provide a dev/test path that uses PocketIC time advancement or a configured
  short turn duration for local playtesting. Real 60-second turns make manual
  debugging slow.
- Make partial movement sync behavior explicit in client docs: a successful
  sync may return `movement_sync_incomplete`, and the client must keep syncing
  after the clock/command rules allow it.

### IcyDB storage evidence

The local game used typed IcyDB repositories and did not expose generic SQL
gameplay paths.

`icydb_snapshot` after the two-player movement/pickup audit reported:

- `corrupted_entries = 0`.
- `corrupted_keys = 0`.
- Total data rows in `DegensStore`: `248`.
- Storage index entries: `782`.
- `PlayerAccount`: `2`.
- `GameParticipant`: `2`.
- `GameSession`: `1`.
- `Champion`: `2`.
- `GameCommand`: `8`.
- `GameEvent`: `13`.
- `LobbyCommand`: `23`.
- `CommandEffect`: `32`.
- `MovementIntent`: `2`.
- `MovementSnapshot`: `4`.
- `ResourceLedgerEntry`: `2`.
- `ResourceLedgerTurnSummary`: `2`.
- `ParticipantObjectVisit`: `2`.
- `MapOccupancy`: `4`.
- `VisibilityChunk`: `18`.
- `ParticipantKnownObject`: `13`.
- `WorldObject`: `19`.
- `MapChunk`: `9`.

Controller-gated `get_diagnostic_storage_snapshot` with smaller entity batches
worked:

- `GameSession`, `GameParticipant`, `PlayerAccount`, `Champion` returned
  counts `1`, `2`, `2`, `2`.
- `MovementIntent`, `MovementSnapshot`, `ResourceLedgerEntry`,
  `ParticipantObjectVisit` returned counts `2`, `4`, `2`, `2`.

`icydb_metrics` showed typed repository activity and no generic SQL writes in
the audited path:

- `WorldObject`: `load_calls=32`, `save_calls=21`,
  `rows_inserted=19`, `rows_updated=2`, `sql_insert_calls=0`,
  `sql_update_calls=0`, `sql_delete_calls=0`.
- `GameParticipant`: `load_calls=59`, `save_calls=12`,
  `rows_inserted=2`, `rows_updated=10`, SQL counters all `0`.
- `Champion`: `load_calls=8`, `save_calls=6`, `rows_inserted=2`,
  `rows_updated=4`, SQL counters all `0`.
- `GameCommand`: `load_calls=24`, `save_calls=32`,
  `rows_inserted=8`, `rows_updated=24`, SQL counters all `0`.
- `MovementSnapshot`: `load_calls=4`, `save_calls=4`,
  `rows_inserted=4`, SQL counters all `0`.
- `MovementIntent`: `load_calls=6`, `save_calls=6`,
  `rows_inserted=2`, `rows_updated=4`, SQL counters all `0`.

Missing work:

- Keep these row counts as a useful baseline for future local canister smoke
  runs.
- Add an automated local/PocketIC smoke that records `icydb_snapshot`,
  selected diagnostic row counts, and `icydb_metrics` after the first two
  player actions.
- The diagnostic snapshot query can exceed the local 5B instruction limit when
  called with many entity names at once. Either document small diagnostic
  batches or optimize the endpoint/pagination.

### SQL endpoint reality

Observed behavior:

- `blast scan` did not expose any SQL query/update endpoint.
- The only IcyDB runtime surfaces visible to `blast` were `icydb_snapshot`,
  `icydb_metrics`, `icydb_metrics_reset`, and controller-gated
  `get_diagnostic_storage_snapshot`.
- The generated DID contains SQL-related metric fields, but not public SQL
  execution endpoints.
- This matches the current spec direction: gameplay uses typed IcyDB
  repositories and generic SQL is disabled for public gameplay.

Missing work:

- V1.1 does not add a SQL endpoint. Update docs to say local DB inspection uses
  `icydb_snapshot`, `icydb_metrics`, and typed/controller-gated diagnostic
  endpoints rather than SQL.

### Runtime bugs and gaps found

1. `preview_move_path` could exceed the local replica instruction limit.

   Current outcome: fixed for the v1.1 first-playable route by bounded
   render/projection work and query-budget coverage.

   Historical evidence:

   - A two-step preview for player one from `(8,24)` toward the adjacent wood
     pile failed with `IC0522`, "Canister exceeded the limit of 5000000000
     instructions for single message execution".
   - The subsequent `submit_move_intent` update for the same path succeeded.

   Historical needed items:

   - Optimize `preview_move_path` or make it use the same bounded/indexed path
     as submit validation.
   - Add a PocketIC/local test that previews a short legal path after setup and
     asserts it does not exceed instruction limits.
   - Add query metrics around preview to identify which repository/projection
     read is exploding.

2. `get_visible_objects` rendered stale semantic projection after movement and
   pickup.

   Current outcome: fixed for the v1.1 first-playable route by runtime/durable
   render projection. Public object/champion views now agree after movement and
   resource pickup in the covered routes.

   Historical evidence:

   - After player one pickup, `get_my_champions` showed Mara at `(9,23)` and
     player one wood was `15`.
   - Public events included `resource_picked_up` for object
     `01KRRZY39S0000000000000002`.
   - IcyDB row counts included `ParticipantObjectVisit=2` and
     `ResourceLedgerEntry=2`.
   - But `get_visible_objects` for the west viewport still reported:
     `champion:west` at opening position `(8,24)` and
     `pile:west-wood-1` with `"state":"available"`.
   - The same stale behavior appeared for player two:
     `get_my_champions` showed Korrin at `(38,25)` and wood `15`, while
     `get_visible_objects` still reported `champion:east` at `(39,24)` and
     `pile:east-wood-1` as available.

   Historical needed items:

   - Rework render projection so visible champion and world-object rows are
     hydrated from persisted `Champion`, `MapOccupancy`, `WorldObject`, and
     visit/state rows, not static `ParticipantKnownObject` scenario snapshots.
   - When a resource pile is consumed, visible objects should either disappear,
     render as consumed, or expose a clear state that the client can hide.
   - Add a canister test that performs pickup through public endpoints and then
     asserts `get_visible_objects` no longer shows stale opening coordinates or
     available consumed piles.

3. Diagnostic and preview query costs are too close to IC limits.

   Historical evidence:

   - `get_diagnostic_storage_snapshot` with 16 entity names exceeded the 5B
     instruction limit on the local replica.
   - Smaller batches of four entity names completed.
   - `preview_move_path` also exceeded the same instruction limit.

   Needed:

   - Add bounded pagination or lower hard limits for diagnostic snapshots.
   - Add performance tests for worst-case public query endpoints after setup.
   - Record per-endpoint instruction/cycle budgets in notes or test output.

4. Local run ergonomics are not yet good enough for a public canister
   playability claim.

   Historical evidence:

   - Manual local deployment required a temporary DFX project, Candid metadata
     patching, controller setup, repeated setup calls, and 60-second wall-clock
     waits.
   - The game logic can be driven with `blast`, but the flow is too brittle and
     undocumented for normal local play.

   Needed:

   - Add a single documented local smoke command or script that builds,
     installs, scans, registers two identities, starts a session, and performs
     a small action.
   - Keep v1.1 proof focused on a documented local smoke command or script;
     do not add a separate client deliverable.

### Extended local play route audit

After the initial two-player pickup route, the same local session was driven
further through economy, recruitment, battle, mine capture, income, and
client-facing read endpoints.

Additional route that worked:

- Player one built `building:freehold-training-yard` in `town:west` through
  `preview_build_town_structure` and `submit_build_town_structure`.
  The build spent `1000` gold, `6` wood, and `2` stone, created the
  `mudhook-levy` recruit pool, and left West Woe with buildings
  `crumbling-hall` and `freehold-training-yard`.
- Player one recruited `4` `mudhook-levy` into the West Woe town garrison
  through `preview_recruit_units` and `submit_recruit_units`. The live town
  view then showed a garrison stack with quantity `4` and a recruit pool with
  `12` remaining.
- The external dwelling `dwelling:west-mudhook` was visible through
  `get_dwelling_pool`, reported `direct_recruit = true`, and allowed
  `submit_dwelling_recruit` for `2` `mudhook-levy` into Mara. Mara's detailed
  champion view then showed `mudhook-levy` quantity `26` and
  `tollroad-skirmisher` quantity `10`.
- Moving Mara to the guarded gold mine at `(12,22)` created battle
  `01KRS0XQGT0000000000000004` against `neutral:west-mine`. `get_battle_state`,
  `sync_battle`, and `submit_battle_action` were enough to resolve it through
  canister endpoints.
- After the battle, moving Mara one step off the mine and back onto `(12,22)`
  emitted `mine_captured` for world object
  `01KRRZXKX70000000000000002`.
- The next session sync emitted `income_materialized` with payload
  `{"gold":250}`. Player one resources reached gold `8830`, wood `9`,
  stone `8`, iron `3`, crystal `3`, ember `3`, and aether `3`.
- IcyDB state after the extended route had `corrupted_entries = 0` and
  `corrupted_keys = 0`. Key row counts included `GameCommand=76`,
  `GameEvent=54`, `MovementIntent=4`, `MovementSnapshot=10`,
  `ResourceLedgerEntry=8`, `ResourceLedgerTurnSummary=3`,
  `Battle=1`, `BattleStack=3`, `ChampionArmyStack=4`,
  `TownBuilding=3`, `TownGarrisonStack=1`, `TownRecruitPool=1`,
  `DwellingPool=1`, `DwellingRecruitment=1`,
  `ParticipantObjectVisit=3`, `WorldObject=19`, and
  `VisibilityChunk=18`.
- Public events after sequence `48` clearly showed the final mine route:
  `movement_intent_submitted`, `movement_sync_incomplete`, `mine_captured`,
  `session_turn_synced` to turn `6`, `income_materialized`, and
  `session_turn_synced` to turn `7`.
- `get_objective_progress` returned the north/south central objectives as
  active with progress `0`, and `get_world_events` returned one active weekly
  omen for `week:1` ending on turn `7`.

Additional runtime bugs and playability gaps:

1. Battle is technically playable, but not manually comfortable.

   Historical evidence:

   - Battle action deadlines are `30_000ms`
     (`crates/domm-game/src/battle/types.rs:12`), but local `blast` calls plus
     human inspection frequently hit timeout paths before the next action could
     be submitted.
   - `submit_battle_action` first applies due timeouts and can fail with
     `battle_sync_incomplete`, telling the caller to run `sync_battle` first:
     `canisters/degens/src/services/battle.rs:72`.
   - `sync_battle` may rotate to a neutral active stack. Submitting an action
     as the player then fails because the active stack is not owned by that
     participant.
   - Repeated compact loops of `sync_battle`, `get_battle_state`, and immediate
     `submit_battle_action` did eventually defeat the neutral, but this is not a
     reasonable human flow without a client that polls and acts quickly.

   Needed:

   - Add a battle client loop that keeps state fresh, surfaces active stack
     ownership, and handles timeout-driven neutral turns without confusing the
     player.
   - Consider a longer dev/local battle deadline or a documented PocketIC route
     for manual debugging.
   - Add e2e coverage for the exact guarded-mine battle path through public
     canister endpoints, including timeout sync, legal action refresh, neutral
     defeat, and aftermath.

2. Guarded mine aftermath does not capture the mine automatically.

   Evidence:

   - After `neutral_defeated` and `battle_aftermath_applied`, Mara returned to
     active status at `(12,22)` on top of the mine, but the mine was not captured
     and income did not start.
   - Capture occurred only after a new movement intent stepped from `(12,22)` to
     `(12,21)` and back to `(12,22)`, which emitted `mine_captured`.
   - The first-playable walkthrough says the guarded mine fight should start
     gold income, so this extra off/on movement is a hidden rule from the
     player's perspective.

   Needed:

   - Apply guarded-object capture in battle aftermath when the victorious
     champion wins the guarded-object battle. No post-battle interaction
     affordance is required for v1.1.
   - Add a canister e2e asserting that defeating `neutral:west-mine` causes
     `mine_captured` and a later `income_materialized` without requiring an
     artificial step away and back.

3. The render projection remained stale after battle, capture, and income.

   Current outcome: fixed for the v1.1 first-playable route by battle
   aftermath projection, runtime-backed views, and Gate L/Gate M coverage.

   Evidence:

   - After mine capture and income, `get_my_champions` showed Mara active at
     `(12,22)`, but `get_visible_objects` for the west viewport still showed
     `champion:west` at the opening coordinate `(8,24)`.
   - The same object page still showed `neutral:west-mine` at `(12,22)`, the
     gold mine with `"state":"available"`, and consumed resource piles with
     `"state":"available"`.
   - Source inspection matches the runtime behavior: `visible_objects` pages
     `ParticipantKnownObject` rows and converts them through static scenario
     details, rather than hydrating current `Champion`, `WorldObject`,
     `ParticipantObjectVisit`, and ownership state:
     `canisters/degens/src/services/render_projection.rs:81`,
     `canisters/degens/src/services/render_projection.rs:448`,
     `canisters/degens/src/services/render_projection.rs:607`.

   Historical needed items:

   - Treat stale projection as a P0 blocker for a visual client. A player cannot
     trust the map while it shows defeated neutrals, already-collected piles,
     captured mines as still available, and old champion locations.
   - Replace or augment `ParticipantKnownObject` projection with dynamic reads
     from the persisted domain rows.
   - Add assertions after pickup, battle victory, mine capture, and income that
     `get_visible_objects` renders the same state as the detail endpoints and
     event log.

4. `get_game_view` is a shell, but its pagination metadata looks like real
   empty pages with more data.

   Evidence:

   - A live `get_game_view` call after the extended route returned session,
     participant resources, render time, content hash, and a
     `sync_session_turn` affordance, but no map chunks, objects, champions,
     towns, battle, or events.
   - It also returned `map_page_info.has_more = true` and
     `object_page_info.has_more = true` when the corresponding arrays were
     empty. Source confirms `get_game_view` intentionally sets empty vectors and
     `has_more` based only on requested limits:
     `canisters/degens/src/services/game_view.rs:19`.

   Needed:

   - Make `get_game_view` metadata impossible to misread. Either return
     `has_more = false` for fields that are intentionally omitted, add explicit
     `omitted_fields`, or document the required dedicated endpoint composition.
   - Do not let a future UI treat `get_game_view.objects = []` as "there are no
     visible objects".

5. Champion roster/detail split is usable but easy to misuse.

   Evidence:

   - `get_my_champions` returned Mara's current coordinate and status but left
     `army_stacks = []`, `artifacts = []`, and
     `strength_label = "details_required"`.
   - `get_champion_view` with the real champion id returned equipped artifacts,
     army stacks, and `strength_label = "modest"`.
   - `get_champion_view` with semantic id `champion:west` failed after movement
     because semantic resolution looks up the opening coordinate.

   Needed:

   - Document the roster/detail split in client code, not only endpoint docs.
   - Either support stable semantic ids for spawned scenario objects or avoid
     exposing semantic ids where later detail queries need durable row ids.

6. Historical champion recruitment preview/submit inconsistency was fixed by
   the current code.

   Historical evidence, now obsolete as of 2026-05-22:

   - `preview_recruit_units` accepts both `TownGarrison` and `Champion` target
     variants and only uses the requested slot for preview:
     `canisters/degens/src/services/town.rs:160`.
   - The submit path spends resources, updates the participant, decrements the
     recruit pool, and only then calls `recruit_to_garrison`:
     `canisters/degens/src/services/town.rs:420`.
   - The old `recruit_to_garrison` path rejected `RecruitTarget::Champion`:
     `canisters/degens/src/services/town.rs:545`.
   - The live audit intentionally did not submit the champion-target town
     recruit because the ordering suggests a possible partial mutation before
     the unsupported-target error.

   Current outcome:

   - Preview and submit agree for same-tile owned active champion targets.
     Invalid champion targets fail before command creation, resource spend,
     pool decrement, or stack mutation.
   - Add a recovery/idempotency test that failed recruitment cannot spend
     resources, decrement pools, or leave pending command state.

7. External dwelling recruitment lacks an observed proximity rule.

   Evidence:

   - `submit_dwelling_recruit` succeeded for Mara after she had moved away from
     the dwelling. The dwelling was at `(10,25)` and Mara was no longer on that
     coordinate.
   - The canister disabled-reason check validates owner, direct-recruit flag,
     champion ownership, unit, quantity, pool, and resources, but not champion
     distance or same-tile interaction:
     `canisters/degens/src/services/economy_expansion.rs:797`.

   Needed:

   - Lock the v1.1 contract as remote direct dwelling recruitment: owned
     `direct_recruit` dwelling pools may recruit into any owned active
     world-map champion in the session. Add active/not-in-battle validation and
     tests; local/same-tile dwelling recruitment is v2.

8. Exploration is partially real but not enough for play.

   Evidence:

   - After moving Mara to `(12,22)`, `get_visible_map_chunks` around the central
     area returned one discovered/visible chunk, so movement refreshes some
     visibility state.
   - `get_visible_objects` over the same central viewport returned no objects,
     even though scenario objects and neutrals exist beyond the starting area.
   - `get_town_view` for `town:east` correctly failed with `not_visible`, so
     town visibility is enforced at least for that detail endpoint.

   Needed:

   - Complete object discovery/materialization for newly visible objects.
   - Add a route that moves toward the central objective, discovers the road
     guard/objective, and verifies visible object pages update as fog changes.

9. `preview_move_path` is still unusable on the live canister.

   Evidence:

   - A later two-step preview from `(12,22)` to `(12,21)` and back to `(12,22)`
     still exceeded the local replica `5_000_000_000` instruction limit with
     `IC0522`.
   - `submit_move_intent` for equivalent short movement paths works, so clients
     cannot rely on preview before submit.

   Needed:

   - Treat preview query cost as a P0 client blocker. A player-facing client
     needs a cheap way to show whether a planned path is legal before spending a
     command.

Current playable verdict from the live canister:

- The backend can be driven through a meaningful v1 slice: lobby, setup,
  movement, pickup, build, recruit, dwelling reinforcement, battle,
  guarded-mine capture, and mine income all persisted in IcyDB.
- Historical 2026-05-16 caveat: it was not yet provably playable through public
  canister endpoints. Current outcome: the backend/client contract is green for
  UI work through the 2026-05-22 benchmark suite, `59/59` endpoint coverage,
  Gate M, render-projection coverage, and documented local canister/blast
  paths. Manual local DFX still uses real-time deadlines, but that is a local
  smoke constraint rather than a v1.1 blocker.

### Public API expansion audit

This pass used the public canister API surface a future client would call,
rather than DB internals, to see whether the client can be developed against
the current contract.

Additional client-facing endpoints that worked:

- `get_content_manifest("domm-first-playable", 1)` returned a usable boot
  manifest with content hash
  `1e7fc4f2b594eb32a08a0059f84a9b07c1a5b89956aae1239182019addd5f0db`,
  `2` factions, `2` champion classes, `6` terrain types, `9` units,
  `8` buildings, `2` spells, `1` artifact, and `5` map object definitions.
- At the time of this historical audit, `get_canister_endpoint_inventory`
  reported an inventory of `54` public methods grouped by domain. The current
  required gameplay inventory is `59`, as recorded in the status table above.
  Endpoint kind is encoded as Candid enum objects such as
  `{ "Update": null }` or `{ "Query": null }`, not a flat string.
- `get_scenario_rules` returned active `central_objectives`, `conquest`,
  `max_turn`, and `quest_victory` rows, plus disabled advanced rows with
  checkpoint disabled reasons.
- `get_skirmish_settings`, `get_procedural_map_state`, `get_naval_routes`, and
  `get_siege_rules` returned coherent status rows. The procedural map is
  `validated`; naval and siege rows are explicitly disabled with
  `checkpoint_25_schema_only`.
- `get_tavern_offers`, `preview_hire_champion`, and `hire_tavern_champion`
  worked for West Woe. Hiring slot `0` created champion
  `01KRS3BE110000000000000007` and changed the tavern offer status to
  `hired`.
- `preview_market_trade` returned useful client states: `gold -> crystal` for
  `2500` was allowed, `wood -> crystal` for `10` was rejected with
  `disabled_reason = "insufficient_resources"`, invalid pairs returned
  `invalid_market_trade`, and oversized trades returned a cap error.
- `submit_market_trade` worked and persisted a `MarketTrade` row. Exact nonce
  replay returned the original applied response without new events or changed
  subjects; nonce reuse with a different payload returned
  `duplicate_nonce_payload_mismatch`.
- `preview_quest`, `accept_quest`, and `claim_quest_reward` worked for
  `quest:opening-ledger`. The quest was already progress-complete after the
  earlier play route, accepted at turn `7`, then claimed for `500` gold.
- `preview_champion_progression` returned real skill choices. Selecting
  `sour_sorcery`, learning `spite-march`, and casting `spite-march` all worked
  through public endpoints.
- `sync_objectives`, `sync_world_events`, `sync_advanced_victory`, and
  `sync_world_generation` all applied and appended public events. These behave
  like status synchronizers more than player-facing gameplay controls.
- `get_command_status` worked by both client nonce and command id. It returns a
  compact command status and `result_json`.
- Cross-player checks were client-safe in the sampled paths. Player two could
  read their own participant, but could not read player one's hidden champion,
  West Woe tavern offers, or player-one audience events.

Additional IcyDB evidence after the client API pass:

- `icydb_snapshot` still reported `corrupted_entries = 0` and
  `corrupted_keys = 0`.
- Total `DegensStore` data entries reached `458`; index user entries reached
  `1489`.
- New relevant row counts included `Champion=3`, `ChampionHire=1`,
  `ChampionSpell=1`, `MarketTrade=1`, `GameCommand=87`, `GameEvent=65`,
  `ResourceLedgerEntry=12`, `QuestState=2`, `ScenarioRuleState=8`,
  `ProceduralMapState=1`, `NavalRouteState=1`, `SiegeRuleState=1`,
  `SkirmishSettingsState=1`, and `TavernOffer=4`.

Client API gaps found:

1. Learned spells are exposed on owned champion detail views.

   Historical evidence from the original audit:

   - `preview_champion_progression` showed Mara had
     `learned_spell_slugs = ["spite-march"]`.
   - `get_champion_view` and `get_my_champions` still returned
     `spell_slugs = []` for the same champion after learning and casting the
     spell.
   - Source then confirmed the canister render projection hard-coded
     `spell_slugs: Vec::new()`.
   - The progression service does have a working `ChampionSpell` lookup:
     `canisters/degens/src/services/champion_magic.rs:572`.

   Current outcome:

   - Fixed for owned champion detail views; `spell_slugs` are hydrated from
     learned `ChampionSpell` rows where the caller may see them. Summary rows
     may still omit detail and require `get_champion_view`.

2. Command retry responses are correct but can be stale relative to current
   state.

   Evidence:

   - Exact replay of `audit-market-gold-crystal-1` returned the original market
     trade response with `resources_after.gold = 6330`, no events, and no
     changed subjects.
   - Current participant resources after later quest/hire actions were
     `gold = 4330`, `wood = 9`, `stone = 8`, `crystal = 4`.

   Needed:

   - Client retry code must treat replayed command responses as proof of command
     outcome, then refresh participant/champion/town state through read
     endpoints before rendering current state.
   - Document this explicitly in the client contract.

3. Event and command payloads are still JSON strings inside structured Candid.

   Evidence:

   - Public event pages return structured event metadata, but `payload` is a
     JSON string.
   - `get_command_status` returns `result_json` as a JSON string rather than the
     same typed `CommandResult` shape returned by update calls.

   Needed:

   - Provide a generated client parser/schema for event payload and
     command-status result JSON, or change the public DTOs to return typed
     variants where practical.
   - At minimum, document per-event payload schemas for the events a v1 client
     must render.

4. Disabled or future-scope systems are exposed in the client API surface.

   Evidence:

   - `get_scenario_rules` exposes disabled advanced victory rows.
   - `get_naval_routes` and `get_siege_rules` expose disabled rows with
     checkpoint-only reasons.
   - `sync_advanced_victory` still appends an `advanced_victory_synced` event
     even though advanced victory is not active v1 gameplay.

   Needed:

   - Keep disabled/future systems out of player action surfaces. Status/debug
     queries may show disabled rows only with `status = "disabled"`,
     `disabled_reason`, and `actionable = false`.
   - Disabled-only sync/update endpoints must return typed disabled/noop
     receipts or be controller/manual-recovery only; they must not append public
     gameplay events.

5. The endpoint inventory is useful, but not enough to verify the public API
   contract.

   Evidence:

   - `get_canister_endpoint_inventory` lists method names, groups, kinds, and
     fixture mappings, but not argument schemas, response schemas, auth rules,
     pagination rules, or event subscriptions.

   Needed:

   - Make the public Candid, endpoint docs, typed DTOs, pagination rules,
     command status semantics, and event schemas sufficient for `blast` smoke
     and future clients without a separate v1.1 client deliverable.

6. Admin diagnostics drifted behind the schema.

   Evidence:

   - `icydb_snapshot` listed newer entities such as `ChampionHire`,
     `ChampionSpell`, `MarketTrade`, and `TavernOffer`.
   - `get_diagnostic_storage_snapshot` rejected `MarketTrade` with
     `unknown_diagnostic_entity`, and the diagnostics service match list does
     not include all newer economy entities.

   Needed:

   - Keep controller diagnostics in sync with every persisted entity or expose a
     generic entity inventory/count endpoint so local client/e2e audits do not
     need hard-coded diagnostic names.

### Spec-driven extra blast audit

After reading the public API, client retry, pagination, lazy growth, movement,
tavern, and first-playable sections of `spec.md`, another `blast` pass targeted
contracts that had not been exercised yet.

Additional behavior that worked:

- `get_my_player` returned a compact player identity view with player id,
  principal, and display name.
- `get_session` returned a valid active session shell, but only included
  `session_id`, `participant_ids`, and `state`.
- `get_match_history(0, 10)` returned an empty page while the match was still
  active, which is reasonable, but it means the basic match-history UI cannot
  be verified until victory/finalization works.
- Public list limits returned clear typed errors: a `49 x 49` viewport returned
  `viewport_too_large` with `details_json = {"tiles":2401,"max":2304}`;
  event limit `201` returned `list_limit_exceeded`; chunk limit `10` returned
  `viewport_chunk_limit_exceeded`.
- Non-adjacent movement submission failed with
  `movement_path_not_adjacent` and did not leave a readable command status,
  which suggests that validation happened before command creation.
- Controller-gated diagnostics correctly rejected player two with
  `controller_required`.
- Hired champion movement can work. Vara, the first tavern hire, submitted a
  one-step move intent from `(6,24)` to `(7,24)`, and `sync_session_turn`
  resolved her to `(7,24)`.
- Late movement intents submitted while backend turn work was pending were
  accepted and later resolved on sync. Mara moved from
  `(12,22)` to `(13,22)` and Vara moved from `(6,24)` to `(7,24)`.
- Mine income continued to materialize on later turn syncs. Syncing to turn `8`
  emitted `income_materialized` for turn `7`; syncing to turn `9` emitted
  `income_materialized` for turn `8`.

Additional IcyDB evidence after this pass:

- `icydb_snapshot` still reported `corrupted_entries = 0` and
  `corrupted_keys = 0`.
- Total `DegensStore` data entries reached `496`; index user entries reached
  `1624`.
- Relevant row counts included `Champion=4` overall, `ChampionHire=2`,
  `GameCommand=93`, `GameEvent=72`, `MapOccupancy=6`,
  `MovementIntent=7`, `MovementSnapshot=13`, `ResourceLedgerEntry=15`, and
  `ResourceLedgerTurnSummary=5`.

New spec-contract gaps found:

1. Tavern week-two offers are missing.

   Current outcome: fixed for v1.1. Weekly tavern offer generation and recruit
   growth are implemented and covered by current week-two/growth tests.

   Evidence:

   - The spec says taverns offer two seeded pseudo-random champions per week.
   - After syncing to turn `8`, `get_tavern_offers("town:west")` returned
     `week_number = 2` and `offers = []`.
   - The week-one offers still existed before the week boundary and both had
     been marked `hired`.

   Needed:

   - Implement bounded weekly tavern offer generation/materialization for later
     weeks, or document that taverns only have opening-week offers in v1.
   - Add a canister test that advances into week 2 and asserts the tavern view
     still returns the expected offer count or an explicit disabled reason.

2. Tavern hire can partially mutate state before failing.

   Evidence:

   - Previewing week-one slot `1` was allowed with cost `3000` gold.
   - Submitting that hire returned a retryable raw error:
     `icydb_repository_error`, message
     `IcyDB repository operation failed: map.create_occupancy_cell`.
   - Immediately after the error, `get_command_status` for the nonce showed
     the command as `Pending` / `Created`.
   - Despite the error, `get_tavern_offers` showed slot `1` as `hired`,
     `get_my_champions` showed the new champion Ketch Underbridge, and player
     gold dropped by the hire cost.
   - Retrying the same nonce later changed the command to
     `Failed` / `insufficient_resources` because the earlier failed attempt had
     already spent the gold.
   - No successful `champion_hired` event was observed for Ketch, even though
     persisted rows changed and `ChampionHire=2` in `icydb_snapshot`.

   Needed:

   - Fix `hire_tavern_champion` as a recovery-safe saga: command state,
     resource spend, champion row, offer row, occupancy, event, and changed
     subjects must either complete idempotently or recover to completion on
     retry.
   - Create occupancy before irreversible spend/offer mutation, or make every
     phase idempotently recoverable from `CommandEffect` rows.
   - Add a regression test for hiring a second champion into an occupied town
     tile, including exact nonce retry after the first attempt traps or returns
     a repository error.

3. Hired champions can spawn on a town tile that is already occupied by another
   hired champion.

   Evidence:

   - Vara hired from tavern slot `0` spawned at West Woe `(6,24)`.
   - Ketch from slot `1` also appeared at `(6,24)`.
   - The second hire hit `map.create_occupancy_cell`, implying the occupancy
     layer objected, but the champion row and offer state were still visible.

   Needed:

   - Define and implement spawn placement for multiple hired champions: stack in
     town/garrisoned status, nearest free adjacent tile, or explicit
     `town_occupied` disabled reason before spend.

4. Movement into an occupied champion tile is accepted as an intent and silently
   discarded on sync.

   Evidence:

   - Ketch at `(6,24)` submitted a move intent to `(7,24)`, where Vara was
     already standing.
   - The submit call returned `Applied` and emitted `movement_intent_submitted`.
   - `sync_session_turn` resolved the movement intent and advanced to turn `9`,
     but Ketch remained at `(6,24)`.
   - The sync response did not include a path-blocked event, failed movement
     reason, or changed champion for Ketch.

   Needed:

   - Either reject occupied destination tiles during `submit_move_intent`, or
     emit a clear blocked/failed movement result during sync.
   - The client needs a reason to explain why a submitted movement did not move
     the champion.

5. Late movement submission differs from the client contract.

   Current outcome: fixed at the accepted-closure boundary. New
   turn-sensitive commands after the current-turn closure job is accepted,
   running, or due fail before command creation with
   `backend_work_pending`/stale-expired semantics; exact retries of
   already-created commands still replay.

   Evidence:

   - `get_game_view` showed pending backend work before the late movement
     submits.
   - `submit_move_intent` still accepted new intents for turn `7`, and a later
     `sync_session_turn` resolved them into turn `8`.
   - The v1.1 contract says old-turn commands must fail after durable
     turn-resolution work is accepted.

   Needed:

   - Enforce the v1.1 contract: turn-sensitive commands are accepted only until
     a durable turn-resolution job is accepted for that turn. After that,
     old-turn commands fail before command creation with `backend_work_pending`
     while processing or `turn_expired` after advancement.

6. Object pagination uses offset-like cursors and paginates stale projection.

   Current outcome: the cursor behavior is now documented as numeric offsets
   over the current sorted projection, with live-state limitations. The stale
   projection part was fixed by render-projection work; clients should still
   tolerate row changes between pages.

   Evidence:

   - `get_visible_objects` with `cursor = 0`, `limit = 2` returned
     `next_cursor = 2`; the next call with `cursor = 2`, `limit = 2` returned
     `next_cursor = 4`.
   - The spec says not to use offset pagination and to use stable ordering /
     cursor pagination.
   - These pages still contained stale rows: defeated neutral, captured mine as
     available, consumed piles, and Mara at the opening coordinate.

   Needed:

   - Historical recommendation: replace offset-like cursors with stable cursor
     tokens or document the live offset behavior and its limitations.
   - Do not build client map pagination on the stale known-object projection.

7. Recruit growth does not materialize or project at the week boundary.

   Evidence:

   - After syncing to turn `8` / week `2`, `get_town_view` still showed the
     `mudhook-levy` recruit pool at `available = 12` and
     `last_growth_week = 1`.
   - `preview_recruit_units` for quantity `16` returned
     `recruit_pool_empty`, `available = 12`.
   - The spec says read APIs may return effective recruitment availability and
     recruitment/town inspection should account for weekly growth.

   Needed:

   - Materialize or at least project recruit growth consistently in town view and
     recruit preview.
   - Add a week-boundary canister test for town recruit pool growth.

8. `get_session` is too thin for the documented render/client contract.

   Evidence:

   - The live response only included `session_id`, `participant_ids`, and
     `state`.
   - The spec's render/client contract expects clients to know current turn,
     max turns, ruleset/content hash, turn timing, map size, and chunk size from
     render-facing responses. Some of this exists in `get_game_view`, but not
     in `get_session`.

   Needed:

   - Keep `get_session` as a lobby/setup shell. Gameplay metadata comes from
     `get_game_view` render metadata, `get_content_manifest`, and dedicated
     bounded render endpoints.

9. Multi-step movement preview does not fit the canister query budget.

   Evidence:

   - At turn `10`, with Mara active at `(14,22)` and
     `get_game_view.render_time.backend_work_pending = false`, a one-step
     `preview_move_path` to `(15,22)` succeeded with `total_cost = 5`.
   - The same endpoint for a two-step path to `(15,22),(16,22)` trapped with
     IC0522, `Canister exceeded the limit of 5000000000 instructions`.
   - Before the turn-10 sync, two-, four-, five-, and eight-step preview paths
     from the same route also hit IC0522.
   - The update path is not equally broken: `submit_move_intent` accepted a
     two-step path for Mara on turn `9`, `sync_session_turn` consumed the first
     step with `movement_sync_incomplete`, and a second sync resolved the
     remaining step and advanced to turn `10`.

   Needed:

   - Make `preview_move_path` bounded for normal multi-step player paths. A
     client cannot offer path planning if only single-tile preview is reliable.
   - Add a canister query-budget test for representative 2, 8, and 64 step
     paths after a session has accumulated realistic commands/events.
   - Consider returning a cheap partial preview instead of trapping when the
     requested path exceeds the available query budget.

10. `get_command_status` is unreliable by client nonce after real play history.

   Evidence:

   - `get_command_status(session, "audit-mara-two-step-submit")` trapped with
     IC0522, while `get_command_status(session,
     "01KRS4DGYJ0000000000000000")` returned the applied movement command.
   - `get_command_status(session, "audit-action-on-resolved-battle-2")`
     trapped with IC0522, while the command id
     `01KRS45YG50000000000000000` returned the failed
     `battle_not_active` status.
   - `get_command_status(session, "audit-market-insufficient-after-hires-2")`
     returned `command_status_not_found`, while command id
     `01KRS462DD0000000000000000` returned the failed
     `insufficient_resources` status.
   - The v1.1 contract now adds
     `get_command_status_by_nonce(session_id, command_type, client_nonce)` for
     exact indexed nonce lookup; legacy nonce guessing is compatibility-only.

   Needed:

   - Implement `get_command_status_by_nonce(session_id, command_type,
     client_nonce)` using the unique command index directly and keep it within
     budget.
   - Ensure failed commands are consistently readable by both nonce and command
     id when the update response included a command id.
   - Until fixed, document that callers must prefer the returned command id for
     polling and treat legacy nonce guessing as unreliable.

11. Spell event idempotency is too coarse for repeated casts.

   Evidence:

   - Learning `hex-spark` on Mara succeeded and created public event seq `73`.
   - Recasting learned adventure spell `spite-march` later applied movement/mana
     effects, but returned the older event seq `60` with event key
     `adventure_spell:champion:spite-march`.
   - `get_events_after(session, "public", 72, 50)` returned only the
     `champion_spell_learned` event for `hex-spark`; there was no new event for
     the later `spite-march` cast.

   Needed:

   - Include command id, turn, cast sequence, or champion id plus turn in
     adventure-spell event keys so separate casts produce separate event rows.
   - Add a canister test for casting the same adventure spell twice across
     turns and verifying both casts are visible in the event feed.

12. Build preview also exceeds budget for normal next buildings.

   Evidence:

   - `preview_build_town_structure` for regular next buildings such as
     `building:skirmisher-stall` and `building:weighhouse` hit IC0522 after the
     session had progressed.
   - Special/error cases still worked: already-built
     `building:freehold-training-yard` returned
     `disabled_reason = "already_built"`, and an unknown slug returned
     `building_not_found`.

   Needed:

   - Keep build preview on a bounded indexed path for ordinary valid candidate
     buildings, not just error paths.
   - Add a query-budget test after a real play route for every building the
     town panel should present.

13. Resolved-battle endpoints need cleaner idempotency semantics.

   Evidence:

   - Direct battle actions against the resolved guarded-mine battle failed
     cleanly with `battle_not_active` for both `Defend` and `Retreat`.
   - Calling `sync_battle` again on the already resolved battle returned
     `Applied`, with old `neutral_defeated` and `battle_aftermath_applied`
     events from turn `5` in the response even though the command was executed
     at effective turn `9`.
   - The resolved battle state was readable by player two, who was a session
     participant but not involved in that neutral fight. This may be intended
     public history, but the spec does not state the visibility rule.

   Needed:

   - Make `sync_battle` on a resolved battle return a stable no-op/replay
     response that does not look like new aftermath work.
   - Clarify whether resolved neutral battle details are public to all session
     participants or redacted by participant involvement.

14. Resource failure paths mostly work, but need command-status coverage.

   Evidence:

   - A `submit_market_trade` of `2500` gold to crystal failed with
     `insufficient_resources` while player one had only `1830` gold.
   - `get_my_participant` afterward still showed gold `1830`, so the failed
     trade did not spend resources.
   - The failed trade was readable by command id but not by client nonce, as
     described above.

   Needed:

   - Keep the no-spend behavior.
   - Add command-status assertions for failed economy commands by both nonce and
     command id.

15. Quest validation works, but one status code is ambiguous after claim.

   Evidence:

   - `preview_quest` for `quest:no-such` returned `quest_not_found`.
   - Reclaiming `quest:opening-ledger` after it was already claimed returned
     `quest_reward_already_claimed`.
   - Re-accepting the same already-claimed quest returned
     `quest_already_accepted`, which is technically true but less useful for a
     client rendering a claimed quest state.

   Needed:

   - Return a claimed-specific non-retryable error when `accept_quest` targets
     a claimed quest; do not emit quest mutation events.

16. Registered non-participant auth behaves correctly in sampled scenario APIs.

   Evidence:

   - Before registering a third identity, session-scoped scenario and worldgen
     calls failed with `player_not_registered`.
   - After registering that identity, `get_scenario_rules` and
     `sync_world_generation` for the active match failed with
     `participant_not_found`.
   - `get_match_history(0, 10)` for the registered non-participant returned an
     empty page, which is reasonable while the match has no finished entry for
     that player.

   Needed:

   - Keep these auth gates and add them to the PocketIC endpoint matrix for the
     scenario/worldgen domains.

17. Historical turn-10 movement render-projection gap.

   Current outcome: fixed for the covered v1.1 runtime/durable render
   projection path; map object pages are no longer treated as static opening
   snapshots for moved champions, consumed piles, defeated neutrals, or captured
   mines.

   Evidence:

   - After Mara moved to `(14,22)`, `get_champion_view` showed `(14,22)` and
     `effective_movement = 230`.
   - `get_visible_objects` for the west viewport still showed
     `champion:west` at `(8,24)`, the defeated `neutral:west-mine`, consumed
     resource piles, and the captured mine as visible objects.
   - The same object DTOs had `details = null`, so the client does not get
     enough dynamic state to correct the stale projection.

   Historical needed items:

   - Treat this as the main map-render blocker: the authoritative champion and
     world-object rows are changing, but the map projection remains anchored to
     opening known-object data.
   - Add an e2e assertion after every movement/capture/pickup route that the map
     object projection matches the dedicated detail endpoints.

18. Sync receipts can under-report returned events.

   Evidence:

   - The second sync after the two-step movement returned two events in the
     `events` array: `income_materialized` seq `76` and
     `session_turn_synced` seq `77`.
   - The embedded `StrategicReceipt` still reported `event_count = 1`.

   Needed:

   - Make receipt counters match the returned event list, or define exactly
     which event class the count represents.

Additional IcyDB evidence after this pass:

- `icydb_snapshot` still reported `corrupted_entries = 0` and
  `corrupted_keys = 0`.
- Relevant row counts reached `GameCommand=111`, `GameEvent=77`,
  `CommandEffect=96`, `MovementIntent=8`, `MovementSnapshot=15`,
  `ResourceLedgerEntry=16`, `Champion=4`, `ChampionSpell=2`,
  `PlayerAccount=3`, `MapOccupancy=6`, `ParticipantKnownObject=13`, and
  `WorldObject=19`.

### Additional fog, town, recruit, and event audit

This pass targeted parts of `spec.md` that were still under-sampled: entity
visibility, visible map chunk DTOs, event audience redaction, town
build/recruit edge cases, lobby-state errors, and pending-command recovery.

Behavior that worked:

- `get_town_view(session, "town:west")` for player one returned West Woe with
  authoritative IcyDB rows. After the later build/recruit pass it showed
  buildings `crumbling-hall`, `freehold-training-yard`, and
  `skirmisher-stall`; recruit pools for `mudhook-levy` and
  `tollroad-skirmisher`; and a mudhook garrison quantity of `5`.
- `get_town_view(session, "town:west")` for player two returned `not_visible`.
  `get_town_view(session, "town:east")` for player one also returned
  `not_visible`, and `get_champion_view` for player two's champion from player
  one returned `not_visible` / `champion is hidden`.
- A registered third identity that is not a match participant was rejected from
  session-scoped scenario/worldgen calls with `participant_not_found`.
- `preview_recruit_units` for an unknown unit returned `unit_not_found`.
  Over-recruiting `13` mudhook levy from an available pool of `11` returned
  `allowed = false`, `disabled_reason = "recruit_pool_empty"`.
- Player two previewing recruitment from player one's West Woe returned
  `allowed = false`, `disabled_reason = "not_owner"`.
- A normal town-garrison recruit still worked. Recruiting `1` mudhook levy into
  West Woe's garrison emitted `units_recruited` seq `78`, reduced gold by `70`,
  reduced the pool, and increased the garrison stack from `4` to `5`.
- A normal build update worked even though build preview for ordinary candidate
  buildings still hits IC0522. `submit_build_town_structure` for
  `building:skirmisher-stall` emitted `town_building_built` seq `79`, spent the
  expected resources, set `last_built_turn = 10`, and created a
  `tollroad-skirmisher` recruit pool with `available = 9`.
- Champion progression failure paths are typed. Selecting `dirty_tactics` when
  Mara had no skill point returned `no_pending_skill_point`; selecting
  `no_such_skill` returned `invalid_skill_choice`; both were readable through
  `get_command_status` by nonce and command id.
- Joining the active/full match as the third identity failed with
  `session_not_joinable`.

Historical gaps found:

1. Historical champion-target recruitment partial-mutation gap is fixed.

   Historical evidence, now obsolete as of 2026-05-22:

   - `preview_recruit_units` allowed recruiting `1` `mudhook-levy` from West
     Woe into Mara even though Mara was at `(14,22)` and West Woe is at
     `(6,24)`.
   - `submit_recruit_units` with the same `Champion` target returned a raw API
     error:
     `canister recruit currently supports town garrison targets`.
   - Despite the error, West Woe's `mudhook-levy` pool dropped from `12` to
     `11`, player one gold dropped by `70`, and the pool's `last_command_id`
     became `01KRS5F9HK0000000000000000`.
   - Mara's army stacks did not gain the unit, and the town garrison did not
     gain it either. The unit/resources were effectively lost.
   - `get_command_status(session, "audit-remote-recruit-mara")` showed the
     command stuck in `Pending` / `Created` with `retryable = false`.
   - A later `sync_session_turn` advanced the session to turn `11`, but the
     remote-recruit command remained `Pending` / `Created`.

   Current outcome:

   - Valid same-tile owned active champion targets are implemented.
   - Invalid champion targets fail before command creation, resource spend,
     pool decrement, stack mutation, or pending-command leakage.

2. Turn sync advanced past a pending command that had already mutated state.

   Evidence:

   - The spec says recovery runs before turn advancement so a half-finished
     command cannot be overtaken by turn-final resolution.
   - After the failed champion-target recruit left command
     `01KRS5F9HK0000000000000000` in `Pending` / `Created`,
     `sync_session_turn("audit-sync-after-pending-remote-recruit")` still
     advanced the session from turn `10` to turn `11`.
   - The pending command status was unchanged after the sync.

   Needed:

   - Make sync recover, fail, or explicitly quarantine pending/applying commands
     before advancing the turn.
   - If a command type is unrecoverable, return a typed repair-required error
     instead of advancing silently.

3. Public events leaked hidden opponent town actions.

   Current outcome: fixed for v1.1. Event audiences are explicit
   (`public` or `participant:{participant_id}`), callers may only request
   authorized audiences, and hidden opponent town activity is not exposed as an
   exact public payload.

   Historical evidence:

   - Player two cannot read West Woe through `get_town_view`; the endpoint
     returns `not_visible`.
   - Player two still received the public event feed entries for player one's
     hidden West Woe actions after seq `77`: `units_recruited` with
     payload `{"unit_slug":"mudhook-levy","quantity":1}` and
     `town_building_built` with payload
     `{"building_slug":"skirmisher-stall"}`.
   - Both events exposed the real West Woe town id and exact action payload with
     `redacted = false`.

   Historical needed items:

   - Apply event audience/redaction rules consistently with entity visibility.
   - Hidden opponent town build/recruit events are owner-private or redacted to
     a public coarse event that does not reveal exact town id, coordinates,
     unit/building choices, or quantities.

4. Visible map chunk responses lacked `page_info` and returned static terrain
   data for undiscovered chunks.

   Current outcome: documented for v1.1. Static terrain, movement, and flags may
   be shipped for surveyed base-map chunks while dynamic objects, owners,
   occupants, battle details, and events remain visibility-gated. Map chunk
   pages expose page metadata.

   Historical evidence:

   - `get_visible_map_chunks` for the west viewport returned `page_info = null`
     instead of the documented `next_cursor`, `has_more`, and `limit` metadata.
   - Player two, who has no discovered or visible bits in west chunk `(0,1)`,
     still received the chunk's `terrain_blob`, `movement_blob`, and
     `flags_blob`. Only `discovered_blob` and `visible_blob` were all zero.
   - Player one and player two both received the same static terrain/movement
     data for that undiscovered chunk.

   Historical needed items:

   - Document surveyed-base-map fog: static terrain/movement/flags may be
     shipped for undiscovered chunks and hidden by client fog overlays, but
     dynamic objects, owners, occupants, battle details, and events remain
     visibility-gated.
   - Always return page metadata on map chunk pages.

5. `get_visible_objects` had no page metadata and remained stale for both sides.

   Current outcome: fixed for v1.1. Object pages expose page metadata and use
   the current sorted projection; clients must still treat object cursors as
   live-state pagination rather than snapshot-isolated cursors.

   Historical evidence:

   - Player one still sees `champion:west` at the opening coordinate `(8,24)`
     in the object projection, while `get_champion_view` shows Mara at
     `(14,22)`.
   - Player two sees `champion:east` at `(39,24)` and `pile:east-wood-1` at
     `(38,25)`, while `get_my_champions` shows Korrin at `(38,25)`.
   - Both object pages returned `page_info = null`, and all sampled object
     `details` were `null`.

   Needed:

   - Fix object projection for both factions, not only the west-side route.
   - Historical recommendation: populate typed object details or document the
     detail endpoint to call by subject kind.
   - Return page metadata consistently.

6. Build/update ordering and preview/update parity still need cleanup.

   Evidence:

   - `preview_build_town_structure` for `building:weighhouse` still hit IC0522
     after `skirmisher-stall` was built.
   - `submit_build_town_structure` for `building:skirmisher-stall` succeeded,
     proving the update path can do work the preview path cannot budget.
   - A second build attempt in the same turn for `building:weighhouse` returned
     raw `insufficient_resources` because player one had only `340` gold after
     the prior build, even though `last_built_turn = current_turn = 10` also
     made the action illegal.

   Needed:

   - Keep preview/update validation order consistent where possible.
   - Return `already_built_this_turn` when the one-build-per-turn rule is the
     primary client-action blocker, or document the validation priority order.

7. `mark_ready` on an active session uses an ambiguous error code.

   Evidence:

   - `mark_ready` by player one on the active match returned
     `session_not_joinable`.
   - `mark_ready` by a registered non-participant also returned
     `session_not_joinable`.
   - The code is accurate for `join_session`, but confusing for a ready-state
     command.

   Needed:

   - Return a ready-specific code such as `session_not_in_lobby` or
     `participant_not_found`, depending on which validation failed first.

8. Sync receipts continue to under-report returned events.

   Evidence:

   - The turn-11 sync returned two events, `income_materialized` seq `80` and
     `session_turn_synced` seq `81`, but its embedded `StrategicReceipt` again
     reported `event_count = 1`.

   Needed:

   - Fix receipt counters or rename them to describe only command-domain
     events.

Additional IcyDB evidence after this pass:

- `icydb_snapshot` still reported `corrupted_entries = 0` and
  `corrupted_keys = 0`.
- Relevant row counts reached `GameCommand=118`, `GameEvent=81`,
  `CommandEffect=99`, `MovementIntent=8`, `MovementSnapshot=15`,
  `ResourceLedgerEntry=23`, `ResourceLedgerTurnSummary=7`, `Champion=4`,
  `PlayerAccount=3`, `MapOccupancy=6`, `ParticipantKnownObject=13`,
  `TownBuilding=4`, `TownGarrisonStack=1`, `TownRecruitPool=2`, and
  `WorldObject=19`.

## Historical P0 Findings From 2026-05-16

### 0. Early end-turn map-turn readiness

Historical status on 2026-05-16: missing. Current status: fixed for the
backend contract by the public `end_turn` endpoint and covered timer/end-turn
tests.

Current outcome:

- Active map turns close when all active participants call `end_turn` or when
  the deadline/sync job resolves.
- The canister exposes `end_turn(session_id, client_nonce)` in the public
  endpoint inventory.
- `mark_ready` remains lobby-only. Active UI should not surface it during play;
  if called after activation, the current canister returns `session_not_joinable`.
- Treat `end_turn` as a readiness marker, not a player lock. A participant who
  ended may still submit movement/build/recruit/trade/spell commands while the
  same map turn remains open.
- Do not clear the ended marker when the player submits later commands in the
  same turn.
- If the final active map participant ends, close the map turn immediately
  through the same deterministic turn-resolution path used by timeout sync.
- Keep battle timing separate: a participant with champions in active battles
  may still end the world-map turn, while battle actions continue through
  `submit_battle_action` / `sync_battle`.
- Add PocketIC coverage for:
  P1 ends then still submits a move before all have ended, and the move
  resolves;
  the last participant ends and advances the turn immediately without waiting
  60 seconds;
  commands aimed at the old turn after immediate closure fail stale/expired or
  require refresh;
  a participant with a champion in battle can still end the map turn.

Backend scheduling design:

- Do not make normal clients responsible for backend progression. Clients
  should submit gameplay commands and read views/events; they should not have
  to call `sync_session_turn`, `sync_battle`, or future `process_next_turn`
  style endpoints just to keep the game alive.
- Use IC timers through `canic_cdk::timers` / `ic-cdk-timers`. The dependency is
  already present through `canic-cdk`, and it supports one-shot timers,
  recurring timers, zero-delay timers, and `clear_timer`.
- Treat timers as wakeups only. Timer IDs are volatile and are not persisted
  across canister upgrades; late or duplicate timer execution is also possible.
  Durable IcyDB state must determine whether work is still due.
- Prefer a durable `SystemJob` IcyDB entity over ad hoc per-feature timer
  globals:
  `job_key`, `job_kind`, `session_id`, optional `battle_id`, target
  `turn_number`, `due_at`, `status`, `lease_owner`, `lease_expires_at`,
  `attempt_count`, `generation`, `command_id`, `cursor_json`, `last_error`,
  `created_at`, and `updated_at`.
- Enforce unique deterministic job keys, for example
  `turn_deadline:{session_id}:{turn_number}`,
  `turn_resolution:{session_id}:{turn_number}`,
  `battle_timeout:{battle_id}:{deadline_ms}`, and
  `turn_resolution_continue:{command_id}`.
- Keep only one in-memory timer handle for the next due system job where
  practical. When an earlier job is inserted, clear the previous volatile
  timer if we still have its `TimerId` and schedule a new one. If cancellation
  fails because the canister upgraded or another timer already fired, the
  stale callback must no-op after checking durable state.
- On session start and on every new map turn, insert or update the
  `turn_deadline` job for `turn_deadline_at` and schedule the global next-job
  timer.
- On `end_turn`, write `ParticipantTurnReady` first. Because IC update
  messages are processed one at a time for a canister, the third of three
  participants can safely observe that all active participants are ready,
  create the `turn_resolution` system job for the current turn, and try to run
  it in the same update call.
- Closing a turn should freeze the closing turn. After a `turn_resolution` job
  is accepted for `(session_id, turn_number)`, commands for that old turn must
  be rejected as stale or require a refreshed view. Commands may be accepted
  again once the session has advanced to the next turn.
- Ending turn remains only a readiness marker until turn resolution starts. A
  participant that ended early can still move, build, recruit, trade, or cast
  while the same map turn remains open.
- If the `end_turn` update has enough instruction budget, it can run a bounded
  `process_turn_resolution_slice` immediately. If work is incomplete or the
  implementation wants a fresh instruction limit, enqueue a continuation job
  and schedule `set_timer(Duration::from_secs(0), ...)`.
- `process_turn_resolution_slice` should be idempotent and resumable:
  recover/apply pending movement, resolve object pickups and battles, write
  resource income, emit events/effects, advance `GameSession.current_turn`,
  reset `turn_started_at` and `turn_deadline_at`, clear readiness for the new
  turn by natural keying, schedule the next deadline job, and complete the
  system command. Store any cursor/progress needed in `SystemJob.cursor_json`
  or command phase fields before scheduling the next slice.
- Historical proposal, now superseded: `sync_session_turn` is a public
  sync/recovery boundary that calls the same bounded turn runner used by timers
  and continuations.
- Battles need the same treatment. `submit_battle_action` already calls
  `apply_due_timeouts`, and `sync_battle` currently exists so clients can
  force auto-defend timeout progress. Instead, when a battle action deadline is
  set or advanced, upsert a `battle_timeout` job. The timer runner should apply
  bounded timeout auto-defends, schedule zero-delay continuation jobs if the
  timeout budget is exhausted, apply battle aftermath when the battle resolves,
  and schedule the next stack action deadline if the battle remains active.
- Historical proposal, now superseded: `sync_battle` is a public battle
  sync/recovery boundary over the same timeout/round/aftermath runner used by
  timers and continuations.
- Persist processing state, not Rust-local locks. A session or job should have
  a durable processing flag/lease such as `processing_kind`,
  `processing_turn`, `processing_command_id`, and `lease_expires_at`. Since IC
  messages are serialized, this protects against interleaving between timer
  callbacks and client updates without relying on an in-memory mutex that
  disappears on upgrade.
- Avoid holding mutable state across `await`. Any phase that uses an await,
  self-call, or timer must first persist the command/job phase and lock state.
  The next message reloads state from IcyDB, verifies the command idempotency
  key and expected turn/deadline, and continues or no-ops.
- On `init` / `post_upgrade`, scan active sessions and active battles in
  bounded slices, repair missing due `SystemJob` rows, and reschedule the next
  global timer. Because timers are not upgrade-persistent, this rescan is
  required for correctness.
- If there are many sessions, the upgrade rescan should enqueue a
  `reschedule_after_upgrade` job and continue via zero-delay timers until every
  active session/battle has been checked.
- Every system job should produce normal `GameCommand`, `GameEvent`, and
  `CommandEffect` rows with actor `"system"` and deterministic idempotency
  keys. That keeps history, recovery, diagnostics, and receipt rendering on the
  same path as player commands.
- Public views should expose enough state for clients to render waiting states:
  `turn_closing`, `turn_deadline_at`, participant readiness, active processing
  job kind, `retry_after_ms`, and battle `action_deadline_at`/`processing`
  fields.

PocketIC coverage needed for the backend-scheduled path:

- Starting an active session schedules a durable `turn_deadline` job and an IC
  timer; advancing PocketIC time past 60 seconds advances the turn without any
  client calling `sync_session_turn`.
- With three participants, the final `end_turn` call closes the turn
  immediately or schedules a zero-delay continuation that closes after ticking
  timers, without waiting for the 60-second deadline.
- A player who ended early can still submit commands while the turn remains
  open; once all players ended and turn resolution starts, old-turn commands
  fail stale or require refresh.
- Replaying the same timer callback, firing a late old deadline timer, or
  inserting the same deterministic `SystemJob` twice does not duplicate
  movement, income, events, or turn increments.
- A simulated upgrade drops volatile timer IDs, then `post_upgrade` reconstructs
  missing jobs/timers from IcyDB and overdue turns/battle timeouts still
  resolve.
- Battle action deadlines auto-defend through timers without a client
  `sync_battle` call, including the existing budgeted case where only part of
  the timeout work fits in one update and a zero-delay continuation completes
  the rest.
- Client updates during `turn_resolution` or `battle_timeout` observe durable
  processing state and return deterministic retryable errors instead of
  racing, partially applying duplicate commands, or requiring the client to
  guess which sync endpoint to call.

### 1. Human-playable client and local run path

Status: still no full UI app in this repo. Current backend/client contract is
green for starting UI work, and the local canister/blast path is documented.

Evidence:

- The workspace contains canister, rules, schema, and testing crates, but no
  runnable UI app or binary client: `Cargo.toml`, `testing/client-probe/Cargo.toml`.
- README only documents smoke/test commands, not how to launch and play:
  `README.md:33`, `TESTING.md:37`.
- `PlayableWebClient::play_first_playable_walkthrough` is scripted and
  hard-codes the whole route: `testing/client-probe/src/web/controller.rs:51`.
- Movement paths, build/recruit choices, and battle defend are fixed in code:
  `testing/client-probe/src/web/controller.rs:131`,
  `testing/client-probe/src/web/controller.rs:241`,
  `testing/client-probe/src/web/controller.rs:326`.

Needed:

- Add documented local launch steps, including canister/Pocket-IC setup,
  wasm target prerequisites, and the local agent-run `blast` smoke command
  sequence. Do not add committed `blast` scripts unless explicitly requested.
- Add one PocketIC route or direct local `blast` evidence proving the public
  canister endpoints complete the first playable route without test-only
  shortcuts.

### 2. Public render composition over canister views

Historical status on 2026-05-16: under-implemented for real play. Current
outcome: fixed for v1.1 by the documented endpoint composition contract in
`docs/canister-endpoints.md`, dedicated bounded detail endpoints, Gate M, and
the current `59/59` endpoint benchmark coverage.

Evidence:

- The canister `get_game_view` intentionally returns a lightweight shell and
  leaves map/object/champion/town/battle detail to dedicated endpoints:
  `canisters/degens/src/services/game_view.rs:19`,
  `canisters/degens/src/services/game_view.rs:47`,
  `docs/canister-endpoints.md:77`.
- The test adapter manually composes the view in
  `testing/pocket-ic/tests/client_probe_canister.rs`, but the public endpoint
  contract is not explicit enough outside tests.

Needed:

- Provide the documented endpoint composition contract that builds renderable
  game state from public canister endpoints. `get_game_view` remains a metadata
  shell with explicit omissions in v1.1.
- Ensure the public contract covers pagination, command status, event refresh,
  backend-work-pending state, battle views, result/history, and typed errors.

### 3. Dynamic exploration, visibility, and object discovery

Historical status on 2026-05-16: missing or inconsistent across pure rules and
canister projection. Current status: fixed for v1.1 render projection and
covered by the benchmark gates listed in the current status table above.

Evidence:

- Pure movement updates champion position/occupancy but does not update
  `VisibilityChunkRecord` or `ParticipantKnownObjectRecord`:
  `crates/domm-game/src/movement/sync.rs:692`.
- Pure movement preview rejects hidden tiles, so exploration can stall once a
  player leaves the initially revealed area:
  `crates/domm-game/src/movement/preview.rs:103`.
- Canister movement refreshes visibility bitsets but does not create newly
  discovered known-object rows:
  `canisters/degens/src/services/movement.rs:1157`.
- `get_visible_objects` is based on `ParticipantKnownObject` and static
  scenario details, so collected piles/captured mines can render stale:
  `canisters/degens/src/services/render_projection.rs:81`,
  `canisters/degens/src/services/render_projection.rs:448`,
  `canisters/degens/src/services/render_projection.rs:607`.

Needed:

- On movement/sync, update discovered and visible tiles around champions.
- Materialize newly discovered objects into known-object rows.
- Render dynamic world-object state from persisted `WorldObject` rows, not only
  static scenario definitions.
- Add canister and pure e2e tests that move into newly revealed territory,
  discover an object, interact with it, then verify the rendered object state
  changes.

### 4. Movement validation parity at the canister boundary

Historical status on 2026-05-16: incomplete. Current outcome: the public spec
has been narrowed to match current canister behavior. Canister v1.1 validates
length, bounds, adjacency, terrain cost, and supported blockers; it allows
static undiscovered terrain and reports, but does not reject by, chunk count.

Historical evidence from the original audit:

- Spec requires movement validation to include visibility/fog rules and v1 path
  caps: `spec.md:4328`, `spec.md:5321`.
- Canister `submit_move_intent` validated path length, bounds, adjacency, and
  terrain cost, but not fog/discovery or the 8-chunk touched cap:
  `canisters/degens/src/services/movement.rs:54`,
  `canisters/degens/src/services/movement.rs:2146`.
- Preview stop reporting was incomplete in the original local pass.

Current contract:

- Canister movement validates active ownership, bounds, adjacency, impassable
  terrain, path cost, path length, and supported blockers.
- It reports `chunks_touched` without rejecting chunk count and allows static
  undiscovered terrain.
- Preview reports supported object/neutral stops.

### 5. Remove synthetic battle/town-capture paths from playable proof

Historical status on 2026-05-16: under-proven; pure/probe route relied on
fixture shortcuts. Current outcome: the canister-backed route is covered by
Gate L/Gate K/Gate M plus battle-round/render-projection evidence; pure fixture
shortcuts remain test helpers, not the public canister proof.

Evidence:

- Pure movement drafts battle starts, but tactical battle setup is still a
  hard-coded fixture for `champion:west` vs `neutral:west-mine`:
  `crates/domm-game/src/movement/sync.rs:1000`,
  `crates/domm-game/src/battle/build.rs:16`.
- The fixture backend lazily builds the fixture battle:
  `crates/domm-game/src/api/backend.rs:1301`.
- Pure playable backend seeds resolved battles to create champion defeat and
  town capture:
  `crates/domm-game/src/playable/backend.rs:221`,
  `crates/domm-game/src/aftermath/smoke.rs:48`.
- The web probe's fixture result calls a separate backend gate final view rather
  than naturally finishing the same client session:
  `testing/client-probe/src/web/controller.rs:378`,
  `testing/client-probe/src/web/service.rs:448`.

Needed:

- Ensure movement-created neutral/champion/town encounters create the tactical
  battle that the client actually opens and resolves.
- Ensure town capture and victory are caused by that battle aftermath, not by
  seeded resolved fixtures.
- Add an e2e assertion that the battle id opened by the client was created by
  the preceding movement command and that its aftermath produced town capture
  and victory.

### 6. Recruitment into champion armies

Historical status on 2026-05-16: exposed in DTO/spec, not implemented in the
canister submit path. Current status: fixed for same-tile owned active champion
targets.

Evidence:

- `RecruitTarget::Champion` exists:
  `crates/domm-game/src/town/types.rs:75`.
- Spec says recruiting into a champion is allowed when the champion is at the
  owned town: `spec.md:4656`.
- The old canister submit path rejected champion targets, so only town garrison
  recruitment worked:
  `canisters/degens/src/services/town.rs:160`,
  `canisters/degens/src/services/town.rs:535`.

Current outcome:

- Same-tile canister-backed recruitment into owned active champion army stacks
  is implemented.
- Current tests cover valid champion recruitment plus invalid-target
  no-mutation behavior.
- Broader recruitment variants remain V2.

### 7. Core command recovery for build/recruit and ledger replay

Historical status on 2026-05-16: risky for IC playability under traps/retries.
Current outcome: fixed for v1.1 command recovery coverage, including
build/recruit/economy retry and ledger no-drift tests.

Evidence:

- Spec requires durable recovery before turn advancement:
  `spec.md:4465`, `spec.md:5651`.
- The recoverable command list omits `submit_build_town_structure` and
  `submit_recruit_units`:
  `canisters/degens/src/services/command_response.rs:104`.
- Build/recruit use participant command rows but may not re-enter recovery when
  stuck pending/applying:
  `canisters/degens/src/services/town.rs:202`,
  `canisters/degens/src/services/town.rs:366`.
- Resource ledger handling skips existing ledger rows without always
  reconciling the participant balance to `balance_after` after a partial trap:
  `canisters/degens/src/services/town.rs:616`,
  `canisters/degens/src/services/town.rs:262`.

Needed:

- Add build/recruit to recoverable command handling or implement equivalent
  domain recovery.
- Prove partial ledger write plus trapped participant update resumes without
  double-spend or lost spend.
- Add native and Pocket-IC retry/recovery tests for build and recruit.

## Historical P1 Findings From 2026-05-16

### 8. Guarded mine capture after guard defeat

Historical status on 2026-05-16: resolved for the direct local canister route
while automated PocketIC coverage was still needed. Current outcome: covered by
Gate L/Gate K/render-projection/battle-round coverage and local blast evidence.

Evidence:

- The scenario walkthrough expects the guarded mine fight to start income:
  `crates/domm-game/src/content.rs:573`.
- Neutral aftermath can move the champion after defeating the guard:
  `crates/domm-game/src/aftermath/actions.rs:199`.
- Object capture exists and validates guard defeat, visibility, and champion
  position:
  `crates/domm-game/src/world_object/actions.rs:247`,
  `crates/domm-game/src/world_object/actions.rs:481`.
- Tests cover direct/manual placement more than the full guarded-mine route:
  `crates/domm-game/src/world_object/tests.rs:224`.
- Earlier live `blast` evidence showed the bug: after defeating
  `neutral:west-mine`, the winning champion stood on the mine but the mine only
  captured after an artificial step away/back.
- 2026-05-17 direct local `blast` session `01KRTT8MHY0000000000000008`
  resolved battle `01KRTTH6XV0000000000000004` without an artificial
  post-battle movement. The final `RangedAttack` emitted `mine_captured`,
  `neutral_defeated`, and `battle_aftermath_applied`.
- `get_visible_objects` for the guarded tile then omitted the defeated neutral,
  returned champion `01KRTTABXS0000000000000002` active at `(12,22)`, and
  rendered `mine:west-gold` owned by participant
  `01KRTT8MHY000000000000000A` with `state:"captured"`.
- The public event feed included `income_materialized` with `{"gold":250}`;
  `get_my_participant` gold increased from `10000` to `10250`; and
  `icydb_snapshot` reported `corrupted_entries = 0` and `corrupted_keys = 0`.

Needed:

- Add automated PocketIC coverage for the public first-playable route so the
  direct local `blast` evidence becomes a regression test.
- Extend the route assertion set to include occupancy/victory/idempotency checks
  around the already-proven capture, render, and income behavior.

### 9. Max-turn stalemate finalization

Historical status on 2026-05-16: incomplete for long-running matches. Current
outcome: max-turn scenario rules can report `max_turn_reached`, but full
stalemate finalization by mines, army power, and a distinct `max_turn_score`
finish reason is not the public UI contract. Clients should treat
`GameSession.state`, `winner_participant_id`, `finish_reason`, victory events,
and match history as authoritative.

Evidence:

- Spec requires max-turn scoring by towns, mines, army power, then seeded
  tie-break: `spec.md:5499`, `spec.md:5507`.
- `sync_session_turn` advances turns but does not finalize max-turn victory:
  `canisters/degens/src/services/movement.rs:158`.
- Scenario progress marks `max_turn_reached` but does not finalize the match:
  `canisters/degens/src/services/scenario_progress.rs:777`.

Needed:

- Finalize max-turn victory from turn sync or a clearly documented scenario
  sync endpoint.
- Add Pocket-IC coverage for max-turn scoring and tie-break determinism.

### 10. Battle spellcasting as a normal legal action

Historical status on 2026-05-16: half surfaced. Current outcome: fixed for the
bounded v1.1 learned-spell slice. `CastAbility` is available through
`submit_battle_action` when legal, with disabled reasons when unavailable;
broader spell/status expansion remains V2.

Evidence:

- Docs say battle spellcasting uses `submit_battle_action` with `CastAbility`:
  `docs/canister-endpoints.md:100`.
- Canister has a `CastAbility` path, but legal actions mark it disabled and
  champion render projection returns no spell slugs:
  `canisters/degens/src/services/battle.rs:236`,
  `crates/domm-game/src/battle/actions.rs:95`,
  `canisters/degens/src/services/render_projection.rs:238`.

Needed:

- Either expose learned spells as legal `CastAbility` actions with disabled
  reasons when unavailable, or remove battle spellcasting from v1 active scope.
- Add a canister test that learns a spell, opens battle, sees the legal action,
  casts it, and verifies mana/status/damage persistence.

### 11. Town economy after capture and unrest

Historical status on 2026-05-16: partial. Current outcome: narrowed and
documented. V1.1 active recurring income is captured mine income; town/building
income effects such as `town_income_gold_250`, unrest penalties, pacification,
recruit-pool halving, and desperation income remain V2/content-deferred.

Evidence:

- Historical spec text referenced town hall income and capture/unrest behavior:
  `spec.md:747`, `spec.md:5561`, `spec.md:5570`.
- Canister income currently scans mines and grants flat gold:
  `canisters/degens/src/services/movement.rs:1907`.
- Town aftermath changes owner/unrest without first materializing owed income:
  `canisters/degens/src/services/battle_aftermath.rs:183`.
- Build/recruit checks do not enforce unrest:
  `canisters/degens/src/services/town.rs:117`,
  `canisters/degens/src/services/town.rs:164`.

Needed:

- Town/building income effects, unrest reduction, unrest penalties,
  pacification, recruit-pool halving, and desperation income are v2. V1.1
  active economy is captured mine income, pickups/rewards, costs, tavern
  hiring, fixed-rate market trade, direct dwelling recruitment, and weekly
  tavern/recruit growth.

## Historical P2 Spec Drift From 2026-05-16

### 12. Retreat and surrender

Historical status on 2026-05-16: spec said active while implementation/docs
disabled them. Current outcome: v1.1 docs now keep retreat/surrender disabled
or deferred; they may appear only as disabled action metadata.

Evidence:

- Spec lists retreat/surrender battle behavior:
  `spec.md:4976`, `spec.md:5546`.
- Pure battle/aftermath disables retreat/surrender:
  `crates/domm-game/src/battle/actions.rs:95`,
  `crates/domm-game/src/aftermath/actions.rs:179`.
- Canister battle service does not implement a surrender path:
  `canisters/degens/src/services/battle.rs:839`.
- Endpoint docs mark retreat/surrender deferred:
  `docs/canister-endpoints.md:136`.

Needed:

- For v1.1 playability, keep retreat/surrender out of active v1 scope and park
  them in `spec.v2.md`; v1.1 may expose only disabled action metadata.

### 13. README/TESTING docs are stale for Pocket-IC and canister play

Historical status on 2026-05-16: stale. Current outcome: README/TESTING now
document current canister and Pocket-IC status; remaining details live in the
test/perf command docs.

Evidence:

- README/TESTING still describe Pocket-IC as a scaffold:
  `README.md:27`, `TESTING.md:27`.
- Final audit says Gate L/Gate M are full canister coverage:
  `docs/first-playable-final-audit.md:15`.
- Pocket-IC tests build wasm internally and assume wasm target/linker setup:
  `testing/pocket-ic/tests/client_probe_canister.rs:1564`,
  `testing/pocket-ic/tests/client_probe_canister.rs:1598`.

Needed:

- Update README/TESTING with current canister test status, prerequisites, and
  local `blast` smoke/play instructions.

## Not Counted As Missing V1 Playability

These are intentionally V2 or explicitly disabled and should not block a first
playable unless they are promoted back into `spec.md`:

- Active siege engines, fortifications, naval movement, boats, shipyards, and
  larger/procedural-map materialization.
- Durable rematch creation, ranked leaderboard, guilds, diplomacy, campaign
  persistence, social/meta systems.
- Neutral roaming/join/bribe behavior and broad neutral AI expansion.
- Large spell trees, additional skill branches, artifact sets, large content
  packs, and full bot opponents.

## Local Blast Evidence - 2026-05-17

Status: local deploy and scan passed after embedding public Candid metadata.

Environment:

- DFX: `0.32.0`
- Host: `http://127.0.0.1:8080` from `dfx info webserver-port`
- Canister: `uxrrr-q7777-77774-qaaaq-cai`
- Deploy identity: `domm-local-smoke`
- Blast identity 1 principal:
  `azbl5-a6gw2-6vetv-yoybw-cgbl5-7umx2-oim6f-rysnw-27g3i-cswt4-bae`
- Blast identity 2 principal:
  `gcese-ge3kk-5f54t-7bl4t-y2vxf-2fga7-cnanr-a5pgw-bwr33-pb4ch-vqe`

Commands run:

```text
DFX_IDENTITY=domm-local-smoke IC_WASM=/nix/store/8qsl9cdb7l6zd0lazygf1y5v5kpaaw54-ic-wasm-10f1b59/bin/ic-wasm dfx deploy degens --network local
CANISTER_ID="$(DFX_IDENTITY=domm-local-smoke dfx canister id degens --network local)"
HOST="http://127.0.0.1:$(dfx info webserver-port)"
blast scan "$CANISTER_ID" --host "$HOST"
blast call "$CANISTER_ID" get_canister_endpoint_inventory '[]' --host "$HOST" --id 1
blast call "$CANISTER_ID" register_player '["blast-p1","Blast Player One","nonce:blast:register:1"]' --host "$HOST" --id 1
blast call "$CANISTER_ID" register_player '["blast-p2","Blast Player Two","nonce:blast:register:2"]' --host "$HOST" --id 2
blast call "$CANISTER_ID" create_session '["Blast Smoke","ruleset:first-playable:v1",42,"nonce:blast:create"]' --host "$HOST" --id 1
blast call "$CANISTER_ID" join_session '["01KRSZGA7J0000000000000008","faction:ashen-ledger","nonce:blast:join"]' --host "$HOST" --id 2
blast call "$CANISTER_ID" mark_ready '["01KRSZGA7J0000000000000008","nonce:blast:ready:1"]' --host "$HOST" --id 1
blast call "$CANISTER_ID" mark_ready '["01KRSZGA7J0000000000000008","nonce:blast:ready:2"]' --host "$HOST" --id 2
blast call "$CANISTER_ID" start_session '["01KRSZGA7J0000000000000008","nonce:blast:start:0"]' --host "$HOST" --id 1
...
blast call "$CANISTER_ID" start_session '["01KRSZGA7J0000000000000008","nonce:blast:start:9"]' --host "$HOST" --id 1
blast call "$CANISTER_ID" get_session '["01KRSZGA7J0000000000000008"]' --host "$HOST" --id 1
blast call "$CANISTER_ID" get_content_manifest '["domm-first-playable",1]' --host "$HOST" --id 1
blast call "$CANISTER_ID" get_game_view '["01KRSZGA7J0000000000000008",{"viewport":{"x":0,"y":0,"width":12,"height":12},"chunk_cursor":null,"chunk_limit":1,"object_cursor":null,"object_limit":1,"events_after_seq":0,"event_limit":1,"include_battle":false}]' --host "$HOST" --id 1
```

Observed results:

- `blast scan` found all public account, render, preview/update, battle,
  scenario, worldgen, diagnostic, and IcyDB metric endpoints.
- `get_canister_endpoint_inventory` returned the expected inventory, including
  `get_object_view`, `end_turn`, `end_battle_turn`, and
  `sync_world_generation`.
- Two blast identities registered, created/joined/readied a match, and phased
  `start_session` reached `state = "active"` on nonce
  `nonce:blast:start:9`.
- `get_content_manifest` returned `domm-first-playable` version `1` with hash
  `1e7fc4f2b594eb32a08a0059f84a9b07c1a5b89956aae1239182019addd5f0db`.
- `get_game_view` returned the active metadata shell with
  `omitted_fields = ["map_chunks", "objects", "champions", "towns"]` and
  omitted page `has_more = false`.

Post-fix green route after live render projection repair:

```text
DFX_IDENTITY=domm-local-smoke IC_WASM=/nix/store/8qsl9cdb7l6zd0lazygf1y5v5kpaaw54-ic-wasm-10f1b59/bin/ic-wasm dfx deploy degens --network local
blast scan "$CANISTER_ID" --host "$HOST"
blast call "$CANISTER_ID" register_player '["blast-p1-green2","Blast Player One","nonce:blast6:register:1"]' --host "$HOST" --id 1
blast call "$CANISTER_ID" register_player '["blast-p2-green2","Blast Player Two","nonce:blast6:register:2"]' --host "$HOST" --id 2
blast call "$CANISTER_ID" create_session '["Blast Green Smoke","ruleset:first-playable:v1",47,"nonce:blast6:create"]' --host "$HOST" --id 1
blast call "$CANISTER_ID" join_session '["01KRT39ZYW0000000000000008","faction:ashen-ledger","nonce:blast6:join"]' --host "$HOST" --id 2
blast call "$CANISTER_ID" mark_ready '["01KRT39ZYW0000000000000008","nonce:blast6:ready:1"]' --host "$HOST" --id 1
blast call "$CANISTER_ID" mark_ready '["01KRT39ZYW0000000000000008","nonce:blast6:ready:2"]' --host "$HOST" --id 2
blast call "$CANISTER_ID" start_session '["01KRT39ZYW0000000000000008","nonce:blast6:start:0"]' --host "$HOST" --id 1
...
blast call "$CANISTER_ID" start_session '["01KRT39ZYW0000000000000008","nonce:blast6:start:9"]' --host "$HOST" --id 1
blast call "$CANISTER_ID" submit_move_intent '["01KRT39ZYW0000000000000008","01KRT3BF0J0000000000000002",[{"x":9,"y":24},{"x":9,"y":23}],"nonce:blast6:move:p1:wood"]' --host "$HOST" --id 1
blast call "$CANISTER_ID" get_visible_objects '["01KRT39ZYW0000000000000008",{"x":0,"y":16,"width":24,"height":24},null,20]' --host "$HOST" --id 1
blast call "$CANISTER_ID" get_my_participant '["01KRT39ZYW0000000000000008"]' --host "$HOST" --id 1
blast call "$CANISTER_ID" get_events_after '["01KRT39ZYW0000000000000008","public",0,80]' --host "$HOST" --id 1
dfx canister update-settings degens --network local --add-controller azbl5-a6gw2-6vetv-yoybw-cgbl5-7umx2-oim6f-rysnw-27g3i-cswt4-bae
blast call "$CANISTER_ID" icydb_snapshot '[]' --host "$HOST" --id 1
blast call "$CANISTER_ID" icydb_metrics '[null]' --host "$HOST" --id 1
blast call "$CANISTER_ID" get_diagnostic_storage_snapshot '[["GameSession","GameCommand","SystemJob","ParticipantTurnReady"]]' --host "$HOST" --id 1
blast call "$CANISTER_ID" get_diagnostic_storage_snapshot '[["WorldObject","Champion","ParticipantKnownObject"]]' --host "$HOST" --id 1
blast call "$CANISTER_ID" get_diagnostic_storage_snapshot '[["ParticipantObjectVisit","ResourceLedgerEntry","MovementSnapshot"]]' --host "$HOST" --id 1
```

Observed final route results:

- Fresh scan listed `63` public methods.
- Session `01KRT39ZYW0000000000000008` reached active on
  `nonce:blast6:start:9`.
- Player-one champion `01KRT3BF0J0000000000000002` moved to `(9,23)`,
  wood increased to `15`, and the public event feed contained `12` events.
- `get_visible_objects` returned `champion:west` at `(9,23)` and did not
  return the collected `pile:west-wood-1`.
- `icydb_snapshot` reported `corrupted_entries = 0` and
  `corrupted_keys = 0`; storage index for `DegensStore` reported `666` user
  entries and `777` total entries.
- Small-batch `get_diagnostic_storage_snapshot` succeeded with counts:
  `GameSession=1`, `GameCommand=12`, `SystemJob=11`,
  `ParticipantTurnReady=2`, `WorldObject=19`, `Champion=2`,
  `ParticipantKnownObject=13`, `ParticipantObjectVisit=1`,
  `ResourceLedgerEntry=1`, and `MovementSnapshot=1`.
- A single larger diagnostic batch still exceeded the local 5B instruction
  limit, so local diagnostics should remain batched.

### Deeper direct `blast` route status after v1.1 local fixes

Additional fresh local routes were run from the agent shell with direct
`blast call ... --id 1` and `--id 2` commands against canister
`uxrrr-q7777-77774-qaaaq-cai` on `http://127.0.0.1:8080`.

What now passes:

- Fresh DFX deploys with embedded public Candid metadata still scan as `63`
  public methods.
- Two blast principals can register, create, join, ready, and drive setup to
  `active`; setup still reaches active on continuation nonce `start:9`.
- `preview_move_path` no longer hits IC0522 on local DFX for the opening route.
  It returned cost `15`, available movement `240`, one touched chunk, and a
  `resource_pile` stop for `pile:west-wood-1`.
- The pickup route moved Mara to `(9,23)`, increased wood to `15`, updated
  `get_visible_objects` to show `champion:west` at `(9,23)`, and hid
  `pile:west-wood-1`.
- Economy actions applied through public calls: `freehold-training-yard` was
  built, `mudhook-levy` recruit pool materialized with `15` remaining after
  recruiting `1`, garrison showed one `mudhook-levy`, and participant
  resources were `gold=8930`, `wood=9`, `stone=8`, `iron=3`, `crystal=3`,
  `ember=3`, `aether=3`.
- Direct guarded-mine movement through public `submit_move_intent` plus
  `sync_session_turn` created `neutral_encounter_pending` with battle
  `01KRT85CDF0000000000000004` in session
  `01KRT7X3RC0000000000000008`.
- `icydb_snapshot` still reported `corrupted_entries = 0` and
  `corrupted_keys = 0` after the battle trigger.
- Small diagnostic batches remained usable after the battle trigger:
  `GameSession=1`, `GameCommand=9`, `SystemJob=8`,
  `ParticipantTurnReady=0`, `WorldObject=19`, `Champion=2`,
  `ParticipantKnownObject=13`, `Battle=1`, `BattleStack=3`,
  `BattleOccupancy=3`, and `BattleObstacle=2`.

Historical remaining blocker:

- `get_battle_state(session_id, battle_id)` still exceeds the local 5B
  instruction limit with IC0522 under DFX/PocketIC after the guarded-mine
  battle is created. This remained true after avoiding battle table page scans
  in the query read model and after removing legal-action/spell enrichment from
  the query path. The deeper direct `blast` gate should stay open until this
  public battle read endpoint works on a fresh local canister.
  Current outcome: fixed for the v1.1 public route by battle read slicing,
  runtime-backed battle views, Gate K/Gate L/Gate M, and the 2026-05-22
  benchmark suite.

### Follow-up all-ready and battle-read audit

A subsequent fresh local deploy added two production-side hardening changes:
internal battle-state assembly now uses bounded non-paged reads for
`BattleStack`, `BattleObstacle`, and `BattleOccupancy`, and partial system jobs
reschedule with a short retry delay instead of immediately re-entering at
`Timestamp::now()`. `sync_session_turn` also accepts all-ready turns before the
wall-clock deadline, so clients can deliberately advance once every active
participant has ended the turn.

Direct `blast` evidence from session `01KRTAY8B10000000000000008`:

- Fresh `blast scan` still returned `63` public methods.
- Setup reached `active` at continuation `start:9`.
- Opening `preview_move_path` returned cost `15` and a `resource_pile` stop.
- All-ready `sync_session_turn` advanced the pickup route before the deadline:
  first sync emitted `movement_sync_incomplete` at `(9,24)`, and the second
  completed the pickup at `(9,23)`.
- Public build/recruit still applied for `freehold-training-yard` and one
  `mudhook-levy`.
- Guarded movement submitted and advanced at least one partial slice to
  `(12,23)`, with public `movement_sync_incomplete` event seq `18`.

Updated blocker:

- The fresh local replica still terminated before the final guarded movement
  sync could return `neutral_encounter_pending`; therefore this run did not
  reach `get_battle_state`. The remaining P0 gate is now the guarded-object
  battle trigger/update path on local DFX, followed by the battle read query.

### Follow-up manual-sync job coordination audit

The next hardening pass made public/manual `sync_session_turn` coordinate with
the durable turn jobs it overlaps:

- A partial manual turn sync now pushes current-turn `turn_resolution` and
  `turn_deadline` jobs out by a 60-second manual recovery window instead of
  leaving the timer free to process the same pending movement immediately.
- A complete manual turn sync now completes current-turn turn jobs, schedules
  the next `turn_deadline`, and schedules the same scenario maintenance jobs as
  timer-owned turn resolution.
- Both manual and timer turn resolution now re-check for pending movement
  intents before advancing the session turn, so a partial movement cursor cannot
  accidentally become a completed turn.

Fresh local DFX evidence after this patch:

- Fresh deploy and `blast scan` still exposed 63 public methods.
- Session `01KRTD5KBY0000000000000008` reached `active` on setup
  continuation `start:9`.
- The opening route still returned preview cost `15` with a `resource_pile`
  stop, and manual all-ready sync completed the pickup with
  `resource_picked_up` plus `session_turn_synced` at `(9,23)`.
- Public economy commands still applied in turn 3:
  `freehold-training-yard`, one `mudhook-levy`, and the guarded movement
  intent.

Remaining blocker:

- Local DFX/blast command latency still allowed the 60-second deadline job to
  start processing the guarded movement before the final player `end_turn`
  could be accepted. The timer advanced the guarded route at least to `(11,23)`,
  but the local PocketIC process later terminated while the guarded route was
  still being resolved. The release gate remains open until a fresh direct
  route returns `neutral_encounter_pending` and `get_battle_state` succeeds on
  that battle id.

### Follow-up delayed partial retry audit

The partial retry delay now applies to both timer-owned and manual turn
resolution slices. A fresh local deploy with direct `blast` commands against
canister `uxrrr-q7777-77774-qaaaq-cai` reached the guarded route without the
previous final-player `end_turn` race.

Fresh local DFX evidence from session `01KRTDXYHB0000000000000008`:

- Fresh `blast scan` still exposed 63 public methods.
- Setup reached `active` on continuation nonce `start:9`.
- The opening `preview_move_path` returned cost `15` with a `resource_pile`
  stop; two manual all-ready sync calls completed the pickup at `(9,23)` with
  `resource_picked_up` and `session_turn_synced`.
- Public economy commands applied in turn 3:
  `freehold-training-yard`, one `mudhook-levy`, and the guarded movement
  intent.
- Player two `end_turn`, build, recruit, guarded movement submit, and player
  one `end_turn` all returned `Applied`.
- The first guarded manual sync returned `movement_sync_incomplete` with Mara at
  `(12,23)` and status `active`.

Updated blocker:

- The second guarded manual sync exceeded the local update instruction limit:
  `IC0522`, "Canister exceeded the limit of 50000000000 instructions for single
  message execution". It failed before returning `neutral_encounter_pending` or
  a battle id, so this run still could not exercise `get_battle_state`.
- The remaining P0 gate is now the final guarded-object battle trigger/update
  budget on local DFX, followed by the public battle read endpoint on the
  returned battle id.

### Follow-up guarded battle phase slicing and cheap battle read audit

The guarded-object battle start path is now split into durable battle substates
instead of trying to create the battle, all stacks, obstacles, active-stack
selection, timeout job, neutral state, and movement stop event in one update.
The public battle read endpoint also avoids the full canister-side
`BattleState` aggregate for the initial tactical view, while preserving the
disabled Retreat/Surrender affordances and learned spell affordances through a
row-based enrichment path.

Fresh local DFX evidence from session `01KRTJPA140000000000000008` against
canister `uxrrr-q7777-77774-qaaaq-cai`:

- Fresh `blast scan` exposed 63 public methods.
- Setup reached `active` on continuation `start:9`.
- The player-one champion was `01KRTJQS1S0000000000000002`.
- Direct guarded `preview_move_path` from `(8,24)` to the west mine returned
  cost `30` and stop reason `guarded_object`.
- Guarded turn resolution returned `movement_sync_incomplete` for sync slices
  `0..8`.
- Sync slice `9` returned `neutral_encounter_pending` and
  `session_turn_synced` with battle `01KRTJYSF60000000000000004`.
- `get_battle_state` returned an `active` neutral battle with active stack
  `01KRTJZ14X0000000000000008`, three stacks, and legal actions including
  enabled `RangedAttack`, `Defend`, and `Wait`.
- `icydb_snapshot` reported `corrupted_entries = 0` and `corrupted_keys = 0`.

Updated status:

- The guarded-object trigger/read blocker is cleared for the direct local route,
  and the follow-up direct local route now also proves guard defeat,
  guarded-mine capture, post-battle render agreement, later mine income, and
  zero IcyDB snapshot corruption.
- Local DFX battle updates can leave very short action windows after slow
  timeout/round-advance calls. The canister battle path now slices timeout work
  to one expired stack per update and keeps a narrow submit/read grace for the
  persisted active stack, which the final direct route exercised successfully.
- A broader native service smoke,
  `timeout 180 cargo test -p domm-degens-canister
  lobby_session_setup_recovers_from_starting_state_and_replays_nonce --
  --nocapture`, timed out after the test had been running for over 60 seconds,
  so it is not counted as passing gate evidence.
- Remaining gap: add automated PocketIC/regression coverage for this exact route
  and broaden assertions to occupancy/victory/idempotency.

### Follow-up PocketIC guarded route assertion audit

The targeted Gate L PocketIC test now includes assertions for guarded-mine
capture and no stale render state after battle resolution:

- exactly one `mine_captured`, `neutral_defeated`, and
  `battle_aftermath_applied` event after the guarded battle;
- replaying `sync_battle` on the resolved neutral battle returns `Applied`
  without duplicate aftermath/capture events;
- `get_visible_objects` omits `neutral:west-mine` and renders
  `mine:west-gold` with the west participant owner and `state:"captured"`;
- final public event refresh expects `income_materialized`.

Follow-up 2026-05-17 evidence clears this automated blocker. The targeted Gate
L PocketIC run passed:

```text
cargo test -p domm-pocket-ic-tests --test canister_endpoints \
  pocket_ic_gate_l_first_playable_canister_e2e_uses_public_endpoints_and_icydb_state
```

The passing run exercised guarded-mine battle creation, guard defeat,
`mine_captured`, `neutral_defeated`, `battle_aftermath_applied`, resolved
`sync_battle` no-op idempotency, defeated-neutral render absence, captured mine
owner/state, later `income_materialized`, champion battle aftermath, town
capture, `victory_finalized`, and final diagnostics including
`ParticipantObjectVisit`. The implementation changes that made this pass were
mostly budget slicing: neutral battle start/activation is split, resolving
battle actions defer map aftermath to `sync_battle`, long champion/town
encounters return as partial movement instead of also closing the map turn, and
hot battle views avoid extra readiness/job queries.
