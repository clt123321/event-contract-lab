use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use dataset_core::{BuildRequest, build_dataset};

const HELP: &str = "\
dataset-cli: build deterministic Parquet and a frozen DatasetManifest v2

USAGE:
  dataset-cli build --transform-manifest <path> --quality-mask <path> \\
    --parquet <path> --manifest <path> --git-commit <sha>

Safety:
  Existing artifacts are never overwritten. This command has no network or venue write capability.
";

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match parse_args(&arguments).and_then(|request| {
        build_dataset(&request)
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

fn parse_args(arguments: &[String]) -> Result<BuildRequest, String> {
    if arguments.is_empty() || matches!(arguments[0].as_str(), "help" | "-h" | "--help") {
        return Err("help".into());
    }
    if arguments[0] != "build" {
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
            .ok_or_else(|| format!("build requires {name} <value>"))
    };
    let allowed = [
        "--transform-manifest",
        "--quality-mask",
        "--parquet",
        "--manifest",
        "--git-commit",
    ];
    if let Some(unknown) = values.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("unknown option: {unknown}"));
    }
    Ok(BuildRequest {
        transform_manifest: PathBuf::from(required("--transform-manifest")?),
        quality_mask: PathBuf::from(required("--quality-mask")?),
        parquet_output: PathBuf::from(required("--parquet")?),
        dataset_manifest_output: PathBuf::from(required("--manifest")?),
        code_commit: required("--git-commit")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_artifact_paths_are_explicit() {
        assert!(parse_args(&["build".into(), "--manifest".into(), "x".into()]).is_err());
    }

    #[test]
    fn unknown_options_are_rejected() {
        let mut args = vec!["build".into()];
        for (flag, value) in [
            ("--transform-manifest", "t"),
            ("--quality-mask", "q"),
            ("--parquet", "p"),
            ("--manifest", "m"),
            ("--git-commit", "c"),
            ("--unsafe", "yes"),
        ] {
            args.extend([flag.into(), value.into()]);
        }
        assert!(parse_args(&args).is_err());
    }
}
