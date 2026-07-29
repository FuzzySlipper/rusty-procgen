use std::env;
use std::fs;
use std::path::PathBuf;

use engine_spatial::VoxelSourceRevision;
use rusty_procgen_engine_spatial::{ExtrusionOptions, SpatialExtrusionHost};
use rusty_procgen_preflight::PiecePlacement;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Evidence {
    kind: &'static str,
    schema_version: u32,
    source_placement: String,
    placement_id: String,
    plan_sha256: String,
    engine_commit: String,
    coordinate_mapping: String,
    enclosure: Enclosure,
    counts: Counts,
    authority: Authority,
    failures: Failures,
    non_claims: [&'static str; 6],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Enclosure {
    floor_y: i64,
    wall_min_y: i64,
    wall_max_y: i64,
    ceiling_y: i64,
    floor_material: u16,
    wall_material: u16,
    ceiling_material: u16,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Counts {
    walkable_cells: usize,
    declared_opening_cells: usize,
    boundary_cells: usize,
    solid_voxels: usize,
    resident_chunks: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Authority {
    mutation_path: &'static str,
    accepted_revision: u64,
    changed_voxels: usize,
    transaction_count: usize,
    max_edits_per_transaction: usize,
    deterministic: bool,
    deterministic_repeat_hash: String,
    reopened_exactly: bool,
    deterministic_continuation: bool,
    continuation_authority_hash: String,
    readout: rusty_procgen_engine_spatial::SpatialReadout,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Failures {
    rejected_unknown_material_without_mutation: bool,
    rejected_stale_revision_without_mutation: bool,
    rejected_malformed_placement_without_mutation: bool,
    rejected_oversized_plan_without_mutation: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = PathBuf::from(env::args().nth(1).unwrap_or_else(|| ".".to_owned()));
    let source_relative = "artifacts/samples/batch-v2/candidate-006/piece-placement.json";
    let source_path = repo_root.join(source_relative);
    let placement: PiecePlacement = serde_json::from_slice(&fs::read(&source_path)?)?;
    let options = ExtrusionOptions::default();

    let mut first = SpatialExtrusionHost::empty(options)?;
    let (plan, receipt) = first.admit_placement(VoxelSourceRevision::INITIAL, &placement)?;
    let first_readout = first.readout();
    let mut repeated = SpatialExtrusionHost::empty(options)?;
    let (_, repeated_receipt) =
        repeated.admit_placement(VoxelSourceRevision::INITIAL, &placement)?;
    let deterministic = first_readout == repeated_receipt.readout;

    let reopened = SpatialExtrusionHost::reopen(
        options,
        &plan,
        VoxelSourceRevision::new(receipt.accepted_revision),
    )?;
    let reopened_exactly = reopened.readout() == first_readout;
    let mut continuation_plan = plan.clone();
    continuation_plan.solid_voxels[0].material =
        if continuation_plan.solid_voxels[0].material == options.wall_material {
            options.floor_material
        } else {
            options.wall_material
        };
    let mut continued_first = SpatialExtrusionHost::reopen(
        options,
        &plan,
        VoxelSourceRevision::new(receipt.accepted_revision),
    )?;
    let mut continued_reopened = SpatialExtrusionHost::reopen(
        options,
        &plan,
        VoxelSourceRevision::new(receipt.accepted_revision),
    )?;
    let continuation_revision = continued_first.source_revision();
    let first_continuation =
        continued_first.admit_plan(continuation_revision, &continuation_plan)?;
    let reopened_continuation =
        continued_reopened.admit_plan(continuation_revision, &continuation_plan)?;
    let deterministic_continuation = first_continuation == reopened_continuation
        && continued_first.readout() == continued_reopened.readout();

    let baseline = serde_json::to_vec(&first_readout)?;
    let mut unknown_material = plan.clone();
    unknown_material.solid_voxels[0].material = 65_535;
    let rejected_unknown_material_without_mutation = first
        .admit_plan(first.source_revision(), &unknown_material)
        .is_err()
        && serde_json::to_vec(&first.readout())? == baseline;
    let rejected_stale_revision_without_mutation = first
        .admit_plan(VoxelSourceRevision::INITIAL, &plan)
        .is_err()
        && serde_json::to_vec(&first.readout())? == baseline;

    let mut malformed = placement.clone();
    malformed.kind = "invalid-placement-kind".to_owned();
    let rejected_malformed_placement_without_mutation = first
        .admit_placement(first.source_revision(), &malformed)
        .is_err()
        && serde_json::to_vec(&first.readout())? == baseline;

    let mut oversized = plan.clone();
    oversized.solid_voxel_count = engine_spatial::MAX_SOLID_VOXELS + 1;
    let rejected_oversized_plan_without_mutation = first
        .admit_plan(first.source_revision(), &oversized)
        .is_err()
        && serde_json::to_vec(&first.readout())? == baseline;

    if !deterministic
        || !reopened_exactly
        || !deterministic_continuation
        || !rejected_unknown_material_without_mutation
        || !rejected_stale_revision_without_mutation
        || !rejected_malformed_placement_without_mutation
        || !rejected_oversized_plan_without_mutation
    {
        return Err("spatial extrusion proof did not satisfy its invariants".into());
    }

    let plan_sha256 = format!("sha256:{:x}", Sha256::digest(serde_json::to_vec(&plan)?));
    let evidence = Evidence {
        kind: "rusty_procgen.evidence.engine_spatial_extrusion.v2",
        schema_version: 2,
        source_placement: source_relative.to_owned(),
        placement_id: plan.placement_id.clone(),
        plan_sha256,
        engine_commit: engine_commit(&repo_root)?,
        coordinate_mapping: plan.coordinate_mapping.clone(),
        enclosure: Enclosure {
            floor_y: options.floor_y,
            wall_min_y: options.wall_min_y,
            wall_max_y: options.wall_max_y,
            ceiling_y: options.ceiling_y,
            floor_material: options.floor_material,
            wall_material: options.wall_material,
            ceiling_material: options.ceiling_material,
        },
        counts: Counts {
            walkable_cells: plan.walkable_cell_count,
            declared_opening_cells: plan.opening_cell_count,
            boundary_cells: plan.boundary_cell_count,
            solid_voxels: plan.solid_voxel_count,
            resident_chunks: plan.resident_chunk_count,
        },
        authority: Authority {
            mutation_path: "engine_spatial::VoxelEditService",
            accepted_revision: receipt.accepted_revision,
            changed_voxels: receipt.changed_voxels,
            transaction_count: receipt.transaction_count,
            max_edits_per_transaction: receipt.max_edits_per_transaction,
            deterministic,
            deterministic_repeat_hash: repeated_receipt.readout.authority_hash,
            reopened_exactly,
            deterministic_continuation,
            continuation_authority_hash: first_continuation.readout.authority_hash,
            readout: first_readout,
        },
        failures: Failures {
            rejected_unknown_material_without_mutation,
            rejected_stale_revision_without_mutation,
            rejected_malformed_placement_without_mutation,
            rejected_oversized_plan_without_mutation,
        },
        non_claims: [
            "not_renderer_proof",
            "not_browser_proof",
            "not_gameplay_proof",
            "not_navigation_quality_proof",
            "not_performance_proof",
            "not_persistence_policy",
        ],
    };
    let canonical = serde_json::to_vec_pretty(&evidence)?;
    let evidence_path = repo_root.join("artifacts/evidence/engine-spatial-extrusion.json");
    let temporary_path = evidence_path.with_extension("json.tmp");
    fs::write(&temporary_path, [&canonical[..], b"\n"].concat())?;
    fs::rename(&temporary_path, &evidence_path)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "evidence": evidence_path.strip_prefix(&repo_root)?.to_string_lossy(),
            "sha256": format!("{:x}", Sha256::digest([&canonical[..], b"\n"].concat())),
            "authorityHash": evidence.authority.readout.authority_hash,
            "planSha256": evidence.plan_sha256,
            "solidVoxels": evidence.counts.solid_voxels,
        }))?
    );
    Ok(())
}

fn engine_commit(repo_root: &std::path::Path) -> Result<String, Box<dyn std::error::Error>> {
    let source: serde_json::Value =
        serde_json::from_slice(&fs::read(repo_root.join("engine-source.json"))?)?;
    source
        .get("commit")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "engine-source.json has no commit".into())
}
