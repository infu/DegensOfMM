# Testing

Run commands from `/srv/shared/icydb/DoMM`.

## Checkpoint Loop

Before starting a checkpoint, run the regression suite from the previous checkpoint:

```text
make regression
```

For checkpoint 0, the regression suite is the newly added harness suite.

## Test Layers

```text
make test-pure       # deterministic fixtures, DTO roundtrips, headless driver
make test-schema     # schema/canister macro compilation surface
make test-generated  # generated-session harness surface
make test-fast       # prebuild selected stable groups and run them in parallel
make test-groups-list
make test-groups GROUPS="gate-k week-two" TEST_JOBS=4
make test-pocket     # parallel, long, and isolated Gate M Pocket-IC phases
make smoke-e2e       # first-playable canister/client e2e fixture with metrics
make build-wasm      # release wasm plus extracted Candid for dfx
make regression      # all workspace tests
make check-canister  # canister crate build check
```

`scripts/run-test-groups.sh` is the timing harness for spec 1.1 test work. It
prebuilds the selected test binaries once, runs named groups with bounded
parallelism, writes logs under `target/test-groups/`, and prints a Markdown
timing table. `make test-pocket` runs `pocket-parallel` with `TEST_JOBS`,
`pocket-long` with `LONG_TEST_JOBS` (default `4`), and the isolated
`pocket-gate-m` phase with `GATE_M_TEST_JOBS`. `make regression` runs
non-PocketIC workspace tests before invoking the same phased PocketIC target.
For direct `scripts/run-test-groups.sh` runs, `DOMM_TEST_JOBS` defaults to
`min(nproc, 8)`. Make targets pass `TEST_JOBS` (default `8`),
`LONG_TEST_JOBS` (default `4`), or `GATE_M_TEST_JOBS` (default `1`)
explicitly. Raise worker counts only for groups that are isolated and stable
under concurrent Pocket-IC instances.

Pocket-IC tests now exercise meaningful canister routes. New v1.1 suites should stay split
by failure mode using the runnable groups `timer-jobs`, `end-turn`,
`battle-round`, `render-projection`, `query-budget`, `command-recovery`, and
`visibility-redaction`, with endpoint auth matrices in the `endpoint-auth` group,
rather than growing one monolithic endpoint test.

## Fixtures

The first playable fixture in `crates/domm-game` provides stable scenario seed text,
principals, timestamps, command nonces, and semantic IDs. Later checkpoints should extend
that fixture instead of introducing ad hoc test constants.

## Manual First-Playable Smoke

Run these from `/srv/shared/icydb/DoMM` after changing gameplay, API, or client code:

```text
make smoke-e2e
cargo test -p domm-client-probe gate_e -- --nocapture
cargo test -p domm-pocket-ic-tests --test client_probe_canister gate_m -- --nocapture
cargo test -p domm-pocket-ic-tests --test canister_endpoints pocket_ic_gate_l -- --nocapture
cargo test -p domm-game gate_d_backend_fixture_reaches_victory_from_public_calls -- --nocapture
make regression
```

Expected result: the first-playable fixture reaches victory, the Gate E client walkthrough
completes, the Gate M canister-backed client route reports Pocket-IC/IcyDB response and row
metrics, Gate L still completes the public canister first-playable route, the Gate D backend
route still reports stable command/event/query/storage counts, and the full workspace
regression suite passes.

## Local DFX And Blast

The reproducible local deploy path is:

```text
make dfx-deploy-local
blast scan "$(dfx canister id degens --network local)" --host "http://127.0.0.1:$(dfx info webserver-port)"
```

`make dfx-deploy-local` starts the local replica from `dfx.json` and writes DFX
startup output to `/tmp/domm-dfx-start.log` plus replica output to
`/tmp/domm-dfx.log`. If the local PocketIC/DFX cache has been patched, replace
or override the runner rather than changing gameplay timers; for example:
`make DFX=/path/to/clean/dfx dfx-deploy-local`.

Use `docs/local-deploy-blast.md` for the full multi-identity command checklist and
diagnostic snapshot evidence.

For UI-facing behavior, pair the smoke commands above with
`docs/client-ui-integration.md`; it describes the endpoint composition and
gameplay loops that Gate M exercises against the real canister.
