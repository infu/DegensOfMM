# Canister Layout

`domm-degens-canister` is split by public API, service, repository, DTO, auth,
error, and metrics domains before IcyDB-backed endpoint bodies are added.

## Public API

Public Candid endpoints live under `canisters/degens/src/api/`:

- `account_lobby_session.rs`: registration, player, lobby, session, and participant endpoints.
- `game_view.rs`: render-facing game, map, object, and champion reads.
- `movement.rs`: movement preview, movement intent, and turn sync endpoints.
- `town.rs`: town reads, build previews/commands, and recruitment previews/commands.
- `battle.rs`: battle read, battle sync, and battle action endpoints.
- `events.rs`: event paging and command-status reads.
- `content.rs`: content manifest reads.
- `history.rs`: match-history reads.
- `cleanup.rs`: future retained-state cleanup endpoints.
- `diagnostics.rs`: future controller-gated diagnostics only.

## Services

Service modules live under `canisters/degens/src/services/` with the same public
API grouping. Checkpoint 19B keeps them as typed placeholders that return
`icydb_repository_not_implemented`; later checkpoints should put command
validation, recovery orchestration, and repository coordination here.

## Repositories

Repository modules live under `canisters/degens/src/repos/` and are grouped by
durable row ownership:

- players
- sessions
- commands/events/effects
- content
- map/visibility/occupancy
- economy
- towns
- champions/artifacts
- movement
- neutrals
- battles
- aftermath/history
- cleanup

Gameplay endpoints must call typed repository helpers instead of generic SQL.
Generated SQL/DDL remains outside public gameplay paths and must stay
controller-gated if enabled for diagnostics or fixture loading.
