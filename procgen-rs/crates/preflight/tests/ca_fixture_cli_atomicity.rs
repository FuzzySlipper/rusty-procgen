use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use rusty_procgen_preflight::cellular_automata::CaScenarioSuite;

#[test]
fn rejected_aggregate_workload_does_not_replace_fixture_output() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "rusty-procgen-ca-fixture-atomicity-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("temporary test directory");
    let input = root.join("over-limit-scenarios.json");
    let output = root.join("published-fixtures.json");
    let sentinel = b"existing published fixture\n";

    let mut suite: CaScenarioSuite =
        serde_json::from_str(include_str!("../../../../fixtures/ca/scenarios.v1.json"))
            .expect("checked CA scenario suite");
    let scenario = suite.scenarios.first_mut().expect("scenario");
    scenario.bounds.min.x = 0;
    scenario.bounds.min.y = 0;
    scenario.bounds.min.z = 0;
    scenario.bounds.max_exclusive.x = 1_024;
    scenario.bounds.max_exclusive.y = 1_024;
    scenario.bounds.max_exclusive.z = 1;
    scenario.steps = 4_096;
    scenario.initial_cells.clear();
    fs::write(
        &input,
        serde_json::to_vec_pretty(&suite).expect("scenario suite JSON"),
    )
    .expect("write scenario suite");
    fs::write(&output, sentinel).expect("write sentinel fixture");

    let result = Command::new(env!("CARGO_BIN_EXE_generate-ca-fixtures"))
        .args([
            input.to_str().expect("input path"),
            output.to_str().expect("output path"),
        ])
        .output()
        .expect("run fixture generator");

    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("cell_step_quota_exceeded"));
    assert_eq!(
        fs::read(&output).expect("published fixture after rejection"),
        sentinel
    );

    fs::remove_dir_all(root).expect("remove temporary test directory");
}
