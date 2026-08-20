//! Single-writer, segmented NDJSON WAL with restart recovery and verifiable manifests.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use event_contracts::{RawEventEnvelope, SEGMENT_MANIFEST_SCHEMA_VERSION, SegmentManifest};
use fs2::FileExt;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const DEFAULT_MAX_SEGMENT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum WalError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid JSON event: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("invalid raw event contract: {0}")]
    InvalidContract(#[from] event_contracts::ContractError),
    #[error("segment size must be positive")]
    InvalidSegmentSize,
    #[error("WAL sync row/interval settings must be positive")]
    InvalidSyncPolicy,
    #[error("another WAL writer owns {0}")]
    WriterLocked(PathBuf),
    #[error("manifest verification failed for {path}: {reason}")]
    Verification { path: PathBuf, reason: String },
    #[error("system clock is before the Unix epoch")]
    InvalidSystemClock,
    #[error("path has no file name: {0}")]
    MissingFileName(PathBuf),
    #[error("refusing to overwrite an existing WAL artifact: {0}")]
    ArtifactAlreadyExists(PathBuf),
}

#[derive(Debug, Clone)]
pub struct WalOptions {
    pub directory: PathBuf,
    pub max_segment_bytes: u64,
    pub git_commit: Option<String>,
    pub sync_every_rows: u64,
    pub sync_interval: Duration,
}

impl WalOptions {
    #[must_use]
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            max_segment_bytes: DEFAULT_MAX_SEGMENT_BYTES,
            git_commit: None,
            sync_every_rows: 1_000,
            sync_interval: Duration::from_secs(1),
        }
    }
}

#[derive(Debug, Default, Clone)]
struct SegmentStats {
    row_count: u64,
    byte_count: u64,
    min_recv_wall_ts_ms: Option<f64>,
    max_recv_wall_ts_ms: Option<f64>,
    min_source_event_ts_ms: Option<f64>,
    max_source_event_ts_ms: Option<f64>,
    sources: BTreeSet<String>,
    streams: BTreeSet<String>,
    schema_versions: BTreeSet<u32>,
}

impl SegmentStats {
    fn observe(&mut self, event: &RawEventEnvelope, bytes: u64) {
        self.row_count += 1;
        self.byte_count += bytes;
        update_min(&mut self.min_recv_wall_ts_ms, event.recv_wall_ts_ms);
        update_max(&mut self.max_recv_wall_ts_ms, event.recv_wall_ts_ms);
        if let Some(value) = event.source_event_ts_ms {
            update_min(&mut self.min_source_event_ts_ms, value);
            update_max(&mut self.max_source_event_ts_ms, value);
        }
        self.sources.insert(event.source.clone());
        self.streams.insert(event.stream.clone());
        self.schema_versions.insert(event.schema_version);
    }
}

fn update_min(target: &mut Option<f64>, value: f64) {
    *target = Some(target.map_or(value, |current| current.min(value)));
}

fn update_max(target: &mut Option<f64>, value: f64) {
    *target = Some(target.map_or(value, |current| current.max(value)));
}

#[derive(Debug)]
struct ActiveSegment {
    path: PathBuf,
    writer: BufWriter<File>,
    stats: SegmentStats,
}

#[derive(Debug)]
pub struct SegmentedWal {
    options: WalOptions,
    _lock_file: File,
    active: Option<ActiveSegment>,
    sealed: Vec<SegmentManifest>,
    next_counter: u64,
    last_sync: Instant,
}

impl SegmentedWal {
    pub fn open(options: WalOptions) -> Result<Self, WalError> {
        if options.max_segment_bytes == 0 {
            return Err(WalError::InvalidSegmentSize);
        }
        if options.sync_every_rows == 0 || options.sync_interval.is_zero() {
            return Err(WalError::InvalidSyncPolicy);
        }
        fs::create_dir_all(&options.directory)
            .map_err(|source| io_error(&options.directory, source))?;
        fs::create_dir_all(options.directory.join("quarantine"))
            .map_err(|source| io_error(options.directory.join("quarantine"), source))?;
        let lock_path = options.directory.join(".wal.lock");
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| io_error(&lock_path, source))?;
        lock_file
            .try_lock_exclusive()
            .map_err(|_| WalError::WriterLocked(lock_path))?;

        let mut wal = Self {
            options,
            _lock_file: lock_file,
            active: None,
            sealed: Vec::new(),
            next_counter: 0,
            last_sync: Instant::now(),
        };
        wal.recover_open_segments()?;
        wal.recover_orphan_segments()?;
        Ok(wal)
    }

    /// Validate and append an event while preserving the caller's exact JSON bytes.
    pub fn append_raw_line(&mut self, line: &str) -> Result<(), WalError> {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let event: RawEventEnvelope = serde_json::from_str(line)?;
        event.validate()?;
        let bytes = u64::try_from(line.len() + 1).expect("line size fits u64");

        if self.active.as_ref().is_some_and(|active| {
            active.stats.row_count > 0
                && active.stats.byte_count + bytes > self.options.max_segment_bytes
        }) {
            self.seal_active(false)?;
        }
        self.ensure_active()?;
        let active = self.active.as_mut().expect("active segment was created");
        active
            .writer
            .write_all(line.as_bytes())
            .and_then(|()| active.writer.write_all(b"\n"))
            .and_then(|()| active.writer.flush())
            .map_err(|source| io_error(&active.path, source))?;
        active.stats.observe(&event, bytes);
        let should_sync = active
            .stats
            .row_count
            .is_multiple_of(self.options.sync_every_rows)
            || self.last_sync.elapsed() >= self.options.sync_interval;
        if should_sync {
            self.sync()?;
        }
        Ok(())
    }

    pub fn append(&mut self, event: &RawEventEnvelope) -> Result<(), WalError> {
        event.validate()?;
        self.append_raw_line(&serde_json::to_string(event)?)
    }

    pub fn sync(&mut self) -> Result<(), WalError> {
        let Some(active) = self.active.as_mut() else {
            return Ok(());
        };
        active
            .writer
            .flush()
            .map_err(|source| io_error(&active.path, source))?;
        active
            .writer
            .get_ref()
            .sync_data()
            .map_err(|source| io_error(&active.path, source))?;
        self.last_sync = Instant::now();
        Ok(())
    }

    pub fn finish(mut self) -> Result<Vec<SegmentManifest>, WalError> {
        self.seal_active(false)?;
        Ok(self.sealed)
    }

    fn ensure_active(&mut self) -> Result<(), WalError> {
        if self.active.is_some() {
            return Ok(());
        }
        let now = unix_time_ms()?;
        loop {
            let name = format!("segment-{now}-{:04}.open", self.next_counter);
            self.next_counter += 1;
            let path = self.options.directory.join(name);
            if path.with_extension("ndjson").exists()
                || path.with_extension("manifest.json").exists()
            {
                continue;
            }
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    self.active = Some(ActiveSegment {
                        path,
                        writer: BufWriter::new(file),
                        stats: SegmentStats::default(),
                    });
                    return Ok(());
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(source) => return Err(io_error(path, source)),
            }
        }
    }

    fn seal_active(&mut self, recovered_after_restart: bool) -> Result<(), WalError> {
        let Some(mut active) = self.active.take() else {
            return Ok(());
        };
        active
            .writer
            .flush()
            .map_err(|source| io_error(&active.path, source))?;
        active
            .writer
            .get_ref()
            .sync_all()
            .map_err(|source| io_error(&active.path, source))?;
        drop(active.writer);

        if active.stats.row_count == 0 {
            fs::remove_file(&active.path).map_err(|source| io_error(&active.path, source))?;
            return Ok(());
        }
        let manifest = seal_file(
            &active.path,
            active.stats,
            self.options.git_commit.clone(),
            recovered_after_restart,
        )?;
        self.sealed.push(manifest);
        Ok(())
    }

    fn recover_open_segments(&mut self) -> Result<(), WalError> {
        let mut paths = fs::read_dir(&self.options.directory)
            .map_err(|source| io_error(&self.options.directory, source))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "open")
            })
            .collect::<Vec<_>>();
        paths.sort();

        for path in paths {
            let stats = prepare_recovered_file(&path, &self.options.directory.join("quarantine"))?;
            if stats.row_count == 0 {
                fs::remove_file(&path).map_err(|source| io_error(&path, source))?;
                continue;
            }
            let manifest = seal_file(&path, stats, self.options.git_commit.clone(), true)?;
            self.sealed.push(manifest);
        }
        Ok(())
    }

    fn recover_orphan_segments(&mut self) -> Result<(), WalError> {
        let mut paths = fs::read_dir(&self.options.directory)
            .map_err(|source| io_error(&self.options.directory, source))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "ndjson")
            })
            .filter(|path| !path.with_extension("manifest.json").exists())
            .collect::<Vec<_>>();
        paths.sort();

        for path in paths {
            let stats = scan_segment(&path)?;
            if stats.row_count == 0 {
                continue;
            }
            let manifest =
                write_manifest_for_sealed(&path, stats, self.options.git_commit.clone(), true)?;
            self.sealed.push(manifest);
        }
        Ok(())
    }
}

fn prepare_recovered_file(path: &Path, quarantine_dir: &Path) -> Result<SegmentStats, WalError> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|source| io_error(path, source))?;

    let complete_len = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |position| position + 1);
    if complete_len < bytes.len() {
        let partial = &bytes[complete_len..];
        quarantine_partial(quarantine_dir, partial)?;
        let file = OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|source| io_error(path, source))?;
        file.set_len(u64::try_from(complete_len).expect("file size fits u64"))
            .map_err(|source| io_error(path, source))?;
        file.sync_all().map_err(|source| io_error(path, source))?;
    }

    scan_segment(path)
}

fn scan_segment(path: &Path) -> Result<SegmentStats, WalError> {
    let file = File::open(path).map_err(|source| io_error(path, source))?;
    let mut stats = SegmentStats::default();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|source| io_error(path, source))?;
        if line.is_empty() {
            continue;
        }
        let event: RawEventEnvelope = serde_json::from_str(&line)?;
        event.validate()?;
        stats.observe(
            &event,
            u64::try_from(line.len() + 1).expect("line size fits u64"),
        );
    }
    Ok(stats)
}

fn seal_file(
    open_path: &Path,
    stats: SegmentStats,
    git_commit: Option<String>,
    recovered_after_restart: bool,
) -> Result<SegmentManifest, WalError> {
    let sealed_path = open_path.with_extension("ndjson");
    if sealed_path.exists() {
        return Err(WalError::ArtifactAlreadyExists(sealed_path));
    }
    let manifest_path = sealed_path.with_extension("manifest.json");
    if manifest_path.exists() {
        return Err(WalError::ArtifactAlreadyExists(manifest_path));
    }
    fs::rename(open_path, &sealed_path).map_err(|source| io_error(open_path, source))?;
    sync_parent(&sealed_path)?;

    write_manifest_for_sealed(&sealed_path, stats, git_commit, recovered_after_restart)
}

fn write_manifest_for_sealed(
    sealed_path: &Path,
    stats: SegmentStats,
    git_commit: Option<String>,
    recovered_after_restart: bool,
) -> Result<SegmentManifest, WalError> {
    let sha256 = sha256_file(sealed_path)?;
    let segment_file = sealed_path
        .file_name()
        .ok_or_else(|| WalError::MissingFileName(sealed_path.to_path_buf()))?
        .to_string_lossy()
        .into_owned();
    let manifest = SegmentManifest {
        schema_version: SEGMENT_MANIFEST_SCHEMA_VERSION,
        segment_file,
        sha256,
        row_count: stats.row_count,
        byte_count: stats.byte_count,
        min_recv_wall_ts_ms: stats.min_recv_wall_ts_ms.expect("non-empty segment"),
        max_recv_wall_ts_ms: stats.max_recv_wall_ts_ms.expect("non-empty segment"),
        min_source_event_ts_ms: stats.min_source_event_ts_ms,
        max_source_event_ts_ms: stats.max_source_event_ts_ms,
        sources: stats.sources,
        streams: stats.streams,
        schema_versions: stats.schema_versions,
        git_commit,
        sealed_at_ms: unix_time_ms()?,
        recovered_after_restart,
    };

    let manifest_path = sealed_path.with_extension("manifest.json");
    if manifest_path.exists() {
        return Err(WalError::ArtifactAlreadyExists(manifest_path));
    }
    let temporary_path = manifest_path.with_file_name(format!(
        ".{}.{}.tmp",
        manifest_path
            .file_name()
            .ok_or_else(|| WalError::MissingFileName(manifest_path.clone()))?
            .to_string_lossy(),
        unix_time_ms()?
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)
        .map_err(|source| io_error(&temporary_path, source))?;
    serde_json::to_writer_pretty(&mut file, &manifest)?;
    file.write_all(b"\n")
        .map_err(|source| io_error(&temporary_path, source))?;
    file.sync_all()
        .map_err(|source| io_error(&temporary_path, source))?;
    fs::rename(&temporary_path, &manifest_path)
        .map_err(|source| io_error(&temporary_path, source))?;
    sync_parent(&manifest_path)?;
    Ok(manifest)
}

fn quarantine_partial(directory: &Path, bytes: &[u8]) -> Result<PathBuf, WalError> {
    let now = unix_time_ms()?;
    for counter in 0_u32..=u32::MAX {
        let path = directory.join(format!("partial-{now}-{counter:04}.bin"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(bytes)
                    .map_err(|source| io_error(&path, source))?;
                file.sync_all().map_err(|source| io_error(&path, source))?;
                sync_parent(&path)?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(source) => return Err(io_error(path, source)),
        }
    }
    unreachable!("u32 quarantine name space exhausted")
}

pub fn verify_manifest(manifest_path: &Path) -> Result<SegmentManifest, WalError> {
    let manifest_file =
        File::open(manifest_path).map_err(|source| io_error(manifest_path, source))?;
    let manifest: SegmentManifest = serde_json::from_reader(manifest_file)?;
    let parent = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let segment_path = parent.join(&manifest.segment_file);
    let metadata = fs::metadata(&segment_path).map_err(|source| io_error(&segment_path, source))?;
    if metadata.len() != manifest.byte_count {
        return Err(WalError::Verification {
            path: manifest_path.to_path_buf(),
            reason: format!(
                "byte_count: expected {}, got {}",
                manifest.byte_count,
                metadata.len()
            ),
        });
    }
    let checksum = sha256_file(&segment_path)?;
    if checksum != manifest.sha256 {
        return Err(WalError::Verification {
            path: manifest_path.to_path_buf(),
            reason: format!("sha256: expected {}, got {checksum}", manifest.sha256),
        });
    }
    let stats = scan_segment(&segment_path)?;
    if stats.row_count != manifest.row_count {
        return Err(WalError::Verification {
            path: manifest_path.to_path_buf(),
            reason: format!(
                "row_count: expected {}, got {}",
                manifest.row_count, stats.row_count
            ),
        });
    }
    Ok(manifest)
}

pub fn verify_directory(directory: &Path) -> Result<Vec<SegmentManifest>, WalError> {
    let mut manifests = fs::read_dir(directory)
        .map_err(|source| io_error(directory, source))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.to_string_lossy().ends_with(".manifest.json"))
        .collect::<Vec<_>>();
    manifests.sort();
    manifests.iter().map(|path| verify_manifest(path)).collect()
}

fn sha256_file(path: &Path) -> Result<String, WalError> {
    let mut file = File::open(path).map_err(|source| io_error(path, source))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|source| io_error(path, source))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn sync_parent(path: &Path) -> Result<(), WalError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| io_error(parent, source))
}

fn unix_time_ms() -> Result<u64, WalError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| WalError::InvalidSystemClock)?;
    u64::try_from(elapsed.as_millis()).map_err(|_| WalError::InvalidSystemClock)
}

fn io_error(path: impl Into<PathBuf>, source: std::io::Error) -> WalError {
    WalError::Io {
        path: path.into(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use event_contracts::RawEventEnvelope;
    use tempfile::TempDir;

    use super::*;

    fn event(sequence: i32) -> RawEventEnvelope {
        let sequence_f64 = f64::from(sequence);
        RawEventEnvelope {
            schema_version: 1,
            record_kind: "market_data".into(),
            session_id: "fixture-session".into(),
            source: "binance".into(),
            stream: "btcusdt@trade".into(),
            instrument: Some("BTCUSDT".into()),
            event_type: "trade".into(),
            source_event_ts_ms: Some(1_787_000_000_000.0 + sequence_f64),
            source_trade_ts_ms: Some(1_787_000_000_000.0 + sequence_f64),
            recv_wall_ts_ms: 1_787_000_000_010.0 + sequence_f64,
            recv_mono_ns: (1000 + sequence).to_string(),
            sequence_start: Some(i64::from(sequence)),
            sequence_end: Some(i64::from(sequence)),
            payload: Some(serde_json::json!({"price": "60000.0"})),
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn rotates_and_verifies_immutable_segments() {
        let temp = TempDir::new().expect("tempdir");
        let mut options = WalOptions::new(temp.path());
        options.max_segment_bytes = 350;
        options.git_commit = Some("test-commit".into());
        let mut wal = SegmentedWal::open(options).expect("open WAL");
        for sequence in 1..=4 {
            wal.append(&event(sequence)).expect("append event");
        }
        let manifests = wal.finish().expect("seal WAL");
        assert!(manifests.len() >= 2);
        let verified = verify_directory(temp.path()).expect("verify directory");
        assert_eq!(verified.iter().map(|item| item.row_count).sum::<u64>(), 4);
        assert!(
            verified
                .iter()
                .all(|item| item.git_commit.as_deref() == Some("test-commit"))
        );
    }

    #[test]
    fn recovers_complete_rows_and_quarantines_partial_tail() {
        let temp = TempDir::new().expect("tempdir");
        let open_path = temp.path().join("segment-crash-0000.open");
        let valid_line = serde_json::to_string(&event(1)).expect("serialize");
        fs::write(&open_path, format!("{valid_line}\n{{\"partial\":")).expect("write crash file");

        let wal = SegmentedWal::open(WalOptions::new(temp.path())).expect("recover WAL");
        let recovered = wal.finish().expect("finish recovery");
        assert_eq!(recovered.len(), 1);
        assert!(recovered[0].recovered_after_restart);
        assert_eq!(recovered[0].row_count, 1);
        assert_eq!(verify_directory(temp.path()).expect("verify").len(), 1);
        assert_eq!(
            fs::read_dir(temp.path().join("quarantine"))
                .expect("quarantine")
                .count(),
            1
        );
    }

    #[test]
    fn rejects_invalid_contract_before_writing() {
        let temp = TempDir::new().expect("tempdir");
        let mut wal = SegmentedWal::open(WalOptions::new(temp.path())).expect("open WAL");
        let invalid = serde_json::json!({
            "schema_version": 99,
            "record_kind": "market_data",
            "session_id": "test",
            "source": "binance",
            "stream": "trade",
            "event_type": "trade",
            "recv_wall_ts_ms": 1,
            "recv_mono_ns": "1"
        });
        assert!(wal.append_raw_line(&invalid.to_string()).is_err());
        assert!(wal.finish().expect("finish").is_empty());
    }

    #[test]
    fn exact_input_bytes_are_preserved() {
        let temp = TempDir::new().expect("tempdir");
        let event = event(1);
        let compact = serde_json::to_string(&event).expect("serialize");
        let spaced = compact.replacen('{', "{  ", 1);
        let mut wal = SegmentedWal::open(WalOptions::new(temp.path())).expect("open WAL");
        wal.append_raw_line(&spaced).expect("append");
        let manifests = wal.finish().expect("finish");
        let stored =
            fs::read_to_string(temp.path().join(&manifests[0].segment_file)).expect("read");
        assert_eq!(stored, format!("{spaced}\n"));
    }

    #[test]
    fn restart_rebuilds_manifest_if_crash_happened_after_segment_rename() {
        let temp = TempDir::new().expect("tempdir");
        let segment_path = temp.path().join("segment-orphan.ndjson");
        let line = serde_json::to_string(&event(7)).expect("serialize");
        fs::write(&segment_path, format!("{line}\n")).expect("write orphan");

        let wal = SegmentedWal::open(WalOptions::new(temp.path())).expect("recover orphan");
        let recovered = wal.finish().expect("finish");
        assert_eq!(recovered.len(), 1);
        assert!(recovered[0].recovered_after_restart);
        assert!(segment_path.with_extension("manifest.json").exists());
        assert_eq!(verify_directory(temp.path()).expect("verify").len(), 1);
    }

    #[test]
    fn refuses_a_second_writer_for_the_same_directory() {
        let temp = TempDir::new().expect("tempdir");
        let first = SegmentedWal::open(WalOptions::new(temp.path())).expect("first writer");
        assert!(matches!(
            SegmentedWal::open(WalOptions::new(temp.path())),
            Err(WalError::WriterLocked(_))
        ));
        drop(first);
        assert!(SegmentedWal::open(WalOptions::new(temp.path())).is_ok());
    }

    #[test]
    fn verification_detects_segment_tampering() {
        let temp = TempDir::new().expect("tempdir");
        let mut wal = SegmentedWal::open(WalOptions::new(temp.path())).expect("open WAL");
        wal.append(&event(1)).expect("append");
        let manifests = wal.finish().expect("finish");
        let segment_path = temp.path().join(&manifests[0].segment_file);
        OpenOptions::new()
            .append(true)
            .open(&segment_path)
            .and_then(|mut file| file.write_all(b"\n"))
            .expect("tamper with segment");
        assert!(matches!(
            verify_directory(temp.path()),
            Err(WalError::Verification { .. })
        ));
    }

    #[test]
    fn sealing_never_overwrites_an_existing_segment() {
        let temp = TempDir::new().expect("tempdir");
        let open_path = temp.path().join("collision.open");
        let sealed_path = temp.path().join("collision.ndjson");
        let line = serde_json::to_string(&event(1)).expect("serialize");
        fs::write(&open_path, format!("{line}\n")).expect("write open segment");
        fs::write(&sealed_path, b"existing evidence\n").expect("write existing segment");
        let stats = scan_segment(&open_path).expect("scan open segment");

        assert!(matches!(
            seal_file(&open_path, stats, None, false),
            Err(WalError::ArtifactAlreadyExists(path)) if path == sealed_path
        ));
        assert_eq!(
            fs::read(&sealed_path).expect("read existing"),
            b"existing evidence\n"
        );
        assert!(open_path.exists());
    }
}
