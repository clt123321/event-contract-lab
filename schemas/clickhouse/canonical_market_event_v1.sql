-- Candidate Silver DDL. Not deployed; validate precision and partition sizing on measured data first.
CREATE TABLE IF NOT EXISTS silver.canonical_market_event_v1
(
    schema_version UInt16,
    canonical_event_id String,
    event_kind LowCardinality(String),
    source LowCardinality(String),
    stream LowCardinality(String),
    session_id String,
    instrument String,
    market_id Nullable(String),
    outcome_id Nullable(String),
    source_event_ts Nullable(DateTime64(6, 'UTC')),
    available_at DateTime64(6, 'UTC'),
    recv_mono_ns UInt128,
    sequence_start Nullable(Int64),
    sequence_end Nullable(Int64),
    price Nullable(Decimal(38, 18)),
    quantity Nullable(Decimal(38, 18)),
    best_bid Nullable(Decimal(38, 18)),
    best_ask Nullable(Decimal(38, 18)),
    bids Array(Tuple(price Decimal(38, 18), quantity Decimal(38, 18))),
    asks Array(Tuple(price Decimal(38, 18), quantity Decimal(38, 18))),
    quality_flags Array(LowCardinality(String)),
    input_sha256 FixedString(64),
    raw_line_number UInt64,
    raw_event_sha256 FixedString(64),
    raw_schema_version UInt16
)
ENGINE = MergeTree
PARTITION BY (toYYYYMM(available_at), source)
ORDER BY (source, instrument, available_at, canonical_event_id);
