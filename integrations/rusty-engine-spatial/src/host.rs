use std::collections::BTreeMap;

use engine_spatial::{
    MaterialVoxel, VoxelCollisionScene, VoxelEdit, VoxelEditApplyError, VoxelEditRejection,
    VoxelEditService, VoxelEditTransaction, VoxelSourceRevision, MAX_VOXEL_EDITS_PER_TRANSACTION,
};
use rusty_procgen_preflight::PiecePlacement;
use serde::{Deserialize, Serialize};

use crate::{
    compile::validate_options, compile_placement_extrusion, ExtrusionOptions, MaterialCount,
    SpatialExtrusionError, SpatialReadout, VoxelExtrusionPlan,
};

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SpatialExtrusionReceipt {
    pub placement_id: String,
    pub revision_before: u64,
    pub accepted_revision: u64,
    pub changed_voxels: usize,
    pub transaction_count: usize,
    pub max_edits_per_transaction: usize,
    pub readout: SpatialReadout,
}

/// One downstream owner composing Procgen policy with Engine spatial services.
///
/// The host has no scheduler, callbacks, renderer, persistence policy, or
/// universal command routing. Concrete accepted voxels stay inside Engine's
/// [`VoxelCollisionScene`].
pub struct SpatialExtrusionHost {
    options: ExtrusionOptions,
    scene: VoxelCollisionScene,
    placement_id: Option<String>,
}

impl SpatialExtrusionHost {
    pub fn empty(options: ExtrusionOptions) -> Result<Self, SpatialExtrusionError> {
        validate_options(options)?;
        let scene = VoxelCollisionScene::from_material_voxels(1.0, options.chunk_size, [])
            .map_err(SpatialExtrusionError::EngineBuild)?;
        Ok(Self {
            options,
            scene,
            placement_id: None,
        })
    }

    pub fn reopen(
        options: ExtrusionOptions,
        plan: &VoxelExtrusionPlan,
        source_revision: VoxelSourceRevision,
    ) -> Result<Self, SpatialExtrusionError> {
        validate_options(options)?;
        validate_plan(options, plan)?;
        let scene = VoxelCollisionScene::from_material_voxels_at_revision(
            1.0,
            options.chunk_size,
            plan.solid_voxels.iter().map(|voxel| MaterialVoxel {
                address: voxel.coord.address(),
                material_slot: voxel.material,
            }),
            source_revision,
        )
        .map_err(SpatialExtrusionError::EngineBuild)?;
        Ok(Self {
            options,
            scene,
            placement_id: Some(plan.placement_id.clone()),
        })
    }

    pub fn source_revision(&self) -> VoxelSourceRevision {
        self.scene.source_revision()
    }

    pub fn placement_id(&self) -> Option<&str> {
        self.placement_id.as_deref()
    }

    pub fn admit_placement(
        &mut self,
        expected_revision: VoxelSourceRevision,
        placement: &PiecePlacement,
    ) -> Result<(VoxelExtrusionPlan, SpatialExtrusionReceipt), SpatialExtrusionError> {
        if expected_revision != self.scene.source_revision() {
            return Err(SpatialExtrusionError::StaleRevision {
                expected: expected_revision,
                actual: self.scene.source_revision(),
            });
        }
        let plan = compile_placement_extrusion(placement, self.options)?;
        let receipt = self.admit_plan(expected_revision, &plan)?;
        Ok((plan, receipt))
    }

    pub fn admit_plan(
        &mut self,
        expected_revision: VoxelSourceRevision,
        plan: &VoxelExtrusionPlan,
    ) -> Result<SpatialExtrusionReceipt, SpatialExtrusionError> {
        if expected_revision != self.scene.source_revision() {
            return Err(SpatialExtrusionError::StaleRevision {
                expected: expected_revision,
                actual: self.scene.source_revision(),
            });
        }
        validate_plan(self.options, plan)?;

        let current = self
            .scene
            .material_voxels()
            .iter()
            .map(|voxel| (voxel.address, voxel.material_slot))
            .collect::<BTreeMap<_, _>>();
        let desired = plan
            .solid_voxels
            .iter()
            .map(|voxel| (voxel.coord.address(), voxel.material))
            .collect::<BTreeMap<_, _>>();
        if desired.len() != plan.solid_voxels.len() {
            return Err(SpatialExtrusionError::malformed(
                "duplicate_voxel_address",
                "extrusion plan repeats a voxel address",
            ));
        }

        let mut edits = current
            .keys()
            .filter(|address| !desired.contains_key(*address))
            .copied()
            .map(|address| VoxelEdit::Clear { address })
            .collect::<Vec<_>>();
        edits.extend(desired.iter().filter_map(|(&address, &material_slot)| {
            if current.get(&address) == Some(&material_slot) {
                None
            } else {
                Some(VoxelEdit::Set {
                    address,
                    material_slot,
                })
            }
        }));
        let revision_before = self.scene.source_revision();
        if edits.is_empty() {
            return Ok(SpatialExtrusionReceipt {
                placement_id: plan.placement_id.clone(),
                revision_before: revision_before.raw(),
                accepted_revision: revision_before.raw(),
                changed_voxels: 0,
                transaction_count: 0,
                max_edits_per_transaction: MAX_VOXEL_EDITS_PER_TRANSACTION,
                readout: self.readout(),
            });
        }

        // Rebuild the currently observed authority off to the side. Each
        // Engine-owned transaction remains bounded, but the live scene is
        // replaced only after every batch and derived projection succeeds.
        let mut candidate = VoxelCollisionScene::from_material_voxels_at_revision(
            1.0,
            self.options.chunk_size,
            self.scene.material_voxels().iter().copied(),
            revision_before,
        )
        .map_err(SpatialExtrusionError::EngineBuild)?;
        let transaction_count = edits.len().div_ceil(MAX_VOXEL_EDITS_PER_TRANSACTION);
        let mut changed_voxels = 0;
        for batch in edits.chunks(MAX_VOXEL_EDITS_PER_TRANSACTION) {
            let candidate_revision = candidate.source_revision();
            let receipt = VoxelEditService::apply(
                &mut candidate,
                VoxelEditTransaction {
                    expected_revision: candidate_revision,
                    edits: batch,
                },
            )
            .map_err(map_edit_error)?;
            changed_voxels += receipt.fact.changed_voxels;
        }
        self.scene = candidate;
        self.placement_id = Some(plan.placement_id.clone());
        let readout = self.readout();
        Ok(SpatialExtrusionReceipt {
            placement_id: plan.placement_id.clone(),
            revision_before: revision_before.raw(),
            accepted_revision: self.scene.source_revision().raw(),
            changed_voxels,
            transaction_count,
            max_edits_per_transaction: MAX_VOXEL_EDITS_PER_TRANSACTION,
            readout,
        })
    }

    pub fn readout(&self) -> SpatialReadout {
        readout(&self.scene)
    }
}

fn validate_plan(
    options: ExtrusionOptions,
    plan: &VoxelExtrusionPlan,
) -> Result<(), SpatialExtrusionError> {
    if plan.schema_version != 1
        || plan.coordinate_mapping != "placement_x_y_to_voxel_x_z"
        || plan.placement_id.is_empty()
    {
        return Err(SpatialExtrusionError::malformed(
            "invalid_plan_identity",
            "extrusion plan identity or coordinate mapping is invalid",
        ));
    }
    if plan.solid_voxel_count > engine_spatial::MAX_SOLID_VOXELS
        || plan.solid_voxels.len() > engine_spatial::MAX_SOLID_VOXELS
    {
        return Err(SpatialExtrusionError::TooManySolidVoxels {
            limit: engine_spatial::MAX_SOLID_VOXELS,
            actual: plan.solid_voxel_count.max(plan.solid_voxels.len()),
        });
    }
    if plan.solid_voxels.is_empty() || plan.solid_voxel_count != plan.solid_voxels.len() {
        return Err(SpatialExtrusionError::malformed(
            "invalid_solid_count",
            "extrusion plan solid count does not match its canonical voxel list",
        ));
    }
    let allowed = options.allowed_materials();
    let mut min = plan.solid_voxels[0].coord;
    let mut max = min;
    let mut chunks = std::collections::BTreeSet::new();
    for voxel in &plan.solid_voxels {
        if !allowed.contains(&voxel.material) {
            return Err(SpatialExtrusionError::UnknownMaterial {
                material: voxel.material,
            });
        }
        min.x = min.x.min(voxel.coord.x);
        min.y = min.y.min(voxel.coord.y);
        min.z = min.z.min(voxel.coord.z);
        max.x = max.x.max(voxel.coord.x);
        max.y = max.y.max(voxel.coord.y);
        max.z = max.z.max(voxel.coord.z);
        let chunk_size = i64::from(options.chunk_size);
        chunks.insert([
            voxel.coord.x.div_euclid(chunk_size),
            voxel.coord.y.div_euclid(chunk_size),
            voxel.coord.z.div_euclid(chunk_size),
        ]);
    }
    let expected_max = [
        max.x.checked_add(1),
        max.y.checked_add(1),
        max.z.checked_add(1),
    ];
    if plan.build_bounds.min != min
        || expected_max.contains(&None)
        || plan.build_bounds.max_exclusive.x != expected_max[0].unwrap_or_default()
        || plan.build_bounds.max_exclusive.y != expected_max[1].unwrap_or_default()
        || plan.build_bounds.max_exclusive.z != expected_max[2].unwrap_or_default()
        || plan.resident_chunk_count != chunks.len()
    {
        return Err(SpatialExtrusionError::malformed(
            "inconsistent_plan_projection",
            "extrusion bounds or resident chunk count do not match canonical voxels",
        ));
    }
    Ok(())
}

fn map_edit_error(error: VoxelEditApplyError) -> SpatialExtrusionError {
    match error {
        VoxelEditApplyError::Rejected(VoxelEditRejection::StaleRevision { expected, actual }) => {
            SpatialExtrusionError::StaleRevision { expected, actual }
        }
        other => SpatialExtrusionError::EngineEdit(other),
    }
}

fn readout(scene: &VoxelCollisionScene) -> SpatialReadout {
    let revision = scene.source_revision();
    let projections = scene.projection_revisions();
    let mut material_counts = BTreeMap::<u16, usize>::new();
    for voxel in scene.material_voxels() {
        *material_counts.entry(voxel.material_slot).or_default() += 1;
    }

    let mut mesh_hash = Fnv64::new();
    let mut mesh_vertex_count = 0_u64;
    let mut mesh_quad_count = 0_u64;
    for chunk in scene.mesh_chunks() {
        for coordinate in chunk.chunk {
            mesh_hash.feed_i64(coordinate);
        }
        mesh_hash.feed_u64(chunk.content_hash);
        mesh_hash.feed_u64(u64::from(chunk.vertices));
        mesh_hash.feed_u64(u64::from(chunk.quads));
        mesh_hash.feed_u64(u64::from(chunk.faces_culled));
        for group in &chunk.groups {
            mesh_hash.feed_u64(u64::from(group.material_slot));
            mesh_hash.feed_u64(u64::from(group.start));
            mesh_hash.feed_u64(u64::from(group.count));
        }
        mesh_vertex_count += u64::from(chunk.vertices);
        mesh_quad_count += u64::from(chunk.quads);
    }

    SpatialReadout {
        source_revision: revision.raw(),
        authority_hash: format!("fnv1a64:{:016x}", scene.authority_hash()),
        projection_revisions_coherent: projections.is_coherent_with(revision),
        solid_voxel_count: scene.solid_voxel_count(),
        resident_chunk_count: scene.resident_chunk_count(),
        collider_chunk_count: scene.collider_chunk_count(),
        navigation_cell_count: scene.navigation_cell_count(),
        navigation_hash: format!("fnv1a64:{:016x}", scene.navigation_hash()),
        mesh_chunk_count: scene.mesh_chunks().len(),
        mesh_vertex_count,
        mesh_quad_count,
        mesh_projection_hash: format!("fnv1a64:{:016x}", mesh_hash.finish()),
        material_counts: material_counts
            .into_iter()
            .map(|(material, voxels)| MaterialCount { material, voxels })
            .collect(),
    }
}

struct Fnv64(u64);

impl Fnv64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    const fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn feed_u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn feed_i64(&mut self, value: i64) {
        self.feed_u64(value as u64);
    }

    const fn finish(self) -> u64 {
        self.0
    }
}
