# Degens of Misery & Mayhem

This repository contains the game implementation for `spec.md`. The adjacent `../icydb`
workspace is a path dependency and reference implementation; game code, tests, notes,
and client work stay in this repository unless a later checkpoint explicitly requires
an IcyDB library change.

## Repo Policy

Run all game commands from this directory:

```text
/srv/shared/icydb/DoMM
```

`DoMM/` is an independent git repository on branch `main`. No remote is configured at
checkpoint 0, so checkpoint commits are local until a remote policy is added.

## Layout

```text
canisters/degens/          IcyDB canister crate and generated actor entrypoint
crates/domm-game/          pure deterministic rules, fixtures, DTOs, and headless driver
schema/degens/             IcyDB canister/store schema root
testing/generated-session/ generated-session regression test layer
testing/macro-tests/       schema/macro regression test layer
testing/pocket-ic/         Pocket-IC canister tests and benchmark gates
```

Public gameplay APIs must be typed query/update methods. Generic SQL and SQL DDL are
disabled by default in `icydb.toml` and are reserved for controller/test-only surfaces.

Latest recorded backend evidence: benchmark run
`target/benchmarks/20260522-164948-5cfd001` at git `5cfd001` passed with
`59/59` required gameplay endpoints. Treat this as recorded benchmark evidence,
not a rerun of later uncommitted worktree changes. The canister-backed client
probe covers the first-playable route through lobby, setup, map, movement,
build, recruit, battle, result/history, and rematch affordance.

For UI implementation, start with `docs/client-ui-integration.md`. It explains
the current canister call order, render composition model, command retry/status
rules, gameplay loops, battle flow, city/economy actions, and gameover/history
screens. `docs/canister-endpoints.md` remains the endpoint inventory and
deferred-endpoint policy.

## Smoke Commands

```text
make smoke
make regression
make check-canister
```

Local canister deployment is committed through `dfx.json`:

```text
make dfx-deploy-local
```

See `TESTING.md` for the test layers and `docs/local-deploy-blast.md` for the
agent-run `blast` smoke checklist.
