# Core

Shared Rust implementation is organized as workspace crates:

- `crates/event-contracts`: versioned Raw, mapping, segment, and dataset contracts;
- `crates/collector-core`: validation, segmented WAL, restart recovery, checksum, and verification.
