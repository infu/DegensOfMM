# Full Spec Expansion Triage

Historical note: this checkpoint-21 triage is no longer the active v1 todo.
The authoritative backlog for non-v1 systems is `spec.v2.md`.

Checkpoint 21 classifies the Part 1 systems that were deliberately left out of
the first playable canister/IcyDB implementation.

## Result

No deferred Part 1 system is approved for immediate runtime implementation.
Every candidate system must first land a bounded design and then be
implemented through typed IcyDB entities, domain repositories, public Candid
endpoints, deterministic command/recovery flows, and Pocket-IC coverage.

## Classification Summary

| Classification | Systems |
| --- | --- |
| implement-now | None. The first playable is complete at Gate N, and adding Part 1 behavior without new Part 2 gates would invalidate the canister/IcyDB contract. |
| promote-to-Part-2-first | Champion skills, level-up choices, spell learning, spellcasting, mana, advanced statuses, taverns, marketplaces, external dwellings, advanced economy, quests, scenario objectives, world events, advanced victories, siege, naval movement, seeded procedural maps, skirmish settings, diplomacy, rematch, ranked, guilds, campaign persistence, and broader history/social systems. Checkpoints 22-25 have implemented the first bounded progression/magic, expanded-economy, scenario-progress, and world-generation-boundary slices; remaining variants still need their own bounded specs. |
| still-deferred or removed | Sequential player turns, hotseat-only backend flow, monolithic `GameState` persistence, unrestricted generic SQL gameplay paths, unbounded full bot opponents, unbounded content packs, and any system without numeric caps and indexed lookup paths. |

## Triage Table

| Part 1 system | Classification | Destination |
| --- | --- | --- |
| Bounded champion progression/magic slice: level cap, spell learning, adventure casting, learned battle `CastAbility`, mana/status persistence, and unsupported-effect responses | promoted and implemented | Checkpoint 22 |
| Broader champion skill trees, skill ranks, advanced status/dispel/stacking rules, and artifact-set-style effect expansion | promote-to-Part-2-first | Future champion expansion |
| Tavern hiring, marketplace trading, external dwellings, and direct map recruitment | promoted and implemented | Checkpoint 23 |
| Defeated champion reappearance, town/building income effects, marketplace ownership rate improvements, advanced economy buildings, and broader resource-source variety | promote-to-Part-2-first | Future economy expansion |
| Central objective tracking, one opening quest, weekly world events, quest reward claim, and typed scenario rules | promoted and implemented | Checkpoint 24 |
| Quest huts, quest chains, monthly world events, artifact victory, king-of-the-hill, survival, scenario-specific defeat, and richer scenario rules beyond the disabled row contract | promote-to-Part-2-first | Future scenario expansion |
| Skirmish settings, deterministic first-playable procedural preview metadata, and disabled naval/siege/larger-map boundary rows | promoted and implemented | Checkpoint 25 |
| Active boats, water movement layers, naval map gameplay, complex siege engines, walls/gates/towers, siege actions, and larger procedural map materialization | promote-to-Part-2-first | Future world-generation expansion |
| Diplomacy, ranked leaderboard, guilds, campaign carryover, campaign persistence, rematch creation, broader match history, social/meta systems | promote-to-Part-2-first | `spec.v2.md` |
| Full bot opponents and general strategic planners | still-deferred | Allowed only by a later AI-specific Part 2 section. V1 keeps neutral battle behavior and bounded autopilot-style command generation only. |
| Sequential player turns and hotseat-only backend rules | removed from implementation scope | Superseded by simultaneous timed turns. Multiple local players can use normal account/session flows. |
| Single serialized `GameState` row | removed from implementation scope | Superseded by durable IcyDB entities, command/effect rows, and event rows. |
| Generic SQL gameplay access | removed from implementation scope | SQL remains controller/test/diagnostic only and must not power public gameplay endpoints. |
| Unbounded content expansions | still-deferred | Needs a content-pack spec with caps, migration policy, content hashes, and canister budget tests. |

## Promotion Gates

Before any promoted bucket can start runtime implementation, the corresponding
V2 subsection must be defined in `spec.v2.md` and then promoted back into
`spec.md` only if it becomes part of the v1 implementation contract. The design
must include all of the following:

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
| 22 | Implemented the bounded progression/magic slice using existing `ChampionSpell`, `SpellDefinition`, `BattleStack.status_keys`, artifact rows, and effect keys where sufficient. Future champion expansion still needs a fresh bounded spec for full skill trees, skill ranks, advanced statuses, dispel/stacking, and artifact-set-style effects. |
| 23 | Implemented tavern offers, hire records, market/trade rows, dwelling pools, indexes by session/week/participant/town/object/offer/command, hire/trade/dwelling preview and update endpoints, and caps for offers, trade amounts, pool growth, and visible candidates. |
| 24 | Implemented objective progress, quest state, world event state, scenario rule state, indexed lookup paths by session/participant/key/window/victory state, quest accept/claim, objective sync, world-event sync, advanced-victory sync, and caps for active quests, objective rows, event rows, rule rows, and victory checks. |
| 25 | Implemented skirmish settings, deterministic first-playable procedural preview metadata, and explicit disabled naval/siege boundary rows. Added indexes by session, generation key, status, route key, and rule key; added skirmish/procedural/naval/siege query endpoints plus `sync_world_generation`; capped generated dimensions, generated chunks per update, water crossings, route/rule rows, and siege battle obstacles. Active boat movement, siege actions, and larger map materialization remain future world-generation expansion work. |
| V2 meta backlog | Define rematch, campaign, leaderboard, guild, diplomacy, and expanded history rows in `spec.v2.md` before implementation. Add indexes by player, principal, session, season, guild, campaign, rank bucket, and history cursor. Add privacy-preserving endpoints for rematch, campaign, ranking, guild, diplomacy, and history flows. Cap rows per player/guild/season, page sizes, retention windows, and hot gameplay writes. |

Future world-generation runtime work is explicitly blocked until a new
bounded spec subsection exists for the exact slice. Siege-engine work must
define engine ownership, placement, ammunition/durability, targeting,
wall/gate/tower/breach state, action DTOs, indexes, recovery, cleanup,
deterministic damage keys, caps, and Pocket-IC budgets before code starts.
Naval and larger-map work have the same requirement for boat occupancy,
water movement, generation jobs, chunk materialization, manifest/version
contracts, visibility fan-out, paging, and public endpoint coverage.

## Audit Finding

Checkpoint 21 found no silently implemented Part 1 system outside the first
playable scope. Deferred systems are represented as content omissions, typed
disabled responses, or future checkpoints rather than partial canister behavior.
