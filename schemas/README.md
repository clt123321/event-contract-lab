# Contract policy

- Schemas use explicit integer versions. Version 1 is the only accepted Raw/WAL version today.
- Additive source-specific fields remain in the raw envelope and are preserved by the WAL.
- A field meaning or type change requires a new schema file and an ADR; an existing schema is never
  edited to reinterpret already captured bytes.
- Raw segments are immutable after sealing. The sidecar manifest binds row/byte counts and SHA-256.
- Silver/Gold migrations are versioned independently because they are rebuildable from Raw.
- Canonical Silver decimals remain strings until an explicitly scaled storage schema is approved.
- Every canonical row binds the input SHA-256, physical line and raw-event SHA-256; transform
  manifests bind accepted output, quarantine and quality report artifacts.
- Public fixtures must be synthetic or explicitly sanitized; production payloads do not enter Git.
- Deployment verification reports have their own schema so local and future host evidence can be compared
  without parsing human log text.

Implemented contracts include Raw/segment, canonical market events, quality policy/report/mask,
Raw→Silver transform manifests, Dataset Manifest v2, Replay Config/Frame/Manifest v1 and deployment
verification reports. `dataset-manifest.v1.schema.json` is retained as a superseded draft; new datasets
must use v2. `clickhouse/` contains candidate DDL, not an applied database migration.
