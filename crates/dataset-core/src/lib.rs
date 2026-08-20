//! Frozen dataset and deterministic Parquet implementation.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::{
    Array, ArrayRef, Float64Array, Int64Array, RecordBatch, StringArray, UInt32Array, UInt64Array,
};
use arrow_schema::{DataType, Field, Schema};
use event_contracts::{
    CANONICAL_EVENT_SCHEMA_VERSION, CanonicalEventKind, CanonicalMarketEvent,
    DATASET_MANIFEST_V2_SCHEMA_VERSION, DatasetInput, DatasetManifestV2, FileArtifact,
    TransformManifest,
};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::arrow_writer::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const DATASET_BUILDER_VERSION: &str = "canonical-parquet-v1";
pub const PARQUET_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QualityMask {
    pub schema_version: u32,
    pub mask_version: String,
    pub exclude_unlisted_flags: bool,
    #[serde(default)]
    pub allowed_flags: BTreeSet<String>,
}

impl QualityMask {
    pub fn validate(&self) -> Result<(), DatasetError> {
        if self.schema_version != 1 {
            return Err(DatasetError::InvalidQualityMask(
                "schema_version must equal 1".into(),
            ));
        }
        if self.mask_version.trim().is_empty() {
            return Err(DatasetError::InvalidQualityMask(
                "mask_version cannot be empty".into(),
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn excluded_flags(&self, event: &CanonicalMarketEvent) -> BTreeSet<String> {
        if !self.exclude_unlisted_flags {
            return BTreeSet::new();
        }
        event
            .quality_flags
            .difference(&self.allowed_flags)
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct BuildRequest {
    pub transform_manifest: PathBuf,
    pub quality_mask: PathBuf,
    pub parquet_output: PathBuf,
    pub dataset_manifest_output: PathBuf,
    pub code_commit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildResult {
    pub status: String,
    pub dataset_id: String,
    pub input_rows: u64,
    pub included_rows: u64,
    pub excluded_rows: u64,
    pub parquet_file: String,
    pub dataset_manifest: String,
}

#[derive(Debug, Error)]
pub enum DatasetError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("invalid quality mask: {0}")]
    InvalidQualityMask(String),
    #[error("artifact verification failed: {0}")]
    ArtifactVerification(String),
    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow_schema::ArrowError),
    #[error("Parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),
    #[error("numeric conversion failed: {0}")]
    NumericConversion(#[from] std::num::TryFromIntError),
    #[error("refusing to overwrite existing artifact: {0}")]
    ArtifactExists(PathBuf),
    #[error("artifact path has no file name: {0}")]
    MissingFileName(PathBuf),
    #[error("Parquet round-trip differs from canonical input")]
    RoundTripMismatch,
    #[error("dataset would contain zero rows after applying the quality mask")]
    EmptyDataset,
}

pub fn build_dataset(request: &BuildRequest) -> Result<BuildResult, DatasetError> {
    ensure_new(&request.parquet_output)?;
    ensure_new(&request.dataset_manifest_output)?;
    if request.code_commit.trim().is_empty() {
        return Err(DatasetError::ArtifactVerification(
            "code commit cannot be empty".into(),
        ));
    }

    let transform_bytes = read_bytes(&request.transform_manifest)?;
    let transform_sha256 = sha256_hex(&transform_bytes);
    let transform: TransformManifest = serde_json::from_slice(&transform_bytes)?;
    let transform_directory = request
        .transform_manifest
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let canonical_path = transform_directory.join(&transform.canonical_output.file);
    verify_artifact(&canonical_path, &transform.canonical_output)?;

    let mask_bytes = read_bytes(&request.quality_mask)?;
    let mask_sha256 = sha256_hex(&mask_bytes);
    let mask: QualityMask = serde_json::from_slice(&mask_bytes)?;
    mask.validate()?;

    let filtered = load_and_filter(&canonical_path, &mask)?;
    let events = filtered.events;
    let input_rows = filtered.input_rows;
    let exclusion_counts = filtered.exclusion_counts;
    if events.is_empty() {
        return Err(DatasetError::EmptyDataset);
    }
    write_canonical_parquet(&request.parquet_output, &events)?;
    let round_trip = read_parquet(&request.parquet_output)?;
    if round_trip != events {
        return Err(DatasetError::RoundTripMismatch);
    }

    let parquet_artifact = file_artifact(&request.parquet_output, u64::try_from(events.len())?)?;
    let sources: BTreeSet<_> = events.iter().map(|event| event.source.clone()).collect();
    let instruments: BTreeSet<_> = events
        .iter()
        .map(|event| event.instrument.clone())
        .collect();
    let min_available_at_ms = events
        .iter()
        .map(|event| event.available_at_ms)
        .reduce(f64::min);
    let max_available_at_ms = events
        .iter()
        .map(|event| event.available_at_ms)
        .reduce(f64::max);
    let included_rows = u64::try_from(events.len())?;
    let excluded_rows = input_rows.saturating_sub(included_rows);
    let input = DatasetInput {
        transform_manifest_file: file_name(&request.transform_manifest)?,
        transform_manifest_sha256: transform_sha256,
        transform_id: transform.transform_id,
    };
    let dataset_id = dataset_identity_v2(
        &input,
        &parquet_artifact,
        &mask_sha256,
        &request.code_commit,
    );
    let manifest = DatasetManifestV2 {
        schema_version: DATASET_MANIFEST_V2_SCHEMA_VERSION,
        dataset_id: dataset_id.clone(),
        dataset_builder_version: DATASET_BUILDER_VERSION.into(),
        code_commit: request.code_commit.clone(),
        canonical_schema_version: CANONICAL_EVENT_SCHEMA_VERSION,
        parquet_schema_version: PARQUET_SCHEMA_VERSION,
        quality_mask_version: mask.mask_version,
        quality_mask_sha256: mask_sha256,
        inputs: vec![input],
        parquet_files: vec![parquet_artifact],
        input_rows,
        included_rows,
        excluded_rows,
        exclusion_counts,
        min_available_at_ms,
        max_available_at_ms,
        sources,
        instruments,
        parameters: BTreeMap::from([
            ("compression".into(), serde_json::json!("zstd-default")),
            ("max_row_group_rows".into(), serde_json::json!(65_536)),
        ]),
    };
    write_json_exclusive(&request.dataset_manifest_output, &manifest)?;
    Ok(BuildResult {
        status: "frozen".into(),
        dataset_id,
        input_rows,
        included_rows,
        excluded_rows,
        parquet_file: request.parquet_output.display().to_string(),
        dataset_manifest: request.dataset_manifest_output.display().to_string(),
    })
}

pub fn load_dataset(
    manifest_path: &Path,
) -> Result<(DatasetManifestV2, Vec<CanonicalMarketEvent>), DatasetError> {
    let manifest: DatasetManifestV2 = serde_json::from_slice(&read_bytes(manifest_path)?)?;
    if manifest.schema_version != DATASET_MANIFEST_V2_SCHEMA_VERSION {
        return Err(DatasetError::ArtifactVerification(format!(
            "unsupported dataset schema {}",
            manifest.schema_version
        )));
    }
    validate_manifest_identity(&manifest)?;
    let directory = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut events = Vec::new();
    for artifact in &manifest.parquet_files {
        let path = directory.join(&artifact.file);
        verify_artifact(&path, artifact)?;
        let mut file_events = read_parquet(&path)?;
        if u64::try_from(file_events.len())? != artifact.row_count {
            return Err(DatasetError::ArtifactVerification(format!(
                "row count mismatch for {}",
                path.display()
            )));
        }
        events.append(&mut file_events);
    }
    if u64::try_from(events.len())? != manifest.included_rows {
        return Err(DatasetError::ArtifactVerification(
            "dataset included_rows does not match Parquet files".into(),
        ));
    }
    let sources: BTreeSet<_> = events.iter().map(|event| event.source.clone()).collect();
    let instruments: BTreeSet<_> = events
        .iter()
        .map(|event| event.instrument.clone())
        .collect();
    let min_available_at_ms = events
        .iter()
        .map(|event| event.available_at_ms)
        .reduce(f64::min);
    let max_available_at_ms = events
        .iter()
        .map(|event| event.available_at_ms)
        .reduce(f64::max);
    if sources != manifest.sources
        || instruments != manifest.instruments
        || min_available_at_ms != manifest.min_available_at_ms
        || max_available_at_ms != manifest.max_available_at_ms
    {
        return Err(DatasetError::ArtifactVerification(
            "dataset metadata does not match Parquet rows".into(),
        ));
    }
    Ok((manifest, events))
}

fn validate_manifest_identity(manifest: &DatasetManifestV2) -> Result<(), DatasetError> {
    if manifest.dataset_builder_version != DATASET_BUILDER_VERSION
        || manifest.canonical_schema_version != CANONICAL_EVENT_SCHEMA_VERSION
        || manifest.parquet_schema_version != PARQUET_SCHEMA_VERSION
    {
        return Err(DatasetError::ArtifactVerification(
            "unsupported dataset builder or event/Parquet schema".into(),
        ));
    }
    if manifest.inputs.len() != 1 || manifest.parquet_files.len() != 1 {
        return Err(DatasetError::ArtifactVerification(
            "canonical-parquet-v1 requires exactly one input and one Parquet artifact".into(),
        ));
    }
    if manifest.included_rows.checked_add(manifest.excluded_rows) != Some(manifest.input_rows)
        || manifest.parquet_files[0].row_count != manifest.included_rows
    {
        return Err(DatasetError::ArtifactVerification(
            "dataset row counts are inconsistent".into(),
        ));
    }
    let expected_id = dataset_identity_v2(
        &manifest.inputs[0],
        &manifest.parquet_files[0],
        &manifest.quality_mask_sha256,
        &manifest.code_commit,
    );
    if manifest.dataset_id != expected_id {
        return Err(DatasetError::ArtifactVerification(
            "dataset_id does not match the frozen inputs".into(),
        ));
    }
    Ok(())
}

struct FilteredEvents {
    events: Vec<CanonicalMarketEvent>,
    input_rows: u64,
    exclusion_counts: BTreeMap<String, u64>,
}

fn load_and_filter(path: &Path, mask: &QualityMask) -> Result<FilteredEvents, DatasetError> {
    let file = File::open(path).map_err(|source| io_error(path, source))?;
    let mut events = Vec::new();
    let mut input_rows = 0_u64;
    let mut exclusions = BTreeMap::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|source| io_error(path, source))?;
        if line.trim().is_empty() {
            continue;
        }
        input_rows += 1;
        let event: CanonicalMarketEvent = serde_json::from_str(&line)?;
        let excluded = mask.excluded_flags(&event);
        if excluded.is_empty() {
            events.push(event);
        } else {
            for flag in excluded {
                *exclusions.entry(flag).or_default() += 1;
            }
        }
    }
    Ok(FilteredEvents {
        events,
        input_rows,
        exclusion_counts: exclusions,
    })
}

fn parquet_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("schema_version", DataType::UInt32, false),
        Field::new("canonical_event_id", DataType::Utf8, false),
        Field::new("event_kind", DataType::Utf8, false),
        Field::new("source", DataType::Utf8, false),
        Field::new("stream", DataType::Utf8, false),
        Field::new("session_id", DataType::Utf8, false),
        Field::new("instrument", DataType::Utf8, false),
        Field::new("market_id", DataType::Utf8, true),
        Field::new("outcome_id", DataType::Utf8, true),
        Field::new("source_event_ts_ms", DataType::Float64, true),
        Field::new("available_at_ms", DataType::Float64, false),
        Field::new("recv_mono_ns", DataType::Utf8, false),
        Field::new("sequence_start", DataType::Int64, true),
        Field::new("sequence_end", DataType::Int64, true),
        Field::new("price", DataType::Utf8, true),
        Field::new("quantity", DataType::Utf8, true),
        Field::new("best_bid", DataType::Utf8, true),
        Field::new("best_ask", DataType::Utf8, true),
        Field::new("bids_json", DataType::Utf8, false),
        Field::new("asks_json", DataType::Utf8, false),
        Field::new("quality_flags_json", DataType::Utf8, false),
        Field::new("input_sha256", DataType::Utf8, false),
        Field::new("raw_line_number", DataType::UInt64, false),
        Field::new("raw_event_sha256", DataType::Utf8, false),
        Field::new("raw_schema_version", DataType::UInt32, false),
        Field::new("canonical_json", DataType::Utf8, false),
    ]))
}

pub fn write_canonical_parquet(
    path: &Path,
    events: &[CanonicalMarketEvent],
) -> Result<(), DatasetError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    }
    let schema = parquet_schema();
    let batch = record_batch(Arc::clone(&schema), events)?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| io_error(path, source))?;
    let properties = WriterProperties::builder()
        .set_created_by("event-contract-lab canonical-parquet-v1".into())
        .set_compression(Compression::ZSTD(ZstdLevel::default()))
        .set_max_row_group_row_count(Some(65_536))
        .build();
    let mut writer = ArrowWriter::try_new(file, schema, Some(properties))?;
    writer.write(&batch)?;
    writer.close()?;
    Ok(())
}

fn record_batch(
    schema: Arc<Schema>,
    events: &[CanonicalMarketEvent],
) -> Result<RecordBatch, DatasetError> {
    let strings = |values: Vec<String>| Arc::new(StringArray::from(values)) as ArrayRef;
    let optional_strings =
        |values: Vec<Option<String>>| Arc::new(StringArray::from(values)) as ArrayRef;
    let json = json_columns(events)?;
    Ok(RecordBatch::try_new(
        schema,
        vec![
            Arc::new(UInt32Array::from_iter_values(
                events.iter().map(|event| event.schema_version),
            )),
            strings(
                events
                    .iter()
                    .map(|event| event.canonical_event_id.clone())
                    .collect(),
            ),
            strings(
                events
                    .iter()
                    .map(|event| event_kind(&event.event_kind).into())
                    .collect(),
            ),
            strings(events.iter().map(|event| event.source.clone()).collect()),
            strings(events.iter().map(|event| event.stream.clone()).collect()),
            strings(
                events
                    .iter()
                    .map(|event| event.session_id.clone())
                    .collect(),
            ),
            strings(
                events
                    .iter()
                    .map(|event| event.instrument.clone())
                    .collect(),
            ),
            optional_strings(events.iter().map(|event| event.market_id.clone()).collect()),
            optional_strings(
                events
                    .iter()
                    .map(|event| event.outcome_id.clone())
                    .collect(),
            ),
            Arc::new(Float64Array::from(
                events
                    .iter()
                    .map(|event| event.source_event_ts_ms)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from_iter_values(
                events.iter().map(|event| event.available_at_ms),
            )),
            strings(
                events
                    .iter()
                    .map(|event| event.recv_mono_ns.clone())
                    .collect(),
            ),
            Arc::new(Int64Array::from(
                events
                    .iter()
                    .map(|event| event.sequence_start)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(
                events
                    .iter()
                    .map(|event| event.sequence_end)
                    .collect::<Vec<_>>(),
            )),
            optional_strings(events.iter().map(|event| event.price.clone()).collect()),
            optional_strings(events.iter().map(|event| event.quantity.clone()).collect()),
            optional_strings(events.iter().map(|event| event.best_bid.clone()).collect()),
            optional_strings(events.iter().map(|event| event.best_ask.clone()).collect()),
            strings(json.bids),
            strings(json.asks),
            strings(json.quality_flags),
            strings(
                events
                    .iter()
                    .map(|event| event.lineage.input_sha256.clone())
                    .collect(),
            ),
            Arc::new(UInt64Array::from_iter_values(
                events.iter().map(|event| event.lineage.line_number),
            )),
            strings(
                events
                    .iter()
                    .map(|event| event.lineage.raw_event_sha256.clone())
                    .collect(),
            ),
            Arc::new(UInt32Array::from_iter_values(
                events.iter().map(|event| event.lineage.raw_schema_version),
            )),
            strings(json.canonical),
        ],
    )?)
}

struct JsonColumns {
    bids: Vec<String>,
    asks: Vec<String>,
    quality_flags: Vec<String>,
    canonical: Vec<String>,
}

fn json_columns(events: &[CanonicalMarketEvent]) -> Result<JsonColumns, serde_json::Error> {
    Ok(JsonColumns {
        bids: events
            .iter()
            .map(|event| serde_json::to_string(&event.bids))
            .collect::<Result<_, _>>()?,
        asks: events
            .iter()
            .map(|event| serde_json::to_string(&event.asks))
            .collect::<Result<_, _>>()?,
        quality_flags: events
            .iter()
            .map(|event| serde_json::to_string(&event.quality_flags))
            .collect::<Result<_, _>>()?,
        canonical: events
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<_, _>>()?,
    })
}

pub fn read_parquet(path: &Path) -> Result<Vec<CanonicalMarketEvent>, DatasetError> {
    let file = File::open(path).map_err(|source| io_error(path, source))?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;
    let mut events = Vec::new();
    for batch in reader {
        let batch = batch?;
        let index = batch
            .schema()
            .index_of("canonical_json")
            .map_err(DatasetError::Arrow)?;
        let values = batch
            .column(index)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| {
                DatasetError::ArtifactVerification("canonical_json is not Utf8".into())
            })?;
        for row in 0..values.len() {
            events.push(serde_json::from_str(values.value(row))?);
        }
    }
    Ok(events)
}

fn verify_artifact(path: &Path, expected: &FileArtifact) -> Result<(), DatasetError> {
    let actual = file_artifact(path, expected.row_count)?;
    if actual.sha256 != expected.sha256 || actual.byte_count != expected.byte_count {
        return Err(DatasetError::ArtifactVerification(format!(
            "checksum/size mismatch for {}",
            path.display()
        )));
    }
    Ok(())
}

fn file_artifact(path: &Path, row_count: u64) -> Result<FileArtifact, DatasetError> {
    let bytes = read_bytes(path)?;
    Ok(FileArtifact {
        file: file_name(path)?,
        sha256: sha256_hex(&bytes),
        byte_count: u64::try_from(bytes.len())?,
        row_count,
    })
}

#[must_use]
pub fn dataset_identity_v2(
    input: &DatasetInput,
    parquet: &FileArtifact,
    mask_sha: &str,
    commit: &str,
) -> String {
    sha256_hex(
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            input.transform_manifest_sha256,
            input.transform_id,
            parquet.sha256,
            mask_sha,
            commit,
            DATASET_BUILDER_VERSION
        )
        .as_bytes(),
    )
}

fn event_kind(kind: &CanonicalEventKind) -> &'static str {
    match kind {
        CanonicalEventKind::Trade => "trade",
        CanonicalEventKind::BestBidAsk => "best_bid_ask",
        CanonicalEventKind::BookSnapshot => "book_snapshot",
        CanonicalEventKind::BookDelta => "book_delta",
    }
}

fn ensure_new(path: &Path) -> Result<(), DatasetError> {
    if path.exists() {
        return Err(DatasetError::ArtifactExists(path.to_owned()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    }
    Ok(())
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, DatasetError> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|source| io_error(path, source))?;
    Ok(bytes)
}

fn write_json_exclusive<T: Serialize>(path: &Path, value: &T) -> Result<(), DatasetError> {
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

fn file_name(path: &Path) -> Result<String, DatasetError> {
    path.file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .ok_or_else(|| DatasetError::MissingFileName(path.to_owned()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn io_error(path: impl Into<PathBuf>, source: std::io::Error) -> DatasetError {
    DatasetError::Io {
        path: path.into(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use event_contracts::{CanonicalEventKind, RawLineage};

    fn event(id: &str, available_at_ms: f64, flags: &[&str]) -> CanonicalMarketEvent {
        CanonicalMarketEvent {
            schema_version: 1,
            canonical_event_id: format!("{}:0", sha256_hex(id.as_bytes())),
            event_kind: CanonicalEventKind::Trade,
            source: "binance".into(),
            stream: "btcusdt@trade".into(),
            session_id: "session".into(),
            instrument: "BTCUSDT".into(),
            market_id: None,
            outcome_id: None,
            source_event_ts_ms: Some(available_at_ms - 2.0),
            available_at_ms,
            recv_mono_ns: format!("{:.0}", available_at_ms * 1_000_000.0),
            sequence_start: Some(1),
            sequence_end: Some(1),
            price: Some("60000.0100".into()),
            quantity: Some("0.0010".into()),
            best_bid: None,
            best_ask: None,
            bids: Vec::new(),
            asks: Vec::new(),
            quality_flags: flags.iter().map(|value| (*value).to_owned()).collect(),
            lineage: RawLineage {
                input_sha256: "a".repeat(64),
                line_number: 1,
                raw_event_sha256: sha256_hex(id.as_bytes()),
                raw_schema_version: 1,
            },
        }
    }

    fn fixture(root: &Path) -> (PathBuf, PathBuf) {
        let canonical = root.join("canonical.ndjson");
        let canonical_bytes = [
            serde_json::to_string(&event("one", 1_000.0, &[])).expect("event"),
            serde_json::to_string(&event("two", 2_000.0, &["stale_event"])).expect("event"),
        ]
        .join("\n")
            + "\n";
        fs::write(&canonical, canonical_bytes).expect("canonical fixture");
        let empty = root.join("quarantine.ndjson");
        fs::write(&empty, []).expect("empty fixture");
        let quality = root.join("quality.json");
        fs::write(&quality, b"{}\n").expect("quality fixture");
        let transform = TransformManifest {
            schema_version: 1,
            transform_id: "b".repeat(64),
            normalizer_version: "test".into(),
            canonical_schema_version: 1,
            quality_policy_version: "test".into(),
            quality_policy_sha256: "c".repeat(64),
            code_commit: "commit".into(),
            input: FileArtifact {
                file: "raw.ndjson".into(),
                sha256: "d".repeat(64),
                byte_count: 1,
                row_count: 2,
            },
            canonical_output: file_artifact(&canonical, 2).expect("canonical artifact"),
            quarantine_output: file_artifact(&empty, 0).expect("quarantine artifact"),
            quality_report: file_artifact(&quality, 1).expect("quality artifact"),
        };
        let transform_path = root.join("transform-manifest.json");
        write_json_exclusive(&transform_path, &transform).expect("transform fixture");
        let mask_path = root.join("mask.json");
        fs::write(
            &mask_path,
            br#"{"schema_version":1,"mask_version":"strict","exclude_unlisted_flags":true,"allowed_flags":[]}"#,
        )
        .expect("mask fixture");
        (transform_path, mask_path)
    }

    #[test]
    fn parquet_round_trip_and_dataset_identity_are_deterministic() {
        let root = tempfile::tempdir().expect("tempdir");
        let (transform, mask) = fixture(root.path());
        let build = |name: &str| {
            let directory = root.path().join(name);
            build_dataset(&BuildRequest {
                transform_manifest: transform.clone(),
                quality_mask: mask.clone(),
                parquet_output: directory.join("canonical.parquet"),
                dataset_manifest_output: directory.join("dataset-manifest.json"),
                code_commit: "commit".into(),
            })
            .expect("dataset build")
        };
        let first = build("first");
        let second = build("second");
        assert_eq!(first.dataset_id, second.dataset_id);
        assert_eq!(first.included_rows, 1);
        assert_eq!(first.excluded_rows, 1);
        assert_eq!(
            fs::read(root.path().join("first/canonical.parquet")).expect("first parquet"),
            fs::read(root.path().join("second/canonical.parquet")).expect("second parquet")
        );
        assert_eq!(
            fs::read(root.path().join("first/dataset-manifest.json")).expect("first manifest"),
            fs::read(root.path().join("second/dataset-manifest.json")).expect("second manifest")
        );
        let (_, events) =
            load_dataset(&root.path().join("first/dataset-manifest.json")).expect("load dataset");
        assert_eq!(events.len(), 1);
        assert!(events[0].quality_flags.is_empty());
    }

    #[test]
    fn tampered_parquet_is_rejected() {
        let root = tempfile::tempdir().expect("tempdir");
        let (transform, mask) = fixture(root.path());
        let directory = root.path().join("dataset");
        let manifest_path = directory.join("dataset-manifest.json");
        build_dataset(&BuildRequest {
            transform_manifest: transform,
            quality_mask: mask,
            parquet_output: directory.join("canonical.parquet"),
            dataset_manifest_output: manifest_path.clone(),
            code_commit: "commit".into(),
        })
        .expect("dataset build");
        OpenOptions::new()
            .append(true)
            .open(directory.join("canonical.parquet"))
            .and_then(|mut file| file.write_all(b"tampered"))
            .expect("tamper parquet");
        assert!(matches!(
            load_dataset(&manifest_path),
            Err(DatasetError::ArtifactVerification(_))
        ));
    }

    #[test]
    fn tampered_dataset_identity_is_rejected() {
        let root = tempfile::tempdir().expect("tempdir");
        let (transform, mask) = fixture(root.path());
        let directory = root.path().join("dataset");
        let manifest_path = directory.join("dataset-manifest.json");
        build_dataset(&BuildRequest {
            transform_manifest: transform,
            quality_mask: mask,
            parquet_output: directory.join("canonical.parquet"),
            dataset_manifest_output: manifest_path.clone(),
            code_commit: "commit".into(),
        })
        .expect("dataset build");
        let mut manifest: DatasetManifestV2 =
            serde_json::from_slice(&fs::read(&manifest_path).expect("manifest bytes"))
                .expect("manifest");
        manifest.dataset_id = "0".repeat(64);
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("tamper manifest");
        assert!(matches!(
            load_dataset(&manifest_path),
            Err(DatasetError::ArtifactVerification(_))
        ));
    }
}
