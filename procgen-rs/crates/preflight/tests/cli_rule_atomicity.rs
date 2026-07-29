use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use rusty_procgen_preflight::core::ProcgenCore;
use rusty_procgen_preflight::SeedIntent;

fn seed_intent() -> SeedIntent {
    serde_json::from_str(include_str!(
        "../../../../fixtures/intents/first-slice.intent.json"
    ))
    .expect("checked seed intent")
}

#[test]
fn cli_repeated_seed_rejects_without_output_or_state_mutation() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "rusty-procgen-cli-rule-atomicity-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("temporary test directory");
    let source = root.join("source.json");
    let accepted = root.join("accepted.json");
    let accepted_receipt = root.join("accepted.receipt.json");
    let rejected = root.join("rejected.json");
    let rejected_receipt = root.join("rejected.receipt.json");
    write_json(
        &source,
        &ProcgenCore::create_candidate(&seed_intent(), 6_201),
    );

    let first = command(&source, &accepted, &accepted_receipt)
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "first application failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let accepted_before = fs::read(&accepted).expect("accepted candidate bytes");

    let second = command(&accepted, &rejected, &rejected_receipt)
        .output()
        .unwrap();
    assert!(!second.status.success());
    assert!(!rejected.exists(), "rejected command published output");
    assert_eq!(
        fs::read(&accepted).expect("accepted candidate after rejection"),
        accepted_before
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(&rejected_receipt).expect("rejection receipt"))
            .expect("rejection receipt JSON");
    assert_eq!(receipt["status"], "rejected");
    assert_eq!(receipt["outputRef"], serde_json::Value::Null);
    assert!(receipt["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .any(|diagnostic| diagnostic["code"] == "duplicate_node_id"));

    fs::remove_dir_all(root).expect("remove temporary test directory");
}

fn command(state: &Path, out: &Path, receipt: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rusty-procgen"));
    command.args([
        "graph",
        "apply-rule",
        "--state",
        state.to_str().expect("state path"),
        "--rule",
        "optional_treasure_detour",
        "--seed",
        "77",
        "--out",
        out.to_str().expect("output path"),
        "--receipt",
        receipt.to_str().expect("receipt path"),
    ]);
    command
}

fn write_json(path: &Path, value: &impl serde::Serialize) {
    let mut bytes = serde_json::to_vec_pretty(value).expect("candidate JSON");
    bytes.push(b'\n');
    fs::write(path, bytes).expect("write candidate");
}
