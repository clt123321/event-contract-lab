# ADR-0001: Data-first monorepo and deny-by-default execution

- Status: Accepted
- Date: 2026-08-20
- Decision owners: project owner and repository maintainers

## Context

The public data path for Binance and Polymarket is already observable, while Predict.fun still needs
authorized Testnet access. Research conclusions need byte-level Raw evidence and deterministic inputs.
Real orders introduce separate legal, account, geographic, loss, and reconciliation risks.

## Decision

Use one repository with Rust for durable collection/recovery/replay/execution primitives, Node/TypeScript
for the existing probes and future control plane, and Python/SQL for research. Establish a versioned
Raw envelope and a single-writer segmented NDJSON WAL before Parquet or ClickHouse.

All venue access is read-only. Live execution is disabled in versioned configuration and checked by CI.
Adding an adapter interface in the future does not grant authority to submit an order. G3 requires a
separate written approval and a code review that changes the safety policy.

## Consequences

- Sealed Raw bytes and manifests become the source of truth; Silver and Gold are rebuildable.
- Unknown source fields are retained instead of discarded during schema evolution.
- A restart seals complete WAL rows; any partial trailing bytes are preserved in quarantine.
- Active WAL pages are flushed per row and `fdatasync` is triggered at least every 1,000 rows or on
  the next event after one second; segment seal uses full `fsync`. Host-crash RPO is therefore bounded
  but not claimed as zero, and must be measured in the soak test.
- Polymarket and Predict.fun market lists are not silently populated from a dynamic popularity query.
- The first implementation is deliberately single-writer and local-disk only; archive upload and
  multi-process ownership need later ADRs and failure tests.

## Alternatives considered

- Direct-to-ClickHouse collection was rejected because database downtime would interrupt evidence capture.
- Automatic selection of current top markets was rejected for formal benchmarks because the dataset scope
  would change without review.
- Building order submission alongside data collection was deferred because it would cross the G3 boundary.

## Rollback and migration

The Rust crates can be removed without changing existing Node collectors. Raw schema v1 files remain
readable NDJSON. Any incompatible manifest or envelope change must add a new schema version and converter.
