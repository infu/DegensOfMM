# Combined Benchmark Function Instruction Averages

Historical benchmark snapshot: superseded by `perf1.measure.md` and
`target/benchmarks/20260522-164948-5cfd001`. Keep this file as the
20260521 all-timers aggregate, not as the current performance report.

Source: all structured summaries from `target/benchmarks/20260521-160211-all-timers`.

- Included benchmark summaries: `endpoint_surface`, `timer_surface`, `gate_j_strategic_loop`, `gate_k_battle_aftermath_victory_history`, `gate_l_first_playable`, `gate_m_web_client_probe`
- Git SHA recorded in source artifacts: `a694492` (the benchmark was run before committing these benchmark harness changes)
- Suite required endpoint coverage: 59/59
- Public method samples combined: 841 calls
- Timer/system-job samples combined: 299 calls
- Total measured method samples combined: 1140 calls
- Public methods covered: 59
- Timer jobs covered: 8
- Aggregation: call-weighted average instruction delta per function and kind
- Unit: billions of instructions (`B`)
- `Measured Calls` excludes calls with missing instruction data; this run measured all 1140 public/timer method calls.
- Rows with `Errors > 0` are included, but their averages should not be treated as clean success-path measurements.

| Function | Kind | Calls | Measured Calls | Avg Instructions B | Avg Instructions | Errors | Sources |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `system_job:turn_resolution` | timer | 1 | 1 | 18.416 | 18415974211 | 0 | timer_surface |
| `runtime_timer:setup_session` | timer | 153 | 153 | 14.5155 | 14515472990 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `system_job:turn_deadline` | timer | 33 | 33 | 13.1504 | 13150432836 | 0 | gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `sync_battle` | update | 53 | 53 | 8.6881 | 8688064307 | 1 | endpoint_surface, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `system_job:battle_round_advance` | timer | 21 | 21 | 3.943 | 3943038237 | 0 | gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `system_job:world_events` | timer | 27 | 27 | 3.8134 | 3813379672 | 0 | gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `system_job:scenario_objectives` | timer | 30 | 30 | 3.5171 | 3517146715 | 0 | gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `end_battle_turn` | update | 3 | 3 | 2.7636 | 2763614205 | 1 | endpoint_surface, timer_surface |
| `system_job:battle_timeout` | timer | 7 | 7 | 2.5367 | 2536675290 | 0 | gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `system_job:advanced_victory` | timer | 27 | 27 | 1.8078 | 1807823817 | 0 | gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `sync_session_turn` | update | 106 | 106 | 1.6226 | 1622643447 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `get_champion_view` | query | 10 | 10 | 1.3412 | 1341229870 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `end_turn` | update | 3 | 3 | 0.8627 | 862693955 | 0 | endpoint_surface, timer_surface |
| `submit_dwelling_recruit` | update | 1 | 1 | 0.7093 | 709314251 | 0 | endpoint_surface |
| `accept_quest` | update | 1 | 1 | 0.7082 | 708222769 | 0 | endpoint_surface |
| `preview_quest` | query | 1 | 1 | 0.7069 | 706945791 | 0 | endpoint_surface |
| `learn_champion_spell` | update | 1 | 1 | 0.7049 | 704916491 | 0 | endpoint_surface |
| `get_setup_progress` | query | 8 | 8 | 0.616 | 616049777 | 0 | endpoint_surface, gate_m_web_client_probe |
| `start_session` | update | 9 | 9 | 0.4887 | 488701436 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `create_session` | update | 9 | 9 | 0.4806 | 480606198 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `register_player` | update | 18 | 18 | 0.4746 | 474608022 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `get_town_view` | query | 20 | 20 | 0.3194 | 319358879 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `submit_battle_action` | update | 68 | 68 | 0.2371 | 237106898 | 1 | endpoint_surface, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_match_history` | query | 20 | 20 | 0.1758 | 175828381 | 0 | endpoint_surface, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_events_after` | query | 49 | 49 | 0.1723 | 172253622 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe |
| `get_visible_objects` | query | 19 | 19 | 0.1218 | 121839638 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `get_visible_map_chunks` | query | 18 | 18 | 0.1173 | 117264083 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `get_battle_state` | query | 127 | 127 | 0.0948 | 94799357 | 1 | endpoint_surface, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `get_my_champions` | query | 30 | 30 | 0.0703 | 70338048 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `get_command_status` | query | 17 | 17 | 0.0416 | 41596988 | 0 | endpoint_surface, gate_l_first_playable, gate_m_web_client_probe |
| `get_game_view` | query | 2 | 2 | 0.0047 | 4691708 | 0 | endpoint_surface, gate_l_first_playable |
| `preview_build_town_structure` | query | 2 | 2 | 0.004 | 3986905 | 0 | endpoint_surface, gate_m_web_client_probe |
| `get_content_manifest` | query | 2 | 2 | 0.0024 | 2369239 | 0 | endpoint_surface, gate_m_web_client_probe |
| `submit_build_town_structure` | update | 4 | 4 | 0.0018 | 1838744 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `submit_recruit_units` | update | 4 | 4 | 0.0018 | 1835101 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `submit_move_intent` | update | 28 | 28 | 0.0018 | 1791525 | 1 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `hire_tavern_champion` | update | 1 | 1 | 0.0018 | 1770804 | 0 | endpoint_surface |
| `sync_objectives` | update | 1 | 1 | 0.0016 | 1640487 | 0 | endpoint_surface |
| `join_session` | update | 9 | 9 | 0.0016 | 1635536 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `preview_recruit_units` | query | 2 | 2 | 0.0016 | 1550203 | 0 | endpoint_surface, gate_m_web_client_probe |
| `get_tavern_offers` | query | 1 | 1 | 0.0015 | 1544205 | 0 | endpoint_surface |
| `preview_hire_champion` | query | 1 | 1 | 0.0015 | 1542727 | 0 | endpoint_surface |
| `preview_dwelling_recruit` | query | 1 | 1 | 0.0015 | 1535930 | 0 | endpoint_surface |
| `get_dwelling_pool` | query | 1 | 1 | 0.0015 | 1513245 | 0 | endpoint_surface |
| `claim_quest_reward` | update | 1 | 1 | 0.0003 | 261900 | 0 | endpoint_surface |
| `sync_world_events` | update | 1 | 1 | 0.0002 | 202166 | 0 | endpoint_surface |
| `cast_adventure_spell` | update | 1 | 1 | 0.0002 | 200309 | 0 | endpoint_surface |
| `submit_market_trade` | update | 1 | 1 | 0.0002 | 197954 | 0 | endpoint_surface |
| `sync_advanced_victory` | update | 1 | 1 | 0.0002 | 194750 | 0 | endpoint_surface |
| `select_champion_level_up` | update | 1 | 1 | 0.0002 | 187514 | 0 | endpoint_surface |
| `sync_world_generation` | update | 1 | 1 | 0.0001 | 145730 | 0 | endpoint_surface |
| `mark_ready` | update | 18 | 18 | 0.0001 | 142593 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `get_object_view` | query | 1 | 1 | 0.0001 | 63799 | 0 | endpoint_surface |
| `get_scenario_rules` | query | 1 | 1 | 0.0001 | 58391 | 0 | endpoint_surface |
| `preview_move_path` | query | 1 | 1 | 0.0001 | 56723 | 1 | endpoint_surface |
| `get_command_status_by_nonce` | query | 1 | 1 | 0.0001 | 50345 | 0 | endpoint_surface |
| `preview_champion_progression` | query | 1 | 1 | 0 | 39794 | 0 | endpoint_surface |
| `get_procedural_map_state` | query | 1 | 1 | 0 | 36101 | 0 | endpoint_surface |
| `get_siege_rules` | query | 1 | 1 | 0 | 35701 | 0 | endpoint_surface |
| `get_naval_routes` | query | 1 | 1 | 0 | 35697 | 0 | endpoint_surface |
| `get_skirmish_settings` | query | 1 | 1 | 0 | 34598 | 0 | endpoint_surface |
| `get_objective_progress` | query | 1 | 1 | 0 | 31641 | 0 | endpoint_surface |
| `get_my_participant` | query | 22 | 22 | 0 | 29094 | 0 | endpoint_surface, gate_j_strategic_loop, gate_l_first_playable, gate_m_web_client_probe |
| `get_world_events` | query | 1 | 1 | 0 | 25818 | 0 | endpoint_surface |
| `preview_market_trade` | query | 1 | 1 | 0 | 23969 | 0 | endpoint_surface |
| `get_session` | query | 131 | 131 | 0 | 6464 | 0 | endpoint_surface, gate_j_strategic_loop, gate_k_battle_aftermath_victory_history, gate_l_first_playable, gate_m_web_client_probe, timer_surface |
| `get_my_player` | query | 1 | 1 | 0 | 5746 | 0 | endpoint_surface |
