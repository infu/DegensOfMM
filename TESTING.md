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
make test-pocket     # Pocket-IC canister test scaffold
make smoke-e2e       # checkpoint 19 first-playable e2e fixture with metrics
make regression      # all workspace tests
make check-canister  # canister crate build check
```

The Pocket-IC crate is intentionally a scaffold at checkpoint 0 because public gameplay
entrypoints are introduced in later checkpoints. Its tests still validate the deterministic
canister/package configuration that future deployment tests will use.

## Fixtures

The first playable fixture in `crates/domm-game` provides stable scenario seed text,
principals, timestamps, command nonces, and semantic IDs. Later checkpoints should extend
that fixture instead of introducing ad hoc test constants.

## Manual First-Playable Smoke

Run these from `/srv/shared/icydb/DoMM` after changing gameplay, API, or client code:

```text
make smoke-e2e
cargo test -p domm-client-probe gate_e -- --nocapture
cargo test -p domm-game gate_d_backend_fixture_reaches_victory_from_public_calls -- --nocapture
make regression
```

Expected result: the checkpoint 19 fixture reaches victory, the Gate E client walkthrough
completes, the Gate D backend route still reports stable command/event/query/storage counts,
and the full workspace regression suite passes.
