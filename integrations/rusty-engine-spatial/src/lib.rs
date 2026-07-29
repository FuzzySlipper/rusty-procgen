//! Downstream dungeon-placement policy composed with Rusty Engine spatial authority.
//!
//! Rusty Procgen owns the meaning of placement cells, enclosure materials, and
//! doorway provenance. Rusty Engine owns the accepted material voxels and every
//! collision, navigation, and mesh projection derived from them.

#![forbid(unsafe_code)]

mod compile;
mod host;
mod model;

pub use compile::compile_placement_extrusion;
pub use host::{SpatialExtrusionHost, SpatialExtrusionReceipt};
pub use model::{
    DoorPortalExtrusion, ExtrusionBounds, ExtrusionOptions, MaterialCount, PlanVoxel,
    SpatialExtrusionError, SpatialReadout, VoxelCoordinate, VoxelExtrusionPlan,
    DEFAULT_CEILING_MATERIAL, DEFAULT_FLOOR_MATERIAL, DEFAULT_WALL_MATERIAL,
};
