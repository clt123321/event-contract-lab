//! Point-in-time deterministic replay implementation.

use std::cmp::Ordering;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use dataset_core::{DatasetError, load_dataset};
use event_contracts::{
    CanonicalMarketEvent, FileArtifact, REPLAY_MANIFEST_SCHEMA_VERSION, ReplayFrame, ReplayManifest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const REPLAY_ENGINE_VERSION: &str = "point-in-time-replay-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayConfig {
    pub schema_version: u32,
    pub config_version: String,
    pub random_seed: u64,
    pub maximum_events: Option<u64>,
}

impl ReplayConfig {
    pub fn validate(&self) -> Result<(), ReplayError> {
        if self.schema_version != 1 {
            return Err(ReplayError::InvalidConfig(
                "schema_version must equal 1".into(),
            ));
        }
        if self.config_version.trim().is_empty() {
            return Err(ReplayError::InvalidConfig(
                "config_version cannot be empty".into(),
            ));
        }
        if self.maximum_events == Some(0) {
            return Err(ReplayError::InvalidConfig(
                "maximum_events must be positive when present".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ReplayRequest {
    pub dataset_manifest: PathBuf,
    pub replay_config: PathBuf,
    pub output: PathBuf,
    pub replay_manifest_output: PathBuf,
    pub code_commit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayResult {
    pub status: String,
    pub replay_id: String,
    pub dataset_id: String,
    pub event_count: u64,
    pub output: String,
    pub replay_manifest: String,
}

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("dataset error: {0}")]
    Dataset(#[from] DatasetError),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("invalid replay config: {0}")]
    InvalidConfig(String),
    #[error("recv_mono_ns is not a positive integer for event {0}")]
    InvalidMonotonicTime(String),
    #[error("refusing to overwrite existing artifact: {0}")]
    ArtifactExists(PathBuf),
    #[error("artifact path has no file name: {0}")]
    MissingFileName(PathBuf),
    #[error("numeric conversion failed: {0}")]
    NumericConversion(#[from] std::num::TryFromIntError),
}

pub fn run_replay(request: &ReplayRequest) -> Result<ReplayResult, ReplayError> {
    ensure_new(&request.output)?;
    ensure_new(&request.replay_manifest_output)?;
    if request.code_commit.trim().is_empty() {
        return Err(ReplayError::InvalidConfig(
            "code commit cannot be empty".into(),
        ));
    }

    let dataset_bytes = read_bytes(&request.dataset_manifest)?;
    let dataset_sha256 = sha256_hex(&dataset_bytes);
    let (dataset, mut events) = load_dataset(&request.dataset_manifest)?;
    let config_bytes = read_bytes(&request.replay_config)?;
    let config_sha256 = sha256_hex(&config_bytes);
    let config: ReplayConfig = serde_json::from_slice(&config_bytes)?;
    config.validate()?;

    validate_monotonic_times(&events)?;
    events.sort_by(point_in_time_order);
    if let Some(maximum) = config.maximum_events {
        events.truncate(usize::try_from(maximum)?);
    }
    let frames = replay_frames(events)?;
    let output_artifact = write_frames(&request.output, &frames)?;
    let replay_id = sha256_hex(
        format!(
            "{}\n{}\n{}\n{}\n{}",
            dataset.dataset_id,
            dataset_sha256,
            config_sha256,
            request.code_commit,
            output_artifact.sha256
        )
        .as_bytes(),
    );
    let manifest = ReplayManifest {
        schema_version: REPLAY_MANIFEST_SCHEMA_VERSION,
        replay_id: replay_id.clone(),
        replay_engine_version: REPLAY_ENGINE_VERSION.into(),
        dataset_manifest_file: file_name(&request.dataset_manifest)?,
        dataset_manifest_sha256: dataset_sha256,
        dataset_id: dataset.dataset_id.clone(),
        replay_config_version: config.config_version,
        replay_config_sha256: config_sha256,
        code_commit: request.code_commit.clone(),
        random_seed: config.random_seed,
        event_count: u64::try_from(frames.len())?,
        first_virtual_time_ms: frames
            .first()
            .map(|frame| stable_float(frame.virtual_time_ms)),
        last_virtual_time_ms: frames
            .last()
            .map(|frame| stable_float(frame.virtual_time_ms)),
        output: output_artifact,
    };
    write_json_exclusive(&request.replay_manifest_output, &manifest)?;
    Ok(ReplayResult {
        status: "replayed".into(),
        replay_id,
        dataset_id: dataset.dataset_id,
        event_count: manifest.event_count,
        output: request.output.display().to_string(),
        replay_manifest: request.replay_manifest_output.display().to_string(),
    })
}

fn replay_frames(events: Vec<CanonicalMarketEvent>) -> Result<Vec<ReplayFrame>, ReplayError> {
    let first_time = events.first().map(|event| event.available_at_ms);
    events
        .into_iter()
        .enumerate()
        .map(|(index, event)| {
            let elapsed = first_time.map_or(0.0, |first| event.available_at_ms - first);
            Ok(ReplayFrame {
                schema_version: 1,
                replay_sequence: u64::try_from(index)?,
                virtual_time_ms: event.available_at_ms,
                elapsed_virtual_ms: elapsed,
                event,
            })
        })
        .collect()
}

fn point_in_time_order(left: &CanonicalMarketEvent, right: &CanonicalMarketEvent) -> Ordering {
    left.available_at_ms
        .total_cmp(&right.available_at_ms)
        .then_with(|| left.source.cmp(&right.source))
        .then_with(|| left.session_id.cmp(&right.session_id))
        .then_with(|| monotonic_value(left).cmp(&monotonic_value(right)))
        .then_with(|| left.canonical_event_id.cmp(&right.canonical_event_id))
}

fn validate_monotonic_times(events: &[CanonicalMarketEvent]) -> Result<(), ReplayError> {
    for event in events {
        if monotonic_value(event) == 0 {
            return Err(ReplayError::InvalidMonotonicTime(
                event.canonical_event_id.clone(),
            ));
        }
    }
    Ok(())
}

fn monotonic_value(event: &CanonicalMarketEvent) -> u128 {
    event.recv_mono_ns.parse::<u128>().unwrap_or(0)
}

fn write_frames(path: &Path, frames: &[ReplayFrame]) -> Result<FileArtifact, ReplayError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| io_error(path, source))?;
    let mut hasher = Sha256::new();
    let mut byte_count = 0_u64;
    for frame in frames {
        let mut bytes = serde_json::to_vec(frame)?;
        bytes.push(b'\n');
        file.write_all(&bytes)
            .map_err(|source| io_error(path, source))?;
        hasher.update(&bytes);
        byte_count += u64::try_from(bytes.len())?;
    }
    file.sync_all().map_err(|source| io_error(path, source))?;
    Ok(FileArtifact {
        file: file_name(path)?,
        sha256: format!("{:x}", hasher.finalize()),
        byte_count,
        row_count: u64::try_from(frames.len())?,
    })
}

fn ensure_new(path: &Path) -> Result<(), ReplayError> {
    if path.exists() {
        return Err(ReplayError::ArtifactExists(path.to_owned()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    }
    Ok(())
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, ReplayError> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|source| io_error(path, source))?;
    Ok(bytes)
}

fn write_json_exclusive<T: Serialize>(path: &Path, value: &T) -> Result<(), ReplayError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| io_error(path, source))?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|source| io_error(path, source))
}

fn file_name(path: &Path) -> Result<String, ReplayError> {
    path.file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .ok_or_else(|| ReplayError::MissingFileName(path.to_owned()))
}

fn stable_float(value: f64) -> String {
    value.to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn io_error(path: impl Into<PathBuf>, source: std::io::Error) -> ReplayError {
    ReplayError::Io {
        path: path.into(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    use dataset_core::{dataset_identity_v2, write_canonical_parquet};
    use event_contracts::{
        CanonicalEventKind, DatasetInput, DatasetManifestV2, RawLineage, ReplayFrame,
    };

    fn event(id: &str, time: f64, source: &str, mono: &str) -> CanonicalMarketEvent {
        CanonicalMarketEvent {
            schema_version: 1,
            canonical_event_id: format!("{}:0", sha256_hex(id.as_bytes())),
            event_kind: CanonicalEventKind::Trade,
            source: source.into(),
            stream: "trade".into(),
            session_id: "session".into(),
            instrument: "BTCUSDT".into(),
            market_id: None,
            outcome_id: None,
            source_event_ts_ms: Some(time - 1.0),
            available_at_ms: time,
            recv_mono_ns: mono.into(),
            sequence_start: None,
            sequence_end: None,
            price: Some("1".into()),
            quantity: Some("1".into()),
            best_bid: None,
            best_ask: None,
            bids: Vec::new(),
            asks: Vec::new(),
            quality_flags: BTreeSet::new(),
            lineage: RawLineage {
                input_sha256: "a".repeat(64),
                line_number: 1,
                raw_event_sha256: sha256_hex(id.as_bytes()),
                raw_schema_version: 1,
            },
        }
    }

    fn dataset_fixture(root: &Path) -> PathBuf {
        let parquet = root.join("canonical.parquet");
        write_canonical_parquet(
            &parquet,
            &[
                event("late", 2_000.0, "binance", "2"),
                event("early-b", 1_000.0, "polymarket", "3"),
                event("early-a", 1_000.0, "binance", "1"),
            ],
        )
        .expect("parquet fixture");
        let parquet_bytes = fs::read(&parquet).expect("parquet bytes");
        let input = DatasetInput {
            transform_manifest_file: "transform.json".into(),
            transform_manifest_sha256: "c".repeat(64),
            transform_id: "d".repeat(64),
        };
        let parquet_artifact = FileArtifact {
            file: "canonical.parquet".into(),
            sha256: sha256_hex(&parquet_bytes),
            byte_count: u64::try_from(parquet_bytes.len()).expect("bytes"),
            row_count: 3,
        };
        let quality_mask_sha256 = "b".repeat(64);
        let code_commit = "dataset-commit";
        let manifest = DatasetManifestV2 {
            schema_version: 2,
            dataset_id: dataset_identity_v2(
                &input,
                &parquet_artifact,
                &quality_mask_sha256,
                code_commit,
            ),
            dataset_builder_version: dataset_core::DATASET_BUILDER_VERSION.into(),
            code_commit: code_commit.into(),
            canonical_schema_version: 1,
            parquet_schema_version: 1,
            quality_mask_version: "strict".into(),
            quality_mask_sha256,
            inputs: vec![input],
            parquet_files: vec![parquet_artifact],
            input_rows: 3,
            included_rows: 3,
            excluded_rows: 0,
            exclusion_counts: BTreeMap::new(),
            min_available_at_ms: Some(1_000.0),
            max_available_at_ms: Some(2_000.0),
            sources: ["binance".into(), "polymarket".into()]
                .into_iter()
                .collect(),
            instruments: ["BTCUSDT".into()].into_iter().collect(),
            parameters: BTreeMap::new(),
        };
        let path = root.join("dataset-manifest.json");
        write_json_exclusive(&path, &manifest).expect("dataset manifest");
        path
    }

    #[test]
    fn replay_is_point_in_time_ordered_and_byte_deterministic() {
        let root = tempfile::tempdir().expect("tempdir");
        let dataset = dataset_fixture(root.path());
        let config = root.path().join("replay.json");
        fs::write(
            &config,
            br#"{"schema_version":1,"config_version":"test","random_seed":42,"maximum_events":null}"#,
        )
        .expect("config");
        let run = |name: &str| {
            let directory = root.path().join(name);
            run_replay(&ReplayRequest {
                dataset_manifest: dataset.clone(),
                replay_config: config.clone(),
                output: directory.join("replay.ndjson"),
                replay_manifest_output: directory.join("replay-manifest.json"),
                code_commit: "replay-commit".into(),
            })
            .expect("replay")
        };
        let first = run("first");
        let second = run("second");
        assert_eq!(first.replay_id, second.replay_id);
        assert_eq!(
            fs::read(root.path().join("first/replay.ndjson")).expect("first output"),
            fs::read(root.path().join("second/replay.ndjson")).expect("second output")
        );
        assert_eq!(
            fs::read(root.path().join("first/replay-manifest.json")).expect("first manifest"),
            fs::read(root.path().join("second/replay-manifest.json")).expect("second manifest")
        );
        let frames = fs::read_to_string(root.path().join("first/replay.ndjson"))
            .expect("frames")
            .lines()
            .map(|line| serde_json::from_str::<ReplayFrame>(line).expect("frame"))
            .collect::<Vec<_>>();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].event.source, "binance");
        assert_eq!(frames[1].event.source, "polymarket");
        assert!((frames[2].virtual_time_ms - 2_000.0).abs() < f64::EPSILON);
        assert!((frames[2].elapsed_virtual_ms - 1_000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn invalid_monotonic_clock_is_rejected() {
        let events = vec![event("bad", 1_000.0, "binance", "not-a-number")];
        assert!(matches!(
            validate_monotonic_times(&events),
            Err(ReplayError::InvalidMonotonicTime(_))
        ));
    }
}
