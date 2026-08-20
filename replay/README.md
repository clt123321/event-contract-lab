# Replay v1

The implemented replay consumes Dataset Manifest v2, verifies every Parquet SHA-256, and schedules
events by point-in-time visibility:

```text
available_at_ms → source → session_id → numeric recv_mono_ns → canonical_event_id
```

It emits immutable `ReplayFrame` NDJSON plus a replay manifest binding dataset, config, seed, code and
output checksum. It does not infer fills, reconstruct a stateful order book, or simulate execution yet.

Run with `make replay DATASET_MANIFEST=<path> OUTPUT_DIR=<new-directory>`.
