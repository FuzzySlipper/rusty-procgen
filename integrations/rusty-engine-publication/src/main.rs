use std::path::PathBuf;

use rusty_procgen_engine_publication::{
    compile_publication, load_checked_inputs, write_evidence_atomic, CompileInput,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../.."));
    let evidence_path = repo_root.join("artifacts/evidence/engine-authored-publication.json");
    let inputs = load_checked_inputs(&repo_root)?;
    let publication = compile_publication(CompileInput {
        catalog: &inputs.catalog,
        shape_match: &inputs.shape_match,
        placement: &inputs.placement,
        configuration: &inputs.configuration,
        source_bodies: &inputs.source_bodies,
    })?;
    let evidence = publication.evidence()?;
    write_evidence_atomic(&evidence_path, &evidence)?;
    println!(
        "published {} Engine-authored scene instances to {}",
        evidence.output.scene_nodes,
        evidence_path.display()
    );
    Ok(())
}
