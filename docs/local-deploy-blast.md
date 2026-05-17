# Local Canister Deploy And Blast Smoke

Run these commands from `/srv/shared/icydb/DoMM`.

## Build And Install

```text
make build-wasm
dfx start --background --clean
dfx deploy degens --network local
dfx canister id degens --network local
```

If the active DFX identity is encrypted, create or select a local plaintext
smoke identity before deploying:

```text
dfx identity new domm-local-smoke --storage-mode plaintext
DFX_IDENTITY=domm-local-smoke dfx deploy degens --network local
```

The committed `dfx.json` builds `domm-degens-canister`, extracts the generated
Candid to `target/dfx/degens/degens.did`, embeds it as public
`candid:service` metadata in `target/dfx/degens/degens.wasm`, and installs that
release wasm. The build expects `candid-extractor` on `$HOME/.cargo/bin` and
`ic-wasm` on `PATH`; if `ic-wasm` is installed elsewhere, pass it explicitly:

```text
IC_WASM=/path/to/ic-wasm dfx deploy degens --network local
```

## Blast Checklist

Set the canister id once:

```text
CANISTER_ID="$(dfx canister id degens --network local)"
HOST="http://127.0.0.1:$(dfx info webserver-port)"
blast scan "$CANISTER_ID" --host "$HOST"
```

Use separate blast identities for players:

```text
blast call "$CANISTER_ID" register_player '["p1", "Player One", "nonce:blast:register:1"]' --host "$HOST" --id 1
blast call "$CANISTER_ID" register_player '["p2", "Player Two", "nonce:blast:register:2"]' --host "$HOST" --id 2
blast call "$CANISTER_ID" create_session '["Blast Smoke", "ruleset:first-playable:v1", 42, "nonce:blast:create"]' --host "$HOST" --id 1
```

Continue with `join_session`, `mark_ready` for both players, one host
`start_session` call, then poll `get_session` until `state == "active"`. Active
play should use `get_game_view`, `get_content_manifest`, visible map/object
queries, champion/town/battle detail queries, and `get_command_status_by_nonce`
for nonce polling.

After moving, collecting, building, recruiting, fighting the guarded mine, and
advancing income, check storage health:

```text
blast call "$CANISTER_ID" icydb_snapshot '[]' --host "$HOST" --id 1
blast call "$CANISTER_ID" icydb_metrics '[null]' --host "$HOST" --id 1
blast call "$CANISTER_ID" get_diagnostic_storage_snapshot '[["GameSession", "GameCommand", "SystemJob", "ParticipantTurnReady"]]' --host "$HOST" --id 1
blast call "$CANISTER_ID" get_diagnostic_storage_snapshot '[["WorldObject", "Champion", "ParticipantKnownObject"]]' --host "$HOST" --id 1
blast call "$CANISTER_ID" get_diagnostic_storage_snapshot '[["ParticipantObjectVisit", "ResourceLedgerEntry", "MovementSnapshot"]]' --host "$HOST" --id 1
```

The diagnostic snapshot is controller-gated. For local smoke, deploy with the
same identity that runs the diagnostic calls, or add that identity as a local
controller with `dfx canister update-settings`. Keep diagnostic entity lists in
small batches; large combined batches can exceed the local replica instruction
limit.

## Cleanup

```text
dfx stop
```
