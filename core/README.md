# Core

Shared Rust implementation is organized as workspace crates:

- `crates/event-contracts`: versioned Raw, mapping, segment, and dataset contracts;
- `crates/collector-core`: validation, segmented WAL, restart recovery, checksum, and verification.
- `crates/normalizer-core`: deterministic Raw→Silver mapping, quality flags and quarantine decisions;
- `apps/normalize-cli`: local transform artifacts and checksummed lineage manifest.
- `crates/dataset-core` / `apps/dataset-cli`: deterministic Arrow/Parquet and frozen datasets;
- `crates/replay-core` / `apps/replay-cli`: verified point-in-time replay and virtual-clock evidence.
