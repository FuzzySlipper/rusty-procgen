use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use engine_spatial::{VoxelEditApplyError, VoxelEditRejection, VoxelSourceRevision};
use rusty_procgen_engine_spatial::{
    compile_placement_extrusion, ExtrusionBounds, ExtrusionOptions, PlanVoxel,
    SpatialExtrusionError, SpatialExtrusionHost, VoxelCoordinate, VoxelExtrusionPlan,
};
use rusty_procgen_preflight::PiecePlacement;

#[test]
fn representative_placement_preserves_the_established_extrusion_policy() {
    let placement = representative_placement();
    let options = ExtrusionOptions::default();
    let plan = compile_placement_extrusion(&placement, options).unwrap();

    assert_eq!(plan.coordinate_mapping, "placement_x_y_to_voxel_x_z");
    assert_eq!(plan.walkable_cell_count, 540);
    assert_eq!(plan.opening_cell_count, 324);
    assert_eq!(plan.boundary_cell_count, 772);
    assert_eq!(plan.solid_voxel_count, 3_396);
    assert_eq!(plan.resident_chunk_count, 1_053);
    assert_eq!(
        plan.build_bounds.min,
        VoxelCoordinate { x: 31, y: 0, z: 31 }
    );
    assert_eq!(
        plan.build_bounds.max_exclusive,
        VoxelCoordinate {
            x: 212,
            y: 5,
            z: 90,
        }
    );
    assert_eq!(
        plan.solid_voxels
            .iter()
            .filter(|voxel| voxel.material == options.wall_material)
            .count(),
        2_316
    );
    assert_eq!(
        plan.solid_voxels
            .iter()
            .filter(|voxel| voxel.material == options.floor_material)
            .count(),
        540
    );
    assert_eq!(
        plan.solid_voxels
            .iter()
            .filter(|voxel| voxel.material == options.ceiling_material)
            .count(),
        540
    );
    assert_eq!(plan.door_portals.len(), placement.gate_portals.len());
    for (projected, authored) in plan.door_portals.iter().zip(&placement.gate_portals) {
        assert_eq!(projected.id, authored.id);
        assert_eq!(projected.source_edge, authored.source_edge);
        assert_eq!(projected.required_item, authored.required_item);
        assert_eq!(projected.traversal, authored.traversal);
        assert_eq!(projected.orientation, authored.orientation);
        assert_eq!(projected.cells.len(), authored.width as usize);
        assert_eq!((projected.min_y, projected.max_exclusive_y), (1, 4));
    }
}

#[test]
fn engine_authority_is_deterministic_coherent_and_reopenable() {
    let placement = representative_placement();
    let options = ExtrusionOptions::default();
    let mut first = SpatialExtrusionHost::empty(options).unwrap();
    let (plan, receipt) = first
        .admit_placement(VoxelSourceRevision::INITIAL, &placement)
        .unwrap();
    let first_readout = first.readout();
    assert_eq!(receipt.revision_before, 0);
    assert_eq!(receipt.accepted_revision, 1);
    assert_eq!(receipt.changed_voxels, 3_396);
    assert_eq!(receipt.transaction_count, 1);
    assert_eq!(
        receipt.max_edits_per_transaction,
        engine_spatial::MAX_VOXEL_EDITS_PER_TRANSACTION
    );
    assert_eq!(first.placement_id(), Some(placement.placement_id.as_str()));
    assert!(first_readout.projection_revisions_coherent);
    assert_eq!(first_readout.solid_voxel_count, 3_396);
    assert_eq!(first_readout.resident_chunk_count, 1_053);
    assert_eq!(first_readout.collider_chunk_count, 1_053);
    assert_eq!(first_readout.mesh_chunk_count, 1_053);
    assert!(first_readout.mesh_vertex_count > 0);
    assert!(first_readout.mesh_quad_count > 0);
    assert!(first_readout.navigation_cell_count > 0);

    let mut repeated = SpatialExtrusionHost::empty(options).unwrap();
    repeated
        .admit_placement(VoxelSourceRevision::INITIAL, &placement)
        .unwrap();
    assert_eq!(repeated.readout(), first_readout);

    let reopened = SpatialExtrusionHost::reopen(options, &plan, first.source_revision()).unwrap();
    assert_eq!(
        reopened.placement_id(),
        Some(placement.placement_id.as_str())
    );
    assert_eq!(reopened.source_revision(), first.source_revision());
    assert_eq!(reopened.readout(), first_readout);

    let mut continuation = plan.clone();
    continuation.solid_voxels[0].material =
        if continuation.solid_voxels[0].material == options.wall_material {
            options.floor_material
        } else {
            options.wall_material
        };
    let mut direct = SpatialExtrusionHost::reopen(options, &plan, first.source_revision()).unwrap();
    let mut after_reopen =
        SpatialExtrusionHost::reopen(options, &plan, first.source_revision()).unwrap();
    let direct_receipt = direct
        .admit_plan(direct.source_revision(), &continuation)
        .unwrap();
    let reopened_receipt = after_reopen
        .admit_plan(after_reopen.source_revision(), &continuation)
        .unwrap();
    assert_eq!(direct_receipt, reopened_receipt);
    assert_eq!(direct.readout(), after_reopen.readout());
}

#[test]
fn malformed_unknown_oversized_stale_and_late_failures_do_not_mutate() {
    let placement = representative_placement();
    let options = ExtrusionOptions::default();
    let mut host = SpatialExtrusionHost::empty(options).unwrap();
    let (plan, _) = host
        .admit_placement(VoxelSourceRevision::INITIAL, &placement)
        .unwrap();
    let before = host.readout();

    let mut malformed = placement.clone();
    malformed.grid_connectivity = rusty_procgen_preflight::GridConnectivity::EightWay;
    assert!(matches!(
        host.admit_placement(host.source_revision(), &malformed),
        Err(SpatialExtrusionError::MalformedPlacement {
            code: "unsupported_connectivity",
            ..
        })
    ));
    assert_eq!(host.readout(), before);

    let mut unknown = plan.clone();
    unknown.solid_voxels[0].material = 4_095;
    assert!(matches!(
        host.admit_plan(host.source_revision(), &unknown),
        Err(SpatialExtrusionError::UnknownMaterial { material: 4_095 })
    ));
    assert_eq!(host.readout(), before);

    let mut oversized = plan.clone();
    oversized.solid_voxel_count = engine_spatial::MAX_SOLID_VOXELS + 1;
    assert!(matches!(
        host.admit_plan(host.source_revision(), &oversized),
        Err(SpatialExtrusionError::TooManySolidVoxels {
            limit: engine_spatial::MAX_SOLID_VOXELS,
            actual
        }) if actual == engine_spatial::MAX_SOLID_VOXELS + 1
    ));
    assert_eq!(host.readout(), before);

    assert!(matches!(
        host.admit_plan(VoxelSourceRevision::INITIAL, &plan),
        Err(SpatialExtrusionError::StaleRevision { .. })
    ));
    assert_eq!(host.readout(), before);

    let mut out_of_bounds = plan.clone();
    out_of_bounds.solid_voxels[0].coord.x = engine_spatial::MAX_VOXEL_COORDINATE_ABS + 1;
    sync_plan_projection(&mut out_of_bounds, options.chunk_size);
    assert!(matches!(
        host.admit_plan(host.source_revision(), &out_of_bounds),
        Err(SpatialExtrusionError::EngineEdit(
            VoxelEditApplyError::Rejected(VoxelEditRejection::CoordinateOutOfBounds { .. })
        ))
    ));
    assert_eq!(host.readout(), before);
}

#[test]
fn large_admission_uses_bounded_transactions_but_publishes_once() {
    let options = ExtrusionOptions::default();
    let voxel_count = engine_spatial::MAX_VOXEL_EDITS_PER_TRANSACTION + 1;
    let plan = VoxelExtrusionPlan {
        schema_version: 1,
        placement_id: "test.multi-transaction".to_owned(),
        coordinate_mapping: "placement_x_y_to_voxel_x_z".to_owned(),
        solid_voxels: (0..voxel_count)
            .map(|x| PlanVoxel {
                coord: VoxelCoordinate {
                    x: x as i64,
                    y: 0,
                    z: 0,
                },
                material: options.floor_material,
            })
            .collect(),
        walkable_cell_count: voxel_count,
        opening_cell_count: 0,
        boundary_cell_count: 0,
        solid_voxel_count: voxel_count,
        resident_chunk_count: voxel_count.div_ceil(options.chunk_size as usize),
        door_portals: Vec::new(),
        build_bounds: ExtrusionBounds {
            min: VoxelCoordinate { x: 0, y: 0, z: 0 },
            max_exclusive: VoxelCoordinate {
                x: voxel_count as i64,
                y: 1,
                z: 1,
            },
        },
    };
    let mut host = SpatialExtrusionHost::empty(options).unwrap();
    let receipt = host
        .admit_plan(VoxelSourceRevision::INITIAL, &plan)
        .unwrap();
    assert_eq!(receipt.transaction_count, 2);
    assert_eq!(receipt.accepted_revision, 2);
    assert_eq!(receipt.changed_voxels, voxel_count);
    assert_eq!(host.readout().solid_voxel_count, voxel_count);
    assert!(host.readout().projection_revisions_coherent);
}

#[test]
fn every_checked_accepted_placement_compiles_deterministically() {
    let selection: serde_json::Value = serde_json::from_slice(
        &fs::read(repo_root().join("artifacts/samples/batch-v2/selection-report.json")).unwrap(),
    )
    .unwrap();
    let accepted = selection["accepted"].as_array().unwrap();
    assert_eq!(accepted.len(), 9);
    for entry in accepted {
        let relative = entry["piecePlacementRef"].as_str().unwrap();
        let placement = read_placement(&repo_root().join(relative));
        let first = compile_placement_extrusion(&placement, ExtrusionOptions::default()).unwrap();
        let repeated =
            compile_placement_extrusion(&placement, ExtrusionOptions::default()).unwrap();
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&repeated).unwrap(),
            "{relative}"
        );
    }
}

fn representative_placement() -> PiecePlacement {
    read_placement(
        &repo_root().join("artifacts/samples/batch-v2/candidate-006/piece-placement.json"),
    )
}

fn read_placement(path: &Path) -> PiecePlacement {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn sync_plan_projection(plan: &mut VoxelExtrusionPlan, chunk_size: u32) {
    let mut min = plan.solid_voxels[0].coord;
    let mut max = min;
    let mut chunks = BTreeSet::new();
    for voxel in &plan.solid_voxels {
        min.x = min.x.min(voxel.coord.x);
        min.y = min.y.min(voxel.coord.y);
        min.z = min.z.min(voxel.coord.z);
        max.x = max.x.max(voxel.coord.x);
        max.y = max.y.max(voxel.coord.y);
        max.z = max.z.max(voxel.coord.z);
        let chunk_size = i64::from(chunk_size);
        chunks.insert([
            voxel.coord.x.div_euclid(chunk_size),
            voxel.coord.y.div_euclid(chunk_size),
            voxel.coord.z.div_euclid(chunk_size),
        ]);
    }
    plan.build_bounds = ExtrusionBounds {
        min,
        max_exclusive: VoxelCoordinate {
            x: max.x + 1,
            y: max.y + 1,
            z: max.z + 1,
        },
    };
    plan.resident_chunk_count = chunks.len();
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
