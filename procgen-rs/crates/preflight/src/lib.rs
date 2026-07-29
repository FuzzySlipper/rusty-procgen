//! Deterministic dungeon-generation library and CLI composition.
//!
//! The public [`core`] facade is the supported in-memory API. The remaining
//! modules retain the established behavior owners while the command adapters
//! translate filesystem inputs and outputs for the `rusty-procgen` binary.

pub(crate) use std::cmp::{Ordering, Reverse};
pub(crate) use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, VecDeque};
pub(crate) use std::fs::{self, OpenOptions};
pub(crate) use std::io::Write;
pub(crate) use std::path::{Path, PathBuf};

pub(crate) use clap::{Args, Parser, Subcommand, ValueEnum};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use serde_json::{json, Value as JsonValue};

#[path = "parts/model.rs"]
mod model;
pub use model::*;

#[path = "parts/cli.rs"]
mod cli;
pub use cli::*;
#[path = "parts/io_utils.rs"]
mod io_utils;
pub(crate) use io_utils::*;
#[path = "parts/common_helpers.rs"]
mod common_helpers;
pub(crate) use common_helpers::*;
#[path = "parts/candidate_graph.rs"]
mod candidate_graph;
pub(crate) use candidate_graph::*;
#[path = "parts/graph_reports.rs"]
mod graph_reports;
pub(crate) use graph_reports::*;
#[path = "parts/intermediate.rs"]
mod intermediate;
pub(crate) use intermediate::*;
#[path = "parts/intermediate_validation.rs"]
mod intermediate_validation;
pub(crate) use intermediate_validation::*;
#[path = "parts/repair_validation.rs"]
mod repair_validation;
pub(crate) use repair_validation::*;
#[path = "parts/scoring_embedding.rs"]
mod scoring_embedding;
pub(crate) use scoring_embedding::*;
#[path = "parts/geometry_preview.rs"]
mod geometry_preview;
pub(crate) use geometry_preview::*;
#[path = "parts/piece_plan.rs"]
mod piece_plan;
pub(crate) use piece_plan::*;
#[path = "parts/catalog_tools.rs"]
mod catalog_tools;
pub(crate) use catalog_tools::*;
#[path = "parts/shape_matching.rs"]
mod shape_matching;
pub(crate) use shape_matching::*;
#[path = "parts/pure_catalog_placement.rs"]
mod pure_catalog_placement;
pub(crate) use pure_catalog_placement::*;
#[path = "parts/piece_placement.rs"]
mod piece_placement;
pub(crate) use piece_placement::*;
#[path = "parts/built_flow.rs"]
mod built_flow;
pub(crate) use built_flow::*;
#[path = "parts/catalog_aware_generation.rs"]
mod catalog_aware_generation;
pub(crate) use catalog_aware_generation::*;
#[path = "parts/batch_artifacts.rs"]
mod batch_artifacts;
pub(crate) use batch_artifacts::*;
#[path = "parts/dispatch.rs"]
mod dispatch;

pub mod cellular_automata;
pub mod core;

/// Parse process arguments and execute the filesystem-backed CLI adapter.
pub fn run_cli() -> Result<(), String> {
    dispatch::run(Cli::parse())
}

#[cfg(test)]
#[path = "parts/tests.rs"]
mod tests;
