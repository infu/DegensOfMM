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

## Pending Taxonomy Slices

- Active worldmap gameplay projection/history-only rows are still tracked in
  `perf1.todo.md` Section 72D.
- Active battle projection/history-only rows are still tracked in
  `perf1.todo.md` Section 72D.
- Index retirement sequencing is still tracked in `perf1.todo.md` Section 72D.
