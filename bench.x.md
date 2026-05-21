# Combined Benchmark Function Instruction Averages

Source: all structured summaries from `target/benchmarks/20260521-143306-all-gate-summaries-postcommit`.

- Included benchmark summaries: `endpoint_surface`, `gate_j_strategic_loop`, `gate_k_battle_aftermath_victory_history`, `gate_l_first_playable`, `gate_m_web_client_probe`
- Git SHA in source artifacts: `b2ca1fa`
- Suite required endpoint coverage: 59/59
- Public method samples combined: 729 calls
- Public methods covered: 59
- Aggregation: call-weighted average instruction delta per public method
- Unit: billions of instructions (`B`)
- `Measured Calls` excludes calls with missing instruction data; this run measured all 729 public method calls.
- Rows with `Errors > 0` are included, but their averages should not be treated as clean success-path measurements.

| Function | Kind | Calls | Measured Calls | Avg Instructions B | Avg Instructions | Errors | Sources |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `sync_battle` | update | 54 | 54 | 8.1988 | 8198814477 | 1 | endpoint_surface, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `sync_session_turn` | update | 89 | 89 | 1.4208 | 1420830018 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_champion_view` | query | 10 | 10 | 1.3448 | 1344754144 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `submit_dwelling_recruit` | update | 1 | 1 | 0.7098 | 709790313 | 0 | endpoint_surface |
| `accept_quest` | update | 1 | 1 | 0.7075 | 707523142 | 0 | endpoint_surface |
| `preview_quest` | query | 1 | 1 | 0.707 | 706950809 | 0 | endpoint_surface |
| `learn_champion_spell` | update | 1 | 1 | 0.7057 | 705666714 | 0 | endpoint_surface |
| `get_setup_progress` | query | 8 | 8 | 0.6179 | 617896682 | 0 | endpoint_surface, gate_m_web_client_probe |
| `start_session` | update | 5 | 5 | 0.4881 | 488084431 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `create_session` | update | 5 | 5 | 0.4804 | 480442749 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `register_player` | update | 10 | 10 | 0.4747 | 474748093 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_town_view` | query | 20 | 20 | 0.3196 | 319644627 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `submit_battle_action` | update | 67 | 67 | 0.304 | 303997123 | 1 | endpoint_surface, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_match_history` | query | 20 | 20 | 0.1763 | 176312328 | 0 | endpoint_surface, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_events_after` | query | 49 | 49 | 0.1726 | 172616935 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_visible_objects` | query | 19 | 19 | 0.1222 | 122181884 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `get_visible_map_chunks` | query | 18 | 18 | 0.1176 | 117623928 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `get_battle_state` | query | 124 | 124 | 0.0977 | 97730589 | 1 | endpoint_surface, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_my_champions` | query | 27 | 27 | 0.0782 | 78240054 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_command_status` | query | 17 | 17 | 0.0417 | 41679223 | 0 | endpoint_surface, gate_l_first_playable, gate_m_web_client_probe |
| `get_game_view` | query | 2 | 2 | 0.0047 | 4661365 | 0 | endpoint_surface, gate_l_first_playable |
| `preview_build_town_structure` | query | 2 | 2 | 0.004 | 3983516 | 0 | endpoint_surface, gate_m_web_client_probe |
| `get_content_manifest` | query | 2 | 2 | 0.0024 | 2369225 | 0 | endpoint_surface, gate_m_web_client_probe |
| `submit_recruit_units` | update | 4 | 4 | 0.0018 | 1834911 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `submit_build_town_structure` | update | 4 | 4 | 0.0018 | 1827718 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `hire_tavern_champion` | update | 1 | 1 | 0.0018 | 1770941 | 0 | endpoint_surface |
| `submit_move_intent` | update | 23 | 23 | 0.0017 | 1747550 | 1 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `sync_objectives` | update | 1 | 1 | 0.0016 | 1644684 | 0 | endpoint_surface |
| `join_session` | update | 5 | 5 | 0.0016 | 1628781 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `preview_recruit_units` | query | 2 | 2 | 0.0016 | 1551380 | 0 | endpoint_surface, gate_m_web_client_probe |
| `get_tavern_offers` | query | 1 | 1 | 0.0015 | 1546255 | 0 | endpoint_surface |
| `preview_hire_champion` | query | 1 | 1 | 0.0015 | 1543074 | 0 | endpoint_surface |
| `preview_dwelling_recruit` | query | 1 | 1 | 0.0015 | 1537326 | 0 | endpoint_surface |
| `get_dwelling_pool` | query | 1 | 1 | 0.0015 | 1514978 | 0 | endpoint_surface |
| `claim_quest_reward` | update | 1 | 1 | 0.0003 | 265394 | 0 | endpoint_surface |
| `sync_world_events` | update | 1 | 1 | 0.0002 | 202734 | 0 | endpoint_surface |
| `cast_adventure_spell` | update | 1 | 1 | 0.0002 | 200053 | 0 | endpoint_surface |
| `submit_market_trade` | update | 1 | 1 | 0.0002 | 199997 | 0 | endpoint_surface |
| `sync_advanced_victory` | update | 1 | 1 | 0.0002 | 193782 | 0 | endpoint_surface |
| `select_champion_level_up` | update | 1 | 1 | 0.0002 | 188518 | 0 | endpoint_surface |
| `end_turn` | update | 1 | 1 | 0.0001 | 147774 | 0 | endpoint_surface |
| `sync_world_generation` | update | 1 | 1 | 0.0001 | 145443 | 0 | endpoint_surface |
| `mark_ready` | update | 10 | 10 | 0.0001 | 135709 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_object_view` | query | 1 | 1 | 0.0001 | 63430 | 0 | endpoint_surface |
| `get_scenario_rules` | query | 1 | 1 | 0.0001 | 58343 | 0 | endpoint_surface |
| `preview_move_path` | query | 1 | 1 | 0.0001 | 55795 | 1 | endpoint_surface |
| `get_command_status_by_nonce` | query | 1 | 1 | 0 | 49780 | 0 | endpoint_surface |
| `preview_champion_progression` | query | 1 | 1 | 0 | 40385 | 0 | endpoint_surface |
| `get_procedural_map_state` | query | 1 | 1 | 0 | 35939 | 0 | endpoint_surface |
| `get_naval_routes` | query | 1 | 1 | 0 | 35788 | 0 | endpoint_surface |
| `get_siege_rules` | query | 1 | 1 | 0 | 35562 | 0 | endpoint_surface |
| `get_skirmish_settings` | query | 1 | 1 | 0 | 34636 | 0 | endpoint_surface |
| `get_objective_progress` | query | 1 | 1 | 0 | 31677 | 0 | endpoint_surface |
| `get_my_participant` | query | 22 | 22 | 0 | 29191 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `get_world_events` | query | 1 | 1 | 0 | 25785 | 0 | endpoint_surface |
| `end_battle_turn` | update | 1 | 1 | 0 | 24437 | 1 | endpoint_surface |
| `preview_market_trade` | query | 1 | 1 | 0 | 24286 | 0 | endpoint_surface |
| `get_session` | query | 79 | 79 | 0 | 7658 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_my_player` | query | 1 | 1 | 0 | 5769 | 0 | endpoint_surface |
