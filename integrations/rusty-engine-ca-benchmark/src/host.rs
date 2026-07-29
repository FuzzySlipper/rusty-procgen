use std::collections::{BTreeMap, BTreeSet};

use engine_spatial::{
    validate_voxel_material_slot, MaterialVoxel, PreparedVoxelEdit, VoxelCollisionScene, VoxelEdit,
    VoxelEditService, VoxelEditTransaction, VoxelMeshChunk, VoxelSourceRevision, MAX_CHUNK_SIZE,
    MAX_VOXEL_EDITS_PER_TRANSACTION,
};
use rusty_procgen_preflight::cellular_automata::{CaAutomaton, CaCellDelta, CaCoord, CaScenario};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::model::{
    elapsed, BenchmarkClock, CaAdmissionTiming, CaAuthorityReadout, CaInitialAuthority,
    CaMeshChunkFact, CaMeshChunkSummary, CaMeshGroupFact, CaProjectionOp, CaSpatialError,
    CaSpatialOptions, CaSpatialStep, CaStepTiming,
};

const VOXEL_SIZE: f64 = 1.0;
const MAX_VALUES_PER_WORST_CASE_VOXEL: usize = 198;

/// One fully prepared CA and Engine transition. Dropping this value publishes
/// neither the procedural state nor Engine authority.
#[derive(Debug)]
pub struct PreparedCaSpatialStep {
    expected_ca_step: u32,
    expected_ca_hash: String,
    candidate_automaton: CaAutomaton,
    ca_evidence: rusty_procgen_preflight::cellular_automata::CaStepEvidence,
    engine_prepared: Option<PreparedVoxelEdit>,
    affected_chunks: Vec<[i64; 3]>,
    canonical_edit_count: usize,
    engine_delta_count: usize,
    timing: CaStepTiming,
}

/// Downstream owner that keeps one procedural workload aligned with one real
/// Rusty Engine spatial authority.
#[derive(Debug)]
pub struct CaSpatialHost {
    automaton: CaAutomaton,
    scene: VoxelCollisionScene,
    options: CaSpatialOptions,
    projection_chunks: BTreeMap<[i64; 3], CaMeshChunkFact>,
    initial: CaInitialAuthority,
    current_projection_state_hash: String,
    previous_trace_hash: String,
}

impl CaSpatialHost {
    pub fn admit(
        scenario: CaScenario,
        options: CaSpatialOptions,
        clock: &mut impl BenchmarkClock,
    ) -> Result<(Self, CaAdmissionTiming), CaSpatialError> {
        validate_options(options)?;
        let automaton = CaAutomaton::new(scenario)?;

        let materialization_start = clock.now_ns();
        let materials = materialize_initial_state(&automaton, options)?;
        let materialization_end = clock.now_ns();

        let build_start = clock.now_ns();
        let scene =
            VoxelCollisionScene::from_material_voxels(VOXEL_SIZE, options.chunk_size, materials)
                .map_err(CaSpatialError::EngineBuild)?;
        let build_end = clock.now_ns();

        let readback_start = clock.now_ns();
        let projection_chunks = capture_projection_chunks(&scene)?;
        enforce_actual_mesh_budget(&projection_chunks, options.max_mesh_values_per_step)?;
        let projection_state_hash = hash_value(&projection_summaries(&projection_chunks))?;
        let readout = capture_readout(&scene, &projection_chunks, &projection_state_hash);
        let initial_ca_state_hash = automaton.initial_state_hash().to_owned();
        let initial_ca_cumulative_hash = automaton.current_scenario_hash().to_owned();
        let trace_hash = hash_value(&(
            "initial",
            automaton.scenario().id.as_str(),
            initial_ca_state_hash.as_str(),
            initial_ca_cumulative_hash.as_str(),
            &readout,
            &projection_state_hash,
        ))?;
        let initial = CaInitialAuthority {
            initial_ca_state_hash,
            initial_ca_cumulative_hash,
            readout,
            projection_chunks: projection_chunks.values().cloned().collect(),
            projection_state_hash: projection_state_hash.clone(),
            trace_hash: trace_hash.clone(),
        };
        let readback_end = clock.now_ns();

        let encoding_start = clock.now_ns();
        serde_json::to_vec(&initial).map_err(serialization_error)?;
        let encoding_end = clock.now_ns();

        Ok((
            Self {
                automaton,
                scene,
                options,
                projection_chunks,
                initial,
                current_projection_state_hash: projection_state_hash,
                previous_trace_hash: trace_hash,
            },
            CaAdmissionTiming {
                state_materialization_ns: elapsed(materialization_start, materialization_end),
                engine_build_ns: elapsed(build_start, build_end),
                evidence_readback_ns: elapsed(readback_start, readback_end),
                artifact_encoding_ns: elapsed(encoding_start, encoding_end),
            },
        ))
    }

    pub fn prepare_next_step(
        &self,
        expected_revision: VoxelSourceRevision,
        clock: &mut impl BenchmarkClock,
    ) -> Result<PreparedCaSpatialStep, CaSpatialError> {
        if expected_revision != self.scene.source_revision() {
            return Err(CaSpatialError::RevisionMismatch {
                expected: expected_revision,
                actual: self.scene.source_revision(),
            });
        }

        let expected_ca_step = self.automaton.completed_steps();
        let expected_ca_hash = self.automaton.current_state_hash()?;
        let mut candidate_automaton = self.automaton.clone();

        let ca_start = clock.now_ns();
        let ca_evidence = candidate_automaton.step()?;
        let ca_end = clock.now_ns();

        let request_start = clock.now_ns();
        let edits = ca_evidence
            .deltas
            .iter()
            .map(|delta| delta_to_edit(delta, self.options))
            .collect::<Vec<_>>();
        if edits.len() > self.options.max_edits_per_step {
            return Err(CaSpatialError::StepEditQuotaExceeded {
                actual: edits.len(),
                limit: self.options.max_edits_per_step,
            });
        }
        let affected_chunks = prospective_mesh_chunks(&edits, self.options.chunk_size);
        enforce_prospective_mesh_budget(
            &affected_chunks,
            self.options.chunk_size,
            self.options.max_mesh_values_per_step,
        )?;
        let request_end = clock.now_ns();

        let preview_start = clock.now_ns();
        let engine_prepared = if edits.is_empty() {
            None
        } else {
            Some(
                VoxelEditService::preview(
                    &self.scene,
                    VoxelEditTransaction {
                        expected_revision,
                        edits: &edits,
                    },
                )
                .map_err(CaSpatialError::EngineEdit)?,
            )
        };
        let preview_end = clock.now_ns();
        let engine_delta_count = engine_prepared
            .as_ref()
            .map_or(0, |prepared| prepared.deltas().len());
        let canonical_edit_count = engine_prepared
            .as_ref()
            .map_or(0, |prepared| prepared.canonical_edits().len());

        Ok(PreparedCaSpatialStep {
            expected_ca_step,
            expected_ca_hash,
            candidate_automaton,
            ca_evidence,
            engine_prepared,
            affected_chunks: affected_chunks.into_iter().collect(),
            canonical_edit_count,
            engine_delta_count,
            timing: CaStepTiming {
                ca_step_ns: elapsed(ca_start, ca_end),
                request_construction_ns: elapsed(request_start, request_end),
                spatial_preview_ns: elapsed(preview_start, preview_end),
                ..CaStepTiming::default()
            },
        })
    }

    pub fn commit_prepared(
        &mut self,
        prepared: PreparedCaSpatialStep,
        clock: &mut impl BenchmarkClock,
    ) -> Result<(CaSpatialStep, CaStepTiming), CaSpatialError> {
        let actual_ca_step = self.automaton.completed_steps();
        let actual_ca_hash = self.automaton.current_state_hash()?;
        if prepared.expected_ca_step != actual_ca_step
            || prepared.expected_ca_hash != actual_ca_hash
        {
            return Err(CaSpatialError::PreparedCaStateChanged {
                expected_step: prepared.expected_ca_step,
                actual_step: actual_ca_step,
                expected_hash: prepared.expected_ca_hash,
                actual_hash: actual_ca_hash,
            });
        }

        let PreparedCaSpatialStep {
            candidate_automaton,
            ca_evidence,
            engine_prepared,
            affected_chunks,
            canonical_edit_count,
            engine_delta_count,
            mut timing,
            ..
        } = prepared;
        let revision_before = self.scene.source_revision();

        let commit_start = clock.now_ns();
        let accepted_revision = if let Some(engine_prepared) = engine_prepared {
            VoxelEditService::commit(&mut self.scene, engine_prepared)
                .map_err(CaSpatialError::EngineEdit)?
                .accepted_revision
        } else {
            revision_before
        };
        self.automaton = candidate_automaton;
        let commit_end = clock.now_ns();
        timing.authority_commit_ns = elapsed(commit_start, commit_end);

        let readback_start = clock.now_ns();
        let projection_ops =
            capture_projection_delta(&self.scene, &mut self.projection_chunks, &affected_chunks)?;
        let projection_delta_hash = hash_value(&projection_ops)?;
        let projection_state_hash = hash_value(&projection_summaries(&self.projection_chunks))?;
        let readout = capture_readout(&self.scene, &self.projection_chunks, &projection_state_hash);
        let previous_trace_hash = self.previous_trace_hash.clone();
        let trace_hash = hash_value(&(
            "step",
            &previous_trace_hash,
            &ca_evidence,
            revision_before.raw(),
            accepted_revision.raw(),
            canonical_edit_count,
            engine_delta_count,
            &readout,
            &projection_delta_hash,
            &projection_state_hash,
        ))?;
        let step = CaSpatialStep {
            engine_changed_voxels: engine_delta_count,
            ca: ca_evidence,
            revision_before: revision_before.raw(),
            accepted_revision: accepted_revision.raw(),
            canonical_edit_count,
            engine_delta_count,
            readout,
            projection_ops,
            projection_delta_hash,
            projection_state_hash,
            previous_trace_hash,
            trace_hash: trace_hash.clone(),
        };
        let readback_end = clock.now_ns();
        timing.evidence_readback_ns = elapsed(readback_start, readback_end);

        let encoding_start = clock.now_ns();
        serde_json::to_vec(&step).map_err(serialization_error)?;
        let encoding_end = clock.now_ns();
        timing.artifact_encoding_ns = elapsed(encoding_start, encoding_end);

        self.current_projection_state_hash = step.projection_state_hash.clone();
        self.previous_trace_hash = trace_hash;
        Ok((step, timing))
    }

    pub const fn source_revision(&self) -> VoxelSourceRevision {
        self.scene.source_revision()
    }

    pub fn authority_hash(&self) -> u64 {
        self.scene.authority_hash()
    }

    pub fn solid_voxel_count(&self) -> usize {
        self.scene.solid_voxel_count()
    }

    pub fn completed_steps(&self) -> u32 {
        self.automaton.completed_steps()
    }

    pub fn current_state_hash(&self) -> Result<String, CaSpatialError> {
        Ok(self.automaton.current_state_hash()?)
    }

    pub fn readout(&self) -> CaAuthorityReadout {
        capture_readout(
            &self.scene,
            &self.projection_chunks,
            &self.current_projection_state_hash,
        )
    }

    pub fn initial(&self) -> &CaInitialAuthority {
        &self.initial
    }
}

fn validate_options(options: CaSpatialOptions) -> Result<(), CaSpatialError> {
    if !(1..=MAX_CHUNK_SIZE).contains(&options.chunk_size) {
        return Err(CaSpatialError::InvalidOptions {
            detail: format!(
                "chunk size {} is outside 1..={MAX_CHUNK_SIZE}",
                options.chunk_size
            ),
        });
    }
    if options.max_edits_per_step == 0
        || options.max_edits_per_step > MAX_VOXEL_EDITS_PER_TRANSACTION
    {
        return Err(CaSpatialError::InvalidOptions {
            detail: format!(
                "step edit limit {} is outside 1..={MAX_VOXEL_EDITS_PER_TRANSACTION}",
                options.max_edits_per_step
            ),
        });
    }
    if options.max_mesh_values_per_step == 0 {
        return Err(CaSpatialError::InvalidOptions {
            detail: "mesh evidence limit must be positive".to_owned(),
        });
    }
    let palette = [
        options.palette.source,
        options.palette.frontier,
        options.palette.trail,
        options.palette.resident_empty,
    ];
    if palette.into_iter().collect::<BTreeSet<_>>().len() != palette.len() {
        return Err(CaSpatialError::InvalidOptions {
            detail: "CA material slots must be distinct".to_owned(),
        });
    }
    for material_slot in palette {
        validate_voxel_material_slot(material_slot).map_err(|error| {
            CaSpatialError::InvalidOptions {
                detail: error.to_string(),
            }
        })?;
    }
    Ok(())
}

fn materialize_initial_state(
    automaton: &CaAutomaton,
    options: CaSpatialOptions,
) -> Result<Vec<MaterialVoxel>, CaSpatialError> {
    let mut materials = Vec::new();
    if options.materialize_empty {
        let bounds = automaton.scenario().bounds;
        for x in bounds.min.x..bounds.max_exclusive.x {
            for y in bounds.min.y..bounds.max_exclusive.y {
                for z in bounds.min.z..bounds.max_exclusive.z {
                    let coord = CaCoord { x, y, z };
                    let state = automaton.state_at(coord)?;
                    materials.push(MaterialVoxel {
                        address: coord_to_address(coord),
                        material_slot: options
                            .palette
                            .material(state, true)
                            .expect("materialized empty state has a material"),
                    });
                }
            }
        }
    } else {
        materials.extend(automaton.active_cells().into_iter().map(|cell| {
            MaterialVoxel {
                address: coord_to_address(cell.coord),
                material_slot: options
                    .palette
                    .material(cell.state, false)
                    .expect("active CA state has a material"),
            }
        }));
    }
    Ok(materials)
}

fn delta_to_edit(delta: &CaCellDelta, options: CaSpatialOptions) -> VoxelEdit {
    let address = coord_to_address(delta.coord);
    match options
        .palette
        .material(delta.current, options.materialize_empty)
    {
        Some(material_slot) => VoxelEdit::Set {
            address,
            material_slot,
        },
        None => VoxelEdit::Clear { address },
    }
}

const fn coord_to_address(coord: CaCoord) -> [i64; 3] {
    [coord.x as i64, coord.y as i64, coord.z as i64]
}

fn capture_projection_chunks(
    scene: &VoxelCollisionScene,
) -> Result<BTreeMap<[i64; 3], CaMeshChunkFact>, CaSpatialError> {
    if !scene
        .mesh_chunks()
        .windows(2)
        .all(|chunks| chunks[0].chunk < chunks[1].chunk)
    {
        return Err(CaSpatialError::Serialization {
            detail: "Engine mesh chunks are not in canonical coordinate order".to_owned(),
        });
    }
    scene
        .mesh_chunks()
        .iter()
        .map(|chunk| Ok((chunk.chunk, mesh_chunk_fact(chunk)?)))
        .collect()
}

fn mesh_chunk_fact(chunk: &VoxelMeshChunk) -> Result<CaMeshChunkFact, CaSpatialError> {
    let groups = chunk
        .groups
        .iter()
        .map(|group| CaMeshGroupFact {
            material_slot: group.material_slot,
            start: group.start,
            count: group.count,
        })
        .collect::<Vec<_>>();
    let buffer_hash = hash_value(&(
        &chunk.translation,
        &chunk.positions,
        &chunk.normals,
        &chunk.indices,
        &groups,
        &chunk.bounds_min,
        &chunk.bounds_max,
    ))?;
    Ok(CaMeshChunkFact {
        chunk: chunk.chunk,
        content_hash: format!("fnv1a64:{:016x}", chunk.content_hash),
        buffer_hash,
        translation: chunk.translation,
        positions: chunk.positions.clone(),
        normals: chunk.normals.clone(),
        indices: chunk.indices.clone(),
        groups,
        bounds_min: chunk.bounds_min,
        bounds_max: chunk.bounds_max,
        vertices: chunk.vertices,
        quads: chunk.quads,
        faces_culled: chunk.faces_culled,
    })
}

fn projection_summaries(chunks: &BTreeMap<[i64; 3], CaMeshChunkFact>) -> Vec<CaMeshChunkSummary> {
    chunks
        .values()
        .map(|chunk| CaMeshChunkSummary {
            chunk: chunk.chunk,
            content_hash: chunk.content_hash.clone(),
            buffer_hash: chunk.buffer_hash.clone(),
            vertices: chunk.vertices,
            quads: chunk.quads,
            faces_culled: chunk.faces_culled,
        })
        .collect()
}

fn capture_projection_delta(
    scene: &VoxelCollisionScene,
    chunks: &mut BTreeMap<[i64; 3], CaMeshChunkFact>,
    affected_chunks: &[[i64; 3]],
) -> Result<Vec<CaProjectionOp>, CaSpatialError> {
    let mut ops = Vec::new();
    for coord in affected_chunks {
        let after = find_mesh_chunk(scene, *coord)
            .map(mesh_chunk_fact)
            .transpose()?;
        match (chunks.get(coord), after) {
            (Some(_), None) => {
                chunks.remove(coord);
                ops.push(CaProjectionOp::Delete { chunk: *coord });
            }
            (Some(before), Some(after)) if before != &after => {
                chunks.insert(*coord, after.clone());
                ops.push(CaProjectionOp::Upsert { chunk: after });
            }
            (None, Some(after)) => {
                chunks.insert(*coord, after.clone());
                ops.push(CaProjectionOp::Upsert { chunk: after });
            }
            _ => {}
        }
    }
    Ok(ops)
}

fn find_mesh_chunk(scene: &VoxelCollisionScene, coord: [i64; 3]) -> Option<&VoxelMeshChunk> {
    scene
        .mesh_chunks()
        .binary_search_by_key(&coord, |chunk| chunk.chunk)
        .ok()
        .map(|index| &scene.mesh_chunks()[index])
}

fn capture_readout(
    scene: &VoxelCollisionScene,
    chunks: &BTreeMap<[i64; 3], CaMeshChunkFact>,
    projection_state_hash: &str,
) -> CaAuthorityReadout {
    let (mesh_vertex_count, mesh_quad_count) =
        chunks.values().fold((0_u64, 0_u64), |counts, chunk| {
            (
                counts.0 + u64::from(chunk.vertices),
                counts.1 + u64::from(chunk.quads),
            )
        });
    CaAuthorityReadout {
        source_revision: scene.source_revision().raw(),
        authority_hash: format!("fnv1a64:{:016x}", scene.authority_hash()),
        projection_revisions_coherent: scene
            .projection_revisions()
            .is_coherent_with(scene.source_revision()),
        solid_voxel_count: scene.solid_voxel_count(),
        resident_chunk_count: scene.resident_chunk_count(),
        collider_chunk_count: scene.collider_chunk_count(),
        navigation_cell_count: scene.navigation_cell_count(),
        navigation_hash: format!("fnv1a64:{:016x}", scene.navigation_hash()),
        mesh_chunk_count: chunks.len(),
        mesh_vertex_count,
        mesh_quad_count,
        mesh_projection_hash: projection_state_hash.to_owned(),
    }
}

fn enforce_actual_mesh_budget(
    chunks: &BTreeMap<[i64; 3], CaMeshChunkFact>,
    limit: usize,
) -> Result<(), CaSpatialError> {
    let actual = chunks.values().try_fold(0_usize, |total, chunk| {
        total.checked_add(mesh_value_count(chunk)).ok_or(
            CaSpatialError::MeshEvidenceQuotaExceeded {
                actual: usize::MAX,
                limit,
            },
        )
    })?;
    if actual > limit {
        return Err(CaSpatialError::MeshEvidenceQuotaExceeded { actual, limit });
    }
    Ok(())
}

fn prospective_mesh_chunks(edits: &[VoxelEdit], chunk_size: u32) -> BTreeSet<[i64; 3]> {
    let chunk_size = i64::from(chunk_size);
    let mut affected_chunks = BTreeSet::new();
    for edit in edits {
        let address = edit.address();
        let own = address.map(|axis| axis.div_euclid(chunk_size));
        affected_chunks.insert(own);
        for axis in 0..3 {
            let mut negative = address;
            negative[axis] -= 1;
            affected_chunks.insert(negative.map(|value| value.div_euclid(chunk_size)));
            let mut positive = address;
            positive[axis] += 1;
            affected_chunks.insert(positive.map(|value| value.div_euclid(chunk_size)));
        }
    }
    affected_chunks
}

fn enforce_prospective_mesh_budget(
    affected_chunks: &BTreeSet<[i64; 3]>,
    chunk_size: u32,
    limit: usize,
) -> Result<(), CaSpatialError> {
    let chunk_size = i64::from(chunk_size);
    let chunk_volume = usize::try_from(chunk_size)
        .ok()
        .and_then(|size| size.checked_pow(3))
        .ok_or(CaSpatialError::MeshEvidenceQuotaExceeded {
            actual: usize::MAX,
            limit,
        })?;
    let actual = affected_chunks
        .len()
        .checked_mul(chunk_volume)
        .and_then(|value| value.checked_mul(MAX_VALUES_PER_WORST_CASE_VOXEL))
        .ok_or(CaSpatialError::MeshEvidenceQuotaExceeded {
            actual: usize::MAX,
            limit,
        })?;
    if actual > limit {
        return Err(CaSpatialError::MeshEvidenceQuotaExceeded { actual, limit });
    }
    Ok(())
}

fn mesh_value_count(chunk: &CaMeshChunkFact) -> usize {
    chunk.positions.len() + chunk.normals.len() + chunk.indices.len() + chunk.groups.len() * 3
}

pub(crate) fn hash_value(value: &impl Serialize) -> Result<String, CaSpatialError> {
    let encoded = serde_json::to_vec(value).map_err(serialization_error)?;
    let digest = Sha256::digest(encoded);
    Ok(format!("sha256:{digest:x}"))
}

pub(crate) fn serialization_error(error: serde_json::Error) -> CaSpatialError {
    CaSpatialError::Serialization {
        detail: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prospective_budget_counts_neighbor_chunks_before_authority_work() {
        let edits = [VoxelEdit::Set {
            address: [7, 7, 7],
            material_slot: 1,
        }];
        assert!(matches!(
            enforce_prospective_mesh_budget(&prospective_mesh_chunks(&edits, 8), 8, 100),
            Err(CaSpatialError::MeshEvidenceQuotaExceeded { .. })
        ));
        enforce_prospective_mesh_budget(&prospective_mesh_chunks(&edits, 8), 8, 2_000_000).unwrap();
    }

    #[test]
    fn material_palette_preserves_state_identity() {
        use rusty_procgen_preflight::cellular_automata::CaCellState;

        let palette = crate::CaMaterialPalette::default();
        assert_eq!(palette.material(CaCellState::Empty, false), None);
        assert_eq!(palette.material(CaCellState::Empty, true), Some(4));
        assert_eq!(palette.material(CaCellState::Source, false), Some(1));
    }
}
