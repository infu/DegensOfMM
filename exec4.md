# Short IcyDB Comparison

| Option | Summary |
| --- | --- |
| Rust + IcyDB | Best fit for DoMM: typed entities, indexes, diagnostics, durable rows, and better auditability. More complexity, but useful structure. |
| Plain Rust | Maximum control and likely fastest if done perfectly, but storage, indexes, migrations, diagnostics, and recovery patterns must be built manually. |
| Motoko | Easiest for simpler canisters, but less ideal for a large entity-heavy game with many indexed views and performance-sensitive systems. |

Note: saga/idempotency patterns are not needed for one synchronous update call
that can safely trap and roll back. They matter when work is split across
messages, timers, self-call continuations, or long jobs that must survive
instruction limits and partial progress.
