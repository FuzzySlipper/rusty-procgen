//! Downstream cellular-automata workloads composed with Rusty Engine spatial authority.
//!
//! Rusty Procgen owns scenario evolution, benchmark orchestration, timing, and
//! trace policy. Rusty Engine owns admitted voxels and coherent
//! collision/navigation/mesh projections.

#![forbid(unsafe_code)]

mod benchmark;
mod host;
mod model;

pub use benchmark::{benchmark_suite, BenchmarkRunConfig};
pub use host::{CaSpatialHost, PreparedCaSpatialStep};
pub use model::*;
