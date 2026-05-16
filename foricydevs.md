# Notes For IcyDB Devs

Compressed notes from wiring DoMM onto IcyDB. This is only about IcyDB
ergonomics, performance, and generated-canister behavior.

## Highest-Value Improvements

1. Cheap row counts and storage stats
   - What happened: a diagnostic endpoint that tried to count every entity in one canister query exceeded the Pocket-IC 5B instruction limit.
   - Workaround: diagnostics are controller-gated, require an explicit entity-name list, and count by loading up to 512 primary-key ordered rows per entity.
   - Ask: expose cheap per-entity row counts, memory/index stats, or approximate metadata counters without loading rows.

2. Structured query plans
   - What happened: repository tests call `explain()` and parse rendered text for `Index`, `ByKey`, `FullScan`, and `limit`.
   - Workaround: each hot path has a local `IndexedQueryPlan` audit handle plus brittle text assertions.
   - Ask: provide a stable structured explain result: access path enum, chosen index fields, full-scan flag, limit, estimated rows/cost, and whether ordering is index-covered.

3. Field and relation typing in queries
   - What happened: app code uses string field names like `FieldRef::new("session_id")`, and relation filters compare against primitive `.key()` values.
   - Workaround: repository modules centralize queries and tests verify indexed hot paths.
   - Ask: generate typed field constants or builders, including relation-aware helpers like `field.session_id().eq_id(session_id)`.

4. Create input defaults
   - What happened: generated `Create<T>` inputs require explicit values for every authorable field, including fields with defaults and nullable fields as `Some(None)`.
   - Workaround: repository create helpers are verbose and carefully fill defaults/nulls.
   - Ask: let generated create builders omit nullable/default-backed fields, or generate ergonomic constructors that apply schema defaults consistently.

5. Upsert / insert-or-load by unique key
   - What happened: command idempotency and setup effects repeatedly needed: query unique key, create, if duplicate then reload, compare payload hash.
   - Workaround: DoMM implements this pattern in service code for `GameCommand`, `LobbyCommand`, effects, events, movement intents, etc.
   - Ask: add typed `insert_or_load_by_unique` / `get_or_create` helpers with duplicate classification, leaving payload comparison to the app.

6. Public-safe storage error classification
   - What happened: raw storage errors can expose schema field names. DoMM sanitizes every repository error into `icydb_repository_error`.
   - Workaround: clients lose useful distinctions unless services add their own validation before storage calls.
   - Ask: expose safe error classes such as duplicate unique key, missing strong relation, invalid cursor, limit exceeded, schema mismatch, without raw internals.

7. Native no-SQL feature separation
   - What happened: building with IcyDB default features disabled exposed SQL-gated imports. DoMM keeps IcyDB default features for native tests but disables generated SQL/DDL through `icydb.toml`.
   - Workaround: gameplay code bans generic SQL by source scan; SQL remains config-disabled/controller-only.
   - Ask: support a clean no-SQL native build and make generated SQL endpoint availability obvious from build output.

8. Generated test harness ergonomics
   - What happened: native repository tests must manually bootstrap the generated canister memory range with `MemoryApi::bootstrap_owner_range("domm-degens-canister", 20, 120)`.
   - Workaround: local helper in repository tests.
   - Ask: generate a small test bootstrap helper for each canister/store schema.

9. Generated canister size and cost visibility
   - What happened: debug wasm exceeded Pocket-IC's 100 MiB chunk-store limit. Later schema/API growth pushed the release canister over the IC code-section limit until the release profile used LTO, one codegen unit, `panic = abort`, and symbol stripping. Size-only optimization reduced wasm size but made setup phases exceed update instruction caps.
   - Workaround: Pocket-IC builds release wasm and the workspace has a tuned release profile.
   - Ask: expose generated-code size contributors and offer IcyDB/codegen size guidance before users discover this at install time.

10. Query/update cost observability
   - What happened: several endpoints crossed instruction limits only after schema growth: full setup, `get_game_view`, `get_town_view`, `get_my_champions`, `get_content_manifest` validation, battle views with embedded events, diagnostics, and movement sync.
   - Workaround: split setup into more phases, denormalize slugs/rosters, move details to dedicated endpoints, reduce movement sync to one microstep per update, and avoid full content row recounts on hot paths.
   - Ask: provide cheaper projection APIs, per-query row/field cost metrics, and guidance/tooling to identify expensive generated reads/writes before Pocket-IC fails.

11. Build/config discovery sharp edges
   - What happened: `icydb.toml` names, `emit_config_for_canister("degens", ...)`, and `config.canister_sql_*_enabled("degens")` must align exactly. The build script also needs a `TypeId::of::<...>()` reference to force schema node registration during macro discovery.
   - Workaround: local docs pin the exact pattern.
   - Ask: make config mismatches and missing schema registration fail with direct diagnostics, or generate the build glue from the schema/canister declaration.

## Other Friction

- Implicit audit fields were surprising. `created_at` can be indexed even when it is not declared in the handwritten entity fields; explicitly adding it caused duplicate-field macro errors. Please document implicit fields and expose generated constants/accessors for them.
- Candid reserved words matter at schema time. A field named `principal` had to become `account_principal`. The rejection is good, but a clearer error with suggested alternatives and a reserved-name list would help.
- `db_default` rules need sharper docs/lints. Function defaults like `Timestamp::now` and `Ulid::generate` are Rust/create defaults, not persisted defaults for existing rows. `db_default` support also appears limited to single primitive values.
- Cursor pagination is live-state, not snapshot-isolated. That is workable, but the generated page API could surface `has_more` directly and document cursor invalidation/change semantics clearly.
- Deletion order is manual. Strong/weak relation cleanup required an app-maintained dependency order. A generated relation graph or delete-plan helper would reduce mistakes.
- Heavy aggregate reads are easy to build and hard to fit in IC budgets. `get_game_view`, `get_battle_state` with embedded events, and all-entity diagnostics all had to be split. Field projection or lightweight row summaries would help avoid loading full rows when DTOs need only a few fields.
- Same-entity `insert_many_atomic` is useful, but setup workflows with parent/child strong relations still need explicit phases. Clearer docs around visibility/relation checks inside batches would prevent overusing batch inserts as transactions.
- Full setup cannot fit in one update once real content/map/visibility/town/champion/object rows exist. Setup became a phased saga with `CommandEffect` markers, and later schema expansion forced even more phase splitting. A generated phase/batch planner would help.
- Full validation scans are expensive. `get_content_manifest` originally tried to validate persisted definition row counts on the hot path; that exceeded the query budget after schema growth. It now checks the ruleset content hash and leaves full validation to setup/tests.
- Hot render queries needed denormalization. Town child rows denormalize building/unit slugs, participants keep a compact `champion_ids` roster, and artifact rendering avoids loading definition rows on the hot path. Generated field projections or cheap relation joins would reduce this pressure.
- Stable-memory growth was hard to interpret. Pocket-IC gates observed large stable-memory page growth while selected row counts were modest. Better per-store/per-index memory attribution would make storage regressions actionable.
- Nullable/cache fields and uniqueness need sharper guidance. DoMM avoided making nullable artifact-owner/slot cache fields authoritative and instead used a separate `ArtifactEquipment` row with unique `champion_id + slot` and unique `artifact_id`.
- Generated model metadata may include bookkeeping fields after declared fields. Schema-evolution tests had to assert declared prefixes rather than exact field lists so safe appends remain possible while renames/reorders still fail.
- Generic occupant keys are a practical escape hatch but move cleanup safety out of the relation system. `MapOccupancy` stores `occupant_kind + occupant_id_text`, so cleanup must delete by generic occupant key before deleting typed target rows. Typed polymorphic relation helpers or generated cleanup checks would help.
- Public DTOs should not expose raw IcyDB rows. That worked, but it required a lot of app-owned projection code. Generated read models or projection helpers could reduce boilerplate while preserving redaction.

## What Worked Well

- Typed entities, strong/weak relations, composite indexes, generated IDs, and cursor pages were enough to build a normalized canister-local game store.
- `explain()` made it possible to enforce indexed hot paths in tests, even though the current text format is not ideal.
- Unique indexes plus command/effect/event rows supported robust idempotent retries.
- Keeping generic SQL out of gameplay was practical once repositories were centralized.
- IcyDB's single-entity atomicity model was manageable when workflows were explicit sagas with `GameCommand`, `CommandEffect`, ledger rows, and deterministic event keys.

## Concrete DoMM Workarounds To Inspect

- `canisters/degens/src/repos/foundation.rs`: shared typed wrappers, limit validation, sanitized storage errors, cursor pages.
- `canisters/degens/src/repos/tests.rs`: create/read/update/page/delete smoke, `insert_many_atomic`, generic SQL ban, indexed explain-plan checks.
- `canisters/degens/src/services/diagnostics.rs`: bounded controller-gated row-count workaround.
- `spec.md` sections 14, 18, and 19: saga/idempotency, performance budgets, schema evolution, and deletion-order rules that exist largely because broad multi-entity transactions and cheap global introspection are not available.
