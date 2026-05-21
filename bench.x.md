# Combined Benchmark Function Instruction Averages

Source: all structured summaries from `target/benchmarks/20260521-150957-timer-inclusive`.

- Included benchmark summaries: `endpoint_surface`, `gate_j_strategic_loop`, `gate_k_battle_aftermath_victory_history`, `gate_l_first_playable`, `gate_m_web_client_probe`
- Git SHA in source artifacts: `0a0996d`
- Suite required endpoint coverage: 59/59
- Public method samples combined: 729 calls
- Timer/system-job samples combined: 35 calls
- Total measured method samples combined: 764 calls
- Public methods covered: 59
- Timer jobs covered: 2
- Aggregation: call-weighted average instruction delta per function and kind
- Unit: billions of instructions (`B`)
- `Measured Calls` excludes calls with missing instruction data; this run measured all 764 public/timer method calls.
- Rows with `Errors > 0` are included, but their averages should not be treated as clean success-path measurements.

| Function | Kind | Calls | Measured Calls | Avg Instructions B | Avg Instructions | Errors | Sources |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `system_job:turn_deadline` | timer | 21 | 21 | 10.579 | 10578999310 | 0 | gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `sync_battle` | update | 54 | 54 | 8.1983 | 8198310446 | 1 | endpoint_surface, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `system_job:battle_round_advance` | timer | 14 | 14 | 3.6758 | 3675844590 | 0 | gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `sync_session_turn` | update | 89 | 89 | 1.4202 | 1420215124 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_champion_view` | query | 10 | 10 | 1.3446 | 1344648911 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `accept_quest` | update | 1 | 1 | 0.7073 | 707312870 | 0 | endpoint_surface |
| `submit_dwelling_recruit` | update | 1 | 1 | 0.7064 | 706443082 | 0 | endpoint_surface |
| `preview_quest` | query | 1 | 1 | 0.7059 | 705868167 | 0 | endpoint_surface |
| `learn_champion_spell` | update | 1 | 1 | 0.7054 | 705420962 | 0 | endpoint_surface |
| `get_setup_progress` | query | 8 | 8 | 0.6176 | 617647401 | 0 | endpoint_surface, gate_m_web_client_probe |
| `start_session` | update | 5 | 5 | 0.488 | 487999322 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `create_session` | update | 5 | 5 | 0.4802 | 480203409 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `register_player` | update | 10 | 10 | 0.4748 | 474786584 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_town_view` | query | 20 | 20 | 0.3201 | 320104579 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `submit_battle_action` | update | 67 | 67 | 0.3039 | 303853215 | 1 | endpoint_surface, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_match_history` | query | 20 | 20 | 0.1764 | 176394542 | 0 | endpoint_surface, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_events_after` | query | 49 | 49 | 0.1726 | 172623459 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_visible_objects` | query | 19 | 19 | 0.1221 | 122118735 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `get_visible_map_chunks` | query | 18 | 18 | 0.1175 | 117542833 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `get_battle_state` | query | 124 | 124 | 0.0977 | 97717720 | 1 | endpoint_surface, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_my_champions` | query | 27 | 27 | 0.0784 | 78374229 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_command_status` | query | 17 | 17 | 0.0418 | 41758442 | 0 | endpoint_surface, gate_l_first_playable, gate_m_web_client_probe |
| `get_game_view` | query | 2 | 2 | 0.0047 | 4689737 | 0 | endpoint_surface, gate_l_first_playable |
| `preview_build_town_structure` | query | 2 | 2 | 0.004 | 3983566 | 0 | endpoint_surface, gate_m_web_client_probe |
| `get_content_manifest` | query | 2 | 2 | 0.0024 | 2368389 | 0 | endpoint_surface, gate_m_web_client_probe |
| `submit_recruit_units` | update | 4 | 4 | 0.0018 | 1832265 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `submit_build_town_structure` | update | 4 | 4 | 0.0018 | 1824636 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `hire_tavern_champion` | update | 1 | 1 | 0.0018 | 1771058 | 0 | endpoint_surface |
| `submit_move_intent` | update | 23 | 23 | 0.0017 | 1746584 | 1 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `sync_objectives` | update | 1 | 1 | 0.0016 | 1643757 | 0 | endpoint_surface |
| `join_session` | update | 5 | 5 | 0.0016 | 1627463 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `preview_recruit_units` | query | 2 | 2 | 0.0015 | 1549534 | 0 | endpoint_surface, gate_m_web_client_probe |
| `get_tavern_offers` | query | 1 | 1 | 0.0015 | 1541328 | 0 | endpoint_surface |
| `preview_hire_champion` | query | 1 | 1 | 0.0015 | 1540320 | 0 | endpoint_surface |
| `preview_dwelling_recruit` | query | 1 | 1 | 0.0015 | 1537614 | 0 | endpoint_surface |
| `get_dwelling_pool` | query | 1 | 1 | 0.0015 | 1511369 | 0 | endpoint_surface |
| `claim_quest_reward` | update | 1 | 1 | 0.0003 | 264218 | 0 | endpoint_surface |
| `sync_world_events` | update | 1 | 1 | 0.0002 | 202657 | 0 | endpoint_surface |
| `cast_adventure_spell` | update | 1 | 1 | 0.0002 | 200484 | 0 | endpoint_surface |
| `submit_market_trade` | update | 1 | 1 | 0.0002 | 197812 | 0 | endpoint_surface |
| `sync_advanced_victory` | update | 1 | 1 | 0.0002 | 194653 | 0 | endpoint_surface |
| `select_champion_level_up` | update | 1 | 1 | 0.0002 | 188918 | 0 | endpoint_surface |
| `end_turn` | update | 1 | 1 | 0.0001 | 148079 | 0 | endpoint_surface |
| `sync_world_generation` | update | 1 | 1 | 0.0001 | 146908 | 0 | endpoint_surface |
| `mark_ready` | update | 10 | 10 | 0.0001 | 135059 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_object_view` | query | 1 | 1 | 0.0001 | 64356 | 0 | endpoint_surface |
| `get_scenario_rules` | query | 1 | 1 | 0.0001 | 59020 | 0 | endpoint_surface |
| `preview_move_path` | query | 1 | 1 | 0.0001 | 58849 | 1 | endpoint_surface |
| `get_command_status_by_nonce` | query | 1 | 1 | 0 | 49832 | 0 | endpoint_surface |
| `preview_champion_progression` | query | 1 | 1 | 0 | 40459 | 0 | endpoint_surface |
| `get_procedural_map_state` | query | 1 | 1 | 0 | 36158 | 0 | endpoint_surface |
| `get_naval_routes` | query | 1 | 1 | 0 | 35797 | 0 | endpoint_surface |
| `get_siege_rules` | query | 1 | 1 | 0 | 35664 | 0 | endpoint_surface |
| `get_skirmish_settings` | query | 1 | 1 | 0 | 34603 | 0 | endpoint_surface |
| `get_objective_progress` | query | 1 | 1 | 0 | 32066 | 0 | endpoint_surface |
| `get_my_participant` | query | 22 | 22 | 0 | 29060 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `get_world_events` | query | 1 | 1 | 0 | 26286 | 0 | endpoint_surface |
| `preview_market_trade` | query | 1 | 1 | 0 | 24267 | 0 | endpoint_surface |
| `end_battle_turn` | update | 1 | 1 | 0 | 24103 | 1 | endpoint_surface |
| `get_session` | query | 79 | 79 | 0 | 7634 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_my_player` | query | 1 | 1 | 0 | 5769 | 0 | endpoint_surface |
