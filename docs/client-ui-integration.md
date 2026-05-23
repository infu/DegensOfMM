# Client UI Integration Guide

Current status as of 2026-05-22: the canister-backed client contract is ready
for UI implementation. The required gameplay inventory is `59` endpoints, the
latest recorded full benchmark suite covered `59/59`, Gate M drove the
canister-backed client probe through gameover, and `docs/canister-endpoints.md`
is checked against `canisters/degens/src/contract.rs`.

This file explains the client-facing flow. Generated Candid remains the exact
wire contract; this guide is the practical call order and recovery model.

## Sources Of Truth

- Endpoint inventory and deferred endpoints: `docs/canister-endpoints.md`.
- Exact public DTOs: `crates/domm-game/src/api/types.rs` plus map, battle,
  champion, town, economy, scenario, and worldgen DTO modules.
- Exported Candid and required endpoint list: `canisters/degens/src/contract.rs`.
- Canister-backed client proof: Gate M in `testing/pocket-ic/tests/client_probe_canister.rs`.
- Current performance/test status: `perf1.measure.md`.

## Session Startup

The UI starts from typed Candid endpoints only. It must not call generic SQL or
diagnostic endpoints for normal play.

1. Load content with `get_content_manifest(ruleset_id, version)`.
2. Register or load identity with `register_player(..., client_nonce)` and
   `get_my_player()`.
3. Create and join a lobby with `create_session`, `join_session`, `mark_ready`,
   and `start_session`.
4. Call `start_session` once. Setup continues through canister-owned jobs and
   timer/continuation work.
5. Poll `get_session(session_id)` and, while setup is not active, optionally
   show `get_setup_progress(session_id)`.
6. Once `get_session.state == "active"`, enter the map view.

`mark_ready` is lobby-only. During active gameplay, map-turn readiness uses
`end_turn(session_id, client_nonce)`.

## Render Composition

The canister `get_game_view` is intentionally a lightweight shell. Use it for
session/participant metadata, render timing, content hash, event page metadata,
and omitted-field hints. Compose the renderable game screen from dedicated
bounded endpoints:

```text
get_game_view(session_id, request)
get_visible_map_chunks(session_id, viewport, cursor, limit)
get_visible_objects(session_id, viewport, cursor, limit)
get_my_champions(session_id)
get_champion_view(session_id, champion_id)
get_town_view(session_id, town_id)
get_object_view(session_id, subject_kind, subject_id_text)
get_battle_state(session_id, battle_id)
get_events_after(session_id, audience_key, events_after_seq, limit)
get_command_status(session_id, command_id_or_client_nonce)
get_command_status_by_nonce(session_id, command_type, client_nonce)
```

Map and object cursors are numeric offsets over the current sorted projection;
they are live-state cursors, not snapshot tokens. Clients must tolerate rows
changing between pages. Event cursors use `events_after_seq`.

Recommended render refresh:

1. Query `get_game_view` for metadata and the opening/default viewport.
2. Page `get_visible_map_chunks` until `has_more == false`.
3. Page `get_visible_objects` until `has_more == false`.
4. Query `get_my_champions`; query `get_champion_view` for selected champions
   or detailed army/artifact/spell panels.
5. For visible owned towns, call `get_town_view` before showing build/recruit
   controls.
6. Read `get_events_after` from the last seen event sequence and apply
   returned `ApiEventView` rows.
7. If `GameView.render_time.sync_required` is true, run the relevant sync
   update before trusting turn-sensitive state.

Derive UI controls from previews, detail endpoints, and battle
`legal_actions_for_caller`. Do not rely on the canister `get_game_view`
`action_affordances` field being populated; the canister shell is deliberately
too small to own full action derivation.

Event audiences:

- Use `public` for the public feed.
- Use `participant:{participant_id}` from `get_my_participant` for the player's
  feed. That returns public plus private rows visible to that participant.
- Other audience keys are not client-selectable and return `audience_not_allowed`.

Fog rules hide dynamic state, not necessarily static terrain bytes. The client
may receive static terrain/movement/flag blobs for surveyed map rendering while
owners, occupants, hidden objects, battle detail, and event payloads remain
visibility-gated or redacted.

## Command Model

Every user intent needs one stable `client_nonce`. Retrying the exact same
intent with the same nonce should replay the original command response.
Reusing a nonce with a different payload returns
`duplicate_nonce_payload_mismatch`.

`CommandResponse` and `LobbyCommandResponse` carry the data a UI needs:

```text
command_id
command_type
client_nonce
payload_hash
status / phase / retryable
effective_turn / durable_turn
events
changed_subjects
result
error
```

After any update:

1. Apply returned events and changed subjects optimistically only within the
   stated response.
2. Query `get_command_status(session_id, command_id)` when you have a
   `command_id`.
3. If you only have `{ command_type, client_nonce }`, query
   `get_command_status_by_nonce`; do not depend on command-type inference from
   nonce text.
4. Refresh affected composed views from the query endpoints.
5. If a response or render metadata says backend work is pending, call the
   public sync/recovery endpoint described below.

Some failures happen before command creation, such as accepted-closure
`backend_work_pending`. Those may not have a durable command-status row; the UI
should refresh/sync rather than wait forever for a status that was never
created.

Public callers do not provide `now_ms`. Movement, battle, and sync endpoints
derive time at the canister boundary.

## Map Turns And Movement

The current map turn stays open until every active map participant has ended
turn or the deadline closes the turn. A player who called `end_turn` may still
submit map commands while the same turn is open.

Movement flow:

1. Use `preview_move_path(session_id, champion_id, path)` for UI feedback.
2. Submit with `submit_move_intent(session_id, champion_id, path, client_nonce)`.
3. A champion has one replaceable movement intent per turn.
4. Movement resolves when turn closure runs through `end_turn`, timer work,
   zero-delay continuation, or `sync_session_turn`.
5. Refresh map chunks, visible objects, champion views, events, and command
   status after resolution.

Current canister movement validates ownership, active state, path length,
bounds, adjacency, terrain cost, and supported blockers. Static undiscovered
terrain is allowed; hidden dynamic state remains hidden. The canister reports
`chunks_touched` but does not reject by chunk count.

Late new turn-sensitive commands are rejected after current-turn closure work is
accepted, running, or due. The error is pre-command
`backend_work_pending`/stale-expired behavior; exact retries of commands that
were already created still replay.

## Towns, Economy, And City Actions

Town UI starts from `get_town_view(session_id, town_id)`. Show enabled controls
from previews, not from local assumptions.

Active v1.1 city/economy actions:

```text
preview_build_town_structure / submit_build_town_structure
preview_recruit_units / submit_recruit_units
get_tavern_offers / preview_hire_champion / hire_tavern_champion
preview_market_trade / submit_market_trade
get_dwelling_pool / preview_dwelling_recruit / submit_dwelling_recruit
```

Recruitment supports town garrisons and same-tile owned active champion targets
for town recruitment. External dwelling direct recruitment can target owned
active world-map champions according to the dwelling rules.

Current marketplace rates are fixed. Active recurring income is captured mine
income. Town/building income effects such as `town_income_gold_250`,
marketplace ownership rate improvements, unrest penalties, pacification,
recruit-pool halving, and desperation income are deferred/content metadata.

The generic canister build path prevents duplicate building slugs and missing
prerequisites, but does not enforce a hard one-building-per-town-per-turn
cooldown for all generic buildings.

## Battles

Battle ids usually arrive through movement/encounter events such as
`neutral_encounter_pending`, `champion_encounter_pending`, or
`town_encounter_pending`. Once a battle id is known:

1. Query `get_battle_state(session_id, battle_id)`.
2. Render `BattleView.grid`, obstacles, stacks, initiative, active stack,
   deadline metadata, `legal_actions_for_caller`, and battle events.
3. Submit one of the enabled legal actions with
   `submit_battle_action(session_id, BattleActionInput, client_nonce)`.
4. If the response reports `battle_processing`, `backend_work_pending`, a
   retryable applying status, or an incomplete sync boundary, call
   `sync_battle(session_id, battle_id, client_nonce)` and refresh.
5. `end_battle_turn(session_id, battle_id, client_nonce)` marks battle-round
   readiness. Remaining stacks are auto-defended by round-advance work, possibly
   over zero-delay continuations.

`BattleActionInput.action` is a string such as `Move`, `MeleeAttack`,
`RangedAttack`, `CastAbility`, `Defend`, or `Wait`. Use `destination`,
`target_stack_id`, and `ability_key` according to the legal action. Retreat and
surrender are disabled/deferred affordances in v1.1 and must not be shown as
enabled commands.

`action_deadline_at` is the timeout target. A valid active-stack action may
still be accepted until `action_deadline_at + 15_000ms` if it wins the race
against timeout processing.

When `get_battle_state.state == "resolved"`, call
`sync_battle(session_id, battle_id, fresh_nonce)` until aftermath has applied or
the response is a stable no-op. Then refresh `get_session`, map/object/champion
and town views, events, and match history. This is the handoff that makes
capture, champion defeat, victory/gameover, and summaries visible to the UI.

## Progression, Quests, And Boundary Systems

Implemented bounded progression/magic:

```text
preview_champion_progression
select_champion_level_up
learn_champion_spell
cast_adventure_spell
```

Implemented bounded scenario progress:

```text
get_objective_progress
get_scenario_rules
get_world_events
preview_quest
accept_quest
claim_quest_reward
sync_objectives
sync_world_events
sync_advanced_victory
```

World-generation boundary endpoints expose current settings and disabled rows:

```text
get_skirmish_settings
get_procedural_map_state
get_naval_routes
get_siege_rules
sync_world_generation
```

Active naval movement, siege actions, larger procedural-map gameplay, rematch
creation, ranked/guild/diplomacy systems, full bot opponents, and broader
content packs are V2/deferred unless a later spec promotes a bounded slice.

## Gameover, Results, And History

Victory can come from battle aftermath, town capture, champion defeat, and
implemented scenario/victory sync paths. Max-turn rules may report
`max_turn_reached`, but full stalemate scoring is not a UI contract unless it
is reflected in the session/result fields below. A finished session is visible
through:

```text
get_session(session_id)
get_match_history(cursor, limit)
get_events_after(session_id, "public", after_seq, limit)
get_champion_view / get_town_view / get_battle_state for final detail
```

The canister stores player match summaries for winner/loser history. After
gameover, normal gameplay commands should be treated as closed; the UI should
switch to result/history screens and avoid issuing new gameplay updates except
documented sync/recovery reads needed to display final state.

## Local Development

For local canister smoke:

```text
make dfx-deploy-local
```

Then set:

```text
CANISTER_ID="$(dfx canister id degens --network local)"
HOST="http://127.0.0.1:$(dfx info webserver-port)"
```

Follow `docs/local-deploy-blast.md` for the multi-identity `blast` route.
Gate M is the automated Pocket-IC canister-backed client probe, not a local DFX
browser runner:

```text
cargo test -p domm-pocket-ic-tests --test client_probe_canister gate_m -- --nocapture
```
