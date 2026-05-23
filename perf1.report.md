# perf1 Executive Report

Historical report status: superseded for current measurement by
`perf1.measure.md`. This file preserves the earlier executive summary from the
2026-05-19 perf1 phase; do not use its "Current State" or "Open Risks" sections
as the latest benchmark status.

Current benchmark status as of 2026-05-22: `DOMM_BENCH_JOBS=5
scripts/run-benchmarks.sh` passed in
`target/benchmarks/20260522-164948-5cfd001`, required endpoint coverage was
`59/59`, Gate M passed, and the hard-target audit reported zero violations.

Source: `perf1.notes.md`, `perf1.todo.md`, benchmark artifacts already on disk, and git history. No tests or benchmarks were run to create this report.

## Executive Summary

The last roughly 8 hours were productive. The audited commit range from `c4dd56b Move submit readiness checks into battle runtime` through `c26b47d Drop redundant turn resolution effects` contains 29 commits.

Yes, performance increased materially. The main perf1 goal was to reduce `submit_battle_action` from about `27B` instructions toward `0.3B`. The recorded full-suite result after the battle runtime work shows that goal was reached:

| metric | before | 2026-05-19 recorded | change |
| --- | ---: | ---: | ---: |
| `submit_battle_action` combined avg | 26.9860B | 0.2789B | -98.97%, about 96.8x faster |
| Gate K `submit_battle_action` avg | 27.4632B | 0.2846B | -98.96% |
| Gate L `submit_battle_action` avg | 26.5272B | 0.2734B | -98.97% |
| `submit_battle_action` avg memory delta | about 165 MB | effectively 0 MB | effectively eliminated |

In this historical 2026-05-19 report, the bottleneck moved away from battle
submits. The main remaining hot path became movement/session turn processing.
Later perf1 work continued past this state; see `perf1.measure.md` for the
current 2026-05-22 measurement.

| metric | movement baseline | 2026-05-19 recorded | change |
| --- | ---: | ---: | ---: |
| Gate J scenario instructions | 404.0368B | 357.9811B | -11.4% |
| `submit_move_intent` avg | 15.3630B | 13.7537B | -10.5% |
| `sync_session_turn` avg | 18.4665B | 14.7341B | -20.2% |
| Gate J scenario memory | 6462.50 MB | 6190.125 MB | -4.2% |

## Audit Checks

| claim | audited source | result |
| --- | --- | --- |
| 29 commits in the reported work range | `git rev-list --count c4dd56b^..c26b47d` | true |
| Battle submit latest combined average | `target/benchmarks/20260519-063353-02c93e3/gate-k/summary.json` and `gate-l/summary.json` | `0.278897495B` weighted avg |
| Battle submit speedup | same summaries plus pinned baseline `26.9860B` | `96.7596x`, `-98.9665%` |
| Endpoint coverage | `target/benchmarks/20260519-081911-fe84689/suite-summary.json` | `59/59`, no missing endpoints |
| Movement latest Gate J numbers | `target/benchmarks/20260519-113052-movement-turn-effect-gate-j/summary.json` | matches the table above |
| Historical partial work at report time | `git status --short` | `movement.rs` modified, `idea.md` untracked |

## What Changed

Battle runtime path:

| area | result |
| --- | --- |
| Tactical battle rows | Active non-spell battle actions now mutate heap `BattleRuntime` instead of repeatedly hydrating/diffing/persisting tactical child rows. |
| Battle events | Active battle action events moved out of per-action durable `GameEvent` writes and into runtime/archive visibility. |
| Battle commands | Active non-spell command receipts moved into runtime; replay/status checks consult runtime receipts before durable rows. |
| Battle header | Active submit no longer projects `Battle` header state every action; runtime is authoritative while active. |
| Timeout/readiness | Runtime readiness/deadline state is used for active battles; durable jobs are wakeup hints rather than per-action authority. |
| Auth cache | A narrow two-slot active submit auth cache removed most of the remaining stable-read tax. |

Benchmark and coverage work:

| area | result |
| --- | --- |
| Endpoint coverage | Added endpoint-surface benchmark gate; recorded `59/59` required public endpoints covered. |
| Benchmark reporting | Public-method instruction/memory/cycle summaries are now captured in readable units. |
| Query metrics | Official suite captured query averages through direct query log output. |
| Code-size management | Freed benchmark Wasm code-section headroom multiple times so perf work could continue under the IC limit. Current recorded headroom after `c26b47d`: `22,026` bytes. |

Movement/session-turn path:

| checkpoint | measured impact |
| --- | --- |
| Reuse movement path serialization and skip one participant reload | Gate J scenario -1.2%; `sync_session_turn` -2.4%. |
| Indexed pending movement loading | Gate J scenario -1.4%; `sync_session_turn` -2.8%. |
| Fresh movement command/effect/event shortcuts | `submit_move_intent` moved from 15.3572B to 14.1924B cumulatively after the indexed baseline. |
| Early not-due sync precheck | Gate J scenario -1.3%; early not-due sync calls about -32%. |
| Fresh sync effect shortcut | Gate J scenario -1.4%; `sync_session_turn` -2.9%. |
| Direct timer refresh for turn jobs | Gate J scenario -3.4%; `sync_session_turn` -7.0%. |
| Pre-deadline scheduled job guard | `submit_move_intent` -3.3% for that checkpoint. |
| Remove movement snapshot and turn-resolution command effects | Gate J scenario -2.0% across the two cuts; `sync_session_turn` 15.3986B -> 14.7341B. |

## Verification Already Recorded

| verification | status |
| --- | --- |
| Full benchmark suite after battle runtime/auth-cache work, run `20260519-063353-02c93e3` | Gate J/K/L/M passed. |
| Full suite with endpoint-surface gate, run `20260519-081911-fe84689` | endpoint-surface + Gate J/K/L/M passed; `59/59` required endpoints covered. |
| Focused Gate K battle benchmarks | Repeatedly passed while cutting battle submit cost down to about `0.3B`. |
| Focused battle PocketIC regressions | Readiness replay, two-player end-round, timer auto-defend, and battle-round paths passed in the notes. |
| Focused Gate J movement benchmarks | Passed after each movement checkpoint through `20260519-113052-movement-turn-effect-gate-j`. |
| Focused service regression | `lobby_session_setup_recovers_from_starting_state_and_replays_nonce` passed after the movement effect cuts. |

## Historical 2026-05-19 State

The game is more playable from a performance standpoint than at the start of perf1. The worst recorded battle submit path is no longer a `25B-28B` instruction operation in the benchmark path; it is now around `0.28B` average in Gate K/L, which is close to the stated normal target.

The bottleneck has moved to session-turn and broader world command processing. `submit_move_intent` and `sync_session_turn` are improved, but still far above the next target of under `5B`. `sync_battle` also remains expensive at about `9.88B` in the recorded battle runs.

## Historical 2026-05-19 Open Risks

| item | status |
| --- | --- |
| Active turn aggregate | Not implemented yet. Movement is still mostly row/effect/event/job driven. |
| Runtime archive durable flush | Tried several versions, but each exceeded the IC Wasm code-section limit. Backed out. |
| `CastAbility` battle action | Still a row-backed fallback; it was not hot in the recorded benchmark suite. |
| Movement targets | `submit_move_intent` is still about `13.75B`; `sync_session_turn` is still about `14.73B`. |
| Historical worktree at report time | There was an uncommitted partial edit removing `movement_intent` command effects from `movement.rs`; it was not benchmarked and was not included in the numbers above. `idea.md` was also untracked. |

## Recommendation

Do not spend much more time on tiny row-level movement cuts unless they are obviously safe and free code size. The battle work proved the real win comes from changing the live mutation model. The next serious perf1 step should be a session-turn/champion-movement runtime aggregate that owns fresh movement intents, turn readiness, event buffers, job/deadline hints, and champion/occupancy deltas while the turn is active.
