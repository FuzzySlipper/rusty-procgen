use rusty_procgen_preflight::core::{
    CorridorRealization, GraphRule, GridConnectivity, ProcgenCore, RuleDisposition,
};
use rusty_procgen_preflight::{SeedIntent, ShapeCatalog};

fn seed_intent() -> SeedIntent {
    serde_json::from_str(include_str!(
        "../../../../fixtures/intents/first-slice.intent.json"
    ))
    .expect("checked seed intent")
}

fn shape_catalog() -> ShapeCatalog {
    serde_json::from_str(include_str!(
        "../../../../fixtures/shape-catalogs/2d-basic.json"
    ))
    .expect("checked shape catalog")
}

#[test]
fn public_rule_api_is_deterministic_and_fail_atomic() {
    let candidate = ProcgenCore::create_candidate(&seed_intent(), 4_103);

    let first = ProcgenCore::apply_rule(&candidate, GraphRule::LockKeyLoop, 4_104);
    let repeated = ProcgenCore::apply_rule(&candidate, GraphRule::LockKeyLoop, 4_104);
    assert_eq!(first.disposition, RuleDisposition::Accepted);
    assert_eq!(repeated.disposition, RuleDisposition::Accepted);
    assert_eq!(
        ProcgenCore::canonical_hash(&first.candidate).expect("first candidate hash"),
        ProcgenCore::canonical_hash(&repeated.candidate).expect("repeated candidate hash")
    );

    let rejected = ProcgenCore::apply_rule(&first.candidate, GraphRule::LockKeyLoop, 4_105);
    assert_eq!(rejected.disposition, RuleDisposition::Rejected);
    assert_eq!(
        ProcgenCore::canonical_hash(&first.candidate).expect("accepted candidate hash"),
        ProcgenCore::canonical_hash(&rejected.candidate).expect("rejected candidate hash")
    );
    assert!(rejected
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "rule_already_applied"));
}

#[test]
fn public_core_runs_the_bounded_pipeline_without_filesystem_io() {
    let candidate = ProcgenCore::apply_rule(
        &ProcgenCore::create_candidate(&seed_intent(), 5_801),
        GraphRule::LockKeyLoop,
        5_802,
    )
    .candidate;
    assert!(ProcgenCore::validate_candidate(&candidate).ok);

    let annotations =
        ProcgenCore::annotate_spatial_intent(&candidate).expect("spatial annotations");
    let intermediate =
        ProcgenCore::breakdown(&candidate, &annotations).expect("intermediate breakdown");
    assert!(ProcgenCore::validate_intermediate(&intermediate).ok);

    let connections =
        ProcgenCore::plan_connections(&candidate, &intermediate).expect("connection plan");
    let policy = ProcgenCore::default_geometry_policy();
    let geometry =
        ProcgenCore::emit_geometry(&candidate, &intermediate, &connections, &policy, 5_803)
            .expect("geometry");
    assert!(ProcgenCore::validate_geometry(&geometry).ok);
    assert!(geometry.rooms.iter().all(|room| {
        room.rect.x >= 0
            && room.rect.y >= 0
            && room.rect.x + room.rect.width <= geometry.bounds.width
            && room.rect.y + room.rect.height <= geometry.bounds.height
    }));

    let piece_plan = ProcgenCore::emit_piece_plan(
        &candidate,
        &intermediate,
        &geometry,
        CorridorRealization::Hybrid,
    )
    .expect("piece plan");
    let catalog = shape_catalog();
    let catalog_inspection = ProcgenCore::inspect_catalog(&catalog);
    assert_eq!(catalog_inspection.shape_count, catalog.shapes.len());
    assert!(catalog_inspection.diagnostics.is_empty());
    let shape_match = ProcgenCore::match_shapes(&catalog, &piece_plan, 5_804);
    assert!(shape_match.ok);
    let placement = ProcgenCore::assemble(
        &catalog,
        &piece_plan,
        &shape_match,
        GridConnectivity::FourWay,
    )
    .expect("piece placement");
    assert!(ProcgenCore::validate_placement(&placement).ok);
    assert!(ProcgenCore::validate_built_flow(&candidate, &geometry, &piece_plan, &placement).ok);

    let repeated =
        ProcgenCore::emit_geometry(&candidate, &intermediate, &connections, &policy, 5_803)
            .expect("repeated geometry");
    assert_eq!(
        ProcgenCore::canonical_hash(&geometry).expect("geometry hash"),
        ProcgenCore::canonical_hash(&repeated).expect("repeated geometry hash")
    );
}

#[test]
fn public_core_rejects_malformed_realization_tiers() {
    assert_eq!(ProcgenCore::realization_scale_multiplier(0), Some(1));
    assert_eq!(ProcgenCore::realization_scale_multiplier(7), Some(8));
    assert_eq!(ProcgenCore::realization_scale_multiplier(u32::MAX), None);
}
