# Collectors

The currently runnable public-source collectors live in [`../benchmark`](../benchmark). Shared durable
ingestion is implemented in `crates/collector-core`; source-specific Rust collectors will move here only
after their payload fixtures and contracts pass review.

Collector scope is read-only. Predict.fun remains blocked on authorized Testnet access.
