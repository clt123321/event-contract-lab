use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use replay_core::{ReplayRequest, run_replay};

const HELP: &str = "\
replay-cli: deterministic point-in-time replay from DatasetManifest v2

USAGE:
  replay-cli run --dataset-manifest <path> --config <path> \\
    --output <replay.ndjson> --manifest <replay-manifest.json> --git-commit <sha>

Ordering:
  available_at_ms, source, session_id, recv_mono_ns numeric, canonical_event_id
";

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match parse_args(&arguments).and_then(|request| {
        run_replay(&request)
            .map_err(|error| error.to_string())
            .and_then(|result| serde_json::to_string(&result).map_err(|error| error.to_string()))
    }) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) if error == "help" => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}\n\n{HELP}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args(arguments: &[String]) -> Result<ReplayRequest, String> {
    if arguments.is_empty() || matches!(arguments[0].as_str(), "help" | "-h" | "--help") {
        return Err("help".into());
    }
    if arguments[0] != "run" {
        return Err(format!("unknown command: {}", arguments[0]));
    }
    let mut values = BTreeMap::new();
    let mut index = 1;
    while index < arguments.len() {
        let flag = &arguments[index];
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| format!("missing value for {flag}"))?;
        if !flag.starts_with("--") || value.starts_with("--") {
            return Err(format!("invalid option/value near {flag}"));
        }
        if values.insert(flag.clone(), value.clone()).is_some() {
            return Err(format!("duplicate option: {flag}"));
        }
        index += 2;
    }
    let required = |name: &str| {
        values
            .get(name)
            .cloned()
            .ok_or_else(|| format!("run requires {name} <value>"))
    };
    let allowed = [
        "--dataset-manifest",
        "--config",
        "--output",
        "--manifest",
        "--git-commit",
    ];
    if let Some(unknown) = values.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("unknown option: {unknown}"));
    }
    Ok(ReplayRequest {
        dataset_manifest: PathBuf::from(required("--dataset-manifest")?),
        replay_config: PathBuf::from(required("--config")?),
        output: PathBuf::from(required("--output")?),
        replay_manifest_output: PathBuf::from(required("--manifest")?),
        code_commit: required("--git-commit")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_inputs_and_outputs_are_required() {
        assert!(parse_args(&["run".into()]).is_err());
    }
}
