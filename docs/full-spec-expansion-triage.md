# Full Spec Expansion Triage

Checkpoint 21 classifies the Part 1 systems that were deliberately left out of
the first playable canister/IcyDB implementation.

## Result

No deferred Part 1 system is approved for immediate runtime implementation.
Every candidate system must first land a bounded Part 2 design and then be
implemented through typed IcyDB entities, domain repositories, public Candid
endpoints, deterministic command/recovery flows, and Pocket-IC coverage.

## Classification Summary

| Classification | Systems |
| --- | --- |
| implement-now | None. The first playable is complete at Gate N, and adding Part 1 behavior without new Part 2 gates would invalidate the canister/IcyDB contract. |
| promote-to-Part-2-first | Champion skills, level-up choices, spell learning, spellcasting, mana, advanced statuses, taverns, marketplaces, external dwellings, advanced economy, quests, scenario objectives, world events, advanced victories, siege, naval movement, seeded procedural maps, skirmish settings, diplomacy, rematch, ranked, guilds, campaign persistence, and broader history/social systems. Checkpoints 22-24 have implemented the first bounded progression/magic, expanded-economy, and scenario-progress slices; remaining variants still need their own bounded specs. |
| still-deferred or removed | Sequential player turns, hotseat-only backend flow, monolithic `GameState` persistence, unrestricted generic SQL gameplay paths, unbounded full bot opponents, unbounded content packs, and any system without numeric caps and indexed lookup paths. |

## Triage Table

| Part 1 system | Classification | Destination |
| --- | --- | --- |
| Champion skill trees, level-up choices, skill ranks, spell learning, battle spellcasting, adventure spellcasting, mana reset rules, advanced statuses, dispel/stacking, artifact-set-style effect expansion | promote-to-Part-2-first | Checkpoint 22 |
| Tavern hiring, marketplace trading, external dwellings, and direct map recruitment | promoted and implemented | Checkpoint 23 |
| Defeated champion reappearance, advanced economy buildings, and broader resource-source variety | promote-to-Part-2-first | Future economy expansion |
| Central objective tracking, one opening quest, weekly world events, quest reward claim, and typed scenario rules | promoted and implemented | Checkpoint 24 |
| Quest huts, quest chains, monthly world events, artifact victory, king-of-the-hill, survival, scenario-specific defeat, and richer scenario rules beyond the disabled row contract | promote-to-Part-2-first | Future scenario expansion |
| Complex siege engines, walls/gates/towers, naval maps, boats, water movement layers, seeded procedural generation, skirmish settings, larger map variants | promote-to-Part-2-first | Checkpoint 25 |
| Diplomacy, ranked leaderboard, guilds, campaign carryover, campaign persistence, rematch creation, broader match history, social/meta systems | promote-to-Part-2-first | Checkpoint 26 |
| Full bot opponents and general strategic planners | still-deferred | Allowed only by a later AI-specific Part 2 section. V1 keeps neutral battle behavior and bounded autopilot-style command generation only. |
| Sequential player turns and hotseat-only backend rules | removed from implementation scope | Superseded by simultaneous timed turns. Multiple local players can use normal account/session flows. |
| Single serialized `GameState` row | removed from implementation scope | Superseded by durable IcyDB entities, command/effect rows, and event rows. |
| Generic SQL gameplay access | removed from implementation scope | SQL remains controller/test/diagnostic only and must not power public gameplay endpoints. |
| Unbounded content expansions | still-deferred | Needs a content-pack spec with caps, migration policy, content hashes, and canister budget tests. |

## Promotion Gates

Before any promoted bucket can start runtime implementation, the corresponding
checkpoint must update `spec.md` with all of the following:

| Gate | Required design detail |
| --- | --- |
| IcyDB schema | New entities, fields, relation strength, defaults, append-only migration behavior, and whether existing rows are sufficient. |
| Indexes | Every hot lookup path, unique/idempotency key, pagination order, cleanup path, and visibility/recovery lookup. |
| Commands and endpoints | Public update/query method names, typed Candid input/output DTOs, preview endpoints, disabled responses, and ownership checks. |
| Recovery and idempotency | `GameCommand`, `CommandEffect`, `PendingEffect`, event keys, retry behavior, partial-application resume order, and budget exhaustion behavior. |
| Deterministic pseudo-random keys | Explicit domain keys and input fields. Gameplay may not use IC raw randomness, wall-clock elapsed time, row order, or mutable RNG cursors. |
| Numeric caps | Per-session, per-turn, per-participant, per-object, per-query, and per-update caps with fail-closed error behavior. |
| DTOs and frontend | Render-ready public views, legal action affordances, disabled reasons, pagination/cursor contracts, redaction rules, and client retry/sync expectations. |
| Tests | Pure unit tests, schema/macro tests, generated-session or repository tests, endpoint inventory tests, and Pocket-IC e2e coverage for every public endpoint in the bucket. |
| Cleanup and retention | Strong/weak relation cleanup order, summary rows, raw log retention, active-session protection, and bounded retry behavior. |

## Bucket-Specific Minimums

| Checkpoint | Required Part 2 additions before code |
| --- | --- |
| 22 | Define skill/spell/status entities or prove existing `ChampionSpell`, `SpellDefinition`, `BattleStack.status_keys`, artifact rows, and effect keys are sufficient. Add indexes by session, champion, participant, battle, skill key, spell key, and status key. Add commands such as level-up choice, spell learning, battle cast, adventure cast, and previews. Cap skill options, spellbook size, casts per turn/round, status instances, and effect targets. |
| 23 | Implemented tavern offers, hire records, market/trade rows, dwelling pools, indexes by session/week/participant/town/object/offer/command, hire/trade/dwelling preview and update endpoints, and caps for offers, trade amounts, pool growth, and visible candidates. |
| 24 | Implemented objective progress, quest state, world event state, scenario rule state, indexed lookup paths by session/participant/key/window/victory state, quest accept/claim, objective sync, world-event sync, advanced-victory sync, and caps for active quests, objective rows, event rows, rule rows, and victory checks. |
| 25 | Define map-generation jobs, generated content manifests, water/boat occupancy, siege objects, fortification state, and skirmish settings. Add indexes by session, generation step, chunk, object, occupant, battle, and scenario hash. Add generation, boat movement, siege action, and skirmish creation endpoints. Cap map dimensions, generated chunks per update, path length, water crossings, siege objects, battle obstacles, and visibility fan-out. |
| 26 | Define rematch, campaign, leaderboard, guild, diplomacy, and expanded history rows. Add indexes by player, principal, session, season, guild, campaign, rank bucket, and history cursor. Add privacy-preserving endpoints for rematch, campaign, ranking, guild, diplomacy, and history flows. Cap rows per player/guild/season, page sizes, retention windows, and hot gameplay writes. |

## Audit Finding

Checkpoint 21 found no silently implemented Part 1 system outside the first
playable scope. Deferred systems are represented as content omissions, typed
disabled responses, or future checkpoints rather than partial canister behavior.
