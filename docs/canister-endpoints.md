# Canister Endpoint Inventory

Checkpoint 19A makes the canister API surface explicit before IcyDB repository
work starts. The production backend is `canisters/degens`; `domm-game` remains
the pure deterministic rules and DTO contract layer.

Gameplay endpoints must use typed Candid methods and typed IcyDB repository
paths. Generic SQL is allowed only for controller-gated diagnostics or test
fixture loading, not for normal gameplay.

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
| `get_my_participant` | query | `FixtureApiBackend::get_my_participant` |
| `get_match_history` | query | `FixtureApiBackend::get_match_history` |
| `get_game_view` | query | `FixtureApiBackend::get_game_view` |
| `get_visible_map_chunks` | query | `FixtureApiBackend::get_visible_map_chunks` |
| `get_visible_objects` | query | `FixtureApiBackend::get_visible_objects` |
| `get_my_champions` | query | `FixtureApiBackend::get_my_champions` |
| `get_champion_view` | query | `FixtureApiBackend::get_champion_view` |
| `get_town_view` | query | `FixtureApiBackend::get_town_view` |
| `get_battle_state` | query | `FixtureApiBackend::get_battle_state` |
| `get_content_manifest` | query | `FixtureApiBackend::get_content_manifest` |
| `get_events_after` | query | `FixtureApiBackend::get_events_after` |
| `get_command_status` | query | `FixtureApiBackend::get_command_status` |
| `preview_move_path` | query | `FixtureApiBackend::preview_move` |
| `preview_build_town_structure` | query | `FixtureApiBackend::preview_build_town_structure` |
| `preview_recruit_units` | query | `FixtureApiBackend::preview_recruit_units` |
| `submit_move_intent` | update | `FixtureApiBackend::submit_move_intent` |
| `sync_session_turn` | update | `FixtureApiBackend::sync_session_turn` |
| `submit_build_town_structure` | update | `FixtureApiBackend::submit_build_town_structure` |
| `submit_recruit_units` | update | `FixtureApiBackend::submit_recruit_units` |
| `sync_battle` | update | `FixtureApiBackend::sync_battle` |
| `submit_battle_action` | update | `FixtureApiBackend::submit_battle_action` |

The canister also exposes `get_canister_endpoint_inventory` for contract tests
and diagnostics.

Endpoint methods are implemented under `canisters/degens/src/api/` by domain,
with matching service boundaries under `services/` and durable row ownership
boundaries under `repos/`. See `docs/canister-layout.md`.

## Time Contract

Movement and battle endpoint decisions derive time at the canister boundary.
Public Candid callers do not provide `now_ms` to `preview_move_path`,
`submit_move_intent`, `sync_session_turn`, `get_battle_state`, `sync_battle`,
or `submit_battle_action`. Pocket-IC tests advance Pocket-IC time when they
need turn deadlines or battle action deadlines to elapse. Server time is also
excluded from command idempotency payloads so exact nonce retries replay the
original command instead of failing due to a later clock value.

## Deferred Endpoint Decisions

| Endpoint | Decision |
| --- | --- |
| `leave_session` | Deferred until lobby cancellation/leave semantics are promoted into the canister API. |
| `cancel_session` | Deferred until lobby cancellation/leave semantics are promoted into the canister API. |
| `surrender` | Explicitly disabled in v1 until Part 2 expands command, event, and victory semantics. |
| `retreat` | Explicitly disabled in v1 until Part 2 expands battle aftermath semantics. |
| `request_rematch` | Client affordance only for v1; durable rematch creation remains deferred to multiplayer meta expansion. |
