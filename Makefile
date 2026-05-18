CANDID_EXTRACTOR ?= $(HOME)/.cargo/bin/candid-extractor
IC_WASM ?= ic-wasm
TEST_JOBS ?= 8

.PHONY: build-wasm check-canister dfx-deploy-local dfx-stop-local regression smoke smoke-e2e test test-fast test-generated test-groups test-groups-list test-pocket test-pure test-schema

test: regression

regression:
	cargo test --workspace --exclude domm-pocket-ic-tests
	$(MAKE) test-pocket

smoke:
	cargo test -p domm-game driver_create_join_start_inspect_smoke

smoke-e2e:
	cargo test -p domm-game checkpoint_19_e2e_fixture -- --nocapture

test-pure:
	cargo test -p domm-game

test-schema:
	cargo test -p domm-schema-macro-tests

test-generated:
	cargo test -p domm-generated-session-tests

test-fast:
	DOMM_TEST_JOBS=$(TEST_JOBS) scripts/run-test-groups.sh fast

test-groups:
	DOMM_TEST_JOBS=$(TEST_JOBS) scripts/run-test-groups.sh $(GROUPS)

test-groups-list:
	scripts/run-test-groups.sh list

test-pocket:
	DOMM_TEST_JOBS=$(TEST_JOBS) scripts/run-test-groups.sh pocket-parallel
	DOMM_TEST_JOBS=1 scripts/run-test-groups.sh pocket-serial

check-canister:
	cargo check -p domm-degens-canister

build-wasm:
	CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc cargo build --target wasm32-unknown-unknown --release -p domm-degens-canister
	mkdir -p target/dfx/degens
	$(CANDID_EXTRACTOR) target/wasm32-unknown-unknown/release/domm_degens_canister.wasm > target/dfx/degens/degens.did
	$(IC_WASM) target/wasm32-unknown-unknown/release/domm_degens_canister.wasm -o target/dfx/degens/degens.wasm metadata candid:service -f target/dfx/degens/degens.did -v public

dfx-deploy-local:
	dfx start --background --clean
	dfx deploy degens --network local

dfx-stop-local:
	dfx stop
