# Local Canister Deploy And Blast Smoke

Run these commands from `/srv/shared/icydb/DoMM`.

## Build And Install

```text
make dfx-deploy-local
dfx canister id degens --network local
```

If the active DFX identity is encrypted, create or select a local plaintext
smoke identity before deploying:

```text
dfx identity new domm-local-smoke --storage-mode plaintext
DFX_IDENTITY=domm-local-smoke dfx deploy degens --network local
```

The committed `dfx.json` builds `domm-degens-canister`, extracts the generated
Candid to `target/dfx/degens/degens.did`, asks DFX to optimize the release wasm
with `Oz`, and declares that DID as public `candid:service` metadata. The build
expects `candid-extractor` on `$HOME/.cargo/bin` or `PATH`; no `IC_WASM` or
`WASM_OPT` environment override is needed.

The canister keeps runtime timers enabled. If local deploy crashes inside a
patched PocketIC/DFX runner, use a clean runner with the Make variable instead
of changing canister behavior:

```text
make DFX=/path/to/clean/dfx dfx-deploy-local
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

Continue with `join_session` for player two and `mark_ready` for both players,
then call `start_session` once with one nonce:

```text
blast call "$CANISTER_ID" join_session '["<session_id>", "faction:ashen-ledger", "nonce:blast:join"]' --host "$HOST" --id 2
blast call "$CANISTER_ID" mark_ready '["<session_id>", "nonce:blast:ready:1"]' --host "$HOST" --id 1
blast call "$CANISTER_ID" mark_ready '["<session_id>", "nonce:blast:ready:2"]' --host "$HOST" --id 2
blast call "$CANISTER_ID" start_session '["<session_id>", "nonce:blast:start"]' --host "$HOST" --id 1
```

Setup advances in bounded slices. After the first `start_session` call, poll
`get_session` or `get_setup_progress`; if the session still reports
`state == "starting"`, call `start_session` again with a new nonce until it
reports `state == "active"`.
Active play should use `get_game_view`, `get_content_manifest`, visible
map/object queries, champion/town/battle detail queries, and
`get_command_status_by_nonce` for nonce polling.

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
same identity that runs the diagnostic calls, or add the blast identity as a
local controller:

```text
dfx canister update-settings degens --network local --add-controller "$(blast principal --id 1)"
```

Keep diagnostic entity lists in small batches; large combined batches can
exceed the local replica instruction limit.

## Cleanup

```text
dfx stop
```
