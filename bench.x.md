# Combined Benchmark Function Instruction Averages

Source: latest structured benchmark summaries from `target/benchmarks/20260521-122351-resolved-battle-visibility`.

- Included benchmark summaries: `endpoint_surface`, `gate_j_strategic_loop`
- Git SHA in source artifacts: `36ccc5d`
- Required endpoint coverage from endpoint surface: 59/59
- Public method samples combined: 122 calls
- Aggregation: call-weighted average instruction delta per public method
- Unit: billions of instructions (`B`)
- Rows with `Errors > 0` are included, but their averages should not be treated as clean performance measurements.

| Function | Kind | Calls | Avg Instructions B | Errors | Sources |
| --- | --- | ---: | ---: | ---: | --- |
| `accept_quest` | update | 1 | 0.707 | 0 | endpoint_surface |
| `preview_quest` | query | 1 | 0.7066 | 0 | endpoint_surface |
| `submit_dwelling_recruit` | update | 1 | 0.7063 | 0 | endpoint_surface |
| `learn_champion_spell` | update | 1 | 0.7055 | 0 | endpoint_surface |
| `start_session` | update | 2 | 0.4875 | 0 | endpoint_surface, gate_j_strategic_loop |
| `create_session` | update | 2 | 0.4805 | 0 | endpoint_surface, gate_j_strategic_loop |
| `register_player` | update | 4 | 0.4747 | 0 | endpoint_surface, gate_j_strategic_loop |
| `get_visible_objects` | query | 2 | 0.0114 | 0 | endpoint_surface, gate_j_strategic_loop |
| `get_game_view` | query | 1 | 0.0047 | 0 | endpoint_surface |
| `get_champion_view` | query | 3 | 0.0046 | 0 | endpoint_surface, gate_j_strategic_loop |
| `preview_build_town_structure` | query | 1 | 0.004 | 0 | endpoint_surface |
| `get_town_view` | query | 3 | 0.003 | 0 | endpoint_surface, gate_j_strategic_loop |
| `get_content_manifest` | query | 1 | 0.0024 | 0 | endpoint_surface |
| `submit_move_intent` | update | 4 | 0.0024 | 1 | endpoint_surface, gate_j_strategic_loop |
| `hire_tavern_champion` | update | 1 | 0.0018 | 0 | endpoint_surface |
| `submit_build_town_structure` | update | 2 | 0.0018 | 0 | endpoint_surface, gate_j_strategic_loop |
| `submit_recruit_units` | update | 2 | 0.0018 | 0 | endpoint_surface, gate_j_strategic_loop |
| `get_tavern_offers` | query | 1 | 0.0016 | 0 | endpoint_surface |
| `join_session` | update | 2 | 0.0016 | 0 | endpoint_surface, gate_j_strategic_loop |
| `sync_objectives` | update | 1 | 0.0016 | 0 | endpoint_surface |
| `get_dwelling_pool` | query | 1 | 0.0015 | 0 | endpoint_surface |
| `preview_dwelling_recruit` | query | 1 | 0.0015 | 0 | endpoint_surface |
| `preview_hire_champion` | query | 1 | 0.0015 | 0 | endpoint_surface |
| `preview_recruit_units` | query | 1 | 0.0015 | 0 | endpoint_surface |
| `claim_quest_reward` | update | 1 | 0.0003 | 0 | endpoint_surface |
| `cast_adventure_spell` | update | 1 | 0.0002 | 0 | endpoint_surface |
| `select_champion_level_up` | update | 1 | 0.0002 | 0 | endpoint_surface |
| `submit_market_trade` | update | 1 | 0.0002 | 0 | endpoint_surface |
| `sync_advanced_victory` | update | 1 | 0.0002 | 0 | endpoint_surface |
| `sync_session_turn` | update | 10 | 0.0002 | 0 | endpoint_surface, gate_j_strategic_loop |
| `sync_world_events` | update | 1 | 0.0002 | 0 | endpoint_surface |
| `end_turn` | update | 1 | 0.0001 | 0 | endpoint_surface |
| `get_command_status` | query | 1 | 0.0001 | 0 | endpoint_surface |
| `get_command_status_by_nonce` | query | 1 | 0.0001 | 0 | endpoint_surface |
| `get_events_after` | query | 5 | 0.0001 | 0 | endpoint_surface, gate_j_strategic_loop |
| `get_object_view` | query | 1 | 0.0001 | 0 | endpoint_surface |
| `get_scenario_rules` | query | 1 | 0.0001 | 0 | endpoint_surface |
| `get_visible_map_chunks` | query | 2 | 0.0001 | 0 | endpoint_surface, gate_j_strategic_loop |
| `mark_ready` | update | 4 | 0.0001 | 0 | endpoint_surface, gate_j_strategic_loop |
| `preview_move_path` | query | 1 | 0.0001 | 1 | endpoint_surface |
| `sync_world_generation` | update | 1 | 0.0001 | 0 | endpoint_surface |
| `end_battle_turn` | update | 1 | 0 | 1 | endpoint_surface |
| `get_battle_state` | query | 1 | 0 | 1 | endpoint_surface |
| `get_match_history` | query | 1 | 0 | 0 | endpoint_surface |
| `get_my_champions` | query | 3 | 0 | 0 | endpoint_surface, gate_j_strategic_loop |
| `get_my_participant` | query | 3 | 0 | 0 | endpoint_surface, gate_j_strategic_loop |
| `get_my_player` | query | 1 | 0 | 0 | endpoint_surface |
| `get_naval_routes` | query | 1 | 0 | 0 | endpoint_surface |
| `get_objective_progress` | query | 1 | 0 | 0 | endpoint_surface |
| `get_procedural_map_state` | query | 1 | 0 | 0 | endpoint_surface |
| `get_session` | query | 27 | 0 | 0 | endpoint_surface, gate_j_strategic_loop |
| `get_setup_progress` | query | 1 | 0 | 0 | endpoint_surface |
| `get_siege_rules` | query | 1 | 0 | 0 | endpoint_surface |
| `get_skirmish_settings` | query | 1 | 0 | 0 | endpoint_surface |
| `get_world_events` | query | 1 | 0 | 0 | endpoint_surface |
| `preview_champion_progression` | query | 1 | 0 | 0 | endpoint_surface |
| `preview_market_trade` | query | 1 | 0 | 0 | endpoint_surface |
| `submit_battle_action` | update | 1 | 0 | 1 | endpoint_surface |
| `sync_battle` | update | 1 | 0 | 1 | endpoint_surface |
