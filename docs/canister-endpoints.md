# Canister Endpoint Inventory

This document tracks the current canister API surface for `canisters/degens`.
`domm-game` remains the deterministic rules and DTO contract layer used by the
canister services and tests.

Current status as of 2026-05-22: the required gameplay inventory is `59`
endpoints. The full benchmark suite at
`target/benchmarks/20260522-164948-5cfd001` covered `59/59` required
endpoints, Gate M's canister-backed web-client probe passed, and the
hard-target audit reported zero violations.

Gameplay endpoints must use typed Candid methods and typed IcyDB repository
paths. Generic SQL is allowed only for controller-gated diagnostics or test
fixture loading, not for normal gameplay.

For the UI call order and gameplay loops built from these endpoints, see
`docs/client-ui-integration.md`.

## Required Endpoints

| Endpoint | Kind | Fixture mapping |
| --- | --- | --- |
| `register_player` | update | `FixtureApiBackend::register_player` |
| `get_my_player` | query | `FixtureApiBackend::get_my_player` |
| `create_session` | update | `FixtureApiBackend::create_session` |
| `join_session` | update | `FixtureApiBackend::join_session` |
| `mark_ready` | update | `FixtureApiBackend::mark_ready` |
| `start_session` | update | `FixtureApiBackend::start_session` |
| `get_session` | query | `FixtureApiBackend::get_session` |
| `get_setup_progress` | query | `FixtureApiBackend::get_setup_progress` |
| `get_my_participant` | query | `FixtureApiBackend::get_my_participant` |
| `get_match_history` | query | `FixtureApiBackend::get_match_history` |
| `get_game_view` | query | `FixtureApiBackend::get_game_view` |
| `get_visible_map_chunks` | query | `FixtureApiBackend::get_visible_map_chunks` |
| `get_visible_objects` | query | `FixtureApiBackend::get_visible_objects` |
| `get_object_view` | query | `FixtureApiBackend::get_object_view` |
| `get_my_champions` | query | `FixtureApiBackend::get_my_champions` |
| `get_champion_view` | query | `FixtureApiBackend::get_champion_view` |
| `preview_champion_progression` | query | `FixtureApiBackend::preview_champion_progression` |
| `select_champion_level_up` | update | `FixtureApiBackend::select_champion_level_up` |
| `learn_champion_spell` | update | `FixtureApiBackend::learn_champion_spell` |
| `cast_adventure_spell` | update | `FixtureApiBackend::cast_adventure_spell` |
| `get_tavern_offers` | query | `FixtureApiBackend::get_tavern_offers` |
| `preview_hire_champion` | query | `FixtureApiBackend::preview_hire_champion` |
| `hire_tavern_champion` | update | `FixtureApiBackend::hire_tavern_champion` |
| `preview_market_trade` | query | `FixtureApiBackend::preview_market_trade` |
| `submit_market_trade` | update | `FixtureApiBackend::submit_market_trade` |
| `get_dwelling_pool` | query | `FixtureApiBackend::get_dwelling_pool` |
| `preview_dwelling_recruit` | query | `FixtureApiBackend::preview_dwelling_recruit` |
| `submit_dwelling_recruit` | update | `FixtureApiBackend::submit_dwelling_recruit` |
| `get_objective_progress` | query | `FixtureApiBackend::get_objective_progress` |
| `get_scenario_rules` | query | `FixtureApiBackend::get_scenario_rules` |
| `get_world_events` | query | `FixtureApiBackend::get_world_events` |
| `preview_quest` | query | `FixtureApiBackend::preview_quest` |
| `accept_quest` | update | `FixtureApiBackend::accept_quest` |
| `claim_quest_reward` | update | `FixtureApiBackend::claim_quest_reward` |
| `sync_objectives` | update | `FixtureApiBackend::sync_objectives` |
| `sync_world_events` | update | `FixtureApiBackend::sync_world_events` |
| `sync_advanced_victory` | update | `FixtureApiBackend::sync_advanced_victory` |
| `get_skirmish_settings` | query | `FixtureApiBackend::get_skirmish_settings` |
| `get_procedural_map_state` | query | `FixtureApiBackend::get_procedural_map_state` |
| `get_naval_routes` | query | `FixtureApiBackend::get_naval_routes` |
| `get_siege_rules` | query | `FixtureApiBackend::get_siege_rules` |
| `sync_world_generation` | update | `FixtureApiBackend::sync_world_generation` |
| `get_town_view` | query | `FixtureApiBackend::get_town_view` |
| `get_battle_state` | query | `FixtureApiBackend::get_battle_state` |
| `get_content_manifest` | query | `FixtureApiBackend::get_content_manifest` |
| `get_events_after` | query | `FixtureApiBackend::get_events_after` |
| `get_command_status` | query | `FixtureApiBackend::get_command_status` |
| `get_command_status_by_nonce` | query | `FixtureApiBackend::get_command_status_by_nonce` |
| `preview_move_path` | query | `FixtureApiBackend::preview_move` |
| `preview_build_town_structure` | query | `FixtureApiBackend::preview_build_town_structure` |
| `preview_recruit_units` | query | `FixtureApiBackend::preview_recruit_units` |
| `submit_move_intent` | update | `FixtureApiBackend::submit_move_intent` |
| `end_turn` | update | `FixtureApiBackend::end_turn` |
| `sync_session_turn` | update | `FixtureApiBackend::sync_session_turn` |
| `submit_build_town_structure` | update | `FixtureApiBackend::submit_build_town_structure` |
| `submit_recruit_units` | update | `FixtureApiBackend::submit_recruit_units` |
| `sync_battle` | update | `FixtureApiBackend::sync_battle` |
| `end_battle_turn` | update | `FixtureApiBackend::end_battle_turn` |
| `submit_battle_action` | update | `FixtureApiBackend::submit_battle_action` |

The canister also exposes `get_canister_endpoint_inventory` for contract tests
and diagnostics.

Endpoint methods are implemented under `canisters/degens/src/api/` by domain,
with matching service boundaries under `services/` and durable row ownership
boundaries under `repos/`. See `docs/canister-layout.md`.

`get_game_view` is intentionally a lightweight session shell on the canister.
It returns session, participant, render-time, content-hash, and opening event
metadata while leaving map chunks, objects, towns, champion roster/detail, and
battle detail to the dedicated endpoints. Combining those pages in one query no
longer fits the Pocket-IC single-query instruction budget after the durable
schema expansion.

`get_my_champions` is intentionally a bounded roster/list query. It returns
owned champion render metadata from the participant's persisted IcyDB
`champion_ids` roster without expanding army stacks or equipped artifacts.
Clients that need stack/artifact detail must call `get_champion_view` for the
specific champion.

## Time Contract

Movement and battle endpoint decisions derive time at the canister boundary.
Public Candid callers do not provide `now_ms` to `preview_move_path`,
`submit_move_intent`, `sync_session_turn`, `get_battle_state`, `sync_battle`,
or `submit_battle_action`. Pocket-IC tests advance Pocket-IC time when they
need turn deadlines or battle action deadlines to elapse. Server time is also
excluded from command idempotency payloads so exact nonce retries replay the
original command instead of failing due to a later clock value.

Checkpoint 22 battle spellcasting uses `submit_battle_action` with
`action = CastAbility` and `ability_key = spell:<slug>`, so it remains under the
same canister-time and command-idempotency contract.
Battle `action_deadline_at` is the timeout target. The current canister accepts
a valid action from the active stack until `action_deadline_at + 15_000ms` if it
wins the race against timeout processing.

Checkpoint 23 expanded-economy updates use standard `GameCommand`
idempotency. Exact retries for tavern hiring, market trading, and external
dwelling recruitment replay the original `CommandResponse`; previews and pool
reads are query-only projections.
Marketplace trade rates are fixed in v1.1. Town/building income effects such
as `town_income_gold_250` may exist in content metadata, but the current
canister income path does not materialize them; active recurring income is
captured mine income, with pickups/rewards/costs handled through the resource
ledger.

Checkpoint 24 scenario-progress updates use the same `GameCommand`
idempotency. Quest accept/claim, objective sync, world-event sync, and
advanced-victory sync exact retries replay the original `CommandResponse`;
objective/rule/event/quest reads are query-only projections over IcyDB rows.

Checkpoint 25 world-generation updates use the same `GameCommand`
idempotency. `sync_world_generation` refreshes the deterministic procedural
preview metadata without appending public gameplay events, and exact retries
replay the original `CommandResponse`; skirmish settings, procedural map state,
naval routes, and siege rules are query-only projections over IcyDB rows. Naval
movement, siege actions, and larger-map gameplay remain disabled by persisted
rows with explicit disabled reasons and `actionable = false` until a later
bounded spec expands them.

Checkpoint 26 implements accepted-closure rejection for late
`submit_move_intent`: the route is still sync-driven, render/query DTOs expose
`sync_required`, and clients call `sync_session_turn` to materialize expired
turn state, but once a current-turn closure job is accepted, running, or due,
new turn-sensitive commands fail before command creation with
`backend_work_pending`/stale-expired semantics. Exact retries of already-created
commands still replay. A stricter raw wall-clock error for a new movement intent
submitted after `turn_deadline_at` but before closure acceptance remains optional
V2 cleanup.

## Deferred Endpoint Decisions

These names are intentionally absent from the v1 exported Candid service and
from the required gameplay endpoint inventory. `retreat` and `surrender` are
still exposed to clients as disabled battle affordances with typed disabled
reasons; the remaining entries stay deferred until their command and replay
contracts are promoted.

| Endpoint | Decision |
| --- | --- |
| `leave_session` | Deferred until lobby cancellation/leave semantics are promoted into the canister API. |
| `cancel_session` | Deferred until lobby cancellation/leave semantics are promoted into the canister API. |
| `surrender` | Explicitly disabled in v1 until Part 2 expands command, event, and victory semantics. |
| `retreat` | Explicitly disabled in v1 until Part 2 expands battle aftermath semantics. |
| `request_rematch` | Client affordance only for v1; durable rematch creation remains deferred to multiplayer meta expansion. |
