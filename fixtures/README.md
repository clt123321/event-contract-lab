# Test fixtures

Everything under this directory is synthetic and safe to publish. Values are deliberately marked
with `fixture: true`; IDs are not production account, wallet, order, or market identifiers.

Do not copy captured production payloads here. New source fixtures must be minimized and sanitized,
then reviewed for credentials and personal data before commit.

`raw/quality-cases.v1.ndjson` deliberately contains duplicate, regressing-sequence, crossed-book and
out-of-range rows. The local release gate asserts exactly which rows are accepted, warned or quarantined.
