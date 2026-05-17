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
make test-pocket     # Pocket-IC public canister route tests
make smoke-e2e       # checkpoint 19 first-playable e2e fixture with metrics
make build-wasm      # release wasm plus extracted Candid for local dfx
make regression      # all workspace tests
make check-canister  # canister crate build check
```

Pocket-IC tests now exercise meaningful canister routes. New v1.1 suites should stay split
by failure mode (`timer_jobs`, `end_turn`, `battle_round_readiness`,
`render_projection`, `query_budgets`, `command_recovery`, and
`visibility_redaction`) rather than growing one monolithic endpoint test.

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

Expected result: the checkpoint 19 fixture reaches victory, the Gate E client walkthrough
completes, the Gate M canister-backed client route reports Pocket-IC/IcyDB response and row
metrics, Gate L still completes the public canister first-playable route, the Gate D backend
route still reports stable command/event/query/storage counts, and the full workspace
regression suite passes.

## Local DFX And Blast

The reproducible local deploy path is:

```text
make build-wasm
dfx start --background --clean
dfx deploy degens --network local
blast scan "$(dfx canister id degens --network local)" --host "http://127.0.0.1:$(dfx info webserver-port)"
```

Use `docs/local-deploy-blast.md` for the full multi-identity command checklist and
diagnostic snapshot evidence.
