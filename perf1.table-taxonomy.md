# perf1 Kernel Table And Index Taxonomy

This file records the Section 72D ownership decisions for IcyDB tables while the
gameplay kernels move live simulation state to heap/stable runtime snapshots.

## Direct IcyDB Authority

These entities stay directly authoritative in IcyDB. They are not planned for
live-kernel ownership because they are identity, static content, diagnostics, or
finished-history surfaces rather than rule-heavy active simulation state.

| Entity or surface | Authority decision | Boundary |
| --- | --- | --- |
| `PlayerAccount` | Direct IcyDB authority | Account identity, principal uniqueness, display/profile data, and player lookup remain durable row concerns. Heap caches may accelerate reads but must mirror or invalidate from durable writes. |
| Account/profile identity | Direct IcyDB authority | Usernames, display names, account principals, and player-facing identity metadata stay durable and queryable outside gameplay kernels. |
| Static content definitions | Direct IcyDB authority | Rulesets, factions, units, buildings, spells, artifacts, objectives, scenario templates, and map/content definitions stay immutable or versioned durable content. Kernels read this content but do not own it. |
| Controller/admin diagnostics | Direct IcyDB authority | Controller-gated diagnostic rows and row-count/index-plan surfaces remain direct storage observability. Runtime diagnostics may summarize kernels, but they do not replace durable admin state. |
| Durable finished match summaries | Direct IcyDB authority | `PlayerMatchSummary` and retained match-history rows stay durable history after a session finishes. Runtime caches may suppress empty reads but must clear before finished-history reads. |
| Lobby/session admission before active gameplay | Direct IcyDB authority until activation | Lobby creation, join, ready, seed/config selection, and admission checks may use caches, but durable `GameSession`/`GameParticipant` rows remain the authority before a session enters active runtime gameplay. |

## Guardrails

- Direct-authority rows can use caches only as mirrors, negative caches, or
  replay helpers; durable rows remain the source of truth for conflicts and
  recovery.
- Direct-authority tables should not be moved into `SessionTurnRuntime` or
  `BattleRuntime` just to reduce a setup/admin read unless a benchmark proves
  they are part of active simulation cost.
- Runtime kernels may reference direct-authority IDs and content snapshots, but
  must not make these tables projection-only without a separate Section 72D
  decision.

## Active Worldmap Projection And History

These entities are projection/history-only while a session has an active
worldmap kernel. `SessionTurnRuntime` and the worldmap kernel facade own live
simulation authority; durable rows remain useful for setup, adoption, explicit
flushes, post-upgrade recovery, diagnostics, history, and row-backed fallback.

| Entity or surface | Active-session authority | Durable-row role |
| --- | --- | --- |
| `GameSession` turn fields | Worldmap kernel | Projection of active turn, phase, deadlines, next-event sequence, winner/finish state, and scenario counters. Lobby/config fields remain direct authority until activation. |
| `GameParticipant` resources/readiness | Worldmap kernel | Projection of active resources, turn readiness, defeat/finish state, and per-turn derived values. Admission/player identity fields remain direct authority before activation. |
| `ParticipantTurnReady` | Worldmap kernel | Compatibility/history projection for readiness; active readiness should be runtime-owned. |
| `MovementIntent` | Worldmap kernel | Command/history projection for submitted movement, replay/debug, and row-backed recovery. Active pending movement belongs to runtime. |
| `MovementSnapshot` | Worldmap kernel | History/debug projection of resolved movement steps. Active path progress and blockers belong to runtime. |
| `CommandEffect` | Worldmap kernel or boundary service | Projection/history for applied command side effects. Use as an idempotency/history surface, not active simulation authority, except for direct-authority boundary commands. |
| `GameCommand` | Worldmap kernel or boundary service | Replay/status/history projection. Active gameplay commands should prefer runtime receipts where implemented. |
| `GameEvent` | Worldmap kernel or boundary service | Feed/history projection. Active gameplay events should be emitted from runtime buffers and flushed explicitly. |
| `ResourceLedgerEntry` | Worldmap kernel | Accounting/history projection for resource deltas; active balances belong to runtime. |
| `ObjectiveProgress` | Worldmap kernel | Scenario progress projection; active objective state belongs to runtime scenario state. |
| `QuestState` | Worldmap kernel | Quest progress projection; active quest state belongs to runtime scenario state. |
| `WorldEventState` | Worldmap kernel | Scenario/world event projection; active schedule/effects belong to runtime scenario state. |
| `ScenarioRuleState` | Worldmap kernel | Victory/objective rule projection; active rule counters belong to runtime scenario state. |
| `VisibilityChunk` | Worldmap kernel | Visibility projection for strong reads, diagnostics, and fallback. Active visibility/contact updates should be runtime-owned. |
| `MapOccupancy` | Worldmap kernel | Spatial projection for strong reads, diagnostics, and fallback. Active occupancy authority belongs to runtime indexes. |
| `ParticipantKnownObject` | Worldmap kernel | Knowledge/fog projection; active discovery and contact state belongs to runtime. |
| `WorldObject` mutable state | Worldmap kernel | Projection of capture, depletion, interaction, owner, and temporary-object state. Static content remains direct authority. |
| Town child rows | Worldmap or town runtime | Projection/history for buildings, recruit pools, garrison, tavern/dwelling growth, and town-local state during active gameplay. |
| Champion child rows | Worldmap or champion runtime | Projection/history for army, spells, artifacts/equipment, cooldown-like state, movement, and battle aftermath mirrors during active gameplay. |
| Tavern/dwelling rows | Worldmap or town runtime | Projection/history for active offers, recruit pools, growth, claims, and availability after session activation. |

Worldmap projection/history rows must be restored or flushed through explicit
kernel barriers and projection queues. They should not become hidden live
authority again just because a query or timer path still has a row-backed
fallback.

## Pending Taxonomy Slices

- Active battle projection/history-only rows are still tracked in
  `perf1.todo.md` Section 72D.
- Index retirement sequencing is still tracked in `perf1.todo.md` Section 72D.
