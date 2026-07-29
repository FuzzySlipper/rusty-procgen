use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use content_store::{ContentManifest, ContentStoreIdentity, ContentWriteSetError};
use rusty_procgen_engine_publication::{
    compile_publication, load_checked_inputs, write_evidence_atomic, CheckedInputs, CompileInput,
    MappedPartSource, PublishDiagnosticCode, PublishError, MAX_SELECTED_INSTANCES,
};

#[test]
fn representative_publication_admits_and_reopens_through_engine_owners() {
    let inputs = inputs();
    let first = compile(&inputs).expect("representative publication");
    let repeated = compile(&inputs).expect("repeated publication");
    let first_evidence = first.evidence().expect("first evidence");
    let repeated_evidence = repeated.evidence().expect("repeated evidence");

    assert_eq!(first.prefab_registry.definitions.len(), 2);
    assert_eq!(first.scene.nodes.len(), 2);
    assert_eq!(first.asset_catalog.entries.len(), 2);
    assert_eq!(first.admitted_state.snapshot().entities.len(), 2);
    assert_eq!(
        serde_json::to_value(&first_evidence).unwrap(),
        serde_json::to_value(&repeated_evidence).unwrap()
    );
    assert_eq!(
        first_evidence
            .output
            .provenance
            .instances
            .iter()
            .map(|instance| (
                instance.procgen_instance_id.as_str(),
                instance.prefab_instance_id,
                instance.prefab_id,
            ))
            .collect::<Vec<_>>(),
        vec![
            ("instance.piece_room_room_region_start", 2_001, 1_003,),
            ("instance.piece_room_room_region_goal", 2_002, 1_002,),
        ]
    );
    assert!(first_evidence.readback.content_load_order_verified);

    let stale = ContentStoreIdentity::from_manifest(7, &ContentManifest::new(Vec::new())).unwrap();
    assert_eq!(
        first.candidate.clone().authorize(&stale).unwrap_err(),
        ContentWriteSetError::StaleStore
    );
}

#[test]
fn named_mapping_and_owner_failures_are_typed_and_leave_output_unchanged() {
    let root = temporary_root();
    let output = root.join("evidence.json");
    fs::write(&output, b"existing-publication\n").unwrap();

    let mut missing_mapping = inputs();
    missing_mapping
        .configuration
        .mappings
        .retain(|mapping| mapping.shape_id != "shape.room.flow_junction.spaced_8_exit");
    expect_unchanged(
        &output,
        missing_mapping,
        PublishDiagnosticCode::MissingPrefabMapping,
    );

    let mut missing_role = inputs();
    missing_role.configuration.mappings[2].stable_role.clear();
    expect_unchanged(
        &output,
        missing_role,
        PublishDiagnosticCode::MissingStableRole,
    );

    let mut incompatible_asset = inputs();
    incompatible_asset.configuration.mappings[2].source = MappedPartSource::VoxelObject {
        asset: "scene/not-a-voxel-object".to_owned(),
    };
    expect_unchanged(
        &output,
        incompatible_asset,
        PublishDiagnosticCode::IncompatibleSourceAsset,
    );

    let mut duplicate_identity = inputs();
    duplicate_identity.configuration.instance_identities[1].prefab_instance_id =
        duplicate_identity.configuration.instance_identities[0].prefab_instance_id;
    expect_unchanged(
        &output,
        duplicate_identity,
        PublishDiagnosticCode::DuplicateInstanceIdentity,
    );

    let mut invalid_transform = inputs();
    let selected = invalid_transform.configuration.selected_instance_ids[0].clone();
    let piece_id = invalid_transform
        .placement
        .instances
        .iter_mut()
        .find(|instance| instance.instance_id == selected)
        .map(|instance| {
            instance.transform = "mirror".to_owned();
            instance.piece_id.clone()
        })
        .unwrap();
    invalid_transform
        .shape_match
        .matches
        .iter_mut()
        .find(|matched| matched.piece_id == piece_id)
        .unwrap()
        .transform = "mirror".to_owned();
    expect_unchanged(
        &output,
        invalid_transform,
        PublishDiagnosticCode::InvalidTransform,
    );

    let mut stale_pin = inputs();
    stale_pin
        .source_bodies
        .get_mut("fixtures/prefab-sources/procgen-standard-room.voxel.json")
        .unwrap()
        .push(b' ');
    expect_unchanged(&output, stale_pin, PublishDiagnosticCode::StalePin);

    let mut over_quota = inputs();
    over_quota.configuration.selected_instance_ids =
        vec!["instance.over-quota".to_owned(); MAX_SELECTED_INSTANCES + 1];
    expect_unchanged(&output, over_quota, PublishDiagnosticCode::QuotaExceeded);

    let mut late_validation = inputs();
    late_validation.configuration.mappings[2]
        .part_namespace
        .clear();
    expect_unchanged(
        &output,
        late_validation,
        PublishDiagnosticCode::LateValidation,
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn evidence_file_is_atomically_replaced_only_after_successful_compilation() {
    let inputs = inputs();
    let evidence = compile(&inputs)
        .expect("publication")
        .evidence()
        .expect("evidence");
    let root = temporary_root();
    let output = root.join("nested/evidence.json");
    write_evidence_atomic(&output, &evidence).expect("atomic evidence publication");
    let reopened: serde_json::Value =
        serde_json::from_slice(&fs::read(&output).expect("published evidence")).unwrap();
    assert_eq!(
        reopened["output"]["candidateHash"],
        evidence.output.candidate_hash
    );
    assert!(!root.join("nested/.evidence.json.next").exists());
    fs::remove_dir_all(root).unwrap();
}

fn inputs() -> CheckedInputs {
    load_checked_inputs(&repo_root()).expect("checked publication inputs")
}

fn compile(
    inputs: &CheckedInputs,
) -> Result<rusty_procgen_engine_publication::CompiledPublication, PublishError> {
    compile_publication(CompileInput {
        catalog: &inputs.catalog,
        shape_match: &inputs.shape_match,
        placement: &inputs.placement,
        configuration: &inputs.configuration,
        source_bodies: &inputs.source_bodies,
    })
}

fn expect_unchanged(output: &Path, inputs: CheckedInputs, expected_code: PublishDiagnosticCode) {
    let before = fs::read(output).unwrap();
    let error = compile(&inputs).expect_err("publication should reject");
    assert_eq!(error.code, expected_code, "{error}");
    assert_eq!(fs::read(output).unwrap(), before);
    assert!(!output.with_extension("json.next").exists());
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn temporary_root() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "rusty-procgen-engine-publication-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}
