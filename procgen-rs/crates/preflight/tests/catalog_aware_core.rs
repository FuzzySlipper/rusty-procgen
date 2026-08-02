use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use rusty_procgen_preflight::core::{
    CatalogAwareGenerationPolicy, CatalogAwareGenerationProvenance, CorridorRealization,
    ProcgenCore,
};
use rusty_procgen_preflight::{
    Candidate, Geometry2dArtifact, IntermediateBreakdown, PieceBuildPlan, PieceShapeMatchReport,
    ShapeCatalog,
};
use serde::Serialize;
use serde_json::Value;

fn checked_candidate() -> Candidate {
    let accepted: Value = serde_json::from_str(include_str!(
        "../../../../artifacts/samples/batch-v2/candidate-000/accepted.json"
    ))
    .expect("checked accepted artifact");
    serde_json::from_value(accepted["candidate"].clone()).expect("checked candidate")
}

fn checked_intermediate() -> IntermediateBreakdown {
    serde_json::from_str(include_str!(
        "../../../../artifacts/samples/batch-v2/candidate-000/intermediate-breakdown.json"
    ))
    .expect("checked intermediate")
}

fn checked_geometry() -> Geometry2dArtifact {
    serde_json::from_str(include_str!(
        "../../../../artifacts/samples/batch-v2/candidate-000/geometry-2d.json"
    ))
    .expect("checked geometry")
}

fn checked_catalog() -> ShapeCatalog {
    serde_json::from_str(include_str!(
        "../../../../fixtures/shape-catalogs/2d-basic.json"
    ))
    .expect("checked catalog")
}

fn checked_policy() -> CatalogAwareGenerationPolicy {
    serde_json::from_str(include_str!(
        "../../../../fixtures/policies/catalog-aware-generation-default.json"
    ))
    .expect("checked catalog-aware policy")
}

fn checked_seed() -> u64 {
    let shape_match: PieceShapeMatchReport = serde_json::from_str(include_str!(
        "../../../../artifacts/samples/batch-v2/candidate-000/piece-shape-match.json"
    ))
    .expect("checked shape match");
    shape_match.seed
}

fn catalog_plan(
    candidate: &Candidate,
    intermediate: &IntermediateBreakdown,
    geometry: &Geometry2dArtifact,
) -> PieceBuildPlan {
    ProcgenCore::emit_piece_plan(
        candidate,
        intermediate,
        geometry,
        CorridorRealization::Catalog,
    )
    .expect("catalog piece plan")
}

fn memory_provenance() -> CatalogAwareGenerationProvenance {
    CatalogAwareGenerationProvenance {
        candidate_ref: "memory/candidate.json".to_owned(),
        geometry_ref: "memory/geometry.json".to_owned(),
        piece_plan_ref: "memory/piece-plan.json".to_owned(),
        catalog_ref: "memory/catalog.json".to_owned(),
        result_ref: "memory/catalog-aware-result.json".to_owned(),
    }
}

fn input_hashes(
    candidate: &Candidate,
    geometry: &Geometry2dArtifact,
    plan: &PieceBuildPlan,
    catalog: &ShapeCatalog,
    policy: &CatalogAwareGenerationPolicy,
) -> Vec<String> {
    vec![
        ProcgenCore::canonical_hash(candidate).expect("candidate hash"),
        ProcgenCore::canonical_hash(geometry).expect("geometry hash"),
        ProcgenCore::canonical_hash(plan).expect("plan hash"),
        ProcgenCore::canonical_hash(catalog).expect("catalog hash"),
        ProcgenCore::canonical_hash(policy).expect("policy hash"),
    ]
}

fn is_room_requirement(kind: &str) -> bool {
    !matches!(kind, "connector" | "corridor" | "bend" | "junction")
}

fn classification(result: &rusty_procgen_preflight::core::CatalogAwareGenerationResult) -> &str {
    result
        .exhausted_classification
        .as_deref()
        .unwrap_or("success")
}

fn write_json(path: &Path, value: &impl Serialize) {
    let encoded = serde_json::to_string_pretty(value).expect("encode fixture");
    fs::write(path, format!("{encoded}\n")).expect("write fixture");
}

struct CliCase<'a> {
    candidate: &'a Candidate,
    geometry: &'a Geometry2dArtifact,
    plan: &'a PieceBuildPlan,
    catalog: &'a ShapeCatalog,
    policy: &'a CatalogAwareGenerationPolicy,
    seed: u64,
}

fn cli_converges(root: &Path, label: &str, case: CliCase<'_>) {
    let case_dir = root.join(label);
    fs::create_dir_all(&case_dir).expect("create CLI case");
    let candidate_path = case_dir.join("candidate.json");
    let geometry_path = case_dir.join("geometry.json");
    let plan_path = case_dir.join("piece-plan.json");
    let catalog_path = case_dir.join("catalog.json");
    let policy_path = case_dir.join("policy.json");
    let result_path = case_dir.join("result.json");
    write_json(&candidate_path, case.candidate);
    write_json(&geometry_path, case.geometry);
    write_json(&plan_path, case.plan);
    write_json(&catalog_path, case.catalog);
    write_json(&policy_path, case.policy);

    let provenance = CatalogAwareGenerationProvenance {
        candidate_ref: candidate_path.to_string_lossy().into_owned(),
        geometry_ref: geometry_path.to_string_lossy().into_owned(),
        piece_plan_ref: plan_path.to_string_lossy().into_owned(),
        catalog_ref: catalog_path.to_string_lossy().into_owned(),
        result_ref: result_path.to_string_lossy().into_owned(),
    };
    let public = ProcgenCore::realize_catalog_aware(
        case.candidate,
        case.geometry,
        case.plan,
        case.catalog,
        case.policy,
        &provenance,
        case.seed,
    )
    .expect("public catalog-aware run");

    let output = Command::new(env!("CARGO_BIN_EXE_rusty-procgen"))
        .args(["build", "realize-catalog-aware", "--candidate"])
        .arg(&candidate_path)
        .arg("--geometry")
        .arg(&geometry_path)
        .arg("--piece-plan")
        .arg(&plan_path)
        .arg("--catalog")
        .arg(&catalog_path)
        .arg("--policy")
        .arg(&policy_path)
        .arg("--seed")
        .arg(case.seed.to_string())
        .arg("--out")
        .arg(&result_path)
        .output()
        .expect("run catalog-aware CLI");
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let cli: Value = serde_json::from_slice(&fs::read(&result_path).expect("read CLI result"))
        .expect("decode CLI result");
    assert_eq!(
        cli,
        serde_json::to_value(public).expect("encode public result"),
        "CLI and public result diverged for {label}"
    );
}

struct CurrentDirGuard(PathBuf);

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.0).expect("restore current directory");
    }
}

#[test]
fn public_catalog_aware_runner_is_filesystem_free_atomic_and_cli_equivalent() {
    let candidate = checked_candidate();
    let intermediate = checked_intermediate();
    let geometry = checked_geometry();
    let plan = catalog_plan(&candidate, &intermediate, &geometry);
    let catalog = checked_catalog();
    let policy = checked_policy();
    let seed = checked_seed();
    let provenance = memory_provenance();
    let before = input_hashes(&candidate, &geometry, &plan, &catalog, &policy);

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let test_root = std::env::temp_dir().join(format!(
        "rusty-procgen-catalog-aware-core-{}-{unique}",
        std::process::id()
    ));
    let empty_dir = test_root.join("empty");
    fs::create_dir_all(&empty_dir).expect("create empty working directory");
    let original_dir = std::env::current_dir().expect("current directory");
    std::env::set_current_dir(&empty_dir).expect("enter empty working directory");
    let guard = CurrentDirGuard(original_dir);

    let accepted = ProcgenCore::realize_catalog_aware(
        &candidate,
        &geometry,
        &plan,
        &catalog,
        &policy,
        &provenance,
        seed,
    )
    .expect("accepted public run");
    let repeated = ProcgenCore::realize_catalog_aware(
        &candidate,
        &geometry,
        &plan,
        &catalog,
        &policy,
        &provenance,
        seed,
    )
    .expect("repeated public run");
    assert!(accepted.ok);
    let accepted_plan = accepted.piece_plan.as_ref().expect("accepted plan");
    let accepted_match = accepted.shape_match.as_ref().expect("accepted match");
    let accepted_placement = accepted.placement.as_ref().expect("accepted placement");
    let validation = ProcgenCore::validate_placement_with_catalog(
        &catalog,
        &plan,
        accepted_plan,
        accepted_match,
        accepted_placement,
    );
    assert!(validation.ok, "{:?}", validation.diagnostics);

    let mut forged_match = accepted_match.clone();
    forged_match.matches[0].candidate_rank += 1;
    let forged_match_validation = ProcgenCore::validate_placement_with_catalog(
        &catalog,
        &plan,
        accepted_plan,
        &forged_match,
        accepted_placement,
    );
    assert!(forged_match_validation
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "catalog_shape_match_stale"));

    let mut synchronized_match = accepted_match.clone();
    synchronized_match.matches[0].candidate_rank += 1;
    let mut synchronized_placement = accepted_placement.clone();
    synchronized_placement
        .catalog_search
        .as_mut()
        .expect("catalog search evidence")
        .selected[0]
        .candidate_rank += 1;
    let synchronized_validation = ProcgenCore::validate_placement_with_catalog(
        &catalog,
        &plan,
        accepted_plan,
        &synchronized_match,
        &synchronized_placement,
    );
    assert!(
        synchronized_validation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "catalog_shape_match_stale"),
        "validator trusted matching forged catalog-aware rank receipts"
    );

    let mut forged_placement = accepted_placement.clone();
    let cell = forged_placement
        .instances
        .iter_mut()
        .flat_map(|instance| instance.occupied_cells.iter_mut())
        .next()
        .expect("catalog-aware fixture occupied cell");
    cell.x += 10_000;
    let forged_cell_validation = ProcgenCore::validate_placement_with_catalog(
        &catalog,
        &plan,
        accepted_plan,
        accepted_match,
        &forged_placement,
    );
    assert!(forged_cell_validation
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "catalog_instance_surface_stale"));
    assert_eq!(
        serde_json::to_value(&accepted).expect("accepted JSON"),
        serde_json::to_value(&repeated).expect("repeated JSON")
    );

    let mut coverage_catalog = catalog.clone();
    coverage_catalog.shapes.retain(|shape| {
        shape
            .piece_kinds
            .iter()
            .any(|kind| matches!(kind.as_str(), "corridor" | "bend"))
    });
    let coverage_gap = ProcgenCore::realize_catalog_aware(
        &candidate,
        &geometry,
        &plan,
        &coverage_catalog,
        &policy,
        &provenance,
        seed,
    )
    .expect("coverage-gap public run");
    assert!(!coverage_gap.ok);
    assert_eq!(classification(&coverage_gap), "catalog_coverage_gap");

    let mut overlapping_plan = plan.clone();
    for requirement in &mut overlapping_plan.requirements {
        if is_room_requirement(requirement.kind.as_str()) {
            requirement
                .placement_hints
                .retain(|hint| !hint.starts_with("geometryRect:"));
            requirement
                .placement_hints
                .push("geometryRect:0:0:160:112".to_owned());
        }
    }
    let infeasible = ProcgenCore::realize_catalog_aware(
        &candidate,
        &geometry,
        &overlapping_plan,
        &catalog,
        &policy,
        &provenance,
        seed,
    )
    .expect("infeasible public run");
    assert!(!infeasible.ok);
    assert_eq!(classification(&infeasible), "generation_infeasibility");

    let budget_policy = CatalogAwareGenerationPolicy {
        max_routing_states_per_section: 100,
        ..policy.clone()
    };
    let exhausted = ProcgenCore::realize_catalog_aware(
        &candidate,
        &geometry,
        &plan,
        &catalog,
        &budget_policy,
        &provenance,
        seed,
    )
    .expect("budget-exhausted public run");
    assert!(!exhausted.ok);
    assert_eq!(classification(&exhausted), "search_budget_exhaustion");

    assert_eq!(
        before,
        input_hashes(&candidate, &geometry, &plan, &catalog, &policy),
        "public runner mutated caller inputs"
    );
    assert_eq!(
        fs::read_dir(&empty_dir)
            .expect("read empty working directory")
            .count(),
        0,
        "public runner created filesystem output"
    );

    drop(guard);
    let cli_root = test_root.join("cli");
    cli_converges(
        &cli_root,
        "accepted",
        CliCase {
            candidate: &candidate,
            geometry: &geometry,
            plan: &plan,
            catalog: &catalog,
            policy: &policy,
            seed,
        },
    );
    cli_converges(
        &cli_root,
        "coverage-gap",
        CliCase {
            candidate: &candidate,
            geometry: &geometry,
            plan: &plan,
            catalog: &coverage_catalog,
            policy: &policy,
            seed,
        },
    );
    cli_converges(
        &cli_root,
        "infeasible",
        CliCase {
            candidate: &candidate,
            geometry: &geometry,
            plan: &overlapping_plan,
            catalog: &catalog,
            policy: &policy,
            seed,
        },
    );
    cli_converges(
        &cli_root,
        "budget-exhausted",
        CliCase {
            candidate: &candidate,
            geometry: &geometry,
            plan: &plan,
            catalog: &catalog,
            policy: &budget_policy,
            seed,
        },
    );
    fs::remove_dir_all(&test_root).expect("remove test directory");
}
