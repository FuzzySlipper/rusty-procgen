use std::env;
use std::fs;
use std::path::PathBuf;

use rusty_procgen_preflight::cellular_automata::CaScenarioSuite;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut arguments = env::args().skip(1);
    let check = matches!(arguments.next().as_deref(), Some("--check"));
    let input = if check {
        arguments.next()
    } else {
        env::args().nth(1)
    }
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from("fixtures/ca/scenarios.v1.json"));
    let output = arguments
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("fixtures/ca/delta-traces.v1.json"));
    if arguments.next().is_some() {
        return Err(
            "usage: generate-ca-fixtures [--check] [scenario-suite] [delta-fixtures]".to_owned(),
        );
    }

    let suite: CaScenarioSuite = serde_json::from_slice(
        &fs::read(&input).map_err(|error| format!("cannot read {}: {error}", input.display()))?,
    )
    .map_err(|error| format!("cannot decode {}: {error}", input.display()))?;
    let fixtures = suite
        .generate_delta_fixtures()
        .map_err(|error| format!("cannot generate fixtures: {error}"))?;
    let generated = serde_json::to_string_pretty(&fixtures)
        .map_err(|error| format!("cannot encode fixtures: {error}"))?
        + "\n";

    if check {
        let current = fs::read_to_string(&output)
            .map_err(|error| format!("cannot read {}: {error}", output.display()))?;
        if current != generated {
            return Err(format!(
                "{} differs from deterministic CA generation",
                output.display()
            ));
        }
        println!(
            "cellular automata fixtures match {} scenarios",
            fixtures.traces.len()
        );
    } else {
        fs::write(&output, generated)
            .map_err(|error| format!("cannot write {}: {error}", output.display()))?;
        println!(
            "wrote {} deterministic cellular automata traces to {}",
            fixtures.traces.len(),
            output.display()
        );
    }
    Ok(())
}
