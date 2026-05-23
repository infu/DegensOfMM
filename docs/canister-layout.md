# Canister Layout

`domm-degens-canister` is split by public API, service, repository, DTO, auth,
error, metrics, runtime, and projection domains. The v1.1 gameplay endpoints
are implemented against IcyDB-backed services plus active runtime kernels where
the hot paths have been promoted.

## Public API

Public Candid endpoints live under `canisters/degens/src/api/`:

- `account_lobby_session.rs`: registration, player, lobby, session, and participant endpoints.
- `game_view.rs`: render-facing game, map, object, and champion reads.
- `movement.rs`: movement preview, movement intent, and turn sync endpoints.
- `town.rs`: town reads, build previews/commands, and recruitment previews/commands.
- `economy_expansion.rs`: tavern offers/hiring, market trades, and external dwelling recruitment.
- `scenario_progress.rs`: objective progress, scenario rules, world events, and quest endpoints.
- `worldgen.rs`: skirmish settings, deterministic procedural preview metadata, and disabled naval/siege boundary endpoints.
- `battle.rs`: battle read, battle sync, and battle action endpoints.
- `events.rs`: event paging and command-status reads.
- `content.rs`: content manifest reads.
- `history.rs`: match-history reads.
- `cleanup.rs`: reserved for retained-state cleanup endpoints.
- `diagnostics.rs`: controller-gated diagnostics and benchmark/projection
  diagnostics.

## Services

Service modules live under `canisters/degens/src/services/` with the same public
API grouping plus shared runtime/projection helpers. They own command
validation, idempotency/replay, recovery orchestration, timer/system-job
drivers, runtime-kernel coordination, event creation, and repository
coordination.

## Repositories

Repository modules live under `canisters/degens/src/repos/` and are grouped by
durable row ownership:

- players
- sessions
- commands/events/effects
- content
- map/visibility/occupancy
- economy
- economy expansion
- scenario progress
- world generation
- towns
- champions/artifacts
- movement
- neutrals
- battles
- aftermath/history
- cleanup

Checkpoint 19C adds `foundation.rs`, which owns shared typed IcyDB helpers,
bounded pagination, sanitized repository error mapping, and common create,
insert, update, load, and delete wrappers. Domain files keep their own lookup
functions and query-plan metadata instead of growing one monolithic repository.

The account/lobby/session service persists lobby commands, setup game commands,
effects, pending effects, setup events, player/session/participant rows, and
match-history shells. Render/gameplay endpoint bodies are implemented in their
domain service files and share typed repository helpers, runtime overlays, and
projection flush/recovery boundaries.

Gameplay endpoints must call typed repository helpers instead of generic SQL.
Generated SQL/DDL remains outside public gameplay paths and must stay
controller-gated if enabled for diagnostics or fixture loading.
