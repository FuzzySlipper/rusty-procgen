use std::fmt;

use engine_spatial::{CollisionSceneError, VoxelEditApplyError, VoxelSourceRevision};
use serde::{Deserialize, Serialize};

pub const DEFAULT_WALL_MATERIAL: u16 = 1;
pub const DEFAULT_FLOOR_MATERIAL: u16 = 2;
pub const DEFAULT_CEILING_MATERIAL: u16 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtrusionOptions {
    pub chunk_size: u32,
    pub floor_y: i64,
    pub wall_min_y: i64,
    pub wall_max_y: i64,
    pub ceiling_y: i64,
    pub floor_material: u16,
    pub wall_material: u16,
    pub ceiling_material: u16,
}

impl Default for ExtrusionOptions {
    fn default() -> Self {
        Self {
            chunk_size: 2,
            floor_y: 0,
            wall_min_y: 1,
            wall_max_y: 3,
            ceiling_y: 4,
            floor_material: DEFAULT_FLOOR_MATERIAL,
            wall_material: DEFAULT_WALL_MATERIAL,
            ceiling_material: DEFAULT_CEILING_MATERIAL,
        }
    }
}

impl ExtrusionOptions {
    pub(crate) fn allowed_materials(self) -> [u16; 3] {
        [
            self.wall_material,
            self.floor_material,
            self.ceiling_material,
        ]
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelCoordinate {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

impl VoxelCoordinate {
    pub const fn address(self) -> [i64; 3] {
        [self.x, self.y, self.z]
    }
}

impl From<[i64; 3]> for VoxelCoordinate {
    fn from(address: [i64; 3]) -> Self {
        Self {
            x: address[0],
            y: address[1],
            z: address[2],
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PlanVoxel {
    pub coord: VoxelCoordinate,
    pub material: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DoorPortalExtrusion {
    pub id: String,
    pub source_edge: String,
    pub required_item: Option<String>,
    pub traversal: String,
    pub orientation: String,
    pub cells: Vec<rusty_procgen_preflight::GridCell>,
    pub min_y: i64,
    pub max_exclusive_y: i64,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExtrusionBounds {
    pub min: VoxelCoordinate,
    pub max_exclusive: VoxelCoordinate,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelExtrusionPlan {
    pub schema_version: u32,
    pub placement_id: String,
    pub coordinate_mapping: String,
    pub solid_voxels: Vec<PlanVoxel>,
    pub walkable_cell_count: usize,
    pub opening_cell_count: usize,
    pub boundary_cell_count: usize,
    pub solid_voxel_count: usize,
    pub resident_chunk_count: usize,
    pub door_portals: Vec<DoorPortalExtrusion>,
    pub build_bounds: ExtrusionBounds,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MaterialCount {
    pub material: u16,
    pub voxels: usize,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SpatialReadout {
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
    pub material_counts: Vec<MaterialCount>,
}

#[derive(Debug)]
pub enum SpatialExtrusionError {
    MalformedPlacement {
        code: &'static str,
        detail: String,
    },
    UnknownMaterial {
        material: u16,
    },
    TooManySolidVoxels {
        limit: usize,
        actual: usize,
    },
    StaleRevision {
        expected: VoxelSourceRevision,
        actual: VoxelSourceRevision,
    },
    EngineBuild(CollisionSceneError),
    EngineEdit(VoxelEditApplyError),
}

impl SpatialExtrusionError {
    pub(crate) fn malformed(code: &'static str, detail: impl Into<String>) -> Self {
        Self::MalformedPlacement {
            code,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for SpatialExtrusionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedPlacement { code, detail } => {
                write!(formatter, "malformed placement ({code}): {detail}")
            }
            Self::UnknownMaterial { material } => {
                write!(formatter, "unknown Procgen enclosure material {material}")
            }
            Self::TooManySolidVoxels { limit, actual } => {
                write!(
                    formatter,
                    "extrusion contains {actual} solid voxels; transaction limit is {limit}"
                )
            }
            Self::StaleRevision { expected, actual } => {
                write!(
                    formatter,
                    "stale spatial revision: expected {}, actual {}",
                    expected.raw(),
                    actual.raw()
                )
            }
            Self::EngineBuild(error) => write!(formatter, "Engine spatial build failed: {error}"),
            Self::EngineEdit(error) => write!(formatter, "Engine spatial edit failed: {error}"),
        }
    }
}

impl std::error::Error for SpatialExtrusionError {}
