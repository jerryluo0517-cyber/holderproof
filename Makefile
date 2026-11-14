.PHONY: fixture verify test rust-check check demo stop-demo setup-env check-env verify-live guest-test historical-check acceptance

fixture:
	python3 fixtures/generate_fixture.py

verify:
	python3 scripts/verify_fixture.py

test:
	python3 -m unittest discover -s tests -v

rust-check:
	cargo check --workspace

check: fixture verify test rust-check

demo:
	./scripts/reproduce-step4.sh

stop-demo:
	./scripts/stop-step4.sh

setup-env:
	./scripts/setup-env.sh

check-env:
	./scripts/setup-env.sh --check

# Read-only assertions against an already running Step 4 demo.
verify-live:
	python3 scripts/verify_step4.py

guest-test:
	shaker test lyquid-indexer/Cargo.toml
	shaker test holderproof-gate-core/Cargo.toml

historical-check:
	forge build --root evm
	python3 scripts/verify_historical_balance.py

acceptance: check guest-test
	./scripts/reproduce-step4.sh
	$(MAKE) historical-check
