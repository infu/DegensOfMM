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
