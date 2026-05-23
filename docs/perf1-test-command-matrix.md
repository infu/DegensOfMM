# Perf1 Test Command Matrix

This matrix defines the local test lanes for the perf1 reliability work. Runtime
estimates are for a warm developer machine; PocketIC and benchmark lanes vary
with the local canister toolchain and CPU load.

| Lane | Command | Expected runtime | Run when |
| --- | --- | --- | --- |
| fast-unit-service | `scripts/run-test-groups.sh perf1-fast` | 2-6 minutes | Before and after scoped pure-game, service, projection, or recovery changes. |
| focused-pocketic-gameplay | `scripts/run-test-groups.sh perf1-focused` | 8-20 minutes | After public endpoint, gameplay route, render projection, battle-round, or command-recovery changes. |
| projection-recovery | `scripts/run-test-groups.sh projection-recovery` | 2-6 minutes | After runtime projection, receipt flush, upgrade-restore, or dirty-queue changes. |
| long-form-pocketic | `scripts/run-test-groups.sh perf1-long-form` | 15+ minutes | Before merging high-entity, multi-battle, or long-route gameplay reliability work. |
| full-benchmark-reliability | `DOMM_BENCH_JOBS=5 scripts/run-benchmarks.sh` | 15+ minutes | Before accepting perf-sensitive changes or updating benchmark baselines. |

The full benchmark suite includes a native `projection-surface` gate. PocketIC
benchmark canisters are installed with `DOMM_CANISTER_FEATURES=benchmark`; the
mixed `benchmark,projection-benchmark` Wasm is still not installed by this
script.

## Focused Follow-Ups

Use these narrower commands when a todo item names a subsystem:

| Area | Command |
| --- | --- |
| pure property matrix | `cargo test -p domm-game property_ -- --nocapture` |
| fast canister service filter | `cargo test -p domm-degens-canister service_ -- --nocapture` |
| projection recovery | `scripts/run-test-groups.sh projection-recovery` |
| endpoint contract | `cargo test -p domm-degens-canister endpoint -- --nocapture` |
| champion magic | `cargo test -p domm-degens-canister champion_magic -- --nocapture` |
| town recruit/build | `cargo test -p domm-degens-canister town_ -- --nocapture` |
| content seeding | `cargo test -p domm-degens-canister service_content -- --nocapture` |
| pure town contract | `cargo test -p domm-game town::tests:: -- --nocapture` |

## Tier Wiring

`scripts/run-test-groups.sh` owns the perf1 tier aliases:

| Alias | Expands to |
| --- | --- |
| `perf1-fast` | `pure-property`, `service-regression`, `projection-recovery`, `canister-check` |
| `perf1-focused` | `endpoint-auth`, `gate-l`, `render-projection`, `battle-round`, `command-recovery`, `visibility-redaction` |
| `perf1-long-form` | `gate-j`, `gate-k`, `gate-l`, `movement`, `stationary`, `week-two`, `gate-m` |

Always pair a focused lane with `cargo fmt --all --check` and `git diff --check`
before handing off the work.

## Failure Artifacts

Failed group executions write a replay bundle to
`target/test-artifacts/<run-id>` by default. Prebuild failures stop before group
logs exist and do not produce this bundle. Set `DOMM_TEST_ARTIFACT_DIR` to
redirect bundles for CI upload or local cleanup.

Each bundle contains:

| File | Contents |
| --- | --- |
| `failure-summary.md` | selected groups, failed groups, source log directory, artifact path, and minimal replay commands |
| `seed.txt` | run id, selected/failed groups, known seed environment variables, extracted seed lines, lock namespace prefix, and relevant environment |
| `step-log.txt` | command/status table plus failed log tails |
| `last-successful-view-snapshots.txt` | extracted view, visible object/map, champion, town, and battle snapshot lines |
| `command-event-ids.txt` | extracted command id, event id, nonce, receipt, and status lines |
| `active-runtime-diagnostics.txt` | extracted runtime, diagnostic, dirty queue, and lag lines |
| `projection-snapshot.txt` | extracted projection, flush, dirty queue, checkpoint, and lag lines |
| `timer-job-snapshot.txt` | extracted timer, job, deadline, repair, wakeup, and lease lines |
| `replay.sh` | one-command-per-failed-group replay script with `DOMM_TEST_JOBS=1` |
| `logs/` | full per-group `.log` and `.result` files |
