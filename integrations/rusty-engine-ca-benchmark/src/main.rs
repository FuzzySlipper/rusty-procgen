use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rusty_procgen_engine_ca_benchmark::{
    benchmark_suite, BenchmarkRunConfig, CaBenchmarkEnvironment, SystemBenchmarkClock,
};
use rusty_procgen_preflight::cellular_automata::CaScenarioSuite;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct EngineSource {
    schema_version: u32,
    public_repository: String,
    commit: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{}: {error}", error.code());
        std::process::exit(1);
    }
}

fn run() -> Result<(), rusty_procgen_engine_ca_benchmark::CaSpatialError> {
    let mut arguments = env::args().skip(1);
    let root = arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| invalid_config("expected repository root argument"))?;
    let repository_commit = arguments
        .next()
        .ok_or_else(|| invalid_config("expected repository code SHA argument"))?;
    if arguments.next().is_some() {
        return Err(invalid_config(
            "usage: rusty-procgen-ca-benchmark <repository-root> <40-character-code-sha>",
        ));
    }

    let suite: CaScenarioSuite = read_json(&root.join("fixtures/ca/scenarios.v1.json"))?;
    let engine_source: EngineSource = read_json(&root.join("engine-source.json"))?;
    if engine_source.schema_version != 1
        || engine_source.public_repository != "https://github.com/FuzzySlipper/rusty-engine"
    {
        return Err(invalid_config("engine-source.json identity is invalid"));
    }
    let evidence = benchmark_suite(
        &suite,
        &repository_commit,
        &engine_source.commit,
        benchmark_environment()?,
        BenchmarkRunConfig::default(),
        SystemBenchmarkClock::default,
    )?;
    let encoded = serde_json::to_vec_pretty(&evidence).map_err(|error| {
        rusty_procgen_engine_ca_benchmark::CaSpatialError::Serialization {
            detail: error.to_string(),
        }
    })?;
    let output = root.join("artifacts/evidence/engine-ca-benchmark.json");
    write_atomic(&output, &encoded)?;
    println!(
        "Engine CA benchmark: {} scenarios, {} recorded runs each; wrote {}",
        evidence.scenarios.len(),
        evidence.config.recorded_runs,
        output.display()
    );
    Ok(())
}

fn benchmark_environment(
) -> Result<CaBenchmarkEnvironment, rusty_procgen_engine_ca_benchmark::CaSpatialError> {
    let output = Command::new("rustc")
        .arg("--version")
        .output()
        .map_err(|error| invalid_config(&format!("cannot inspect rustc version: {error}")))?;
    if !output.status.success() {
        return Err(invalid_config("rustc --version failed"));
    }
    let rustc_version = String::from_utf8(output.stdout)
        .map_err(|error| invalid_config(&format!("rustc version is not UTF-8: {error}")))?
        .trim()
        .to_owned();
    Ok(CaBenchmarkEnvironment {
        operating_system: env::consts::OS.to_owned(),
        architecture: env::consts::ARCH.to_owned(),
        rustc_version,
        build_profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
        .to_owned(),
        clock: "std::time::Instant monotonic nanoseconds".to_owned(),
    })
}

fn read_json<T: serde::de::DeserializeOwned>(
    path: &Path,
) -> Result<T, rusty_procgen_engine_ca_benchmark::CaSpatialError> {
    let bytes =
        fs::read(path).map_err(|error| invalid_config(&format!("{}: {error}", path.display())))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| invalid_config(&format!("{} is invalid JSON: {error}", path.display())))
}

fn write_atomic(
    path: &Path,
    bytes: &[u8],
) -> Result<(), rusty_procgen_engine_ca_benchmark::CaSpatialError> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_config("evidence output has no parent"))?;
    fs::create_dir_all(parent)
        .map_err(|error| invalid_config(&format!("{}: {error}", parent.display())))?;
    let staging = parent.join(".engine-ca-benchmark.json.staging");
    let mut with_newline = bytes.to_vec();
    with_newline.push(b'\n');
    fs::write(&staging, with_newline)
        .map_err(|error| invalid_config(&format!("{}: {error}", staging.display())))?;
    fs::rename(&staging, path)
        .map_err(|error| invalid_config(&format!("{}: {error}", path.display())))
}

fn invalid_config(detail: &str) -> rusty_procgen_engine_ca_benchmark::CaSpatialError {
    rusty_procgen_engine_ca_benchmark::CaSpatialError::InvalidRunConfig {
        detail: detail.to_owned(),
    }
}
