TEST_JOBS ?= 8
LONG_TEST_JOBS ?= 4
GATE_M_TEST_JOBS ?= 1
DFX ?= dfx
DFX_LOCAL_START_LOG ?= /tmp/domm-dfx-start.log
DFX_LOCAL_REPLICA_LOG ?= /tmp/domm-dfx.log
DFX_LOCAL_CANISTER_CYCLES ?= 50000000000000

.PHONY: bench build-wasm check-canister dfx-deploy-local dfx-stop-local regression smoke smoke-e2e test test-fast test-generated test-groups test-groups-list test-pocket test-pure test-schema

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

bench:
	scripts/run-benchmarks.sh

test-pocket:
	DOMM_TEST_JOBS=$(TEST_JOBS) scripts/run-test-groups.sh pocket-parallel
	DOMM_TEST_JOBS=$(LONG_TEST_JOBS) scripts/run-test-groups.sh pocket-long
	DOMM_TEST_JOBS=$(GATE_M_TEST_JOBS) scripts/run-test-groups.sh pocket-gate-m

check-canister:
	cargo check -p domm-degens-canister

build-wasm:
	scripts/dfx-build-degens.sh

dfx-deploy-local:
	$(DFX) start --background --clean --log file --logfile $(DFX_LOCAL_REPLICA_LOG) > $(DFX_LOCAL_START_LOG) 2>&1
	$(DFX) deploy degens --network local
	$(DFX) canister deposit-cycles $(DFX_LOCAL_CANISTER_CYCLES) degens --network local

dfx-stop-local:
	$(DFX) stop
