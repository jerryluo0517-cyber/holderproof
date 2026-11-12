.PHONY: fixture verify test rust-check check

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
