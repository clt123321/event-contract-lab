# Contract policy

- Schemas use explicit integer versions. Version 1 is the only accepted Raw/WAL version today.
- Additive source-specific fields remain in the raw envelope and are preserved by the WAL.
- A field meaning or type change requires a new schema file and an ADR; an existing schema is never
  edited to reinterpret already captured bytes.
- Raw segments are immutable after sealing. The sidecar manifest binds row/byte counts and SHA-256.
- Silver/Gold migrations will be versioned independently because they are rebuildable from Raw.
- Canonical Silver decimals remain strings until an explicitly scaled storage schema is approved.
- Every canonical row binds the input SHA-256, physical line and raw-event SHA-256; transform
  manifests bind accepted output, quarantine and quality report artifacts.
- Public fixtures must be synthetic or explicitly sanitized; production payloads do not enter Git.
- Deployment verification reports have their own schema so local and future host evidence can be compared
  without parsing human log text.

Implemented v1 files include Raw/segment/dataset, canonical market events, quality policy/report,
Raw→Silver transform manifests and deployment verification reports. `clickhouse/` contains candidate DDL,
not an applied database migration.
