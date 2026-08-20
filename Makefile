.DEFAULT_GOAL := check

.PHONY: bootstrap check safety readiness verify-local verify-release verify-host compare-verify rust-check rust-test node-test format format-check wal-import wal-verify normalize

bootstrap:
	npm --prefix benchmark ci
	cargo fetch --locked

check: safety format-check rust-check rust-test node-test

safety:
	node scripts/check-live-safety.mjs

readiness:
	node scripts/report-readiness.mjs

# Fully local and read-only. Dirty worktrees are reported as warnings during development.
verify-local:
	node scripts/verify-deployment.mjs --mode local $(VERIFY_ARGS)

# Use immediately before tagging/building a deployable revision.
verify-release:
	node scripts/verify-deployment.mjs --mode local --require-clean $(VERIFY_ARGS)

# Run on a future host after deployment. Dynamic Polymarket discovery is smoke-only.
verify-host:
	node scripts/verify-deployment.mjs --mode host-smoke $(VERIFY_ARGS)

# Usage: make compare-verify BEFORE=data/verification/<old>/report.json AFTER=data/verification/<new>/report.json
compare-verify:
	test -n "$(BEFORE)"
	test -n "$(AFTER)"
	node scripts/compare-verification.mjs --before "$(BEFORE)" --after "$(AFTER)"

rust-check:
	cargo clippy --workspace --all-targets --locked -- -D warnings

rust-test:
	cargo test --workspace --locked

node-test:
	npm --prefix benchmark test

format:
	cargo fmt --all

format-check:
	cargo fmt --all -- --check

# Usage: make wal-import INPUT=benchmark/data/raw/<capture>.ndjson
wal-import:
	test -n "$(INPUT)"
	cargo run --locked -p wal-cli -- import --input "$(INPUT)" --wal-dir data/wal --git-commit "$$(git rev-parse --verify HEAD)"

wal-verify:
	cargo run --locked -p wal-cli -- verify --wal-dir data/wal

# Usage: make normalize INPUT=data/wal/<segment>.ndjson OUTPUT_DIR=data/silver/<run-id>
normalize:
	test -n "$(INPUT)"
	test -n "$(OUTPUT_DIR)"
	cargo run --locked -p normalize-cli -- normalize \
		--input "$(INPUT)" \
		--output "$(OUTPUT_DIR)/canonical.ndjson" \
		--quarantine "$(OUTPUT_DIR)/quarantine.ndjson" \
		--quality-report "$(OUTPUT_DIR)/quality.json" \
		--manifest "$(OUTPUT_DIR)/transform-manifest.json" \
		--quality-policy config/quality-policy.v1.json \
		--git-commit "$$(git rev-parse --verify HEAD)"
