//! Deterministic `RawEventEnvelope` to canonical Silver normalization and quality decisions.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use event_contracts::{
    CANONICAL_EVENT_SCHEMA_VERSION, CanonicalEventKind, CanonicalMarketEvent, PriceLevel,
    RawEventEnvelope, RawLineage,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const NORMALIZER_VERSION: &str = "raw-to-silver-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QualityPolicy {
    pub schema_version: u32,
    pub policy_version: String,
    pub timestamp_future_tolerance_ms: f64,
    pub stale_event_threshold_ms: f64,
    pub reject_duplicate_raw_event: bool,
    pub reject_sequence_regression: bool,
    pub reject_crossed_book: bool,
    pub event_contract_price_min: f64,
    pub event_contract_price_max: f64,
}

impl QualityPolicy {
    pub fn validate(&self) -> Result<(), NormalizeError> {
        if self.schema_version != 1 {
            return Err(NormalizeError::InvalidPolicy(
                "schema_version must equal 1".into(),
            ));
        }
        if self.policy_version.trim().is_empty() {
            return Err(NormalizeError::InvalidPolicy(
                "policy_version cannot be empty".into(),
            ));
        }
        if !self.timestamp_future_tolerance_ms.is_finite()
            || self.timestamp_future_tolerance_ms < 0.0
            || !self.stale_event_threshold_ms.is_finite()
            || self.stale_event_threshold_ms <= 0.0
        {
            return Err(NormalizeError::InvalidPolicy(
                "timestamp thresholds must be finite and non-negative".into(),
            ));
        }
        if !self.event_contract_price_min.is_finite()
            || !self.event_contract_price_max.is_finite()
            || self.event_contract_price_min >= self.event_contract_price_max
        {
            return Err(NormalizeError::InvalidPolicy(
                "event contract price bounds are invalid".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum NormalizeError {
    #[error("invalid quality policy: {0}")]
    InvalidPolicy(String),
    #[error("raw JSON cannot be decoded: {0}")]
    InvalidJson(String),
    #[error("invalid raw event contract: {0}")]
    InvalidRawContract(#[from] event_contracts::ContractError),
    #[error("unsupported source/event pair: {venue}/{event_type}")]
    UnsupportedEvent { venue: String, event_type: String },
    #[error("record kind `{0}` is not a canonical market event")]
    UnsupportedRecordKind(String),
    #[error("required payload field `{0}` is missing")]
    MissingPayloadField(&'static str),
    #[error("payload field `{0}` is not a finite decimal")]
    InvalidDecimal(&'static str),
    #[error("payload field `{0}` must be positive")]
    NonPositiveDecimal(&'static str),
    #[error("event-contract price `{field}` is outside the configured bounds")]
    EventContractPriceOutOfRange { field: &'static str },
    #[error("book level `{0}` must be [price, quantity] or an object with price and size")]
    InvalidBookLevel(&'static str),
    #[error("book is crossed")]
    CrossedBook,
    #[error("source timestamp is too far ahead of receive timestamp")]
    FutureTimestamp,
    #[error("duplicate raw event")]
    DuplicateRawEvent,
    #[error("sequence regressed within a collector session")]
    SequenceRegression,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuarantineRecord {
    pub schema_version: u32,
    pub line_number: u64,
    pub raw_event_sha256: String,
    pub reason_code: String,
    pub reason: String,
    pub raw_line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct QualitySummary {
    pub input_rows: u64,
    pub canonical_rows: u64,
    pub quarantined_rows: u64,
    pub skipped_rows: u64,
    pub warning_rows: u64,
    pub reason_counts: BTreeMap<String, u64>,
    pub skip_counts: BTreeMap<String, u64>,
    pub flag_counts: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QualityReport {
    pub schema_version: u32,
    pub normalizer_version: String,
    pub quality_policy_version: String,
    pub quality_policy_sha256: String,
    pub input_sha256: String,
    pub summary: QualitySummary,
}

#[derive(Debug, Clone)]
pub enum NormalizeOutcome {
    Canonical(Box<CanonicalMarketEvent>),
    Quarantined(QuarantineRecord),
    Skipped {
        line_number: u64,
        reason_code: String,
    },
}

#[derive(Debug)]
pub struct Normalizer {
    policy: QualityPolicy,
    input_sha256: String,
    seen_raw_events: BTreeSet<String>,
    last_sequences: HashMap<String, i64>,
    summary: QualitySummary,
}

impl Normalizer {
    pub fn new(policy: QualityPolicy, input_sha256: String) -> Result<Self, NormalizeError> {
        policy.validate()?;
        Ok(Self {
            policy,
            input_sha256,
            seen_raw_events: BTreeSet::new(),
            last_sequences: HashMap::new(),
            summary: QualitySummary::default(),
        })
    }

    pub fn normalize_line(&mut self, line_number: u64, raw_line: &str) -> NormalizeOutcome {
        self.summary.input_rows += 1;
        let raw_sha256 = sha256_hex(raw_line.as_bytes());
        let result = self.normalize_inner(line_number, raw_line, &raw_sha256);
        match result {
            Ok(event) => {
                self.summary.canonical_rows += 1;
                if !event.quality_flags.is_empty() {
                    self.summary.warning_rows += 1;
                }
                for flag in &event.quality_flags {
                    *self.summary.flag_counts.entry(flag.clone()).or_default() += 1;
                }
                NormalizeOutcome::Canonical(Box::new(event))
            }
            Err(NormalizeError::UnsupportedRecordKind(record_kind)) => {
                self.summary.skipped_rows += 1;
                let reason_code = format!("record_kind:{record_kind}");
                *self
                    .summary
                    .skip_counts
                    .entry(reason_code.clone())
                    .or_default() += 1;
                NormalizeOutcome::Skipped {
                    line_number,
                    reason_code,
                }
            }
            Err(error) => {
                self.summary.quarantined_rows += 1;
                let reason_code = reason_code(&error).to_owned();
                *self
                    .summary
                    .reason_counts
                    .entry(reason_code.clone())
                    .or_default() += 1;
                NormalizeOutcome::Quarantined(QuarantineRecord {
                    schema_version: 1,
                    line_number,
                    raw_event_sha256: raw_sha256,
                    reason_code,
                    reason: error.to_string(),
                    raw_line: raw_line.to_owned(),
                })
            }
        }
    }

    #[must_use]
    pub fn summary(&self) -> &QualitySummary {
        &self.summary
    }

    fn normalize_inner(
        &mut self,
        line_number: u64,
        raw_line: &str,
        raw_sha256: &str,
    ) -> Result<CanonicalMarketEvent, NormalizeError> {
        if !self.seen_raw_events.insert(raw_sha256.to_owned())
            && self.policy.reject_duplicate_raw_event
        {
            return Err(NormalizeError::DuplicateRawEvent);
        }
        let raw: RawEventEnvelope = serde_json::from_str(raw_line)
            .map_err(|error| NormalizeError::InvalidJson(error.to_string()))?;
        raw.validate()?;

        let mut event =
            normalize_supported_event(&raw, raw_sha256, line_number, &self.input_sha256)?;
        apply_timestamp_quality(&raw, &self.policy, &mut event.quality_flags)?;
        apply_price_quality(&event, &self.policy)?;
        if self.policy.reject_crossed_book && is_crossed(&event)? {
            return Err(NormalizeError::CrossedBook);
        }
        self.apply_sequence_quality(&raw, &mut event.quality_flags)?;
        Ok(event)
    }

    fn apply_sequence_quality(
        &mut self,
        raw: &RawEventEnvelope,
        flags: &mut BTreeSet<String>,
    ) -> Result<(), NormalizeError> {
        let Some((start, end)) = raw.sequence_start.zip(raw.sequence_end) else {
            return Ok(());
        };
        let key = format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}",
            raw.session_id,
            raw.source,
            raw.stream,
            raw.instrument.as_deref().unwrap_or("")
        );
        if let Some(previous) = self.last_sequences.get(&key).copied() {
            if start <= previous && self.policy.reject_sequence_regression {
                return Err(NormalizeError::SequenceRegression);
            }
            if start > previous.saturating_add(1) {
                flags.insert("sequence_gap".into());
            }
        }
        self.last_sequences.insert(key, end);
        Ok(())
    }
}

fn normalize_supported_event(
    raw: &RawEventEnvelope,
    raw_sha256: &str,
    line_number: u64,
    input_sha256: &str,
) -> Result<CanonicalMarketEvent, NormalizeError> {
    if raw.record_kind != "market_data" {
        return Err(NormalizeError::UnsupportedRecordKind(
            raw.record_kind.clone(),
        ));
    }
    let payload = raw
        .payload
        .as_ref()
        .and_then(Value::as_object)
        .ok_or(NormalizeError::MissingPayloadField("payload"))?;
    let mut event = base_event(raw, raw_sha256, line_number, input_sha256)?;

    match (raw.source.as_str(), raw.event_type.as_str()) {
        ("binance", "trade") => {
            event.event_kind = CanonicalEventKind::Trade;
            event.price = Some(positive_decimal(payload.get("p"), "p")?);
            event.quantity = Some(positive_decimal(payload.get("q"), "q")?);
        }
        ("binance", "bookTicker") => {
            event.event_kind = CanonicalEventKind::BestBidAsk;
            event.best_bid = Some(positive_decimal(payload.get("b"), "b")?);
            event.best_ask = Some(positive_decimal(payload.get("a"), "a")?);
        }
        ("binance", "depthUpdate") => {
            event.event_kind = CanonicalEventKind::BookDelta;
            event.bids = parse_levels(payload.get("b"), "b")?;
            event.asks = parse_levels(payload.get("a"), "a")?;
            flag_empty_book(&mut event);
        }
        ("polymarket", "trade" | "last_trade_price") => {
            event.event_kind = CanonicalEventKind::Trade;
            event.price = Some(positive_decimal(
                payload.get("price").or_else(|| payload.get("p")),
                "price",
            )?);
            event.quantity = payload
                .get("size")
                .or_else(|| payload.get("quantity"))
                .map(|value| positive_decimal(Some(value), "size"))
                .transpose()?;
        }
        ("polymarket", "best_bid_ask") => {
            event.event_kind = CanonicalEventKind::BestBidAsk;
            event.best_bid = Some(positive_decimal(payload.get("best_bid"), "best_bid")?);
            event.best_ask = Some(positive_decimal(payload.get("best_ask"), "best_ask")?);
        }
        ("polymarket", "book") => {
            event.event_kind = CanonicalEventKind::BookSnapshot;
            event.bids = parse_levels(payload.get("bids"), "bids")?;
            event.asks = parse_levels(payload.get("asks"), "asks")?;
            flag_empty_book(&mut event);
        }
        ("polymarket", "price_change") => populate_polymarket_delta(payload, &mut event)?,
        _ => {
            return Err(NormalizeError::UnsupportedEvent {
                venue: raw.source.clone(),
                event_type: raw.event_type.clone(),
            });
        }
    }
    Ok(event)
}

fn base_event(
    raw: &RawEventEnvelope,
    raw_sha256: &str,
    line_number: u64,
    input_sha256: &str,
) -> Result<CanonicalMarketEvent, NormalizeError> {
    Ok(CanonicalMarketEvent {
        schema_version: CANONICAL_EVENT_SCHEMA_VERSION,
        canonical_event_id: format!("{raw_sha256}:0"),
        event_kind: CanonicalEventKind::Trade,
        source: raw.source.clone(),
        stream: raw.stream.clone(),
        session_id: raw.session_id.clone(),
        instrument: raw
            .instrument
            .clone()
            .ok_or(NormalizeError::MissingPayloadField("instrument"))?,
        market_id: string_value(raw.extra.get("market")),
        outcome_id: string_value(raw.extra.get("asset_id")),
        source_event_ts_ms: raw.source_event_ts_ms.or(raw.source_trade_ts_ms),
        available_at_ms: raw.recv_wall_ts_ms,
        recv_mono_ns: raw.recv_mono_ns.clone(),
        sequence_start: raw.sequence_start,
        sequence_end: raw.sequence_end,
        price: None,
        quantity: None,
        best_bid: None,
        best_ask: None,
        bids: Vec::new(),
        asks: Vec::new(),
        quality_flags: BTreeSet::new(),
        lineage: RawLineage {
            input_sha256: input_sha256.to_owned(),
            line_number,
            raw_event_sha256: raw_sha256.to_owned(),
            raw_schema_version: raw.schema_version,
        },
    })
}

fn populate_polymarket_delta(
    payload: &serde_json::Map<String, Value>,
    event: &mut CanonicalMarketEvent,
) -> Result<(), NormalizeError> {
    event.event_kind = CanonicalEventKind::BookDelta;
    let changes = payload
        .get("price_changes")
        .and_then(Value::as_array)
        .ok_or(NormalizeError::MissingPayloadField("price_changes"))?;
    for change in changes {
        let side = change.get("side").and_then(Value::as_str).unwrap_or("");
        let level = PriceLevel {
            price: positive_decimal(change.get("price"), "price")?,
            quantity: decimal(change.get("size"), "size")?,
        };
        match side.to_ascii_uppercase().as_str() {
            "BUY" | "BID" => event.bids.push(level),
            "SELL" | "ASK" => event.asks.push(level),
            _ => return Err(NormalizeError::MissingPayloadField("side")),
        }
    }
    flag_empty_book(event);
    Ok(())
}

fn flag_empty_book(event: &mut CanonicalMarketEvent) {
    if event.bids.is_empty() && event.asks.is_empty() {
        event.quality_flags.insert("empty_book".into());
    } else if event.bids.is_empty() || event.asks.is_empty() {
        event.quality_flags.insert("one_sided_book".into());
    }
}

fn apply_timestamp_quality(
    raw: &RawEventEnvelope,
    policy: &QualityPolicy,
    flags: &mut BTreeSet<String>,
) -> Result<(), NormalizeError> {
    let Some(source_timestamp) = raw.source_event_ts_ms.or(raw.source_trade_ts_ms) else {
        flags.insert("missing_source_timestamp".into());
        return Ok(());
    };
    let latency = raw.recv_wall_ts_ms - source_timestamp;
    if latency < -policy.timestamp_future_tolerance_ms {
        return Err(NormalizeError::FutureTimestamp);
    }
    if latency < 0.0 {
        flags.insert("source_timestamp_after_receive".into());
    }
    if latency > policy.stale_event_threshold_ms {
        flags.insert("stale_event".into());
    }
    Ok(())
}

fn apply_price_quality(
    event: &CanonicalMarketEvent,
    policy: &QualityPolicy,
) -> Result<(), NormalizeError> {
    if event.source != "polymarket" {
        return Ok(());
    }
    for (field, value) in event
        .price
        .iter()
        .map(|value| ("price", value))
        .chain(event.best_bid.iter().map(|value| ("best_bid", value)))
        .chain(event.best_ask.iter().map(|value| ("best_ask", value)))
        .chain(event.bids.iter().map(|level| ("bid.price", &level.price)))
        .chain(event.asks.iter().map(|level| ("ask.price", &level.price)))
    {
        let numeric = value
            .parse::<f64>()
            .map_err(|_| NormalizeError::InvalidDecimal("price"))?;
        if numeric < policy.event_contract_price_min || numeric > policy.event_contract_price_max {
            return Err(NormalizeError::EventContractPriceOutOfRange { field });
        }
    }
    Ok(())
}

fn is_crossed(event: &CanonicalMarketEvent) -> Result<bool, NormalizeError> {
    if !matches!(
        event.event_kind,
        CanonicalEventKind::BestBidAsk | CanonicalEventKind::BookSnapshot
    ) {
        return Ok(false);
    }
    let direct = event.best_bid.as_ref().zip(event.best_ask.as_ref());
    if let Some((bid, ask)) = direct {
        return Ok(parse_decimal(bid, "best_bid")? > parse_decimal(ask, "best_ask")?);
    }
    let best_bid = event
        .bids
        .iter()
        .map(|level| parse_decimal(&level.price, "bid.price"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .reduce(f64::max);
    let best_ask = event
        .asks
        .iter()
        .map(|level| parse_decimal(&level.price, "ask.price"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .reduce(f64::min);
    Ok(best_bid.zip(best_ask).is_some_and(|(bid, ask)| bid > ask))
}

fn parse_levels(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Vec<PriceLevel>, NormalizeError> {
    let levels = value
        .and_then(Value::as_array)
        .ok_or(NormalizeError::MissingPayloadField(field))?;
    levels
        .iter()
        .map(|level| {
            if let Some(items) = level.as_array() {
                if items.len() < 2 {
                    return Err(NormalizeError::InvalidBookLevel(field));
                }
                return Ok(PriceLevel {
                    price: positive_decimal(items.first(), field)?,
                    quantity: decimal(items.get(1), field)?,
                });
            }
            if level.is_object() {
                return Ok(PriceLevel {
                    price: positive_decimal(level.get("price"), field)?,
                    quantity: decimal(level.get("size").or_else(|| level.get("quantity")), field)?,
                });
            }
            Err(NormalizeError::InvalidBookLevel(field))
        })
        .collect()
}

fn positive_decimal(value: Option<&Value>, field: &'static str) -> Result<String, NormalizeError> {
    let encoded = decimal(value, field)?;
    if parse_decimal(&encoded, field)? <= 0.0 {
        return Err(NormalizeError::NonPositiveDecimal(field));
    }
    Ok(encoded)
}

fn decimal(value: Option<&Value>, field: &'static str) -> Result<String, NormalizeError> {
    let value = value.ok_or(NormalizeError::MissingPayloadField(field))?;
    let encoded = match value {
        Value::String(value) => value.trim().to_owned(),
        Value::Number(value) => value.to_string(),
        _ => return Err(NormalizeError::InvalidDecimal(field)),
    };
    parse_decimal(&encoded, field)?;
    Ok(encoded)
}

fn parse_decimal(value: &str, field: &'static str) -> Result<f64, NormalizeError> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| NormalizeError::InvalidDecimal(field))?;
    if !parsed.is_finite() {
        return Err(NormalizeError::InvalidDecimal(field));
    }
    Ok(parsed)
}

fn string_value(value: Option<&Value>) -> Option<String> {
    value.and_then(|item| match item {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    })
}

fn reason_code(error: &NormalizeError) -> &'static str {
    match error {
        NormalizeError::InvalidPolicy(_) => "invalid_policy",
        NormalizeError::InvalidJson(_) => "invalid_json",
        NormalizeError::InvalidRawContract(_) => "invalid_raw_contract",
        NormalizeError::UnsupportedEvent { .. } => "unsupported_event",
        NormalizeError::UnsupportedRecordKind(_) => "unsupported_record_kind",
        NormalizeError::MissingPayloadField(_) => "missing_payload_field",
        NormalizeError::InvalidDecimal(_) => "invalid_decimal",
        NormalizeError::NonPositiveDecimal(_) => "non_positive_decimal",
        NormalizeError::EventContractPriceOutOfRange { .. } => "price_out_of_range",
        NormalizeError::InvalidBookLevel(_) => "invalid_book_level",
        NormalizeError::CrossedBook => "crossed_book",
        NormalizeError::FutureTimestamp => "future_timestamp",
        NormalizeError::DuplicateRawEvent => "duplicate_raw_event",
        NormalizeError::SequenceRegression => "sequence_regression",
    }
}

#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> QualityPolicy {
        QualityPolicy {
            schema_version: 1,
            policy_version: "quality-v1".into(),
            timestamp_future_tolerance_ms: 1_000.0,
            stale_event_threshold_ms: 60_000.0,
            reject_duplicate_raw_event: true,
            reject_sequence_regression: true,
            reject_crossed_book: true,
            event_contract_price_min: 0.0,
            event_contract_price_max: 1.0,
        }
    }

    fn normalize_one(line: &str) -> NormalizeOutcome {
        Normalizer::new(policy(), "input-sha".into())
            .expect("policy")
            .normalize_line(1, line)
    }

    #[test]
    fn binance_trade_preserves_decimal_strings_and_lineage() {
        let line = r#"{"schema_version":1,"record_kind":"market_data","session_id":"s","source":"binance","stream":"btcusdt@trade","instrument":"BTCUSDT","event_type":"trade","source_event_ts_ms":1000,"recv_wall_ts_ms":1002,"recv_mono_ns":"1","sequence_start":42,"sequence_end":42,"payload":{"p":"60000.0100","q":"0.0010"}}"#;
        let NormalizeOutcome::Canonical(event) = normalize_one(line) else {
            panic!("expected canonical event");
        };
        assert_eq!(event.price.as_deref(), Some("60000.0100"));
        assert_eq!(event.quantity.as_deref(), Some("0.0010"));
        assert_eq!(event.lineage.line_number, 1);
        assert_eq!(event.lineage.input_sha256, "input-sha");
    }

    #[test]
    fn crossed_polymarket_bbo_is_quarantined() {
        let line = r#"{"schema_version":1,"record_kind":"market_data","session_id":"s","source":"polymarket","stream":"market","instrument":"yes","event_type":"best_bid_ask","source_event_ts_ms":1000,"recv_wall_ts_ms":1002,"recv_mono_ns":"1","payload":{"best_bid":"0.61","best_ask":"0.60"}}"#;
        let NormalizeOutcome::Quarantined(record) = normalize_one(line) else {
            panic!("expected quarantine");
        };
        assert_eq!(record.reason_code, "crossed_book");
    }

    #[test]
    fn duplicate_and_sequence_regression_are_visible() {
        let first = r#"{"schema_version":1,"record_kind":"market_data","session_id":"s","source":"binance","stream":"btcusdt@trade","instrument":"BTCUSDT","event_type":"trade","source_event_ts_ms":1000,"recv_wall_ts_ms":1002,"recv_mono_ns":"1","sequence_start":42,"sequence_end":42,"payload":{"p":"1","q":"1"}}"#;
        let second = r#"{"schema_version":1,"record_kind":"market_data","session_id":"s","source":"binance","stream":"btcusdt@trade","instrument":"BTCUSDT","event_type":"trade","source_event_ts_ms":1001,"recv_wall_ts_ms":1003,"recv_mono_ns":"2","sequence_start":41,"sequence_end":41,"payload":{"p":"1","q":"1"}}"#;
        let mut normalizer = Normalizer::new(policy(), "input-sha".into()).expect("policy");
        assert!(matches!(
            normalizer.normalize_line(1, first),
            NormalizeOutcome::Canonical(_)
        ));
        let NormalizeOutcome::Quarantined(duplicate) = normalizer.normalize_line(2, first) else {
            panic!("expected duplicate quarantine");
        };
        assert_eq!(duplicate.reason_code, "duplicate_raw_event");
        let NormalizeOutcome::Quarantined(regression) = normalizer.normalize_line(3, second) else {
            panic!("expected sequence quarantine");
        };
        assert_eq!(regression.reason_code, "sequence_regression");
    }

    #[test]
    fn missing_timestamp_and_one_sided_book_are_quality_flags() {
        let line = r#"{"schema_version":1,"record_kind":"market_data","session_id":"s","source":"polymarket","stream":"market","instrument":"yes","event_type":"book","recv_wall_ts_ms":1002,"recv_mono_ns":"1","payload":{"bids":[{"price":"0.50","size":"2"}],"asks":[]}}"#;
        let NormalizeOutcome::Canonical(event) = normalize_one(line) else {
            panic!("expected canonical event");
        };
        assert!(event.quality_flags.contains("missing_source_timestamp"));
        assert!(event.quality_flags.contains("one_sided_book"));
    }

    #[test]
    fn a_depth_delta_is_not_mistaken_for_a_crossed_snapshot() {
        let line = r#"{"schema_version":1,"record_kind":"market_data","session_id":"s","source":"binance","stream":"btcusdt@depth@100ms","instrument":"BTCUSDT","event_type":"depthUpdate","source_event_ts_ms":1000,"recv_wall_ts_ms":1002,"recv_mono_ns":"1","sequence_start":1,"sequence_end":1,"payload":{"b":[["101","1"]],"a":[["100","1"]]}}"#;
        let NormalizeOutcome::Canonical(event) = normalize_one(line) else {
            panic!("a delta alone cannot establish a crossed book");
        };
        assert_eq!(event.event_kind, CanonicalEventKind::BookDelta);
    }

    #[test]
    fn connection_lifecycle_is_skipped_not_quarantined() {
        let line = r#"{"schema_version":1,"record_kind":"connection","session_id":"s","source":"binance","stream":"btcusdt","event_type":"connection_open","recv_wall_ts_ms":1002,"recv_mono_ns":"1"}"#;
        let mut normalizer = Normalizer::new(policy(), "input-sha".into()).expect("policy");
        assert!(matches!(
            normalizer.normalize_line(1, line),
            NormalizeOutcome::Skipped { .. }
        ));
        assert_eq!(normalizer.summary().skipped_rows, 1);
        assert_eq!(normalizer.summary().quarantined_rows, 0);
    }
}
