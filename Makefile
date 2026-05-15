.PHONY: check-canister regression smoke test test-generated test-pocket test-pure test-schema

test: regression

regression:
	cargo test --workspace

smoke:
	cargo test -p domm-game driver_create_join_start_inspect_smoke

test-pure:
	cargo test -p domm-game

test-schema:
	cargo test -p domm-schema-macro-tests

test-generated:
	cargo test -p domm-generated-session-tests

test-pocket:
	cargo test -p domm-pocket-ic-tests

check-canister:
	cargo check -p domm-degens-canister
