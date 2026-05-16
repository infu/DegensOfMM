# First Playable Final Audit

Checkpoint 20 audits the implemented first-playable scope against `spec.md`
Part 2 and the canister/IcyDB gates added in checkpoints 19A-19K.

## Gate N Result

Status: pass. `make regression` completed successfully for checkpoint 20.

The first playable production backend is `canisters/degens`. Public gameplay
uses typed Candid endpoints backed by IcyDB rows in `schema/degens`; the pure
`domm-game` fixture remains the deterministic rules, DTO, and fixture contract
layer.

## Implemented Scope

| Area | Audit result |
| --- | --- |
| IcyDB schema and repositories | Implemented with typed domain repositories, indexed hot-path plan tests, relation/deletion tests, and generic SQL scans limited to diagnostics. |
| Public canister endpoints | 28 required 19A endpoints were exported in Candid, inventoried, and called by Pocket-IC tests at Gate N. Checkpoint 22 expands the live inventory to 32 with champion progression and magic endpoints. |
| Lobby/session/setup | IcyDB-backed registration, create/join/ready/start, setup phases, setup recovery, participant rows, and match-history shells are implemented. |
| Content/opening views | First-playable content, map, visibility, objects, champions, towns, neutrals, economy, and opening viewport projections are persisted and queryable. |
| Strategic gameplay | Movement intents, sync slicing, resource pickup, mine capture/income, building, recruitment, object visits, visibility, and battle handoff persist IcyDB rows. |
| Battle/aftermath/victory | Battle views/actions/sync, timeout commands, neutral defeat, champion defeat, town capture, victory, summaries, and history persist IcyDB rows. |
| Web/client contract | Fixture-backed and canister-backed client probes complete the first-match route through result/history panels. |
| Idempotency and recovery | Command nonce replay, payload mismatch, setup recovery, movement sync recovery, battle action retry, and timeout sync recovery are covered. |
| Query contracts | Render/query endpoints enforce limits and remain projection/read surfaces; update endpoints own materialization and recovery. |
| Determinism | Gameplay pseudo-randomness uses explicit keyed helpers; canister public time-sensitive endpoints derive time at the boundary rather than trusting caller time. |
| Performance and storage | Pocket-IC gates record update/query counts, response sizes, selected row growth, and stable-memory observations. |

## Audit Fixes In Checkpoint 20

- Removed public `now_ms` Candid arguments from movement and battle endpoints.
- Switched Pocket-IC tests to advance Pocket-IC time for turn and battle deadlines.
- Removed server time from time-sensitive command idempotency payloads.
- Added native guard tests for caller-controlled time arguments, fixture/placeholder backend calls, and `now_ms` payload drift.
- Removed the stale canister service placeholder file.

## Deferred Scope

The following Part 1 systems remain deferred until checkpoints 21-27 promote
them into bounded Part 2 specs: champion skill-tree choices, large spellbooks,
advanced quests, naval movement, complex siege, procedural maps, taverns,
marketplaces, external dwellings, ranked, guilds, diplomacy, campaign
persistence, and durable rematch creation. Checkpoint 21 records the
authoritative classification in `docs/full-spec-expansion-triage.md`.

## Known Follow-Up

Hard rejection of late `submit_move_intent` calls remains a separate contract
cleanup. The public API no longer trusts caller time, but the current playable
route still uses the existing sync-driven turn submission behavior.
