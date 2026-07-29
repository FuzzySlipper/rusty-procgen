use std::fmt::{self, Display};
use std::time::Instant;

use engine_spatial::{
    CollisionSceneError, VoxelEditApplyError, VoxelSourceRevision, MAX_VOXEL_EDITS_PER_TRANSACTION,
};
use rusty_procgen_preflight::cellular_automata::{
    CaBoundaryPolicy, CaBounds, CaCellState, CaError, CaNeighborhood, CaStepEvidence,
    CaWorkloadClass,
};
use serde::{Deserialize, Serialize};

pub const CA_SPATIAL_BENCHMARK_KIND: &str = "rusty_procgen.evidence.engine_ca_benchmark.v1";
pub const CA_SPATIAL_TRACE_KIND: &str = "rusty_procgen.engine_ca_authority_trace.v1";
pub const DEFAULT_CA_CHUNK_SIZE: u32 = 8;
pub const DEFAULT_MAX_MESH_VALUES_PER_STEP: usize = 2_000_000;
pub const DEFAULT_MAX_RECORDED_RUNS: u32 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaMaterialPalette {
    pub source: u16,
    pub frontier: u16,
    pub trail: u16,
    pub resident_empty: u16,
}

impl Default for CaMaterialPalette {
    fn default() -> Self {
        Self {
            source: 1,
            frontier: 2,
            trail: 3,
            resident_empty: 4,
        }
    }
}

impl CaMaterialPalette {
    pub const fn material(self, state: CaCellState, materialize_empty: bool) -> Option<u16> {
        match state {
            CaCellState::Empty if materialize_empty => Some(self.resident_empty),
            CaCellState::Empty => None,
            CaCellState::Source => Some(self.source),
            CaCellState::Frontier => Some(self.frontier),
            CaCellState::Trail => Some(self.trail),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaSpatialOptions {
    pub chunk_size: u32,
    pub palette: CaMaterialPalette,
    pub max_edits_per_step: usize,
    pub max_mesh_values_per_step: usize,
    pub materialize_empty: bool,
}

impl CaSpatialOptions {
    pub fn for_workload(workload: CaWorkloadClass) -> Self {
        Self {
            chunk_size: DEFAULT_CA_CHUNK_SIZE,
            palette: CaMaterialPalette::default(),
            max_edits_per_step: MAX_VOXEL_EDITS_PER_TRANSACTION,
            max_mesh_values_per_step: DEFAULT_MAX_MESH_VALUES_PER_STEP,
            materialize_empty: workload == CaWorkloadClass::LargeResidentSmallHotRegion,
        }
    }
}

pub trait BenchmarkClock {
    /// Monotonic nanoseconds in an arbitrary caller-owned epoch.
    fn now_ns(&mut self) -> u64;
}

#[derive(Debug)]
pub struct SystemBenchmarkClock {
    origin: Instant,
}

impl Default for SystemBenchmarkClock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl BenchmarkClock for SystemBenchmarkClock {
    fn now_ns(&mut self) -> u64 {
        u64::try_from(self.origin.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaAdmissionTiming {
    pub state_materialization_ns: u64,
    pub engine_build_ns: u64,
    pub evidence_readback_ns: u64,
    pub artifact_encoding_ns: u64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaStepTiming {
    pub ca_step_ns: u64,
    pub request_construction_ns: u64,
    pub spatial_preview_ns: u64,
    pub authority_commit_ns: u64,
    pub evidence_readback_ns: u64,
    pub artifact_encoding_ns: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaAuthorityReadout {
    pub source_revision: u64,
    pub authority_hash: String,
    pub projection_revisions_coherent: bool,
    pub solid_voxel_count: usize,
    pub resident_chunk_count: usize,
    pub collider_chunk_count: usize,
    pub navigation_cell_count: usize,
    pub navigation_hash: String,
    pub mesh_chunk_count: usize,
    pub mesh_vertex_count: u64,
    pub mesh_quad_count: u64,
    pub mesh_projection_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaMeshGroupFact {
    pub material_slot: u16,
    pub start: u32,
    pub count: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaMeshChunkFact {
    pub chunk: [i64; 3],
    pub content_hash: String,
    pub buffer_hash: String,
    pub translation: [f32; 3],
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub indices: Vec<u32>,
    pub groups: Vec<CaMeshGroupFact>,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub vertices: u32,
    pub quads: u32,
    pub faces_culled: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    tag = "op",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum CaProjectionOp {
    Upsert { chunk: CaMeshChunkFact },
    Delete { chunk: [i64; 3] },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaMeshChunkSummary {
    pub chunk: [i64; 3],
    pub content_hash: String,
    pub buffer_hash: String,
    pub vertices: u32,
    pub quads: u32,
    pub faces_culled: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaInitialAuthority {
    pub initial_ca_state_hash: String,
    pub readout: CaAuthorityReadout,
    pub projection_chunks: Vec<CaMeshChunkFact>,
    pub projection_state_hash: String,
    pub trace_hash: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaSpatialStep {
    pub ca: CaStepEvidence,
    pub revision_before: u64,
    pub accepted_revision: u64,
    pub engine_changed_voxels: usize,
    pub canonical_edit_count: usize,
    pub engine_delta_count: usize,
    pub readout: CaAuthorityReadout,
    pub projection_ops: Vec<CaProjectionOp>,
    pub projection_delta_hash: String,
    pub projection_state_hash: String,
    pub previous_trace_hash: String,
    pub trace_hash: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaSpatialTrace {
    pub kind: String,
    pub schema_version: u32,
    pub scenario_id: String,
    pub workload: CaWorkloadClass,
    pub rule_id: String,
    pub seed: u64,
    pub bounds: CaBounds,
    pub neighborhood: CaNeighborhood,
    pub boundary: CaBoundaryPolicy,
    pub materialize_empty: bool,
    pub initial: CaInitialAuthority,
    pub steps: Vec<CaSpatialStep>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaRecordedRun {
    pub run: u32,
    pub structural_hash: String,
    pub admission_timing: CaAdmissionTiming,
    pub step_timings: Vec<CaStepTiming>,
    pub encoded_trace_bytes: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaScenarioBenchmark {
    pub scenario_id: String,
    pub warmup_runs: u32,
    pub recorded_runs: Vec<CaRecordedRun>,
    pub deterministic_structural_evidence: bool,
    pub trace: CaSpatialTrace,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaBenchmarkConfig {
    pub warmup_runs: u32,
    pub recorded_runs: u32,
    pub chunk_size: u32,
    pub max_edits_per_step: usize,
    pub max_mesh_values_per_step: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaBenchmarkEnvironment {
    pub operating_system: String,
    pub architecture: String,
    pub rustc_version: String,
    pub build_profile: String,
    pub clock: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CaSpatialBenchmarkEvidence {
    pub kind: String,
    pub schema_version: u32,
    pub repository_commit: String,
    pub engine_commit: String,
    pub environment: CaBenchmarkEnvironment,
    pub config: CaBenchmarkConfig,
    pub scenarios: Vec<CaScenarioBenchmark>,
    pub non_claims: Vec<String>,
}

#[derive(Debug)]
pub enum CaSpatialError {
    Scenario(CaError),
    InvalidOptions {
        detail: String,
    },
    EngineBuild(CollisionSceneError),
    EngineEdit(VoxelEditApplyError),
    StepEditQuotaExceeded {
        actual: usize,
        limit: usize,
    },
    MeshEvidenceQuotaExceeded {
        actual: usize,
        limit: usize,
    },
    PreparedCaStateChanged {
        expected_step: u32,
        actual_step: u32,
        expected_hash: String,
        actual_hash: String,
    },
    RevisionMismatch {
        expected: VoxelSourceRevision,
        actual: VoxelSourceRevision,
    },
    InvalidRunConfig {
        detail: String,
    },
    Serialization {
        detail: String,
    },
}

impl CaSpatialError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Scenario(_) => "invalid_ca_scenario",
            Self::InvalidOptions { .. } => "invalid_benchmark_options",
            Self::EngineBuild(_) => "engine_spatial_build_failed",
            Self::EngineEdit(_) => "engine_spatial_edit_failed",
            Self::StepEditQuotaExceeded { .. } => "step_edit_quota_exceeded",
            Self::MeshEvidenceQuotaExceeded { .. } => "mesh_evidence_quota_exceeded",
            Self::PreparedCaStateChanged { .. } => "prepared_ca_state_changed",
            Self::RevisionMismatch { .. } => "stale_spatial_revision",
            Self::InvalidRunConfig { .. } => "invalid_run_config",
            Self::Serialization { .. } => "evidence_serialization_failed",
        }
    }
}

impl Display for CaSpatialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: ", self.code())?;
        match self {
            Self::Scenario(error) => error.fmt(formatter),
            Self::InvalidOptions { detail }
            | Self::InvalidRunConfig { detail }
            | Self::Serialization { detail } => formatter.write_str(detail),
            Self::EngineBuild(error) => error.fmt(formatter),
            Self::EngineEdit(error) => error.fmt(formatter),
            Self::StepEditQuotaExceeded { actual, limit } => {
                write!(formatter, "step has {actual} edits; limit is {limit}")
            }
            Self::MeshEvidenceQuotaExceeded { actual, limit } => {
                write!(formatter, "mesh evidence has {actual} values; limit is {limit}")
            }
            Self::PreparedCaStateChanged {
                expected_step,
                actual_step,
                expected_hash,
                actual_hash,
            } => write!(
                formatter,
                "prepared from step {expected_step}/{expected_hash}, current is {actual_step}/{actual_hash}"
            ),
            Self::RevisionMismatch { expected, actual } => write!(
                formatter,
                "expected Engine revision {}, current is {}",
                expected.raw(),
                actual.raw()
            ),
        }
    }
}

impl std::error::Error for CaSpatialError {}

impl From<CaError> for CaSpatialError {
    fn from(error: CaError) -> Self {
        Self::Scenario(error)
    }
}

pub(crate) fn elapsed(start: u64, end: u64) -> u64 {
    end.saturating_sub(start)
}
