//! Public, filesystem-free deterministic generation facade.
//!
//! Methods accept and return typed values. The private CLI adapter may attach
//! filesystem paths to artifact provenance, but no method in this module reads
//! or writes a file.

use std::path::{Path, PathBuf};

use serde::Serialize;

pub use crate::catalog_aware_generation::{
    CatalogAwareAttemptEvidence, CatalogAwareConstraintMiss, CatalogAwareGenerationPolicy,
    CatalogAwareGenerationProvenance, CatalogAwareGenerationResult, CatalogAwareOutcomeComparison,
    CatalogAwareOutcomeConstraints, CatalogAwareOutcomeEvaluation, CatalogAwareOutcomeMetric,
    CatalogAwareOutcomeMetrics, CatalogAwareOutcomePreferences,
};
pub use crate::catalog_generation_trace::{
    replay_catalog_generation_trace, validate_catalog_generation_trace_limits,
    CatalogAwareGenerationRun, CatalogGenerationReplay, CatalogGenerationReplayAttempt,
    CatalogGenerationReplayFrame, CatalogGenerationTrace, CatalogGenerationTraceError,
    CatalogGenerationTraceEvent, CatalogGenerationTraceEventBody,
    CatalogGenerationTraceInputHashes, CatalogGenerationTraceLimits, CatalogGenerationTraceRequest,
    CatalogGenerationTraceRoomCandidate, CatalogGenerationTraceRoomPlacement,
    CatalogGenerationTraceRoute, CatalogGenerationTraceSelection, DEFAULT_CATALOG_TRACE_MAX_EVENTS,
    DEFAULT_CATALOG_TRACE_MAX_EVENT_BODY_BYTES, DEFAULT_CATALOG_TRACE_MAX_VISUAL_CELLS,
};
use crate::{
    analyze_graph, apply_graph_rule, assemble_piece_placement, compatible_rules_report,
    create_initial_candidate, default_geometry_layout_policy, embed_2d,
    emit_geometry_2d_with_policy, emit_piece_build_plan, fork_candidate, hash_json,
    inspect_shape_catalog, intermediate_breakdown, match_shapes, plan_physical_connections,
    realization_scale_multiplier, repair_report, run_catalog_aware_generation,
    run_catalog_aware_generation_traced, score_graph, spatial_intent_report, validate_built_flow,
    validate_geometry_2d, validate_geometry_layout_policy, validate_graph,
    validate_intermediate_breakdown, validate_piece_placement,
    validate_piece_placement_with_catalog, BuildAssembleArgs, BuildEmitPiecePlanArgs,
    BuildMatchShapesArgs, BuildValidateFlowArgs, Candidate, CatalogAwareGenerationInput,
    CatalogInspectionReport, Diagnostic, Geometry2dArtifact, GeometryEmit2dArgs,
    GeometryLayoutPolicy, GraphAnalysisReport, IntermediateBreakdown, LayoutArtifact,
    PhysicalConnectionPlan, PhysicalConnectionPlanArgs, PieceBuildPlan, PiecePlacement,
    PieceShapeMatchReport, RepairReport, RuleCompatibilityReport, ScoreReport, SeedIntent,
    Severity, ShapeCatalog, SpatialIntentReport, ValidationReport,
};

pub use crate::{CorridorRealization, GraphRule, GridConnectivity, RepairAction};

const MEMORY_CANDIDATE: &str = "memory/candidate.json";
const MEMORY_SPATIAL_INTENT: &str = "memory/spatial-intent.json";
const MEMORY_INTERMEDIATE: &str = "memory/intermediate.json";
const MEMORY_CONNECTION_PLAN: &str = "memory/physical-connection-plan.json";
const MEMORY_GEOMETRY: &str = "memory/geometry.json";
const MEMORY_CATALOG: &str = "memory/shape-catalog.json";
const MEMORY_PIECE_PLAN: &str = "memory/piece-plan.json";
const MEMORY_SHAPE_MATCH: &str = "memory/shape-match.json";
const MEMORY_PLACEMENT: &str = "memory/piece-placement.json";

/// Whether a requested transformation was accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuleDisposition {
    /// The rule was applied and the returned candidate includes provenance.
    Accepted,
    /// The rule was rejected and the returned candidate is byte-equivalent to
    /// the input candidate.
    Rejected,
}

/// Result of applying one graph rule without mutating the caller's candidate.
#[derive(Clone, Debug)]
pub struct RuleApplication {
    pub disposition: RuleDisposition,
    pub candidate: Candidate,
    pub diagnostics: Vec<Diagnostic>,
}

impl RuleApplication {
    pub fn accepted(&self) -> bool {
        self.disposition == RuleDisposition::Accepted
    }
}

/// Stateless entry point for deterministic in-memory Procgen operations.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcgenCore;

impl ProcgenCore {
    pub fn create_candidate(intent: &SeedIntent, seed: u64) -> Candidate {
        create_initial_candidate(intent, seed)
    }

    pub fn fork_candidate(candidate: Candidate, label: &str, seed: u64) -> Candidate {
        fork_candidate(candidate, label, seed)
    }

    /// Apply a graph rule fail-atomically.
    pub fn apply_rule(candidate: &Candidate, rule: GraphRule, seed: u64) -> RuleApplication {
        let mut proposed = candidate.clone();
        let diagnostics = apply_graph_rule(&mut proposed, rule, seed);
        if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Fatal)
        {
            return RuleApplication {
                disposition: RuleDisposition::Rejected,
                candidate: candidate.clone(),
                diagnostics,
            };
        }
        proposed.provenance.push(crate::ProvenanceStep {
            step: proposed.provenance.len() as u32 + 1,
            command: format!("graph apply-rule {}", rule.as_str()),
            seed: Some(seed),
            summary: format!("Applied {}", rule.as_str()),
        });
        RuleApplication {
            disposition: RuleDisposition::Accepted,
            candidate: proposed,
            diagnostics,
        }
    }

    pub fn canonical_hash<T: Serialize>(value: &T) -> Result<String, String> {
        hash_json(value)
    }

    pub fn analyze(candidate: &Candidate) -> Result<GraphAnalysisReport, String> {
        analyze_graph(candidate)
    }

    pub fn compatible_rules(candidate: &Candidate) -> Result<RuleCompatibilityReport, String> {
        compatible_rules_report(candidate)
    }

    pub fn annotate_spatial_intent(candidate: &Candidate) -> Result<SpatialIntentReport, String> {
        spatial_intent_report(candidate, None)
    }

    pub fn breakdown(
        candidate: &Candidate,
        annotations: &SpatialIntentReport,
    ) -> Result<IntermediateBreakdown, String> {
        intermediate_breakdown(candidate, annotations, Path::new(MEMORY_SPATIAL_INTENT))
    }

    pub fn validate_intermediate(breakdown: &IntermediateBreakdown) -> ValidationReport {
        validate_intermediate_breakdown(breakdown)
    }

    pub fn plan_connections(
        candidate: &Candidate,
        intermediate: &IntermediateBreakdown,
    ) -> Result<PhysicalConnectionPlan, String> {
        plan_physical_connections(
            candidate,
            intermediate,
            &PhysicalConnectionPlanArgs {
                candidate: PathBuf::from(MEMORY_CANDIDATE),
                intermediate: PathBuf::from(MEMORY_INTERMEDIATE),
                out: PathBuf::from(MEMORY_CONNECTION_PLAN),
            },
        )
    }

    pub fn default_geometry_policy() -> GeometryLayoutPolicy {
        default_geometry_layout_policy()
    }

    pub fn emit_geometry(
        candidate: &Candidate,
        intermediate: &IntermediateBreakdown,
        connection_plan: &PhysicalConnectionPlan,
        layout_policy: &GeometryLayoutPolicy,
        seed: u64,
    ) -> Result<Geometry2dArtifact, String> {
        emit_geometry_2d_with_policy(
            candidate,
            intermediate,
            connection_plan,
            layout_policy,
            &GeometryEmit2dArgs {
                candidate: PathBuf::from(MEMORY_CANDIDATE),
                intermediate: PathBuf::from(MEMORY_INTERMEDIATE),
                connection_plan: PathBuf::from(MEMORY_CONNECTION_PLAN),
                layout_policy: None,
                seed,
                out: PathBuf::from(MEMORY_GEOMETRY),
            },
            seed,
        )
    }

    pub fn validate_geometry(geometry: &Geometry2dArtifact) -> ValidationReport {
        validate_geometry_2d(geometry)
    }

    pub fn validate_geometry_policy(layout_policy: &GeometryLayoutPolicy) -> Result<(), String> {
        validate_geometry_layout_policy(layout_policy)
    }

    pub fn emit_piece_plan(
        candidate: &Candidate,
        intermediate: &IntermediateBreakdown,
        geometry: &Geometry2dArtifact,
        corridor_realization: CorridorRealization,
    ) -> Result<PieceBuildPlan, String> {
        emit_piece_build_plan(
            candidate,
            intermediate,
            geometry,
            &BuildEmitPiecePlanArgs {
                candidate: PathBuf::from(MEMORY_CANDIDATE),
                intermediate: PathBuf::from(MEMORY_INTERMEDIATE),
                geometry: PathBuf::from(MEMORY_GEOMETRY),
                corridor_realization,
                out: PathBuf::from(MEMORY_PIECE_PLAN),
            },
        )
    }

    pub fn inspect_catalog(catalog: &ShapeCatalog) -> CatalogInspectionReport {
        inspect_shape_catalog(catalog, Path::new(MEMORY_CATALOG))
    }

    pub fn match_shapes(
        catalog: &ShapeCatalog,
        plan: &PieceBuildPlan,
        seed: u64,
    ) -> PieceShapeMatchReport {
        match_shapes(
            catalog,
            plan,
            &BuildMatchShapesArgs {
                catalog: PathBuf::from(MEMORY_CATALOG),
                piece_plan: PathBuf::from(MEMORY_PIECE_PLAN),
                seed,
                out: PathBuf::from(MEMORY_SHAPE_MATCH),
            },
        )
    }

    pub fn assemble(
        catalog: &ShapeCatalog,
        plan: &PieceBuildPlan,
        shape_match: &PieceShapeMatchReport,
        connectivity: GridConnectivity,
    ) -> Result<PiecePlacement, String> {
        assemble_piece_placement(
            catalog,
            plan,
            shape_match,
            &BuildAssembleArgs {
                catalog: PathBuf::from(MEMORY_CATALOG),
                piece_plan: PathBuf::from(MEMORY_PIECE_PLAN),
                shape_match: PathBuf::from(MEMORY_SHAPE_MATCH),
                connectivity,
                out: PathBuf::from(MEMORY_PLACEMENT),
            },
        )
    }

    pub fn realize_catalog_aware(
        candidate: &Candidate,
        source_geometry: &Geometry2dArtifact,
        source_plan: &PieceBuildPlan,
        catalog: &ShapeCatalog,
        policy: &CatalogAwareGenerationPolicy,
        provenance: &CatalogAwareGenerationProvenance,
        seed: u64,
    ) -> Result<CatalogAwareGenerationResult, String> {
        run_catalog_aware_generation(CatalogAwareGenerationInput {
            candidate,
            source_geometry,
            source_plan,
            catalog,
            policy,
            provenance,
            seed,
        })
    }

    pub fn realize_catalog_aware_traced(
        request: CatalogGenerationTraceRequest<'_>,
    ) -> Result<CatalogAwareGenerationRun, CatalogGenerationTraceError> {
        run_catalog_aware_generation_traced(
            CatalogAwareGenerationInput {
                candidate: request.candidate,
                source_geometry: request.source_geometry,
                source_plan: request.source_plan,
                catalog: request.catalog,
                policy: request.generation_policy,
                provenance: request.provenance,
                seed: request.seed,
            },
            request.trace_limits,
        )
    }

    pub fn validate_placement(placement: &PiecePlacement) -> ValidationReport {
        validate_piece_placement(placement)
    }

    /// Validate a placement against the exact catalog and accepted shape-match
    /// artifacts that produced it, including transformed scene sockets.
    pub fn validate_placement_with_catalog(
        catalog: &ShapeCatalog,
        plan: &PieceBuildPlan,
        shape_match: &PieceShapeMatchReport,
        placement: &PiecePlacement,
    ) -> ValidationReport {
        validate_piece_placement_with_catalog(catalog, plan, shape_match, placement)
    }

    pub fn validate_built_flow(
        candidate: &Candidate,
        geometry: &Geometry2dArtifact,
        plan: &PieceBuildPlan,
        placement: &PiecePlacement,
    ) -> crate::BuiltFlowValidationReport {
        validate_built_flow(
            candidate,
            geometry,
            plan,
            placement,
            &BuildValidateFlowArgs {
                candidate: PathBuf::from(MEMORY_CANDIDATE),
                geometry: PathBuf::from(MEMORY_GEOMETRY),
                piece_plan: PathBuf::from(MEMORY_PIECE_PLAN),
                piece_placement: PathBuf::from(MEMORY_PLACEMENT),
                out: PathBuf::from("memory/built-flow-validation.json"),
            },
        )
    }

    pub fn validate_candidate(candidate: &Candidate) -> ValidationReport {
        validate_graph(candidate)
    }

    pub fn repair(candidate: &Candidate) -> Result<RepairReport, String> {
        repair_report(candidate)
    }

    pub fn score(candidate: &Candidate) -> ScoreReport {
        score_graph(candidate)
    }

    pub fn embed(candidate: &Candidate, seed: u64) -> LayoutArtifact {
        embed_2d(candidate, seed)
    }

    /// Convert a persisted realization tier into its bounded scale
    /// multiplier. Invalid or overflowing tiers are rejected.
    pub fn realization_scale_multiplier(realization_scale_tier: u32) -> Option<i32> {
        realization_scale_multiplier(realization_scale_tier)
    }
}
