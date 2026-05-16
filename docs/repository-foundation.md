# IcyDB Repository Foundation

Checkpoint 19C establishes the typed repository layer used by later canister
endpoint bodies.

Repository helpers live under `canisters/degens/src/repos/` and start from a
fresh generated `db()` session for each operation. Gameplay repositories use
typed IcyDB APIs only:

- `db().create(...)` for normal generated-id/default-aware row creation.
- `db().insert(...)` and `insert_many_atomic(...)` for fixture/full-row writes.
- `db().load::<E>()` with `FieldRef` filters for indexed lookups.
- `db().update(...)` for full typed row updates.
- cursor pagination through `FluentLoadQuery::page()`.
- typed deletes through `db().delete::<E>().by_id(...)`.

`repos/foundation.rs` maps IcyDB errors to stable `ApiError` values without
returning raw storage messages to clients. Native tests assert that duplicate
storage errors are sanitized.

Gameplay repository files are intentionally free of generic SQL and `core_db()`.
The test `gameplay_repositories_do_not_use_generic_sql_or_core_db` scans the
repository sources for those forbidden surfaces.

Every hot-path lookup has an `IndexedQueryPlan` audit handle naming the entity,
schema index fields, and bounded limit. Native tests build the generated IcyDB
explain plan for the required hot paths and fail on full scans or missing
limits. Covered lookup classes include account principal, ruleset/content,
session/participant, command idempotency, event feeds, map chunks, visibility,
occupancy, town ownership, champion ownership, movement intents, battles, and
match history.
