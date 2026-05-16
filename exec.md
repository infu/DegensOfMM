# DoMM Executive Code Report

Counts are Rust source only, excluding `target/` and `.git/`. The test suite was not run for this report.

## Code Size

- Total Rust source: `63,441` physical lines across `202` `.rs` files.
- Core product code: `54,852` lines.
  - `crates/domm-game`: `30,969`
  - `canisters/degens`: `21,297`
  - `schema/degens`: `2,586`
- Testing/probe crates: `8,589` lines.
  - `testing/client-probe`: `1,922`
  - `testing/pocket-ic`: `5,822`
  - `testing/macro-tests`: `827`
  - `testing/generated-session`: `18`

## Test Count

There are `213` Rust test functions across `35` test files.

- `domm-game`: `177`
- `canisters/degens`: `13`
- `testing/client-probe`: `2`
- `testing/pocket-ic`: `8`
- `testing/macro-tests`: `11`
- `testing/generated-session`: `2`
- `schema/degens`: `0`

## Client Test Coverage

Client coverage is small but high-level.

- Opening viewport test: loads the first playable game view, renders a 24x24 viewport, checks visible champion/town/resources/neutrals/events, and confirms hidden enemy data is not exposed.
- Playable web client test: runs the first-match path through the client model: lobby, map, movement, turn sync, build, recruit, neutral battle, battle sync, result, match history, rematch state, and command retry/idempotency behavior.
- Pocket-IC client probe test: runs the same client flow against the real canister adapter, then checks DTO parity, IcyDB row growth, battle/entity persistence, query/update metrics, and response sizes.

## Bottom Line

Most tests are pure game-rule tests in `domm-game`. Client coverage is an executive smoke path for the playable flow and canister adapter, not broad UI/browser coverage.
