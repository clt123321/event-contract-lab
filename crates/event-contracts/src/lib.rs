//! Versioned contracts shared by collection, recovery, replay, and research tooling.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const RAW_EVENT_SCHEMA_VERSION: u32 = 1;
pub const SEGMENT_MANIFEST_SCHEMA_VERSION: u32 = 1;
pub const DATASET_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContractError {
    #[error("unsupported raw event schema version {0}")]
    UnsupportedSchema(u32),
    #[error("required field `{0}` is empty")]
    EmptyField(&'static str),
    #[error("recv_wall_ts_ms must be finite and positive")]
    InvalidReceiveWallTime,
    #[error("recv_mono_ns must contain a positive integer")]
    InvalidReceiveMonotonicTime,
    #[error("source timestamp `{0}` must be finite and positive when present")]
    InvalidSourceTime(&'static str),
    #[error("sequence_start and sequence_end must either both be present or both be absent")]
    IncompleteSequenceRange,
    #[error("sequence_start cannot be greater than sequence_end")]
    InvalidSequenceRange,
    #[error("market mapping requires at least one evidence item")]
    MissingMappingEvidence,
    #[error("market mapping confidence must be between 0 and 1")]
    InvalidMappingConfidence,
}

/// The immutable source envelope. Unknown source fields are retained in `extra` so that a parser
/// rollout cannot silently erase evidence that was already present in the collector output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawEventEnvelope {
    pub schema_version: u32,
    pub record_kind: String,
    pub session_id: String,
    pub source: String,
    pub stream: String,
    #[serde(default)]
    pub instrument: Option<String>,
    pub event_type: String,
    #[serde(default)]
    pub source_event_ts_ms: Option<f64>,
    #[serde(default)]
    pub source_trade_ts_ms: Option<f64>,
    pub recv_wall_ts_ms: f64,
    pub recv_mono_ns: String,
    #[serde(default)]
    pub sequence_start: Option<i64>,
    #[serde(default)]
    pub sequence_end: Option<i64>,
    #[serde(default)]
    pub payload: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl RawEventEnvelope {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.schema_version != RAW_EVENT_SCHEMA_VERSION {
            return Err(ContractError::UnsupportedSchema(self.schema_version));
        }
        for (name, value) in [
            ("record_kind", self.record_kind.as_str()),
            ("session_id", self.session_id.as_str()),
            ("source", self.source.as_str()),
            ("stream", self.stream.as_str()),
            ("event_type", self.event_type.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ContractError::EmptyField(name));
            }
        }
        if !self.recv_wall_ts_ms.is_finite() || self.recv_wall_ts_ms <= 0.0 {
            return Err(ContractError::InvalidReceiveWallTime);
        }
        if !matches!(self.recv_mono_ns.parse::<u128>(), Ok(value) if value > 0) {
            return Err(ContractError::InvalidReceiveMonotonicTime);
        }
        for (name, value) in [
            ("source_event_ts_ms", self.source_event_ts_ms),
            ("source_trade_ts_ms", self.source_trade_ts_ms),
        ] {
            if value.is_some_and(|timestamp| !timestamp.is_finite() || timestamp <= 0.0) {
                return Err(ContractError::InvalidSourceTime(name));
            }
        }
        if self.sequence_start.is_some() != self.sequence_end.is_some() {
            return Err(ContractError::IncompleteSequenceRange);
        }
        if self
            .sequence_start
            .zip(self.sequence_end)
            .is_some_and(|(start, end)| start > end)
        {
            return Err(ContractError::InvalidSequenceRange);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketIdentity {
    pub source: String,
    pub market_id: String,
    pub event_id: Option<String>,
    pub outcome_id: Option<String>,
    pub rules_fingerprint: String,
    pub close_time_ms: Option<i64>,
    pub resolution_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MappingEvidence {
    pub evidence_type: String,
    pub reference: String,
    pub observed_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketMapping {
    pub mapping_id: String,
    pub left: MarketIdentity,
    pub right: MarketIdentity,
    pub confidence: f64,
    pub evidence: Vec<MappingEvidence>,
    pub manually_approved_by: Option<String>,
    pub manually_approved_at_ms: Option<i64>,
}

impl MarketMapping {
    pub fn validate(&self) -> Result<(), ContractError> {
        if !(0.0..=1.0).contains(&self.confidence) {
            return Err(ContractError::InvalidMappingConfidence);
        }
        if self.evidence.is_empty() {
            return Err(ContractError::MissingMappingEvidence);
        }
        Ok(())
    }

    #[must_use]
    pub fn is_research_ready(&self) -> bool {
        self.validate().is_ok()
            && self.manually_approved_by.is_some()
            && self.manually_approved_at_ms.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SegmentManifest {
    pub schema_version: u32,
    pub segment_file: String,
    pub sha256: String,
    pub row_count: u64,
    pub byte_count: u64,
    pub min_recv_wall_ts_ms: f64,
    pub max_recv_wall_ts_ms: f64,
    pub min_source_event_ts_ms: Option<f64>,
    pub max_source_event_ts_ms: Option<f64>,
    pub sources: BTreeSet<String>,
    pub streams: BTreeSet<String>,
    pub schema_versions: BTreeSet<u32>,
    pub git_commit: Option<String>,
    pub sealed_at_ms: u64,
    pub recovered_after_restart: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatasetManifest {
    pub schema_version: u32,
    pub dataset_id: String,
    pub created_at_ms: u64,
    pub segment_manifests: Vec<String>,
    pub feature_version: String,
    pub quality_mask_version: String,
    pub code_commit: String,
    pub random_seed: u64,
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_event() -> RawEventEnvelope {
        RawEventEnvelope {
            schema_version: 1,
            record_kind: "market_data".into(),
            session_id: "test-session".into(),
            source: "binance".into(),
            stream: "btcusdt@trade".into(),
            instrument: Some("BTCUSDT".into()),
            event_type: "trade".into(),
            source_event_ts_ms: Some(1_787_000_000_000.0),
            source_trade_ts_ms: Some(1_787_000_000_000.0),
            recv_wall_ts_ms: 1_787_000_000_001.0,
            recv_mono_ns: "123456789".into(),
            sequence_start: Some(42),
            sequence_end: Some(42),
            payload: Some(serde_json::json!({"p": "60000.1"})),
            extra: BTreeMap::from([("arrival_latency_ms".into(), serde_json::json!(1.0))]),
        }
    }

    #[test]
    fn raw_event_round_trip_preserves_extra_fields() {
        let event = valid_event();
        let encoded = serde_json::to_string(&event).expect("serialize");
        let decoded: RawEventEnvelope = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, event);
        assert_eq!(decoded.extra["arrival_latency_ms"], serde_json::json!(1.0));
        assert_eq!(decoded.validate(), Ok(()));
    }

    #[test]
    fn invalid_sequence_range_is_rejected() {
        let mut event = valid_event();
        event.sequence_start = Some(2);
        event.sequence_end = Some(1);
        assert_eq!(event.validate(), Err(ContractError::InvalidSequenceRange));
    }

    #[test]
    fn incomplete_sequence_range_is_rejected() {
        let mut event = valid_event();
        event.sequence_end = None;
        assert_eq!(
            event.validate(),
            Err(ContractError::IncompleteSequenceRange)
        );
    }

    #[test]
    fn mappings_require_evidence_and_manual_approval_for_research() {
        let identity = MarketIdentity {
            source: "venue".into(),
            market_id: "m1".into(),
            event_id: None,
            outcome_id: Some("yes".into()),
            rules_fingerprint: "sha256:example".into(),
            close_time_ms: None,
            resolution_source: None,
        };
        let mapping = MarketMapping {
            mapping_id: "map-1".into(),
            left: identity.clone(),
            right: identity,
            confidence: 0.9,
            evidence: Vec::new(),
            manually_approved_by: None,
            manually_approved_at_ms: None,
        };
        assert_eq!(
            mapping.validate(),
            Err(ContractError::MissingMappingEvidence)
        );
        assert!(!mapping.is_research_ready());
    }
}
