use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};

fn main() {
    let cli = Cli::parse();
    if let Err(error) = run(cli) {
        eprintln!("rusty-procgen failed:");
        eprintln!("- {error}");
        std::process::exit(1);
    }
}

// These files are included into one module to keep the current private helper
// surface intact while the workbench is still moving quickly.
include!("parts/cli.rs"); // clap args and command enums
include!("parts/model.rs"); // artifact and report contracts
include!("parts/dispatch.rs"); // top-level CLI dispatch
include!("parts/preflight.rs"); // downstream repo posture checks
include!("parts/candidate_graph.rs"); // graph construction and rules
include!("parts/graph_reports.rs"); // graph summaries, analysis, compatibility
include!("parts/intermediate.rs"); // spatial intent and intermediate breakdowns
include!("parts/geometry_preview.rs"); // 2D geometry, validation, HTML preview
include!("parts/piece_plan.rs"); // explicit catalog-piece build plans
include!("parts/catalog_tools.rs"); // shape catalog inspection
include!("parts/shape_matching.rs"); // catalog shape matching
include!("parts/pure_catalog_placement.rs"); // exact prefab-only occupancy search
include!("parts/piece_placement.rs"); // deterministic piece occupancy placement
include!("parts/built_flow.rs"); // source-to-cell traversal equivalence
include!("parts/catalog_aware_generation.rs"); // catalog-first room and corridor composition
include!("parts/intermediate_validation.rs"); // intermediate validation
include!("parts/common_helpers.rs"); // shared graph/id helpers
include!("parts/repair_validation.rs"); // graph validation and repair advice
include!("parts/scoring_embedding.rs"); // scoring and simple 2D embedding
include!("parts/batch_artifacts.rs"); // sample/batch artifact generation
include!("parts/io_utils.rs"); // JSON, receipts, hashing, diagnostics
include!("parts/tests.rs"); // crate-level behavior tests
