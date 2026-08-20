use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use event_contracts::{
    CANONICAL_EVENT_SCHEMA_VERSION, FileArtifact, TRANSFORM_MANIFEST_SCHEMA_VERSION,
    TransformManifest,
};
use normalizer_core::{
    NORMALIZER_VERSION, NormalizeOutcome, Normalizer, QualityPolicy, QualityReport, sha256_hex,
};
use sha2::{Digest, Sha256};

const HELP: &str = "\
normalize-cli: deterministically transform RawEventEnvelope NDJSON into canonical Silver NDJSON

USAGE:
  normalize-cli normalize --input <raw.ndjson> --output <silver.ndjson> \\
    --quarantine <quarantine.ndjson> --quality-report <quality.json> \\
    --manifest <transform-manifest.json> --quality-policy <policy.json> \\
    --git-commit <sha>

Safety:
  Read-only with respect to venues. Existing output files are never overwritten.
";

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Normalize {
        input: PathBuf,
        output: PathBuf,
        quarantine: PathBuf,
        quality_report: PathBuf,
        manifest: PathBuf,
        quality_policy: PathBuf,
        git_commit: String,
    },
    Help,
}

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match parse_args(&args).and_then(|command| run(command).map_err(|error| error.to_string())) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}\n\n{HELP}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help" | "help") {
        return Ok(Command::Help);
    }
    if args[0] != "normalize" {
        return Err(format!("unknown command: {}", args[0]));
    }
    let mut values = std::collections::BTreeMap::new();
    let mut index = 1;
    while index < args.len() {
        let flag = &args[index];
        if !flag.starts_with("--") {
            return Err(format!("unexpected positional argument: {flag}"));
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value for {flag}"))?;
        if value.starts_with("--") {
            return Err(format!("missing value for {flag}"));
        }
        if values.insert(flag.clone(), value.clone()).is_some() {
            return Err(format!("duplicate option: {flag}"));
        }
        index += 2;
    }
    let allowed = [
        "--input",
        "--output",
        "--quarantine",
        "--quality-report",
        "--manifest",
        "--quality-policy",
        "--git-commit",
    ];
    if let Some(unknown) = values.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("unknown option: {unknown}"));
    }
    let required = |name: &str| {
        values
            .get(name)
            .cloned()
            .ok_or_else(|| format!("normalize requires {name} <value>"))
    };
    let git_commit = required("--git-commit")?;
    if git_commit.trim().is_empty() {
        return Err("--git-commit cannot be empty".into());
    }
    Ok(Command::Normalize {
        input: PathBuf::from(required("--input")?),
        output: PathBuf::from(required("--output")?),
        quarantine: PathBuf::from(required("--quarantine")?),
        quality_report: PathBuf::from(required("--quality-report")?),
        manifest: PathBuf::from(required("--manifest")?),
        quality_policy: PathBuf::from(required("--quality-policy")?),
        git_commit,
    })
}

fn run(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Normalize {
        input,
        output,
        quarantine,
        quality_report,
        manifest,
        quality_policy,
        git_commit,
    } = command
    else {
        print!("{HELP}");
        return Ok(());
    };

    ensure_unique_targets([&output, &quarantine, &quality_report, &manifest])?;
    let input_artifact = input_artifact(&input)?;
    let policy_bytes = fs::read(&quality_policy)?;
    let policy_sha256 = sha256_hex(&policy_bytes);
    let policy: QualityPolicy = serde_json::from_slice(&policy_bytes)?;
    let policy_version = policy.policy_version.clone();
    let mut normalizer = Normalizer::new(policy, input_artifact.sha256.clone())?;
    let mut canonical_writer = ArtifactWriter::create(&output)?;
    let mut quarantine_writer = ArtifactWriter::create(&quarantine)?;

    let input_file = File::open(&input)?;
    for (index, line) in BufReader::new(input_file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let line_number = u64::try_from(index + 1)?;
        match normalizer.normalize_line(line_number, &line) {
            NormalizeOutcome::Canonical(event) => canonical_writer.write_json(&event)?,
            NormalizeOutcome::Quarantined(record) => quarantine_writer.write_json(&record)?,
            NormalizeOutcome::Skipped { .. } => {}
        }
    }
    let canonical_output = canonical_writer.finish()?;
    let quarantine_output = quarantine_writer.finish()?;

    let report = QualityReport {
        schema_version: 1,
        normalizer_version: NORMALIZER_VERSION.into(),
        quality_policy_version: policy_version.clone(),
        quality_policy_sha256: policy_sha256.clone(),
        input_sha256: input_artifact.sha256.clone(),
        summary: normalizer.summary().clone(),
    };
    let quality_report_artifact = write_json_artifact(&quality_report, &report, 1)?;
    let transform_id = sha256_hex(
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}",
            input_artifact.sha256,
            canonical_output.sha256,
            quarantine_output.sha256,
            quality_report_artifact.sha256,
            policy_sha256,
            git_commit,
            NORMALIZER_VERSION,
        )
        .as_bytes(),
    );
    let transform = TransformManifest {
        schema_version: TRANSFORM_MANIFEST_SCHEMA_VERSION,
        transform_id,
        normalizer_version: NORMALIZER_VERSION.into(),
        canonical_schema_version: CANONICAL_EVENT_SCHEMA_VERSION,
        quality_policy_version: policy_version,
        quality_policy_sha256: policy_sha256,
        code_commit: git_commit,
        input: input_artifact,
        canonical_output,
        quarantine_output,
        quality_report: quality_report_artifact,
    };
    write_json_exclusive(&manifest, &transform)?;
    println!(
        "{}",
        serde_json::json!({
            "status": "normalized",
            "manifest": manifest,
            "transform_id": transform.transform_id,
            "input_rows": transform.input.row_count,
            "canonical_rows": transform.canonical_output.row_count,
            "quarantined_rows": transform.quarantine_output.row_count,
            "skipped_rows": report.summary.skipped_rows,
            "warning_rows": report.summary.warning_rows,
        })
    );
    Ok(())
}

fn ensure_unique_targets<'a>(paths: impl IntoIterator<Item = &'a PathBuf>) -> std::io::Result<()> {
    for path in paths {
        if path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("refusing to overwrite {}", path.display()),
            ));
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn input_artifact(path: &Path) -> Result<FileArtifact, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut bytes = 0_u64;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        bytes += u64::try_from(count)?;
    }
    let row_count = BufReader::new(File::open(path)?)
        .lines()
        .filter(|line| line.as_ref().is_ok_and(|value| !value.trim().is_empty()))
        .count();
    Ok(FileArtifact {
        file: artifact_name(path)?,
        sha256: format!("{:x}", hasher.finalize()),
        byte_count: bytes,
        row_count: u64::try_from(row_count)?,
    })
}

struct ArtifactWriter {
    path: PathBuf,
    writer: BufWriter<File>,
    hasher: Sha256,
    bytes: u64,
    rows: u64,
}

impl ArtifactWriter {
    fn create(path: &Path) -> std::io::Result<Self> {
        let file = OpenOptions::new().write(true).create_new(true).open(path)?;
        Ok(Self {
            path: path.to_owned(),
            writer: BufWriter::new(file),
            hasher: Sha256::new(),
            bytes: 0,
            rows: 0,
        })
    }

    fn write_json<T: serde::Serialize>(
        &mut self,
        value: &T,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut encoded = serde_json::to_vec(value)?;
        encoded.push(b'\n');
        self.writer.write_all(&encoded)?;
        self.hasher.update(&encoded);
        self.bytes += u64::try_from(encoded.len())?;
        self.rows += 1;
        Ok(())
    }

    fn finish(mut self) -> Result<FileArtifact, Box<dyn std::error::Error>> {
        self.writer.flush()?;
        self.writer.get_ref().sync_all()?;
        Ok(FileArtifact {
            file: artifact_name(&self.path)?,
            sha256: format!("{:x}", self.hasher.finalize()),
            byte_count: self.bytes,
            row_count: self.rows,
        })
    }
}

fn write_json_artifact<T: serde::Serialize>(
    path: &Path,
    value: &T,
    rows: u64,
) -> Result<FileArtifact, Box<dyn std::error::Error>> {
    let mut encoded = serde_json::to_vec_pretty(value)?;
    encoded.push(b'\n');
    write_exclusive(path, &encoded)?;
    Ok(FileArtifact {
        file: artifact_name(path)?,
        sha256: sha256_hex(&encoded),
        byte_count: u64::try_from(encoded.len())?,
        row_count: rows,
    })
}

fn write_json_exclusive<T: serde::Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut encoded = serde_json::to_vec_pretty(value)?;
    encoded.push(b'\n');
    write_exclusive(path, &encoded)?;
    Ok(())
}

fn write_exclusive(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn artifact_name(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(path
        .file_name()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "artifact path has no file name",
            )
        })?
        .to_string_lossy()
        .into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_all_explicit_artifact_paths() {
        let args = ["normalize", "--input", "raw.ndjson"]
            .map(str::to_owned)
            .to_vec();
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn refuses_duplicate_options() {
        let args = ["normalize", "--input", "a", "--input", "b"]
            .map(str::to_owned)
            .to_vec();
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn repeated_transform_is_byte_deterministic_and_refuses_overwrite() {
        let root = tempfile::tempdir().expect("tempdir");
        let raw = root.path().join("raw.ndjson");
        let policy = root.path().join("policy.json");
        fs::write(
            &raw,
            concat!(
                r#"{"schema_version":1,"record_kind":"market_data","session_id":"s","source":"binance","stream":"btcusdt@trade","instrument":"BTCUSDT","event_type":"trade","source_event_ts_ms":1000,"recv_wall_ts_ms":1002,"recv_mono_ns":"1","sequence_start":1,"sequence_end":1,"payload":{"p":"60000.0100","q":"0.0010"}}"#,
                "\n"
            ),
        )
        .expect("raw fixture");
        fs::write(
            &policy,
            r#"{"schema_version":1,"policy_version":"q1","timestamp_future_tolerance_ms":1000,"stale_event_threshold_ms":60000,"reject_duplicate_raw_event":true,"reject_sequence_regression":true,"reject_crossed_book":true,"event_contract_price_min":0,"event_contract_price_max":1}"#,
        )
        .expect("policy fixture");

        let command = |directory: &Path| Command::Normalize {
            input: raw.clone(),
            output: directory.join("canonical.ndjson"),
            quarantine: directory.join("quarantine.ndjson"),
            quality_report: directory.join("quality.json"),
            manifest: directory.join("manifest.json"),
            quality_policy: policy.clone(),
            git_commit: "same-commit".into(),
        };
        let first = root.path().join("first");
        let second = root.path().join("second");
        run(command(&first)).expect("first transform");
        run(command(&second)).expect("second transform");
        for artifact in [
            "canonical.ndjson",
            "quarantine.ndjson",
            "quality.json",
            "manifest.json",
        ] {
            assert_eq!(
                fs::read(first.join(artifact)).expect("first artifact"),
                fs::read(second.join(artifact)).expect("second artifact")
            );
        }
        assert!(run(command(&first)).is_err());
    }
}
