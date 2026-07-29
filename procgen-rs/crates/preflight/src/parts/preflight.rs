fn run_preflight_command(repo_root: &Path) -> Result<(), String> {
    let summary = run_preflight(repo_root)?;
    println!(
        "rusty-procgen preflight OK: engine source {}, rust lane {}",
        summary.engine_source, summary.rust_dir
    );
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct PreflightSummary {
    engine_source: String,
    rust_dir: String,
}

fn run_preflight(repo_root: &Path) -> Result<PreflightSummary, String> {
    let engine_source = "../asha-engine";
    reject_private_engine_path("engine source", engine_source)?;

    let engine_root = repo_root.join(engine_source);
    if !engine_root.exists() {
        return Err(format!(
            "expected sibling ASHA engine checkout at {}",
            engine_root.display()
        ));
    }

    Ok(PreflightSummary {
        engine_source: engine_source.to_owned(),
        rust_dir: "procgen-rs".to_owned(),
    })
}

fn reject_private_engine_path(label: &str, value: &str) -> Result<(), String> {
    let forbidden_fragments = vec![
        format!("{}/{}", "../asha-engine", "engine-rs"),
        format!("{}/{}", "../asha-engine", "ts/packages"),
        format!("{}/{}", "../asha", "engine-rs"),
        format!("{}/{}", "../asha", "ts/packages"),
    ];
    for fragment in forbidden_fragments {
        if value.contains(fragment.as_str()) {
            return Err(format!(
                "{label} must not reference private ASHA internals: {value}"
            ));
        }
    }
    Ok(())
}
