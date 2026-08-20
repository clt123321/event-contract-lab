# Contract policy

- Schemas use explicit integer versions. Version 1 is the only accepted Raw/WAL version today.
- Additive source-specific fields remain in the raw envelope and are preserved by the WAL.
- A field meaning or type change requires a new schema file and an ADR; an existing schema is never
  edited to reinterpret already captured bytes.
- Raw segments are immutable after sealing. The sidecar manifest binds row/byte counts and SHA-256.
- Silver/Gold migrations will be versioned independently because they are rebuildable from Raw.
- Public fixtures must be synthetic or explicitly sanitized; production payloads do not enter Git.
- Deployment verification reports have their own schema so local and future host evidence can be compared
  without parsing human log text.
