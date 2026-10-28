# Common entry points. Building the pinned tools: docs/run/reproduce.md.
BUNDLE ?= testnet-auction-nu7-v2
NARGO := .deps/noir-1.0.0-rc.2/nargo
PY_TESTS := test_native_layout test_regtest_prepared_gate test_regtest_recovery test_regtest_resources test_review_kit

.PHONY: help doctor demo demo-testnet verify verify-all verify-docker cheat trail test

help:
	@echo "make doctor         check the pinned tools, bundles, regtest node and disk"
	@echo "make demo           local demo at http://127.0.0.1:8877; new auctions on regtest"
	@echo "make demo-testnet   the same, with new auctions on public Zcash testnet"
	@echo "make verify         verify the recorded testnet settlement, no secrets (BUNDLE=$(BUNDLE))"
	@echo "make verify-all     verify every committed bundle and refuse a corrupted proof"
	@echo "make verify-docker  the same in a clean Linux container built from git archive"
	@echo "make cheat          the cheat lab: three dishonest changes and an unchanged control"
	@echo "make trail          the public data behind each claim, with consistency checks"
	@echo "make test           the CI suites: Noir, Rust and Python"

doctor:
	@python3 -B scripts/doctor.py

demo:
	./demo.sh

demo-testnet:
	./demo.sh --chain testnet

verify:
	./verify.sh --funded demo/artifacts/$(BUNDLE) --authority demo/authority/$(BUNDLE).json

verify-all:
	python3 -B scripts/docker_verify_all.py

verify-docker:
	scripts/docker_verify.sh

cheat:
	python3 -B scripts/cheat_lab.py none book price payout

trail:
	python3 -B scripts/proof_trail.py demo/artifacts/$(BUNDLE) demo/authority/$(BUNDLE).json

test:
	for d in circuits/*/; do $(NARGO) test --program-dir "$$d" || exit 1; done
	cargo test --release --locked --workspace
	cd scripts && python3 -B -m unittest $(PY_TESTS)
