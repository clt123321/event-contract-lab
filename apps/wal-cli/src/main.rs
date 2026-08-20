use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use collector_core::{DEFAULT_MAX_SEGMENT_BYTES, SegmentedWal, WalOptions, verify_directory};

const HELP: &str = "\
wal-cli: validate, segment, recover, and verify RawEventEnvelope NDJSON

USAGE:
  wal-cli import --input <path|-> [--wal-dir <path>] [--max-segment-bytes <n>] [--git-commit <sha>]
  wal-cli verify [--wal-dir <path>]

Defaults:
  --wal-dir data/wal
  --max-segment-bytes 67108864 (64 MiB)

Safety:
  This utility contains no venue order or credential code. It only reads local NDJSON and writes WAL files.
";

#[derive(Debug)]
enum Command {
    Import {
        input: String,
        wal_dir: PathBuf,
        max_segment_bytes: u64,
        git_commit: Option<String>,
    },
    Verify {
        wal_dir: PathBuf,
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
    let action = args[0].as_str();
    let mut input = None;
    let mut wal_dir = PathBuf::from("data/wal");
    let mut max_segment_bytes = DEFAULT_MAX_SEGMENT_BYTES;
    let mut git_commit = env::var("GIT_SHA")
        .ok()
        .or_else(|| option_env!("GIT_SHA").map(str::to_owned));

    let mut index = 1;
    while index < args.len() {
        let flag = &args[index];
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--input" => input = Some(value.clone()),
            "--wal-dir" => wal_dir = PathBuf::from(value),
            "--max-segment-bytes" => {
                max_segment_bytes = value
                    .parse::<u64>()
                    .map_err(|_| "--max-segment-bytes must be a positive integer".to_owned())?;
                if max_segment_bytes == 0 {
                    return Err("--max-segment-bytes must be positive".into());
                }
            }
            "--git-commit" => git_commit = Some(value.clone()),
            _ => return Err(format!("unknown option: {flag}")),
        }
        index += 2;
    }

    match action {
        "import" => Ok(Command::Import {
            input: input.ok_or_else(|| "import requires --input <path|->".to_owned())?,
            wal_dir,
            max_segment_bytes,
            git_commit,
        }),
        "verify" => {
            if input.is_some() {
                return Err("verify does not accept --input".into());
            }
            Ok(Command::Verify { wal_dir })
        }
        _ => Err(format!("unknown command: {action}")),
    }
}

fn run(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Command::Help => {
            print!("{HELP}");
        }
        Command::Import {
            input,
            wal_dir,
            max_segment_bytes,
            git_commit,
        } => {
            let mut options = WalOptions::new(&wal_dir);
            options.max_segment_bytes = max_segment_bytes;
            options.git_commit = git_commit;
            let mut wal = SegmentedWal::open(options)?;
            let imported_rows = if input == "-" {
                import_reader(io::stdin().lock(), &mut wal)?
            } else {
                let file = File::open(&input)?;
                import_reader(BufReader::new(file), &mut wal)?
            };
            wal.sync()?;
            let manifests = wal.finish()?;
            println!(
                "{}",
                serde_json::json!({
                    "status": "sealed",
                    "input": input,
                    "wal_dir": wal_dir,
                    "imported_rows": imported_rows,
                    "segments_sealed_or_recovered": manifests.len(),
                    "segment_rows": manifests.iter().map(|item| item.row_count).sum::<u64>()
                })
            );
        }
        Command::Verify { wal_dir } => {
            let manifests = verify_directory(Path::new(&wal_dir))?;
            println!(
                "{}",
                serde_json::json!({
                    "status": "verified",
                    "wal_dir": wal_dir,
                    "segments": manifests.len(),
                    "rows": manifests.iter().map(|item| item.row_count).sum::<u64>(),
                    "bytes": manifests.iter().map(|item| item.byte_count).sum::<u64>()
                })
            );
        }
    }
    Ok(())
}

fn import_reader(
    reader: impl BufRead,
    wal: &mut SegmentedWal,
) -> Result<u64, Box<dyn std::error::Error>> {
    let mut rows = 0;
    for (line_index, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        wal.append_raw_line(&line).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("line {}: {error}", line_index + 1),
            )
        })?;
        rows += 1;
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_zero_segment_size() {
        let args = ["import", "--input", "-", "--max-segment-bytes", "0"]
            .map(str::to_owned)
            .to_vec();
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn verify_has_safe_default_directory() {
        let args = vec!["verify".into()];
        let Command::Verify { wal_dir } = parse_args(&args).expect("parse") else {
            panic!("expected verify command");
        };
        assert_eq!(wal_dir, PathBuf::from("data/wal"));
    }
}
