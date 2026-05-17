# Degens of Misery & Mayhem Spec 1.1

This spec turns `spec.missing.md` into an implementation plan for getting the
current IC canister game to a playable v1.1 state.

Spec 1.1 is not a content expansion. It is a repair and hardening pass around
the existing first-playable scope: local deployment, IcyDB-backed canister
state, automatic backend progression, reliable public endpoints, truthful map
rendering, command recovery, and a public canister play path.

Out of scope for 1.1 unless promoted explicitly: active siege engines, naval
movement, large procedural maps, diplomacy, guilds, ranked play, durable
rematch, broad neutral AI, large spell trees, full bot opponents, and other
items already parked in `spec.v2.md`.

## 0. Testing Speed, Parallelism, And Checkpoint Discipline

Priority: P0.

This testing work comes before the remaining spec 1.1 implementation points
until the suite is fast, trustworthy, and easy to run repeatedly. The machine
has enough CPU to run many independent checks at once, so test work should
exploit parallelism wherever the harness and state isolation permit it.

Checkpoint rules:

- Testing should be fast. Prefer small targeted groups, prebuilt test binaries,
  isolated temp/target directories where useful, and parallel workers over
  one large serial run.
- Do not assume PocketIC tests are parallel just because multiple cargo
  processes were launched. Current runs wait on `/tmp/canic-pocket-ic.lock`, so
  the first optimization checkpoint is removing or isolating that global lock.
- Ramp PocketIC concurrency deliberately after the lock fix. Start with 4-8
  independent PocketIC instances, verify port/temp/state isolation, then
  increase worker count only when memory and process overhead are stable.
- Run pure Rust/unit groups in parallel with canister/PocketIC groups whenever
  cargo build locks allow it.
- Each test group below owns its own checkbox. Mark it `[x]` only after the
  group is passing or the named checkpoint is complete, and leave a timing or
  evidence note.
- Commit after every completed checkpoint with a clear commit message that
  explains what was accomplished. Each checkpoint commit should include the
  relevant `spec.1.1.md` checkbox/evidence update.

Current timing notes from 2026-05-17:

| Test group | Last observed result/time | Parallel status / next action |
| --- | --- | --- |
| PocketIC harness parallelism baseline | Fixed 2026-05-17: `canic-testkit` lock is repo-patched to default to process-local namespaces, with `CANIC_POCKET_IC_LOCK_NAMESPACE` available for explicit shared shards | Verified by `cargo test --manifest-path vendor/canic-testkit/Cargo.toml process_lock --lib`, `cargo test -p domm-pocket-ic-tests --test pic_lock -- --nocapture` in 0.81s, and `cargo check -p domm-pocket-ic-tests --tests` in 33.7s |
| Fast timing harness | Added 2026-05-17: `scripts/run-test-groups.sh`, `make test-fast`, `make test-groups`, and `make test-groups-list` | `make test-fast` passed after prebuild; group wall times: pure 2.277s, schema 0.482s, generated 0.521s, canister-check 0.500s, pocket-lock 1.373s |
| Endpoint inventory/public surface | Passed 2026-05-17 in 260.810s after render list and battle query-budget fixes | Still too slow for an inner loop; keep optimizing after the group is green |
| Gate J strategic loop/IcyDB rows | Passed 2026-05-17 in 219.521s after movement fast-path snapshot and sync-budget fixes | Still slow; keep out of fast inner loop and retime with broader PocketIC parallelism |
| Gate K battle/victory/history | Passed in 549.3s observed | Retime after PocketIC parallelism fix |
| Gate L first-playable route | Passed clean in 433.1s | Long smoke remains valid; keep out of fast inner loop |
| Movement crossing conflict | Passed 2026-05-17 in 181.928s after crossing-conflict fast path | Still slow; keep out of fast inner loop and retime with broader PocketIC parallelism |
| Stationary enemy blocker | Passed 2026-05-17 in 194.921s after turn-sync yield split | Still slow; keep out of fast inner loop and retime with broader PocketIC parallelism |
| Week-two tavern/recruit growth | Passed in 269.5s observed | Retime after PocketIC parallelism fix |
| Gate M web client probe | Passed 2026-05-17 in 345.324s after render-query, PocketIC clock, battle-view, and aftermath-sync fixes | Still too slow for an inner loop; keep optimizing with the remaining PocketIC groups |
| Timer jobs PocketIC group | Passed 2026-05-17 in 59.586s after post-upgrade repair, heartbeat backstop, and expired-lease recovery fixes | New focused group is fast enough for repeated timer regression checks |
| End-turn PocketIC group | Passed 2026-05-17 in 59.621s after ready-row stale action guard and replay coverage | New focused group is fast enough for repeated end-turn regression checks |
| Battle-round readiness PocketIC group | Passed 2026-05-17 in 89.423s after auto-ready, round auto-defend, `end_battle_turn`, and replay coverage | Focused battle readiness route is isolated from the longer Gate K/L battle routes |
| Render projection PocketIC group | Passed 2026-05-17 in 228.116s after participant-known render candidates, fog/cursor assertions, and guarded-mine aftermath projection coverage | Focused render route is green but still long; consumed-pile disappearance remains covered by the Gate J route until pickup sync is optimized |
| Query budget PocketIC group | Passed 2026-05-17 in 54.613s after bounded movement preview, submit, object pages, compact game view, and response-size assertions | New focused group avoids long sync progression and is fast enough for repeated public query-budget checks |
| Command recovery PocketIC group | Passed 2026-05-17 in 269.686s after ledger replay reconciliation plus build, recruit, tavern hire, dwelling recruit, and battle aftermath retry/idempotency coverage | Green but too slow for an inner loop; split economy replay and battle aftermath routes during test-speed optimization |

Testing-first todo:

- [x] Capture the current PocketIC timing/failure inventory in this spec.
- [x] Remove, shard, or replace the cross-process PocketIC lock so independent
  PocketIC instances can run concurrently on this 32-core machine. Completed
  2026-05-17 by vendoring `canic-testkit` and replacing the single
  `/tmp/canic-pocket-ic.lock` path with process-local lock namespaces by
  default. Evidence: `cargo test --manifest-path
  vendor/canic-testkit/Cargo.toml process_lock --lib`, `cargo test -p
  domm-pocket-ic-tests --test pic_lock -- --nocapture`, and `cargo check -p
  domm-pocket-ic-tests --tests`.
- [x] Add a fast timing harness or make target that prebuilds once, runs
  independent groups with bounded parallelism, and reports per-group wall time.
  Completed 2026-05-17 with `scripts/run-test-groups.sh`, `make test-fast`,
  `make test-groups`, and `make test-groups-list`. Evidence: `bash -n
  scripts/run-test-groups.sh`, `make test-groups-list`, `DOMM_TEST_JOBS=4
  scripts/run-test-groups.sh pocket-lock`, and `make test-fast`.
- [x] Endpoint inventory/public surface group:
  `pocket_ic_canister_exposes_every_required_game_endpoint`. Completed
  2026-05-17 with `DOMM_TEST_JOBS=1 scripts/run-test-groups.sh endpoint`
  passing in 260.810s after prebuild. Evidence also included
  `cargo check -p domm-degens-canister`.
- [x] Gate J strategic loop/IcyDB persistence group:
  `pocket_ic_gate_j_strategic_loop_persists_icydb_rows`. Completed
  2026-05-17 with `DOMM_TEST_JOBS=1 scripts/run-test-groups.sh gate-j`
  passing in 219.521s after prebuild. Evidence also included
  `cargo check -p domm-degens-canister`.
- [x] Gate K battle aftermath/victory/history group:
  `pocket_ic_gate_k_battle_aftermath_victory_history_persist_icydb_rows`
  currently passes; retime after the shared PocketIC lock is fixed.
- [x] Gate L first-playable canister route group:
  `pocket_ic_gate_l_first_playable_canister_e2e_uses_public_endpoints_and_icydb_state`
  passed cleanly in about 7.2 minutes.
- [x] Movement crossing conflict group:
  `pocket_ic_movement_crossing_conflict_uses_persisted_sync_cursor`. Completed
  2026-05-17 with `DOMM_TEST_JOBS=1 scripts/run-test-groups.sh movement`
  passing in 181.928s after prebuild. Evidence also included
  `cargo check -p domm-degens-canister`.
- [x] Stationary enemy blocker group:
  `pocket_ic_stationary_enemy_blocker_starts_champion_encounter`. Completed
  2026-05-17 with `DOMM_TEST_JOBS=2 scripts/run-test-groups.sh movement
  stationary` passing the stationary group in 194.921s after prebuild. Evidence
  also included `cargo check -p domm-degens-canister`.
- [x] Week-two tavern/recruit growth group:
  `pocket_ic_week_two_tavern_and_recruit_growth_materialize_on_turn_advance`
  currently passes; retime after the shared PocketIC lock is fixed.
- [x] Gate M web client probe group:
  `gate_m_web_client_probe_runs_against_pocket_ic_canister_adapter`. Completed
  2026-05-17 with `DOMM_TEST_JOBS=1 scripts/run-test-groups.sh gate-m`
  passing in 345.324s after prebuild. Evidence also included `cargo check -p
  domm-degens-canister`.
- [x] Timer jobs PocketIC group: scheduling, duplicate timer no-op, expired
  lease recovery, post-upgrade repair, and deadline progression. Completed
  2026-05-17 with `DOMM_TEST_JOBS=1 scripts/run-test-groups.sh timer-jobs`
  passing in 59.586s after prebuild. Evidence also included `cargo check -p
  domm-degens-canister` and a focused no-run build for
  `pocket_ic_timer_jobs_repair_deadlines_and_recover_expired_leases`.
- [x] End-turn PocketIC group: ended-player-still-acts, final participant
  closes the turn, stale turn commands, and replay semantics. Completed
  2026-05-17 with `DOMM_TEST_JOBS=1 scripts/run-test-groups.sh end-turn`
  passing in 59.621s after prebuild. Evidence also included `cargo check -p
  domm-degens-canister` and a focused no-run build for
  `pocket_ic_end_turn_closes_turn_and_blocks_stale_actions`.
- [x] Battle-round readiness PocketIC group: timer auto-defend,
  `end_battle_turn`, auto-ready stacks, and replay semantics. Completed
  2026-05-17 with `DOMM_TEST_JOBS=1 scripts/run-test-groups.sh
  battle-round` passing in 89.423s after prebuild. Evidence also included
  `cargo check -p domm-degens-canister` and a focused no-run build for
  `pocket_ic_battle_round_readiness_advances_and_replays`.
- [x] Render projection PocketIC group: live object hydration, consumed piles,
  defeated neutrals, captured mines, champion coordinates, fog, and cursors.
  Completed 2026-05-17 with `DOMM_TEST_JOBS=1
  scripts/run-test-groups.sh render-projection` passing in 228.116s after
  prebuild. Evidence also included `cargo check -p domm-degens-canister`, a
  focused no-run build for `pocket_ic_render_projection`, and focused
  repository hot-path inventory coverage. The new group covers opening live
  object hydration, fog, cursors, defeated neutrals, captured mines, and
  champion coordinates; consumed-pile disappearance remains covered by the
  already-passing Gate J route while the pickup sync path is tracked under the
  query budget/test-speed work.
- [x] Query budget group: representative preview/submit movement paths,
  bounded render reads, and query instruction ceilings. Completed 2026-05-17
  with `DOMM_TEST_JOBS=1 scripts/run-test-groups.sh query-budget` passing in
  54.613s after prebuild. Evidence also included `cargo check -p
  domm-degens-canister` and a focused no-run build for
  `pocket_ic_query_budget_keeps_preview_submit_and_render_bounded`. The route
  covers bounded `preview_move_path`, `submit_move_intent`, cursor-paged
  `get_visible_objects`, compact `get_game_view`, and a 64 KiB max Candid
  response-size ceiling while verifying the queries execute below PocketIC
  instruction limits.
- [x] Command recovery group: build, recruit, tavern hire, dwelling recruit,
  ledger effects, and battle aftermath retry/idempotency. Completed 2026-05-17
  with `DOMM_TEST_JOBS=1 scripts/run-test-groups.sh command-recovery` passing
  in 269.686s after prebuild. Evidence also included `cargo check -p
  domm-degens-canister`, a focused no-run build for
  `pocket_ic_command_recovery_replays_economy_and_battle_effects`, and `bash
  -n scripts/run-test-groups.sh`. The route verifies exact-nonce replay
  stability for build, town recruit, tavern hire, and dwelling recruit;
  resource ledgers and command effects do not duplicate on replay, and resolved
  battle aftermath exact/fresh retries remain no-ops for event/effect rows.
- [ ] Visibility/redaction group: opponent events, neutral battle details,
  town build/recruit visibility, and public/private payload separation.
- [x] Pure rules group: `cargo test -p domm-game`. Completed 2026-05-17
  with `DOMM_TEST_JOBS=4 scripts/run-test-groups.sh pure schema generated
  canister-check` passing the pure group in 2.018s after prebuild.
- [x] Canister crate group: `cargo check -p domm-degens-canister` and focused
  canister tests. Completed 2026-05-17 with `cargo check -p
  domm-degens-canister`, focused `repository_hot_path_plans_are_indexed_and_bounded`,
  focused `lobby_session_setup_recovers_from_starting_state_and_replays_nonce`
  passing in 288.42s, and full `cargo test -p domm-degens-canister` passing
  14 tests in 288.02s.
- [x] Generated-session/schema group: `make test-generated` and repository
  inventory coverage. Completed 2026-05-17 with `DOMM_TEST_JOBS=4
  scripts/run-test-groups.sh pure schema generated canister-check` passing
  schema in 0.366s and generated-session in 0.349s after prebuild. The
  equivalent make targets are `make test-schema` and `make test-generated`.
- [ ] Local DFX/blast smoke group: deploy, scan public endpoints, and play the
  required direct `blast call` route with IcyDB diagnostics.
- [ ] Full regression orchestration group: `make regression`,
  `make check-canister`, and `make test-pocket` with a final timing table and
  parallel-safe command recipe.

## Five-Agent Investigation Synthesis

On 2026-05-16, five read-only domain agents reviewed `spec.md`,
`spec.1.1.md`, `spec.missing.md`, and the current codebase from separate
perspectives: gameplay, public API, ICP/timers, IcyDB, and testing. Their
shared conclusion is that DoMM should not hide current backend issues behind
adapters or scripted probes. The backend spine and public endpoint contract must
be fixed first.

Consensus P0 order:

- Durable backend progression:
  add `SystemJob`, one-call timer-driven `start_session` setup,
  `ParticipantTurnReady`, `end_turn`, timer scheduling, zero-delay continuation
  jobs, and `post_upgrade` repair before claiming the game can progress on its
  own.
- Command recovery:
  build, recruit, tavern hire, dwelling recruit, ledger effects, and battle
  aftermath must not partially mutate state and then fail unrecoverably. Turn
  resolution must not advance past pending/applying mutated commands.
- Render truth:
  `get_visible_objects` and related map render endpoints must hydrate from live
  `Champion`, `WorldObject`, occupancy, visit, neutral, town, and battle rows.
  Static `ParticipantKnownObject` rows are discovery memory, not authoritative
  current state.
- Movement contract:
  preview, submit, and execution must share bounded validation for
  surveyed-base-map fog, dynamic-state redaction, occupancy, chunk caps,
  blockers, stop reasons, and late-turn semantics.
- Battle route:
  battle timeouts must be timer-driven; guarded mine victory must capture or
  expose an explicit post-battle interaction without requiring an artificial
  step away and back.
- Battle round readiness:
  battles need their own early end-turn flow. A participant can end the current
  battle round, and a participant with no stacks able to perform a meaningful
  action is auto-ended for that round.
- Client/API surface:
  v1.1 playability is proven through documented canister endpoints, local
  `blast` smoke, PocketIC routes, and render contracts that a future client can
  consume without test-only shortcuts.

Core implementation files called out by the investigation:

- `schema/degens/src/schema/entities.rs`
- `canisters/degens/src/repos/mod.rs`
- `canisters/degens/src/repos/system_jobs.rs`
- `canisters/degens/src/repos/turn_ready.rs`
- `canisters/degens/src/repos/battle_round_ready.rs`
- `canisters/degens/src/services/system_jobs.rs`
- `canisters/degens/src/lib.rs`
- `canisters/degens/src/services/account_lobby_session.rs`
- `canisters/degens/src/services/movement.rs`
- `canisters/degens/src/services/battle.rs`
- `canisters/degens/src/services/command_response.rs`
- `canisters/degens/src/services/town.rs`
- `canisters/degens/src/services/economy_expansion.rs`
- `canisters/degens/src/services/render_projection.rs`
- `canisters/degens/src/services/game_view.rs`
- `canisters/degens/src/services/events.rs`
- `canisters/degens/src/services/diagnostics.rs`

## Required Release Gates

- `make regression` passes.
- `make check-canister` passes.
- `make test-pocket` passes with timer-driven turn and battle progression.
- A local canister can be built, installed, scanned, and played with `blast`
  without manual Candid metadata patching.
- An agent-run local play route registers multiple identities, starts a
  session, moves, collects, builds, recruits, fights a guarded mine battle,
  captures the mine, receives income, and verifies IcyDB row health.
- Public render endpoints never show stale champion coordinates, consumed piles
  as available, defeated neutrals as active, or captured mines as uncaptured.
- Normal clients do not need to call `sync_session_turn`, `sync_battle`,
  `sync_objectives`, `sync_world_events`, `sync_advanced_victory`, or any
  future `process_next_turn` endpoint to keep backend state moving.

## Decision Lock-In

These v1.1 choices resolve the remaining open-choice items and should be treated
as the implementation contract unless a later audited patch changes both this
file and `spec.md`:

- Turn-sensitive commands are accepted only while the persisted current turn is
  still open and no durable turn-resolution job has been accepted for that
  turn. Once closure is accepted, old-turn commands fail before command
  creation with `backend_work_pending` while processing or `turn_expired` after
  advancement. Exact retries of commands created before closure replay stored
  command status.
- `get_session` is a lobby/setup shell. Active gameplay metadata comes from
  `get_game_view` render metadata, `get_content_manifest`, and dedicated
  bounded render endpoints.
- All `sync_*` endpoints are internal maintenance concepts, not normal client
  actions. If the public methods remain temporarily exposed, they are
  admin/debug/manual recovery wrappers over the same durable `SystemJob`
  runners used by timers, zero-delay continuations, awaited self-call
  continuations, and ordinary gameplay commands. They must not be required in
  client flow or shown as gameplay action affordances.
- Canister `get_game_view` remains a metadata shell, not a large aggregate. It
  must return `omitted_fields` when heavy collections are omitted, and omitted
  collections must report `has_more = false` and `next_cursor = None`.
- V1.1 uses surveyed-base-map fog. Static map chunk terrain, movement, and
  flags may be returned for undiscovered chunks, but dynamic objects, owners,
  occupants, battle details, and events remain visibility-gated.
- Town recruitment supports `RecruitTarget::TownGarrison` only. The
  `RecruitTarget::Champion` DTO variant is reserved for v2 and must reject
  before command creation, resource spending, or pool mutation.
- External dwellings with `direct_recruit = true` are remote rally points. They
  may recruit to any owned active world-map champion in the same session; no
  same-tile or distance check is required in v1.1.
- V1.1 active economy is mine income, resource pickups/rewards, costs, tavern
  hiring, marketplace trade, direct dwelling recruitment, and weekly
  tavern/recruit growth. Town hall income, captured-town unrest mechanics,
  pacification, recruit-pool halving, and desperation income are v2.
- Tactical neutral battle detail is private to involved participants. Hidden
  opponent town build/recruit events and hidden neutral battle outcomes are
  private or redacted according to visibility; no exact public hidden payloads.
- Battle spellcasting remains v1.1 scope for learned v1.1 spells. Retreat and
  surrender are disabled/deferred actions in v1.1.
- Disabled/future systems may appear only as non-actionable status/debug rows
  with disabled metadata. They must not appear as enabled affordances or append
  public gameplay events.
- `accept_quest` on claimed quests returns a claimed-specific non-retryable
  error and emits no events.

## Test Classes

Use these test classes consistently across every topic below:

- Pure rules tests: `cargo test -p domm-game`.
- Canister crate checks: `make check-canister`.
- PocketIC tests: `make test-pocket` or targeted
  `cargo test -p domm-pocket-ic-tests --test <name> <case> -- --nocapture`.
- Full workspace regression: `make regression`.
- Local blast smoke: an agent-run/manual command sequence. The canister is
  deployed locally with DFX using generated public Candid metadata, then the
  agent runs `blast scan` and `blast call` commands directly with multiple
  identities and records endpoint/IcyDB evidence. Do not add committed `blast`
  scripts unless explicitly requested.

New PocketIC suites should be split by failure mode instead of added to the
already-large endpoint tests:

- `testing/pocket-ic/tests/timer_jobs.rs`
- `testing/pocket-ic/tests/end_turn.rs`
- `testing/pocket-ic/tests/battle_round_readiness.rs`
- `testing/pocket-ic/tests/render_projection.rs`
- `testing/pocket-ic/tests/query_budgets.rs`
- `testing/pocket-ic/tests/command_recovery.rs`
- `testing/pocket-ic/tests/visibility_redaction.rs`

## 1. Local Canister Deployment And Blast Smoke

Priority: P0.

### Current Problems

- There is no committed root `dfx.json` or checked-in local deployment flow for
  DoMM.
- The local audit required a temporary DFX project and manual wasm Candid
  metadata patching before `blast scan` could see public endpoints.
- Local controller setup for diagnostics is undocumented.
- The live canister is playable through `blast`, but the setup path is brittle
  and too manual to be a release gate.

### Reproducible Tests

- Build the release wasm from a fresh checkout and install it on local DFX
  using only committed commands.
- Run `blast scan` against the installed canister and assert all required game
  endpoints are present.
- Use at least two `blast` identities to register, create, join, mark ready,
  start, inspect, move, and read events.
- Call `icydb_snapshot`, `icydb_metrics`, and small-batch
  `get_diagnostic_storage_snapshot` after the route and assert
  `corrupted_entries = 0` and `corrupted_keys = 0`.

### Proposed Solution

- Add a committed local deployment path: `dfx.json`, generated DID or
  reproducible DID extraction, build/install commands, and cleanup commands.
- Ensure packaged wasm artifacts include public Candid metadata automatically.
- Add a documented agent-run `blast` command checklist that creates or selects
  multiple identities and drives the first session setup path directly from the
  shell after local deployment.
- Document how `blast` identities map to players and how controller-gated
  diagnostics are enabled locally.
- Keep generic SQL endpoints out of v1.1. Local inspection uses
  `icydb_snapshot`, `icydb_metrics`, and typed/controller-gated diagnostics.

## 2. Backend-Scheduled Turn And Battle Progression

Priority: P0.

### Current Problems

- `sync_session_turn` is a client-visible update call and currently enforces
  the 60-second turn deadline.
- `sync_battle` is also client-visible and is needed when battle timeout work is
  incomplete.
- `sync_objectives`, `sync_world_events`, and `sync_advanced_victory` are
  client-visible materialization hooks. They currently make objective progress,
  world-event rows, and advanced victory checks look like gameplay calls even
  though those changes should be driven by turn close, quest/capture/battle
  aftermath, timers, and recovery jobs.
- Battles have no early round readiness path. If both battle participants are
  done, or one participant has no stacks that can still move or act, the
  current implementation still relies on active-stack sequencing and timeout
  processing instead of advancing the battle round immediately.
- The canister has no timer or heartbeat runner today.
- Clients are being asked to drive backend progression that the canister should
  own.
- IC timer IDs are volatile and are not persisted across upgrades, so any
  timer design that stores only in-memory handles is unsafe.
- `canic-cdk` already provides `canic_cdk::timers` through
  `ic-cdk-timers`; timer callbacks are volatile, at-least-once wakeups that may
  run late and cost canister-call cycles.
- IC messages are serialized for a canister until an `await`, which lets update
  calls and timer callbacks safely claim durable IcyDB jobs if domain mutation
  slices avoid holding state across awaits.

### Reproducible Tests

- PocketIC: start an active session, advance time past the 60-second deadline,
  tick timers, and assert the session advances without calling
  `sync_session_turn`.
- PocketIC: create a battle, advance time past `action_deadline_at`, tick
  timers, and assert timeout auto-defend applies without calling `sync_battle`.
- PocketIC: force a budgeted timeout or turn-resolution slice that cannot
  finish in one update, then assert a zero-delay timer continues the work.
- PocketIC: capture a mine, claim a quest reward, and advance a turn; assert
  objective progress, world-event rows, and advanced victory state update
  without calling `sync_objectives`, `sync_world_events`, or
  `sync_advanced_victory`.
- PocketIC: simulate a canister upgrade after timers are scheduled, then assert
  `post_upgrade` reconstructs due work from IcyDB and schedules a new wakeup.
- PocketIC: fire a late or duplicate timer callback and assert no duplicate
  turn advancement, movement resolution, income, battle event, or command row.

### Proposed Solution

- Add a durable `SystemJob` IcyDB entity. Required fields:
  `job_key`, `job_kind`, `session_id`, optional `battle_id`, optional
  `turn_number`, `due_at`, `status`, `lease_owner`, `lease_expires_at`,
  `attempt_count`, `generation`, `command_id`, `cursor_json`, `last_error`,
  `created_at`, and `updated_at`.
- Add `SystemJob` indexes:
  unique `job_key`,
  `status + due_at`,
  `session_id + status + due_at`,
  `battle_id + status + due_at`,
  and optional `command_id`.
- Use deterministic unique job keys:
  `turn_deadline:{session_id}:{turn_number}`,
  `turn_resolution:{session_id}:{turn_number}`,
  `turn_resolution_continue:{command_id}`,
  `battle_timeout:{battle_id}:{deadline_ms}`,
  `scenario_objectives:{session_id}:{turn_number}`,
  `world_events:{session_id}:{turn_number}`,
  `advanced_victory:{session_id}:{turn_number}`, and
  `repair_after_upgrade:{generation}:{cursor}`.
- Treat IC timers as wakeups only. Every timer callback must reload IcyDB state,
  verify the job is still due, verify the session turn or battle deadline still
  matches, and no-op if stale.
- Keep one in-memory global timer handle for the nearest due job where
  practical. On earlier job insertion, clear the old volatile handle if it is
  still present and schedule a new timer.
- Use `canic_cdk::timers::set_timer` for deadline wakeups and
  zero-second continuation timers. Use recurring timers only for defensive
  watchdogs or diagnostics, and keep them idempotent.
- On `init` and `post_upgrade`, scan active sessions and battles in bounded
  slices, repair missing `SystemJob` rows, and reschedule the nearest due job.
- Convert `sync_session_turn`, `sync_battle`, `sync_objectives`,
  `sync_world_events`, and `sync_advanced_victory` into admin/debug/manual
  recovery drivers or thin wrappers around the same internal job runners.
  Normal clients should not need them.
- Use a canister-side instruction soft limit with `instruction_counter()`.
  Before a slice approaches the limit, persist cursor state, set the job back
  to `scheduled` with `due_at = now`, and schedule a zero-delay timer.
- Current movement parking mutates the remaining `MovementIntent.path_json` and
  restarts step indexes on later syncs. The first job-runner implementation
  should use deterministic per-slice system command nonces stored in
  `SystemJob.cursor_json`, unless movement snapshot indexing is changed.
- Schedule `turn_deadline:{session_id}:{turn_number}` when a session becomes
  active and whenever a new map turn starts.
- Schedule `battle_timeout:{battle_id}:{deadline_ms}` whenever a battle action
  deadline is created or advanced.
- Schedule or inline-run scenario maintenance jobs from the real mutation that
  made them due:
  turn close schedules world events, max-turn/victory checks, and objective
  refresh;
  battle aftermath schedules objective/victory refresh and guarded-object
  capture checks;
  quest accept/claim schedules objective/victory refresh;
  mine/town/object ownership changes schedule objective/victory refresh.
- Scenario maintenance jobs are idempotent and event emission is tied to real
  state changes. Re-running a wrapper, stale timer, or zero-delay continuation
  must not append duplicate `objectives_synced`, `world_event_synced`, or
  `advanced_victory_synced` events for the same logical turn/effect.
- Add battle-round readiness jobs:
  `battle_round_ready:{battle_id}:{participant_id}:{round_number}` and
  `battle_round_advance:{battle_id}:{round_number}`.
- `battle_round_advance` runs immediately when every alive battle participant
  is ready for the current round, or when auto-readiness detects that every
  participant with living stacks has no remaining meaningful action.

## 3. Early End Turn Readiness

Priority: P0.

### Current Problems

- The spec requires turns to close early when all active map participants have
  ended, but the canister has no active-turn `end_turn` endpoint.
- Existing `mark_ready` is lobby-only and returns `session_not_joinable` during
  active play.
- There is no `ParticipantTurnReady` persistence.
- The current turn only closes through deadline-driven `sync_session_turn`.
- `GameParticipant.ready_turn` exists, but it belongs to lobby/session setup
  semantics. Do not repurpose it for active map-turn readiness.

### Reproducible Tests

- PocketIC: in a three-player active session, P1 calls `end_turn`, then still
  submits a legal movement/build/recruit command before everyone else ends.
- PocketIC: the third participant calls `end_turn`; the canister closes the map
  turn immediately in that update or through a zero-delay continuation timer.
- PocketIC: after turn resolution starts, a command targeting the old turn is
  rejected as stale or requires a refreshed view.
- PocketIC: a participant with a champion in an active battle can still end the
  world-map turn while battle actions continue separately.

### Proposed Solution

- Add `ParticipantTurnReady` keyed by
  `session_id + participant_id + turn_number`.
- Add `ParticipantTurnReady` fields:
  `session_id`, `participant_id`, `turn_number`, `command_id`, and `ended_at`.
- Add `ParticipantTurnReady` indexes:
  unique `session_id + participant_id + turn_number`,
  nonunique `session_id + turn_number`,
  and optional `command_id`.
- Add public update endpoint:
  `end_turn(session_id, client_nonce) -> CommandResponse`.
- Treat `end_turn` as a readiness marker, not a player lock. Ended players may
  keep acting while the same map turn remains open.
- Do not clear readiness when a player acts later in the same turn.
- Because IC messages are serialized per canister, the last `end_turn` call can
  safely observe all readiness rows, create a `turn_resolution` system job, and
  attempt a bounded resolution slice immediately.
- Once turn resolution is accepted for a turn, freeze that closing turn. New
  commands for the old turn fail stale or require refresh.

## 4. Turn Resolution Job Runner

Priority: P0.

### Current Problems

- Current turn sync resolves pending movement, materializes income, increments
  `current_turn`, and emits events in one client-triggered path.
- There is no durable processing lease that blocks conflicting updates while a
  turn is closing.
- Pending/applying commands can be overtaken by turn advancement in at least one
  observed route.
- Timer-driven progression will amplify partial mutation bugs if command
  recovery is not fixed first.

### Reproducible Tests

- PocketIC: create a pending/applying command state, then attempt turn
  resolution. The system must recover, fail, or quarantine the command before
  advancing the turn.
- PocketIC: run a turn with multiple movement intents, pickups, income, and
  battle starts. Force partial progress and assert the continuation finishes
  without duplicate events or double-spend.
- Blast smoke: after each turn advancement, verify `GameCommand`, `GameEvent`,
  `CommandEffect`, movement, resource ledger, and session rows are consistent.

### Proposed Solution

- Add a durable processing lease on `SystemJob` and, if needed, session-level
  fields such as `processing_kind`, `processing_turn`,
  `processing_command_id`, and `lease_expires_at`.
- Implement `process_turn_resolution_slice(session_id, job_key, now_ms)`.
- The slice must recover/apply pending movement, resolve object interactions,
  materialize income, emit events/effects, advance `GameSession.current_turn`,
  reset turn timing, schedule the next deadline job, and complete the system
  command.
- Store cursor/progress in `SystemJob.cursor_json` or command phase fields
  before scheduling another zero-delay timer.
- Never hold mutable domain state across `await`. Persist phase and lease
  state first, then continue in a fresh message.
- Recover both `pending` and `applying` commands before advancing a turn.
- Add build and recruit commands to recoverable command handling or implement
  equivalent domain recovery before enabling automated turn closure.
- Avoid adding a new unique `CommandEffect(session_id, effect_key)` index over
  existing data. For once-per-domain effects like battle aftermath or guarded
  capture, prefer a new domain-effect entity or use existing
  `PendingEffect(session_id, effect_key)` style markers.

## 5. Public API And Render Contract

Priority: P0.

### Current Problems

- Existing client-probe routes are scripted and hard-code movement, build,
  recruit, and battle choices.
- `get_game_view` intentionally returns a lightweight shell with empty map,
  object, champion, town, battle, and event vectors.
- The test adapter composes dedicated endpoints, but the public canister
  contract is not explicit enough for a future client or tool to do the same
  without reading test internals.
- `get_command_status` by nonce currently guesses command types from nonce text;
  callers need a reliable `{command_type, client_nonce}` lookup or stable
  command IDs everywhere.

### Reproducible Tests

- Local `blast` smoke: public endpoints can register, create/join, start,
  inspect map state, submit movement, build, recruit, fight, and view
  results/history without test-only shortcuts.
- Endpoint contract test: compose render state from public endpoints, including
  pagination, command status, events, battle state, town views, and typed
  errors, without using repository internals.
- Blast smoke: exact nonce retry returns previous command outcome, then public
  read endpoints expose the current persisted state.

### Proposed Solution

- Do not add a separate client deliverable to v1.1.
- Treat the canister Candid, endpoint docs, typed DTOs, pagination rules,
  command status semantics, and local `blast` smoke as the v1.1 public client
  contract.
- Make `get_game_view` impossible to misread. It remains a metadata shell in
  v1.1, must return explicit `omitted_fields`, and must set `has_more = false`
  plus `next_cursor = None` for omitted collections.
- Document that replayed update responses prove command outcome, not current
  render state. Callers must refresh affected views after retry responses.

## 6. Render Projection, Fog, Discovery, And Pagination

Priority: P0.

### Current Problems

- `get_visible_objects` renders stale static known-object projection after
  movement, pickup, battle, capture, and income.
- Champions appear at opening coordinates even after movement.
- Consumed resource piles still render as available.
- Defeated neutrals and captured mines can still render as active/available.
- Newly visible objects are not reliably materialized into known-object rows.
- Object and map chunk pages often return `page_info = null`.
- Object pagination uses offset-like cursors rather than stable cursor tokens.
- Map chunks return public static terrain/movement/flags even for undiscovered
  chunks; dynamic state must remain redacted by visibility.

### Reproducible Tests

- PocketIC: move a champion, then assert `get_visible_objects` and
  `get_champion_view` agree on the champion coordinate.
- PocketIC: collect a resource pile, then assert the object page hides it or
  marks it consumed.
- PocketIC: defeat a neutral guard and capture a mine, then assert object pages
  do not show the defeated neutral as active or the mine as uncaptured.
- PocketIC: move into newly visible territory, discover an object, and assert it
  appears through public object pagination.
- PocketIC: request paged map chunks and objects, then assert stable page
  metadata and cursor behavior.
- Visibility test: an opponent who has not discovered a chunk or object cannot
  infer hidden dynamic object state through object pages.

### Proposed Solution

- Hydrate render projection from current persisted rows:
  `Champion`, `MapOccupancy`, `WorldObject`, `ParticipantObjectVisit`,
  battle/neutral state, town ownership, and visibility rows.
- Keep `ParticipantKnownObject` as discovery memory only, not as the source of
  live semantic state.
- On movement and battle aftermath, update visible/discovered tiles and
  materialize newly discovered objects.
- Return typed `ObjectDetails` as the primary v1.1 contract; keep JSON detail
  payloads compatibility-only where they already exist.
- Add `get_object_view(session_id, subject_kind, subject_id_text)` for selected
  map-object details. Full detail endpoints remain
  `get_town_view`, `get_champion_view`, and authorized `get_battle_state`.
- Use stable cursor tokens for object pagination. Offset pagination is not a
  v1.1 public contract.
- Always return page metadata for paged endpoints.
- Document surveyed-base-map fog: map chunk `terrain_blob`, `movement_blob`,
  and `flags_blob` are public static map data and may be returned for
  undiscovered chunks. `discovered_blob` and `visible_blob` remain authoritative
  for fog overlays and interaction. Dynamic objects, owners, occupants, and
  events stay visibility-gated.

## 7. Movement Preview, Submit Validation, And Occupancy

Priority: P0.

### Current Problems

- `preview_move_path` can exceed the 5B local replica instruction limit for
  short or normal multi-step paths after real play history accumulates.
- Submit validation is cheaper than preview and can accept paths preview cannot
  evaluate.
- Canister submit validates bounds, adjacency, path length, and terrain cost,
  but not all fog/discovery and chunk-cap rules required by the spec.
- Preview always returns `stop: None` even when execution can stop on objects,
  blockers, towns, enemies, or neutrals.
- Movement into an occupied champion tile can be accepted as an intent and then
  silently fail to move during sync.
- Late movement while backend work is pending can be accepted today, which
  differs from the v1.1 contract once a turn-resolution job has been accepted.

### Reproducible Tests

- PocketIC query-budget tests for 1, 2, 8, and 64 step previews after realistic
  setup and play history.
- PocketIC: movement into undiscovered static terrain is allowed within normal
  movement/path caps, but preview does not reveal hidden dynamic blockers,
  objects, owners, occupants, or events.
- PocketIC: too-many-chunks movement is rejected.
- PocketIC: preview reports stops for pickups, guards, towns, enemy blockers,
  and occupied destination tiles.
- PocketIC: moving into a currently occupied friendly champion tile is rejected
  on submit; simultaneous resolution-time conflicts produce clear blocked
  movement events.
- Contract test: once a turn-resolution job is accepted for the current turn,
  old-turn submits reject before command creation with `backend_work_pending`
  while the job is processing or `turn_expired` after advancement.

### Proposed Solution

- Move preview onto the same indexed/bounded validation path as submit and
  execution.
- Add query budget guardrails and return a typed partial/too-expensive preview
  response instead of trapping.
- Align preview, submit, and execution validation for visibility, path caps,
  terrain, occupancy, and stop reasons.
- Add occupancy checks before command creation where possible.
- Enforce the v1.1 late-submit contract: turn-sensitive commands may still be
  accepted for `GameSession.current_turn` until a durable turn-resolution job is
  accepted for that turn. After that closure point, old-turn commands reject
  before `GameCommand` creation. Exact retries of commands created before
  closure replay stored command status.

## 8. Battle And Encounter Integration

Priority: P0 for timeout automation and guarded-mine route. P1 for battle
spellcasting polish.

### Current Problems

- Battle can be driven through canister endpoints, but manual play is awkward
  because 30-second action deadlines trigger timeout flows quickly.
- `submit_battle_action` can fail with `battle_sync_incomplete`, telling the
  caller to run `sync_battle`.
- Battle rounds do not have an early end-turn/readiness command. If both sides
  are done with the current round, or one side has no stacks that can still move
  or act, the canister does not currently have a durable readiness path to
  advance the battle round without waiting on active-stack timeout machinery.
- Movement-created encounters and battle setup still have fixture-like or
  synthetic paths in pure/client proof layers.
- Guarded mine battle victory does not automatically capture the mine; the live
  route required stepping away and back.
- `sync_battle` on resolved battles can look like it is replaying old aftermath
  as a fresh command.
- Battle spellcasting is exposed in docs and service paths, but legal actions
  and champion spell projection do not support it consistently.

### Reproducible Tests

- PocketIC: moving onto the guarded west mine creates a real battle id that the
  client opens and resolves.
- PocketIC: defeating the guard captures the mine in battle aftermath. No
  artificial step away/back or explicit post-battle interaction is required.
- PocketIC: after capture, the next turn produces mine income.
- PocketIC: battle timeout auto-defend is timer-driven and does not require
  client `sync_battle`.
- PocketIC: one battle participant calls `end_battle_turn`, the other
  participant also ends, and the battle advances to the next round immediately
  without waiting for active-stack deadlines.
- PocketIC: a participant whose living stacks have all acted, are disabled, or
  have no enabled meaningful action is marked battle-round ready automatically.
- PocketIC: when a participant ends the battle round, any still-unacted stacks
  they own are deterministically marked skipped/defended for that round and
  cannot later submit old-round battle actions.
- PocketIC: timeout work that exceeds per-update budget schedules and completes
  a zero-delay continuation.
- PocketIC: repeated `sync_battle` or timer wakeups on a resolved battle return
  a stable no-op/replay response without duplicating events.
- PocketIC: learn a battle spell, enter battle, see a legal `CastAbility`, cast
  it, and verify mana/status/damage persistence.

### Proposed Solution

- Schedule `battle_timeout` system jobs whenever a battle action deadline is
  set or advanced.
- Add `BattleParticipantRoundReady` IcyDB state keyed by
  `battle_id + participant_id + round_number`, with `session_id`, `command_id`,
  `ready_reason`, and `ended_at`. Reasons include `player_end_turn`,
  `auto_no_actions`, `auto_all_stacks_acted`, and `battle_resolved`.
- Add public update endpoint:
  `end_battle_turn(session_id, battle_id, client_nonce) -> CommandResponse`.
  This is separate from map `end_turn`.
- Battle `end_battle_turn` is a commitment for that battle round, unlike map
  `end_turn`. The participant's remaining eligible stacks for that battle round
  are resolved through deterministic system skip/defend commands, and later
  old-round battle actions from that participant fail stale.
- On every battle action, timeout job, battle view refresh driver, and
  `end_battle_turn`, recompute auto-readiness. A participant is auto-ready when
  no living stack they own can perform an enabled meaningful action this round.
  Passive `Defend` and `EndBattleTurn` affordances do not prevent auto-ready.
- When all alive battle participants are ready for the current round, enqueue
  or run `battle_round_advance:{battle_id}:{round_number}`. The job verifies
  the battle is still active, advances `current_round`, selects the next active
  stack, sets the next `action_deadline_at`, schedules the next
  `battle_timeout`, or resolves max-round/end-of-battle conditions.
- Let `submit_battle_action` run a cheap due-work pass, but return a clear
  retryable `battle_processing` error if another timeout job holds the lease.
- Make battle timeout commands actor `"system"` with deterministic idempotency
  keys.
- Ensure movement-created neutral/champion/town encounters create the actual
  tactical battle the client opens.
- Apply guarded-object capture in battle aftermath when the victorious champion
  wins the guarded-object battle. No post-battle step away/back or extra
  interaction is required for v1.1.
- Make resolved battle sync idempotent and visibly no-op.
- Fully wire battle spellcasting for learned v1.1 spells into legal actions,
  render projection, mana/resource costs, effects, persistence, and tests.
- Keep `Retreat` and `Surrender` disabled/deferred in v1.1. They may appear
  only as disabled legal-action metadata with explicit disabled reasons, and
  submit must reject them before command creation.

## 9. Command Lifecycle, Recovery, Status, And Receipts

Priority: P0.

### Current Problems

- Build, recruit, tavern hire, and some economy commands can partially mutate
  state before failing.
- `submit_build_town_structure` and `submit_recruit_units` are not included in
  the core recoverable command list.
- Champion-target recruit can spend gold and decrement a pool before returning
  `unsupported_recruit_target`, leaving a pending command and lost resources.
- Tavern hire can create champion/offer/resource mutations before an occupancy
  error and then fail differently on retry.
- Turn sync can advance past a pending command that already mutated state.
- `get_command_status` by client nonce can exceed query budget or miss commands
  that are readable by command id.
- Receipt event counters can under-report the events returned in the response.

### Reproducible Tests

- Trap/retry test: build writes a ledger row then traps before participant
  balance update. Retry must complete without double-spend or lost spend.
- Trap/retry test: recruit decrements a pool then traps. Retry must complete or
  roll forward consistently.
- Regression: unsupported champion-target recruit cannot create a command,
  spend gold, decrement a pool, or leave a pending command unless the target is
  fully implemented.
- Regression: tavern hire into an occupied spawn tile cannot partially mutate
  state; exact nonce retry returns a recovered applied result or a stable failed
  command with no leaked effects.
- PocketIC: turn resolution refuses to advance while unrecovered pending or
  applying commands exist.
- Query-budget test: `get_command_status_by_nonce(session_id, command_type,
  client_nonce)` and command-id polling return the same status for applied and
  failed commands after a long play history.
- Receipt test: `event_count` equals the response event list or the field is
  renamed and documented as a narrower domain count.

### Proposed Solution

- Add every mutating gameplay command to a recoverable saga model or reject
  before command creation when unsupported.
- Write deterministic `CommandEffect` rows for each irreversible phase:
  resource spend/refund, pool decrement, champion creation, occupancy creation,
  building creation, event append, and changed subjects.
- Recover commands by replaying effects to completion before any turn
  advancement.
- Validate unsupported target variants before command creation and before
  resource mutation.
- Add `get_command_status_by_nonce(session_id, command_type, client_nonce)` and
  make it use indexed idempotency keys instead of scanning. Preserve legacy
  nonce guessing only as a compatibility fallback outside the v1.1 contract.
- Make failed commands readable by both nonce and command id when the update
  response exposed a command id.
- Normalize receipt counters and command status result payloads.

## 10. Town, Recruitment, Tavern, Dwelling, And Economy

Priority: P0 for no-loss mutations and client parity. P1 for weekly growth and
town economy rules.

### Current Problems

- Town-to-champion recruitment is present in DTO/spec previews but unsupported
  by canister submit.
- Preview and submit disagree for some recruit targets.
- External dwelling recruitment succeeded remotely; v1.1 now documents that as
  intentional, but active/not-in-battle target validation still needs hardening.
- Tavern week-two offers are missing after the week boundary.
- Multiple hired champions can try to spawn on the same town tile.
- Recruit pool weekly growth does not materialize or project consistently in
  town view or preview.
- Build preview can exceed query budget for ordinary next buildings.
- Build validation order can return resource errors before
  `already_built_this_turn`, which may confuse the client.
- Town hall income, capture unrest, unrest reduction, and desperation income
  are deferred to v2; active v1.1 docs and endpoints must stop presenting them
  as mechanical systems.

### Reproducible Tests

- PocketIC: preview and submit for every recruit target variant agree.
- PocketIC: champion-target town recruitment returns `unsupported_recruit_target`
  before command creation, resource spend, pool decrement, or stack mutation.
- PocketIC: direct dwelling recruitment succeeds remotely into an owned active
  world-map champion and rejects non-owned, inactive, defeated, garrisoned, or
  in-battle targets before mutation.
- PocketIC: advance to week 2 and assert tavern offers are generated
  idempotently.
- PocketIC: hire two champions from the same town and assert placement is valid
  and occupancy rows are consistent.
- PocketIC: advance a week and assert recruit pool growth is visible in town
  view and preview.
- Query-budget: preview every visible building candidate after a real play route
  without IC0522.

### Proposed Solution

- Make preview and submit share target validation and resource validation.
- For champion-target town recruitment, reject `RecruitTarget::Champion` before
  command creation, resource spend, pool decrement, or stack mutation.
  `RecruitTarget::Champion` remains a reserved v2 DTO variant.
- Direct external dwelling recruitment is remote in v1.1. If the dwelling pool
  is owned, `direct_recruit = true`, and the target champion is owned, active,
  on the world map, and not in battle, no same-tile or distance check is
  required.
- Implement weekly tavern offer generation in bounded jobs.
- Define champion spawn placement: town garrison/status, nearest free adjacent
  tile, or pre-submit `town_occupied` error.
- Materialize or project weekly recruit growth consistently.
- Move expensive build preview reads onto indexed bounded paths.
- Keep v1.1 active economy to mine income, pickups/rewards, costs, tavern
  hiring, market trade, direct dwelling recruitment, and weekly tavern/recruit
  growth. Town hall income, captured-town unrest mechanics, pacification,
  recruit-pool halving, and desperation income are deferred to v2; any
  `unrest_until_turn` value is non-mechanical capture metadata in v1.1.

## 11. Champion Progression, Spells, And Events

Priority: P1, except event idempotency is P0 where it affects client history.

### Current Problems

- Learned adventure spells are visible in progression preview but not in
  `get_champion_view` or roster rows.
- Recasting the same adventure spell can reuse an old event key, so later casts
  do not appear as new public events.
- Battle spellcasting is not consistently exposed as a legal battle action.
- Event and command payloads are JSON strings inside structured Candid, which
  requires client-specific parsing.

### Reproducible Tests

- PocketIC: learn a spell, then assert `get_champion_view` and allowed roster
  views expose the learned spell slug.
- PocketIC: cast the same adventure spell on two different turns and assert two
  distinct events exist.
- Cast a learned battle spell and verify legal action, resource cost, effects,
  and persistence.
- Client parser test: parse every v1.1 event payload and command-status result
  needed by the UI.

### Proposed Solution

- Hydrate champion spell slugs from `ChampionSpell` rows.
- Include command id, turn, cast sequence, or champion id plus turn in spell
  event keys.
- Provide generated or hand-written public payload schemas for v1.1 event
  payloads and `result_json`.
- Keep battle spellcasting in v1.1 for learned v1.1 spells and move
  retreat/surrender to v2/deferred action metadata.

## 12. Visibility, Event Redaction, And Auth

Priority: P0.

### Current Problems

- Public events can leak hidden opponent town actions even when the opponent
  cannot see the town via `get_town_view`.
- Some visibility/auth behavior works correctly, including hidden champion and
  non-participant scenario access checks.
- Resolved neutral battle visibility now has a locked contract, but the current
  implementation still needs enforcement for uninvolved participants.

### Reproducible Tests

- PocketIC: player two cannot see player one's hidden town build/recruit details
  through public event feed while `get_town_view` returns `not_visible`.
- PocketIC: hidden town events are private, delayed, or redacted according to a
  documented rule.
- PocketIC: registered non-participants are rejected from session-scoped
  scenario/worldgen endpoints with `participant_not_found`.
- PocketIC: resolved neutral battle details are visible only to involved
  participants; uninvolved participants learn outcomes only through redacted
  events or later visible map/object state.

### Proposed Solution

- Align event audience with entity visibility.
- Town build/recruit events are never public detailed events. The owner
  participant receives exact payloads. Other participants receive exact town
  activity only if `get_town_view` would be visible for them at that turn.
  Optional public summaries must omit town id, coordinates, building slug, unit
  slug, quantities, and set `redacted = true`.
- Resolved neutral battle tactical detail is private to involved participants.
  `get_battle_state` returns full active or resolved `BattleView` only to
  participants that owned a battle stack or initiating/defending subject in
  that battle. Uninvolved participants receive `battle_not_visible`; public
  state is limited to redacted summaries and visibility-revealed map/object
  outcomes.
- Add visibility assertions to the public endpoint matrix.

## 13. One-Call Session Setup, Metadata, And Game View Contract

Priority: P0 for one-call setup. P1 for metadata polish.

### Current Problems

- `start_session` is currently a setup saga that may require many repeated
  client calls with fresh nonces before activation.
- That repeated-call model is not acceptable for a playable client. Starting a
  match should be one client update call; the canister should drive setup
  forward through fresh-message continuations: zero-delay timers by default, or
  awaited self-calls/inter-canister calls where that shape is simpler.
- Public responses do not expose enough setup progress.
- `get_session` is intentionally thin; docs and tests must direct active
  gameplay callers to the render metadata and dedicated endpoints.
- Some render-time metadata exists in `get_game_view`, but that view is also a
  shell for most gameplay collections.
- `mark_ready` on an active session returns `session_not_joinable`, which is
  accurate for join but confusing for readiness.

### Reproducible Tests

- PocketIC: one `start_session` call moves the session to `starting`, creates a
  durable setup job, schedules or awaits fresh-message continuation work, and
  eventually reaches `active` without any further client `start_session` calls.
- PocketIC: each setup phase runs in a fresh update message, so an artificial
  low instruction budget or phase cap still completes through zero-delay
  continuations.
- PocketIC: replaying the original `start_session` nonce while setup is
  `starting` returns the original/start-in-progress response and does not
  duplicate setup rows.
- PocketIC: upgrading while setup is `starting` reconstructs the setup job from
  durable IcyDB state and resumes activation.
- Endpoint contract test: call `start_session` once, then poll/subscribe to
  `get_session`, setup progress, and events until `state == "active"`.
- Contract test: `get_game_view`, `get_content_manifest`, setup progress, and
  dedicated endpoints expose current turn, max turns, turn timing, content hash,
  map size, chunk size, and setup state needed by callers.
- Error test: `mark_ready` in active state returns a readiness-specific error.

### Proposed Solution

- Keep setup as an internal saga, but change the public client contract:
  `start_session` is called once by the host. It creates or loads a durable
  `setup_session:{session_id}` `SystemJob`, moves the session to `starting`,
  and starts fresh-message continuation work.
- The preferred continuation is `set_timer(Duration::from_secs(0), ...)`
  because it decouples setup progress from the client response. An awaited
  self-call sequence is also acceptable:
  `start_session -> await setup_phase_1() -> await setup_phase_2() -> ...`,
  but each awaited phase must be a real IC call boundary. A local async
  function that performs only local work does not reset the instruction limit.
- The setup job stores the current phase/cursor in `SystemJob.cursor_json` and
  writes deterministic `CommandEffect` rows for each phase. Each continuation
  runs one bounded setup slice, persists progress, and either schedules another
  zero-delay timer or awaits the next self-call until setup is complete.
- Before every await or timer scheduling boundary, persist the setup phase,
  command/effect status, and cursor. After every continuation, reload session
  and job state from IcyDB and no-op if setup has already advanced or completed.
- Only after all required content, map, town, champion, stack, object,
  occupancy, visibility, scenario, economy, and start-event rows are committed
  may the setup job set `GameSession.state = "active"`.
- `start_session` replay while setup is `starting` must not require a fresh
  client nonce and must not run another phase inline. It returns current setup
  progress or the original command result.
- On `post_upgrade`, repair any `starting` session by upserting
  `setup_session:{session_id}` and scheduling the nearest/zero-delay job.
- Return setup progress in lobby command responses or add a setup progress
  query.
- `get_session` remains a lobby/setup shell in v1.1: session id, state, and
  participants. Gameplay metadata comes from `get_game_view` render metadata,
  `get_content_manifest`, and dedicated map, object, champion, town, battle,
  event, and command-status endpoints.
- Replace ambiguous readiness errors with `session_not_in_lobby`,
  `participant_not_found`, or a similarly specific code.

## 14. Scenario Progress, Victory, And Match History

Priority: P1.

### Current Problems

- Max-turn stalemate scoring is specified but not finalized from turn sync.
- Scenario progress can mark `max_turn_reached` without finalizing the match.
- Match history cannot be verified until victory/finalization paths work.
- Some scenario/world-generation sync endpoints behave like status
  synchronizers and may append events even when the system is disabled or
  future-scoped.

### Reproducible Tests

- PocketIC: advance to max turn and assert winner scoring by towns, mines, army
  power, and seeded tie-break.
- PocketIC: finalization creates match history visible to the right players.
- PocketIC: objective and world-event sync/status endpoints are idempotent and
  do not expose future disabled systems as active gameplay or append public
  gameplay events for disabled-only work.

### Proposed Solution

- Finalize max-turn victory from the backend turn-resolution job or a clearly
  documented scenario job.
- Add deterministic scoring helpers and tests in pure rules and canister tests.
- Ensure match history is created by every victory/finalization path.
- Disabled/future systems are not player-action endpoints in v1.1. Public
  status queries may return disabled rows only with `status = "disabled"`,
  `disabled_reason`, and `actionable = false`; they must not appear in action
  affordances. Future-scope update/sync endpoints are controller/manual-recovery
  only or return `disabled_feature` without appending public gameplay events.

## 15. Diagnostics, IcyDB Evidence, And Query Budgets

Priority: P1, with query-budget blockers promoted to P0 where they affect
normal client play.

### Current Problems

- `get_diagnostic_storage_snapshot` can exceed the local 5B instruction limit
  when called with many entity names.
- Diagnostics do not include all newer persisted entities, such as market,
  tavern, and champion-spell rows.
- Preview and command-status query paths can also exceed instruction limits.
- Local DB inspection currently relies on snapshot/metrics/diagnostics, not
  generic SQL.

### Reproducible Tests

- PocketIC/local: diagnostic snapshot with documented small batches succeeds.
- PocketIC/local: requesting too many diagnostic entities returns a typed
  pagination/limit error instead of trapping.
- Diagnostics inventory test: every persisted entity has a row count endpoint
  or appears in a generic entity inventory.
- Query-budget regression: preview movement, build preview, visible objects,
  command status by nonce, and diagnostic snapshots stay under practical limits
  after a realistic play route.

### Proposed Solution

- Add pagination and lower hard limits to diagnostic snapshots.
- Keep diagnostics in sync with every persisted entity or add a generic entity
  inventory/count endpoint.
- Record query budget expectations in tests or test output.
- Do not promise SQL inspection in v1.1. Snapshot, metrics, and typed
  diagnostics are the local evidence path.

## 16. Documentation And Spec Drift

Priority: P1.

### Current Problems

- README and TESTING still describe PocketIC as a scaffold even though canister
  tests are now meaningful.
- Endpoint docs and active spec text still mention some deferred or disabled
  systems.
- Retreat/surrender are specified in places but disabled in implementation and
  endpoint docs.
- Disabled siege/naval/advanced systems are visible through some public status
  endpoints.

### Reproducible Tests

- Docs test: a fresh developer can run the documented local deployment and
  first-playable smoke without tribal knowledge.
- Endpoint inventory test: disabled/future systems are omitted from the v1.1
  player action surface or include enough metadata to render disabled.
- Spec audit: every active `spec.md` v1.1 rule has implementation and tests, or
  is moved to `spec.v2.md`.

### Proposed Solution

- Update README and TESTING after the local deploy and `blast` smoke route are
  real.
- Keep `spec.md` as the active v1.1 contract and `spec.v2.md` as the future
  backlog.
- Keep retreat/surrender in `spec.v2.md`; v1.1 may expose them only as disabled
  action metadata with explicit disabled reasons.
- Keep disabled systems visible only as debug/status rows or omitted from
  player-facing action surfaces.

## Suggested Implementation Order

1. `SystemJob` and `ParticipantTurnReady` schema plus generated-session tests.
2. Repositories for system jobs, turn readiness, and bounded job queries.
3. Timer runner scaffold, nearest-job scheduling, zero-delay continuation, and
   `init` / `post_upgrade` repair.
4. One-call `start_session` setup using fresh-message continuations.
5. `end_turn` endpoint and early close job creation.
6. Timer-driven turn deadline processing without client `sync_session_turn`.
7. Timer-driven battle timeout processing without client `sync_battle`.
8. Timer/job-driven objective, world-event, and advanced-victory maintenance
   without client `sync_objectives`, `sync_world_events`, or
   `sync_advanced_victory`.
9. Battle-round readiness, `end_battle_turn`, auto-ready, and immediate
   round-advance jobs.
10. Command recovery for build, recruit, tavern hire, dwelling recruit, ledger
   effects, and battle aftermath.
11. Local deploy and `blast` scan/smoke path.
12. Render projection rewrite for champions, objects, mines, piles, and defeated
   neutrals.
13. Movement preview/query-budget and validation parity.
14. Guarded mine aftermath capture and first-playable battle route.
15. Visibility/redaction hardening.
16. Economy/tavern/recruit weekly growth and remaining P1 polish.
17. Docs and final audit.

The first implementation gate is complete when PocketIC proves:

- a turn deadline advances the turn without `sync_session_turn`;
- one `start_session` call reaches `active` through setup continuations without
  repeated client calls;
- all participants ending early closes the turn before 60 seconds;
- a battle timeout auto-defends without `sync_battle`;
- objective progress, current world events, and advanced victory state refresh
  after due gameplay mutations without the scenario `sync_*` endpoints;
- both battle participants ending the battle round advances immediately;
- battle participants with no meaningful stack actions auto-end for the round;
- `post_upgrade` reschedules due work from durable IcyDB state;
- stale or duplicate timer wakeups do not duplicate income, events, movement,
  battle actions, or turn increments.

## Acceptance Definition

Spec 1.1 is complete when a new developer can run the documented local setup,
execute the agent-run local `blast` smoke route through the canister, and
inspect IcyDB evidence showing the same state returned by public read
endpoints. The backend must continue progressing turns, battle timeouts,
objective refresh, world events, and advanced victory checks through IC timers
or other canister-owned continuations, not through client-maintained sync calls.

## Spec 1.1 Todo

This todo is ordered by dependency. Do not advance to a later item if the
current item has failing tests, known spec drift, unsafe recovery behavior, or
client-visible behavior that contradicts `spec.md` or this file.

Every todo item must use this loop:

```text
1. Re-read the relevant `spec.md` and `spec.1.1.md` sections.
2. Add or update focused tests before or with the implementation.
3. Implement the smallest complete vertical slice.
4. Run focused tests for the changed area.
5. Run `make check-canister` and the relevant PocketIC/generated/pure suite.
6. Audit `spec.md` and `spec.1.1.md` for drift; update the specs if the chosen
   implementation clarifies the contract.
7. Record any limitation that still affects playability in `spec.missing.md`.
```

Status note, 2026-05-17: checked items below are implemented or directly
exercised in the current codebase. Named full gate-test rows stay unchecked
unless the named gate has been rerun and recorded. The first-playable local
`blast` route now opens and reads the guarded-object tactical battle; the
remaining route gap is battle resolution, guarded-mine capture, later mine
income, and full regression/PocketIC evidence.

### Gate 1. Durable Job And Readiness Schema

- [x] Add `SystemJob` to `schema/degens/src/schema/entities.rs` with the fields
  and indexes required by this spec: unique `job_key`, `status + due_at`,
  `session_id + status + due_at`, `battle_id + status + due_at`, optional
  `command_id`, lease fields, generation, cursor, and error fields.
- [x] Add `ParticipantTurnReady` for map-turn early readiness.
- [x] Add `BattleParticipantRoundReady` for battle-round readiness.
- [x] Generate/update schema bindings and repository access.
- [x] Add generated-session tests proving create/load/update/page behavior for
  all three entities.
- [x] Audit `spec.md`: entity definitions, indexes, relation strength, field
  defaults, and lifecycle semantics match the generated schema.
- [x] Audit `spec.1.1.md`: Topic 2, Topic 3, Topic 8, and implementation order
  still match the actual schema.
- [x] Gate tests: `make test-generated`, `make check-canister`.

### Gate 2. System Job Repositories And Timer Runner Skeleton

- [x] Add `repos/system_jobs.rs` and `repos/turn_ready.rs`; add battle readiness
  repo support in `repos/battle_round_ready.rs` or the battle repo.
- [x] Add `services/system_jobs.rs` with nearest-job scheduling, one volatile
  timer handle, job claiming, lease expiry, stale-job no-op, and zero-delay
  continuation helpers.
- [x] Add lifecycle repair hooks in `canisters/degens/src/lib.rs` for
  `init`/`post_upgrade` rescheduling.
- [x] Keep timers as wakeups only; every callback must reload IcyDB state before
  mutating.
- [ ] Add PocketIC tests for scheduling, duplicate timer no-op, expired lease
  recovery, and post-upgrade rescheduling.
- [x] Audit `spec.md`: timer/job semantics, idempotency keys, and recovery
  language match the implementation.
- [x] Audit `spec.1.1.md`: Topic 2 and first-gate criteria remain accurate.
- [ ] Gate tests: `cargo test -p domm-pocket-ic-tests --test timer_jobs`,
  `make check-canister`.

### Gate 3. One-Call `start_session`

- [ ] Change `start_session` so the host calls it once. It moves the session to
  `starting`, creates/loads `setup_session:{session_id}`, persists cursor state,
  and starts fresh-message continuation work.
- [ ] Support zero-delay timer continuation as the default.
- [ ] Allow awaited self-call/inter-canister continuation only if every phase is
  a real IC message boundary and persists cursor/effect state before awaiting.
- [ ] Ensure replaying the original `start_session` nonce while setup is
  `starting` does not require fresh nonces and does not duplicate setup rows.
- [ ] Add setup progress to the lobby response or a dedicated setup progress
  query.
- [ ] Add PocketIC tests proving one call reaches `active`, artificial phase
  caps continue through fresh messages, replay is idempotent, and upgrade during
  `starting` resumes.
- [ ] Audit `spec.md`: session setup saga, setup phase list, row caps, and
  public API contract all describe one client call.
- [ ] Audit `spec.1.1.md`: Topic 13 and first-gate criteria no longer mention
  repeated client-driven setup.
- [ ] Gate tests: targeted setup tests, `make test-pocket`,
  `make check-canister`.

### Gate 4. Map `end_turn` And Early Turn Closure

- [x] Add `end_turn(session_id, client_nonce) -> CommandResponse`.
- [x] Write `ParticipantTurnReady` idempotently for the caller/current turn.
- [x] Keep map `end_turn` as a readiness marker, not a lock: ended players may
  continue to act while the same map turn is open.
- [x] When all active map participants are ready, enqueue or run
  `turn_resolution:{session_id}:{turn_number}` immediately.
- [x] Reject old-turn commands once turn resolution has been accepted.
- [ ] Add PocketIC tests for ended-player-still-acts, final participant closes
  before 60 seconds, old-turn stale rejection, and active battle participants
  still ending the map turn.
- [x] Audit `spec.md`: `ParticipantTurnReady`, public API shape,
  `CommandResult::EndTurn`, turn advancement rule, and map/battle separation
  agree with code.
- [x] Audit `spec.1.1.md`: Topic 3 and first-gate criteria are complete.
- [ ] Gate tests: `cargo test -p domm-pocket-ic-tests --test end_turn`,
  `make check-canister`.

### Gate 5. Timer-Driven Turn Resolution

- [x] Split `sync_session_turn` into a manual recovery wrapper over an internal
  `process_turn_resolution_slice`.
- [x] Split `sync_objectives`, `sync_world_events`, and
  `sync_advanced_victory` into manual recovery wrappers over internal scenario
  maintenance job runners.
- [x] Schedule `turn_deadline:{session_id}:{turn_number}` on activation and on
  every new turn.
- [x] Add scenario job keys and runners:
  `scenario_objectives:{session_id}:{turn_number}`,
  `world_events:{session_id}:{turn_number}`, and
  `advanced_victory:{session_id}:{turn_number}`.
- [x] Resolve pending movement, pickups, visibility, income, victory checks, and
  event/effect writes through deterministic system commands.
- [x] Trigger scenario jobs from real gameplay mutations: turn close,
  battle aftermath, guarded-object capture, quest accept/claim,
  town/mine/object ownership changes, and max-turn checks.
- [x] Ensure scenario maintenance is idempotent: direct wrapper calls, stale
  timers, duplicate timers, and zero-delay continuations do not emit duplicate
  objective/world/victory events or mutate rows twice.
- [x] Use cursor state and zero-delay continuations when the slice approaches
  budget.
- [x] Recover or quarantine pending/applying commands that can affect turn
  closure before advancing.
- [ ] Add PocketIC tests proving a 60-second deadline advances without
  `sync_session_turn`, multi-step movement finishes through continuations, and
  duplicate/stale timer fires do not duplicate effects.
- [ ] Add PocketIC tests proving objectives, world events, and advanced victory
  refresh without calling `sync_objectives`, `sync_world_events`, or
  `sync_advanced_victory`.
- [x] Audit `spec.md`: turn advancement, sync semantics, recovery-before-turn,
  scenario maintenance, and query/no-speculation rules match code.
- [x] Audit `spec.1.1.md`: Topics 2 and 4 reflect the implemented runner.
- [ ] Gate tests: `timer_jobs`, movement/turn tests, `make test-pocket`.

### Gate 6. Timer-Driven Battle Timeout And Battle Round Readiness

- [x] Split `sync_battle` into a manual recovery wrapper over internal battle
  timeout/round jobs.
- [x] Schedule `battle_timeout:{battle_id}:{deadline_ms}` whenever an action
  deadline is created or advanced.
- [x] Add `end_battle_turn(session_id, battle_id, client_nonce)`.
- [x] Write `BattleParticipantRoundReady` for player end-turn and auto-ready
  reasons.
- [x] Auto-ready participants whose living stacks have all acted, are disabled,
  or have no enabled meaningful action.
- [x] When all alive battle participants are ready, run
  `battle_round_advance:{battle_id}:{round_number}` immediately.
- [x] Ensure battle `end_battle_turn` is a commitment: remaining eligible stacks
  are deterministically skipped/defended and old-round actions fail stale.
- [ ] Add PocketIC tests for timer auto-defend without `sync_battle`,
  zero-delay timeout catchup, both players ending battle round, auto-ready, and
  resolved battle no-op replay.
- [x] Audit `spec.md`: `BattleParticipantRoundReady`, `end_battle_turn`,
  `EndBattleTurn`, battle DTO readiness fields, and battle timeout rules match.
- [x] Audit `spec.1.1.md`: Topic 8 and first-gate criteria match code.
- [ ] Gate tests: `timer_jobs`, `battle_round_readiness`, battle service tests,
  `make check-canister`.

### Gate 7. Command Recovery Hardening

- [x] Add build, recruit, tavern hire, dwelling recruit, and any missing economy
  commands to recoverable saga handling or reject unsupported paths before
  command creation.
- [x] Reject champion-target town recruitment before command creation so
  unsupported targets cannot spend, decrement pools, or leave pending commands.
- [x] Fix tavern hire placement/recovery so occupancy failure cannot leak
  champion, offer, or resource mutations.
- [x] Make battle aftermath and guarded-object capture idempotent per
  battle/session, not per incidental sync command.
- [x] Add `get_command_status_by_nonce(session_id, command_type, client_nonce)`
  using an indexed idempotency lookup.
- [x] Add trap/retry tests for partial ledger writes, build, recruit, tavern
  hire, dwelling recruit, and pending command turn-blocking.
- [x] Audit `spec.md`: command lifecycle, ledger recovery, effect markers, and
  idempotency rules are fully implemented or explicitly deferred.
- [x] Audit `spec.1.1.md`: Topic 9 no longer lists already-fixed P0 recovery
  gaps.
- [ ] Gate tests: `command_recovery`, town/economy service tests,
  `make regression`.

### Gate 8. Local Deploy And `blast` Smoke

- [x] Add committed local deployment path: root `dfx.json`, DID/metadata build
  flow, install commands, and cleanup commands.
- [x] Ensure local/release wasm includes public Candid metadata without manual
  wasm patching.
- [x] Before any deeper `blast` gameplay checks, deploy a fresh local canister
  with DFX and generated metadata, then run `blast scan <canister_id> --host
  http://127.0.0.1:4943` directly from the agent shell.
- [x] Compare the direct `blast scan` output with generated Candid and
  `get_canister_endpoint_inventory`; record any missing endpoint in
  `spec.missing.md`.
- [ ] Use direct `blast call ... --id 1`, `--id 2`, and when needed `--id 3`
  commands to register, create, join, ready, start once, wait for active, move,
  collect, build, recruit, fight guarded mine, capture, income, and read
  diagnostics.
  - 2026-05-17 direct local `blast` evidence now reaches guarded battle open:
    fresh scan exposed 63 methods, setup reached `active` at `start:9`,
    guarded preview returned cost `30` with `guarded_object`, sync slices
    `0..8` returned `movement_sync_incomplete`, sync `9` returned
    `neutral_encounter_pending` plus `session_turn_synced`, and
    `get_battle_state` returned an `active` neutral battle with legal actions.
    Follow-up direct local evidence from session `01KRTT8MHY0000000000000008`
    resolved battle `01KRTTH6XV0000000000000004`, emitted `mine_captured`,
    `neutral_defeated`, `battle_aftermath_applied`, and later
    `income_materialized` with `{"gold":250}`, rendered `mine:west-gold` as
    owned/captured, and reported `icydb_snapshot` corruption counts at zero.
    Keep this item open because this route still skips the full
    collect/build/recruit walkthrough and one-call setup contract.
- [x] Do not create committed `blast` scripts unless explicitly requested; keep
  the `blast` evidence as copied command lines and observed outputs.
- [x] Audit `spec.md`: public API shape and endpoint inventory agree with
  `blast scan`.
- [x] Audit `spec.1.1.md`: Topic 1 and test classes point to the real local
  deploy path and agent-run `blast` command checklist.
- [ ] Gate evidence: direct local `blast` smoke plus `icydb_snapshot`
  corruption checks.

### Gate 9. Render Projection Truth

- [x] Rework `get_visible_objects` to hydrate from live `Champion`,
  `WorldObject`, `MapOccupancy`, object visits, neutral state, town state, and
  battle state.
- [x] Keep `ParticipantKnownObject` as discovery memory only.
- [x] Materialize newly discovered objects when movement/visibility changes.
- [x] Add `GameView.omitted_fields`; omitted collections must report
  `has_more = false` and `next_cursor = None`.
- [x] Return typed `ObjectDetails` as the primary object-detail contract and
  add/document `get_object_view` for selected map object details.
- [x] Document surveyed-base-map fog: static terrain/movement/flags may be
  returned for undiscovered chunks, but dynamic state stays visibility-gated.
- [x] Ensure consumed piles disappear or render consumed, defeated neutrals do
  not render active, captured mines render owned/captured, and champion
  coordinates match `get_champion_view`.
- [x] Return stable page metadata and compatible cursor behavior.
- [ ] Add PocketIC tests after movement, pickup, battle victory, mine capture,
  income, and discovery.
- [x] Audit `spec.md`: bounded render endpoint contract, fog/redaction, object
  details, and pagination match code.
- [x] Audit `spec.1.1.md`: Topic 6 is reduced to remaining non-P0 work.
- [ ] Gate tests: `render_projection`, visibility service tests,
  `make test-pocket`.

### Gate 10. Movement Preview And Validation Parity

- [x] Move `preview_move_path` onto the same bounded validation rules as submit
  and execution.
- [x] Enforce surveyed-base-map fog, path caps, chunk caps, terrain, blockers,
  dynamic-state redaction, and occupancy consistently.
- [x] Return stop/blocker reasons for pickups, guards, towns, enemy blockers,
  and occupied friendly tiles.
- [x] Add query-budget guardrails so representative 1, 2, 8, and 64 step paths
  do not trap after realistic play history.
- [x] Enforce the v1.1 late-submit contract around accepted turn-resolution
  jobs: old-turn commands fail before command creation with
  `backend_work_pending` while closing work is active or `turn_expired` after
  advancement.
- [x] Audit `spec.md`: movement rules, query-budget expectations, and error
  codes match implementation.
- [x] Audit `spec.1.1.md`: Topic 7 reflects the final late-submit contract.
- [ ] Gate tests: `query_budgets`, movement service tests, pure movement tests.

### Gate 11. Guarded Mine And First-Playable Battle Route

- [x] Ensure moving onto the guarded mine creates the real tactical battle the
  client opens.
- [x] Ensure defeating the guard captures the mine in battle aftermath; no
  artificial step away/back or explicit post-battle interaction is allowed.
- [x] Ensure later turn resolution produces mine income from the captured mine.
- [x] Ensure battle aftermath updates champion status/position, occupancy,
  object ownership, visibility, events, and victory checks idempotently.
  - 2026-05-17 direct local `blast` evidence verified the main aftermath
    projection path: champion `01KRTTABXS0000000000000002` returned active at
    `(12,22)`, `get_visible_objects` no longer rendered the defeated neutral on
    the guarded tile, `mine:west-gold` rendered with owner
    `01KRTT8MHY000000000000000A` and `state:"captured"`, and the public event
    feed included `mine_captured`, `neutral_defeated`,
    `battle_aftermath_applied`, and `income_materialized`. Follow-up Gate L
    PocketIC evidence on 2026-05-17 verified champion active position after the
    neutral battle, captured mine owner/state, defeated-neutral render absence,
    exactly one neutral aftermath/capture event set, resolved `sync_battle`
    no-op idempotency, final map occupancy/storage diagnostics, and
    `victory_finalized` on the full first-playable route.
- [x] Add PocketIC first-playable route assertions for guarded mine battle,
  capture, income, and no stale render state.
  - 2026-05-17 implementation added guarded-route assertions for
    `mine_captured`, `neutral_defeated`, `battle_aftermath_applied`, resolved
    `sync_battle` no-op idempotency, defeated-neutral render absence,
    captured-mine owner/state, and final `income_materialized`. Clean evidence:
    `cargo test -p domm-pocket-ic-tests --test canister_endpoints
    pocket_ic_gate_l_first_playable_canister_e2e_uses_public_endpoints_and_icydb_state`
    passed on 2026-05-17 with 108 updates, 130 queries, 214 observed events,
    122 command rows, 146 event rows, and final diagnostics covering battles,
    movement snapshots, resource ledger summaries, object visits, objectives,
    world events, town/garrison rows, world objects, and neutrals.
- [ ] Audit `spec.md`: battle start, aftermath, object capture, income start,
  and victory sections agree.
- [ ] Audit `spec.1.1.md`: Topic 8 and Topic 10 no longer list the guarded mine
  route as under-proven.
- [ ] Gate tests: guarded route PocketIC test, battle/aftermath pure tests,
  `make regression`.

### Gate 12. Visibility, Redaction, And Auth Hardening

- [x] Make hidden town build/recruit events audience-scoped or redacted
  consistently with `get_town_view` visibility.
- [x] Enforce resolved neutral battle visibility: full tactical detail is
  visible only to involved participants; uninvolved participants get
  `battle_not_visible` and only redacted/public map outcomes.
- [ ] Add non-participant and opponent visibility tests across scenario,
  worldgen, events, champions, towns, battles, and objects.
- [x] Audit `spec.md`: fog/redaction and event audience rules match code.
- [x] Audit `spec.1.1.md`: Topic 12 has no remaining P0 ambiguity.
- [ ] Gate tests: `visibility_redaction`, endpoint auth matrix tests.

### Gate 13. Economy, Tavern, Recruitment, Spells, And Victory Polish

- [x] Reject champion-target town recruitment in preview and submit before
  command creation; keep `RecruitTarget::Champion` reserved for v2.
- [x] Document and test remote direct dwelling recruitment into owned active
  world-map champions; reject invalid targets before spend/pool mutation.
- [x] Implement week-two tavern offers and recruit growth projection/materialization.
- [x] Hydrate learned spells in champion views and fix repeated spell event
  idempotency.
- [x] Implement max-turn finalization and match-history creation.
- [x] Audit `spec.md`: economy, tavern, recruitment, spell, and victory rules
  are active only where implemented.
- [x] Audit `spec.1.1.md`: Topics 10, 11, and 14 match the locked v1.1
  decisions and any deferred mechanics are in `spec.v2.md`.
- [ ] Gate tests: focused pure/canister tests plus `make regression`.

### Gate 14. Final Spec And Playability Audit

- [ ] Run `make regression`, `make check-canister`, `make test-pocket`,
  generated-session tests, and the agent-run local `blast` smoke checklist.
- [ ] Audit every active `spec.md` v1.1 rule against implementation and tests.
- [ ] Audit every topic in `spec.1.1.md` and mark remaining work as fixed,
  deferred to `spec.v2.md`, or still blocking in `spec.missing.md`.
- [x] Update README, TESTING, docs/canister-endpoints, and local deployment
  instructions.
- [x] Confirm generic SQL is not used for public gameplay paths.
- [ ] Confirm public read endpoint state matches IcyDB diagnostics after the
  first-playable route.
- [ ] Final gate: a new developer can deploy locally, start once, play through
  the first playable route, and verify the same state through IcyDB evidence.
