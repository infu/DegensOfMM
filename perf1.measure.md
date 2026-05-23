# Perf1 Measurement Report

Measured on 2026-05-22 with:

```bash
DOMM_BENCH_JOBS=5 scripts/run-benchmarks.sh
```

- Git: `5cfd001`
- Output: `/srv/shared/icydb/DoMM/target/benchmarks/20260522-164948-5cfd001`
- Suite result: passed
- Suite required endpoint coverage: `59/59`
- Parallel gate jobs: `5`

The suite completed successfully. All benchmark gates passed and the hard-target audit reported zero violations.

## Suite Performance

| Suite | Status | Elapsed | Calls | Scenarios | Required endpoints | Row growth | Stable pages | Instructions B | Cycles T | Memory MB |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: |
| endpoint-surface | passed | 3m50s | 245 | 1 | 59/59 | 224 | 2049 -> 118273 | 601.1412 | 0.2943 | 7236.5520 |
| timer-surface | passed | 5m05s | 307 | 1 | 13/59 | 436 | 2049 -> 234497 | 1152.7489 | 0.3962 | 14500.5520 |
| gate-j | passed | 2m59s | 106 | 1 | 17/59 | 35 | 2049 -> 62465 | 278.8742 | 0.0827 | 3684.5520 |
| gate-k | passed | 3m31s | 221 | 2 | 15/59 | 89 | 2049 -> 79617 | 455.9467 | 0.3863 | 4820.5520 |
| gate-l | passed | 3m36s | 273 | 5 | 23/59 | 148 | 2049 -> 80769 | 475.4274 | 0.3475 | 4892.5520 |
| gate-m | passed | 1m55s | 467 | 1 | 26/59 | 103 | 2049 -> 94081 | 541.7708 | 0.6575 | 5724.5520 |
| projection-surface | passed | 0m08s | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |

## Hard Target Audit

- Target band: `0.3B-0.6B` average instruction delta for gameplay kernel commands.
- Method observations audited: `45`
- Passing or below band: `34`
- Named durable boundaries: `11`
- Violations: `0`

| Gate | Method | Calls | Avg instructions B | Status | Boundary reason |
| --- | --- | ---: | ---: | --- | --- |
| endpoint-surface | accept_quest | 1 | 0.7092 | named-boundary | quest acceptance still crosses durable quest/objective boundary |
| endpoint-surface | cast_adventure_spell | 1 | 0.0002 | pass | n/a |
| endpoint-surface | claim_quest_reward | 1 | 0.0003 | pass | n/a |
| endpoint-surface | end_battle_turn | 1 | 0.4806 | pass | n/a |
| endpoint-surface | end_turn | 1 | 0.0002 | pass | n/a |
| endpoint-surface | hire_tavern_champion | 1 | 0.0018 | pass | n/a |
| endpoint-surface | learn_champion_spell | 1 | 0.0002 | pass | n/a |
| endpoint-surface | select_champion_level_up | 1 | 0.0002 | pass | n/a |
| endpoint-surface | submit_battle_action | 5 | 0.2886 | pass | n/a |
| endpoint-surface | submit_build_town_structure | 1 | 0.0018 | pass | n/a |
| endpoint-surface | submit_dwelling_recruit | 1 | 0.7073 | named-boundary | dwelling recruit still crosses durable recruit-pool/garrison boundary |
| endpoint-surface | submit_market_trade | 1 | 0.0002 | pass | n/a |
| endpoint-surface | submit_move_intent | 2 | 0.0046 | pass | n/a |
| endpoint-surface | submit_recruit_units | 1 | 0.0018 | pass | n/a |
| endpoint-surface | sync_advanced_victory | 1 | 0.0002 | pass | n/a |
| endpoint-surface | sync_battle | 8 | 5.7532 | named-boundary | endpoint-surface sync_battle samples battle timeout/recovery/aftermath durable handoff boundary |
| endpoint-surface | sync_objectives | 1 | 0.0016 | pass | n/a |
| endpoint-surface | sync_session_turn | 9 | 0.2627 | pass | n/a |
| endpoint-surface | sync_world_events | 1 | 0.0002 | pass | n/a |
| endpoint-surface | sync_world_generation | 1 | 0.0001 | pass | n/a |
| timer-surface | end_battle_turn | 2 | 0.2401 | pass | n/a |
| timer-surface | end_turn | 2 | 0.9426 | named-boundary | timer-surface end_turn samples the durable turn-close boundary |
| timer-surface | submit_move_intent | 5 | 0.0020 | pass | n/a |
| timer-surface | sync_battle | 1 | 0.0000 | pass | n/a |
| timer-surface | sync_session_turn | 14 | 1.2022 | named-boundary | timer-surface sync_session_turn includes turn timer and projection boundary work |
| gate-j | submit_build_town_structure | 1 | 0.0018 | pass | n/a |
| gate-j | submit_move_intent | 3 | 0.0031 | pass | n/a |
| gate-j | submit_recruit_units | 1 | 0.0018 | pass | n/a |
| gate-j | sync_session_turn | 10 | 0.3070 | pass | n/a |
| gate-k | submit_battle_action | 21 | 0.1702 | pass | n/a |
| gate-k | submit_move_intent | 5 | 0.0020 | pass | n/a |
| gate-k | sync_battle | 26 | 4.7840 | named-boundary | scenario sync_battle crosses timeout/aftermath durable handoff boundary |
| gate-k | sync_session_turn | 19 | 1.6509 | named-boundary | scenario sync crosses battle/scenario durable handoff boundary |
| gate-l | submit_battle_action | 23 | 0.1764 | pass | n/a |
| gate-l | submit_build_town_structure | 1 | 0.0018 | pass | n/a |
| gate-l | submit_move_intent | 7 | 0.0015 | pass | n/a |
| gate-l | submit_recruit_units | 1 | 0.0018 | pass | n/a |
| gate-l | sync_battle | 28 | 5.0101 | named-boundary | scenario sync_battle crosses timeout/aftermath durable handoff boundary |
| gate-l | sync_session_turn | 26 | 1.3286 | named-boundary | scenario sync crosses battle/scenario durable handoff boundary |
| gate-m | submit_battle_action | 23 | 0.1554 | pass | n/a |
| gate-m | submit_build_town_structure | 1 | 0.0019 | pass | n/a |
| gate-m | submit_move_intent | 7 | 0.0015 | pass | n/a |
| gate-m | submit_recruit_units | 1 | 0.0020 | pass | n/a |
| gate-m | sync_battle | 7 | 16.0897 | named-boundary | scenario sync_battle crosses timeout/aftermath durable handoff boundary |
| gate-m | sync_session_turn | 33 | 0.9472 | named-boundary | scenario sync crosses battle/scenario durable handoff boundary |

## Fixed Items

| Area | Previous result | Current result |
| --- | --- | --- |
| gate-m | Failed with duplicate web-client event sequence `1`. | Passed with 467 calls and 103 row growth. |
| endpoint-surface.submit_build_town_structure | `2.1114B`, hard-target violation. | `0.0018B`, pass. |
| endpoint-surface.sync_battle | `5.7511B`, hard-target violation due missing boundary reason. | `5.7532B`, named durable boundary. |

## Source Artifacts

| Artifact | Path |
| --- | --- |
| Suite summary | `target/benchmarks/20260522-164948-5cfd001/suite-summary.md` |
| Hard-target audit | `target/benchmarks/20260522-164948-5cfd001/hard-targets.md` |
| Gate M summary | `target/benchmarks/20260522-164948-5cfd001/gate-m/summary.md` |
