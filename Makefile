.DEFAULT_GOAL := check

.PHONY: bootstrap check safety readiness rust-check rust-test node-test format format-check wal-import wal-verify

bootstrap:
	npm --prefix benchmark ci
	cargo fetch --locked

check: safety format-check rust-check rust-test node-test

safety:
	node scripts/check-live-safety.mjs

readiness:
	node scripts/report-readiness.mjs

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
