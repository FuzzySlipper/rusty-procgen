#[allow(unused_imports)]
use crate::*;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedIntent {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub target_dimension: String,
    pub desired_patterns: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub kind: String,
    pub schema_version: u32,
    pub candidate_id: String,
    pub seed: u64,
    pub dimension_model: String,
    pub source_intent: Option<String>,
    pub provenance: Vec<ProvenanceStep>,
    pub graph: IntentGraph,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceStep {
    pub step: u32,
    pub command: String,
    pub seed: Option<u64>,
    pub summary: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    pub tags: Vec<String>,
    pub grants_item: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Start,
    Goal,
    Gate,
    Key,
    Treasure,
    Shortcut,
    Secret,
    Hazard,
    Resource,
    Junction,
}

impl NodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            NodeKind::Start => "start",
            NodeKind::Goal => "goal",
            NodeKind::Gate => "gate",
            NodeKind::Key => "key",
            NodeKind::Treasure => "treasure",
            NodeKind::Shortcut => "shortcut",
            NodeKind::Secret => "secret",
            NodeKind::Hazard => "hazard",
            NodeKind::Resource => "resource",
            NodeKind::Junction => "junction",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub traversal: TraversalKind,
    pub required_item: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    CriticalPath,
    KeyBranch,
    OptionalBranch,
    Shortcut,
    SecretBypass,
}

impl EdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeKind::CriticalPath => "critical_path",
            EdgeKind::KeyBranch => "key_branch",
            EdgeKind::OptionalBranch => "optional_branch",
            EdgeKind::Shortcut => "shortcut",
            EdgeKind::SecretBypass => "secret_bypass",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TraversalKind {
    Open,
    Locked,
    OneWayReturn,
    Hidden,
}

impl TraversalKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TraversalKind::Open => "open",
            TraversalKind::Locked => "locked",
            TraversalKind::OneWayReturn => "one_way_return",
            TraversalKind::Hidden => "hidden",
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub kind: String,
    pub schema_version: u32,
    pub command: String,
    pub status: String,
    pub seed: Option<u64>,
    pub input_hash: Option<String>,
    pub output_hash: Option<String>,
    pub output_ref: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub node: Option<String>,
    pub edge: Option<String>,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repair_hint: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Fatal,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub kind: String,
    pub schema_version: u32,
    pub state_hash: String,
    pub ok: bool,
    pub fatal_count: usize,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreReport {
    pub kind: String,
    pub schema_version: u32,
    pub state_hash: String,
    pub overall: f64,
    pub metrics: BTreeMap<String, f64>,
    pub notes: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleMetadataReport {
    pub kind: String,
    pub schema_version: u32,
    pub rules: Vec<RuleMetadata>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleMetadata {
    pub id: String,
    pub intent: String,
    pub required_patterns: Vec<String>,
    pub duplicate_markers: Vec<String>,
    pub emitted_node_tags: Vec<String>,
    pub emitted_edge_tags: Vec<String>,
    pub compatibility_hints: Vec<String>,
    pub repair_hints: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphSummaryReport {
    pub kind: String,
    pub schema_version: u32,
    pub candidate_id: String,
    pub state_hash: String,
    pub validation_ok: bool,
    pub fatal_count: usize,
    pub score_overall: f64,
    pub metrics: BTreeMap<String, f64>,
    pub node_count: usize,
    pub edge_count: usize,
    pub tags: Vec<String>,
    pub locked_items: Vec<String>,
    pub dead_ends: Vec<String>,
    pub provenance_tail: Vec<ProvenanceStep>,
    pub nodes: Vec<NodeSummary>,
    pub edges: Vec<EdgeSummary>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairReport {
    pub kind: String,
    pub schema_version: u32,
    pub candidate_id: String,
    pub state_hash: String,
    pub validation_ok: bool,
    pub fatal_count: usize,
    pub suggestions: Vec<RepairSuggestion>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairSuggestion {
    pub code: String,
    pub severity: Severity,
    pub node: Option<String>,
    pub edge: Option<String>,
    pub detail: String,
    pub repair_hint: Option<String>,
    pub suggested_actions: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphAnalysisReport {
    pub kind: String,
    pub schema_version: u32,
    pub candidate_id: String,
    pub state_hash: String,
    pub critical_path: Vec<String>,
    pub dominators: Vec<String>,
    pub optional_branches: Vec<BranchAnalysis>,
    pub lock_key_order: Vec<LockKeyAnalysis>,
    pub loop_signals: Vec<LoopSignal>,
    pub shortcut_bypass_risks: Vec<ShortcutRisk>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchAnalysis {
    pub edge_id: String,
    pub from: String,
    pub to: String,
    pub classification: String,
    pub rejoins_goal_route: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LockKeyAnalysis {
    pub edge_id: String,
    pub required_item: String,
    pub provider_node: Option<String>,
    pub provider_reachable_before_lock: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopSignal {
    pub edge_id: String,
    pub signal: String,
    pub detail: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutRisk {
    pub edge_id: String,
    pub risk: String,
    pub detail: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCompatibilityReport {
    pub kind: String,
    pub schema_version: u32,
    pub candidate_id: String,
    pub state_hash: String,
    pub rules: Vec<RuleCompatibility>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCompatibility {
    pub rule: String,
    pub status: String,
    pub reasons: Vec<String>,
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpatialIntentReport {
    pub kind: String,
    pub schema_version: u32,
    pub candidate_id: String,
    pub state_hash: String,
    pub analysis_ref: Option<String>,
    pub annotations: Vec<SpatialIntentAnnotation>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpatialIntentAnnotation {
    pub target_type: String,
    pub target_id: String,
    pub intents: Vec<String>,
    pub rationale: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntermediateBreakdown {
    pub kind: String,
    pub schema_version: u32,
    pub candidate_id: String,
    pub state_hash: String,
    pub annotation_ref: String,
    pub regions: Vec<IntermediateRegion>,
    pub connectors: Vec<IntermediateConnector>,
    pub constraints: Vec<IntermediateConstraint>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntermediateRegion {
    pub id: String,
    pub node_ids: Vec<String>,
    pub role: String,
    pub anchor_node: Option<String>,
    #[serde(default)]
    pub geometry_role: String,
    #[serde(default)]
    pub footprint_class: String,
    #[serde(default)]
    pub scale_band: String,
    #[serde(default)]
    pub anchor_quality: String,
    #[serde(default)]
    pub entrance_expectations: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntermediateConnector {
    pub id: String,
    pub edge_id: String,
    pub from_region: String,
    pub to_region: String,
    pub intents: Vec<String>,
    #[serde(default)]
    pub affordances: Vec<String>,
    #[serde(default)]
    pub traversal_hint: String,
    #[serde(default)]
    pub constraint_refs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntermediateConstraint {
    pub code: String,
    pub target: String,
    #[serde(default)]
    pub target_type: String,
    #[serde(default)]
    pub source_intents: Vec<String>,
    #[serde(default)]
    pub graph_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalConnectionPlan {
    pub kind: String,
    pub schema_version: u32,
    pub plan_id: String,
    pub candidate_id: String,
    pub source_candidate_ref: String,
    pub source_intermediate_ref: String,
    pub sections: Vec<PhysicalConnectionSection>,
    pub edge_mappings: Vec<PhysicalEdgeMapping>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalConnectionSection {
    pub id: String,
    pub topology: String,
    pub terminal_regions: Vec<String>,
    pub source_connectors: Vec<String>,
    pub source_edges: Vec<String>,
    pub traversal_refs: Vec<PhysicalTraversalRef>,
    pub width: i32,
    pub semantic_tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalTraversalRef {
    pub connector_id: String,
    pub edge_id: String,
    pub from_region: String,
    pub to_region: String,
    pub traversal: String,
    pub required_item: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalEdgeMapping {
    pub edge_id: String,
    pub connector_id: String,
    pub section_id: String,
    pub from_region: String,
    pub to_region: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSummary {
    pub id: String,
    pub kind: NodeKind,
    pub tags: Vec<String>,
    pub grants_item: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeSummary {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub traversal: TraversalKind,
    pub required_item: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutArtifact {
    pub kind: String,
    pub schema_version: u32,
    pub layout_id: String,
    pub candidate_id: String,
    pub seed: u64,
    pub rooms: Vec<LayoutRoom>,
    pub links: Vec<LayoutLink>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutRoom {
    pub node_id: String,
    pub kind: NodeKind,
    pub label: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutLink {
    pub edge_id: String,
    pub from_node: String,
    pub to_node: String,
    pub kind: EdgeKind,
    pub traversal: TraversalKind,
    pub required_item: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Geometry2dArtifact {
    pub kind: String,
    pub schema_version: u32,
    pub geometry_id: String,
    pub candidate_id: String,
    pub seed: u64,
    pub source_candidate_ref: String,
    pub source_intermediate_ref: String,
    pub source_connection_plan_ref: String,
    pub connection_plan_id: String,
    pub layout_policy: GeometryLayoutPolicy,
    pub layout_search: GeometryLayoutSearchEvidence,
    pub bounds: GeometryBounds,
    pub rooms: Vec<GeometryRoom>,
    pub corridors: Vec<GeometryCorridor>,
    pub contents: Vec<GeometryContent>,
    pub skipped_connectors: Vec<SkippedConnector>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryLayoutPolicy {
    pub kind: String,
    pub schema_version: u32,
    pub initial_room_margin: i32,
    pub initial_column_gap: i32,
    pub initial_row_gap: i32,
    pub room_margin_growth: i32,
    pub column_gap_growth: i32,
    pub row_gap_growth: i32,
    pub max_spacing_tiers: u32,
    pub room_order_attempts_per_tier: u32,
    pub max_search_attempts: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometrySpacing {
    pub room_margin: i32,
    pub column_gap: i32,
    pub row_gap: i32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryLayoutSearchEvidence {
    pub spacing_tier: u32,
    pub room_order_attempt: u32,
    pub port_order_attempt: u32,
    pub route_order_attempt: u32,
    pub search_attempts: u32,
    pub effective_spacing: GeometrySpacing,
    #[serde(default = "default_geometry_embedding_kind")]
    pub embedding_kind: String,
    #[serde(default)]
    pub embedding_id: String,
    #[serde(default)]
    pub embedding_faces: u32,
    #[serde(default)]
    pub embedding_target_faces: u32,
    #[serde(default)]
    pub embedding_search_steps: u32,
    #[serde(default)]
    pub route_decisions: u32,
    #[serde(default)]
    pub route_backtracks: u32,
    #[serde(default)]
    pub route_path_alternatives: u32,
    #[serde(default)]
    pub route_repairs: u32,
    #[serde(default)]
    pub route_grid_expansions: u32,
    #[serde(default)]
    pub route_path_expansion_exhaustions: u32,
    #[serde(default)]
    pub route_last_failed_section: String,
    #[serde(default)]
    pub route_blocking_owners: Vec<String>,
    #[serde(default)]
    pub valid_layout_candidates: u32,
    #[serde(default)]
    pub compactness_portal_capacity_penalty: u32,
    #[serde(default)]
    pub compactness_envelope_area: i64,
    #[serde(default)]
    pub compactness_corridor_centerline_length: i64,
    #[serde(default)]
    pub compactness_routed_shell_cost: i64,
    #[serde(default)]
    pub compactness_bend_count: u32,
}

pub(crate) fn default_geometry_embedding_kind() -> String {
    "depth_columns".to_owned()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryBounds {
    pub width: i32,
    pub height: i32,
    pub grid: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryRoom {
    pub id: String,
    pub source_region: String,
    pub source_nodes: Vec<String>,
    pub role: String,
    pub geometry_role: String,
    pub footprint_class: String,
    pub rect: GeometryRect,
    pub ports: Vec<GeometryRoomPort>,
    pub style_tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryRoomPort {
    pub id: String,
    pub section_id: String,
    pub side: String,
    pub point: GeometryPoint,
    pub width: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryCorridor {
    pub id: String,
    pub physical_section: String,
    pub source_connector: String,
    pub source_edge: String,
    pub source_connectors: Vec<String>,
    pub source_edges: Vec<String>,
    pub traversal_refs: Vec<PhysicalTraversalRef>,
    pub from_room: String,
    pub to_room: String,
    pub traversal_hint: String,
    pub semantic_tags: Vec<String>,
    pub width: i32,
    pub from_port: String,
    pub to_port: String,
    pub points: Vec<GeometryPoint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryContent {
    pub id: String,
    pub room_id: String,
    pub source_ref: String,
    pub kind: String,
    pub label: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PieceBuildPlan {
    pub kind: String,
    pub schema_version: u32,
    pub plan_id: String,
    pub candidate_id: String,
    pub geometry_id: String,
    #[serde(default)]
    pub corridor_realization: CorridorRealization,
    pub source_candidate_ref: String,
    pub source_intermediate_ref: String,
    pub source_geometry_ref: String,
    pub requirements: Vec<PieceRequirement>,
    pub links: Vec<PieceLink>,
    pub content_requirements: Vec<PieceContentRequirement>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PieceRequirement {
    pub piece_id: String,
    pub kind: String,
    pub role: String,
    pub source_refs: Vec<String>,
    pub required_exits: Vec<PieceExitRequirement>,
    pub required_sockets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_shape_tags: Vec<String>,
    pub tags: Vec<String>,
    pub placement_hints: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PieceExitRequirement {
    pub id: String,
    pub direction: String,
    pub width: i32,
    #[serde(default)]
    pub order: i32,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PieceLink {
    pub id: String,
    pub from_piece: String,
    pub from_exit: String,
    pub to_piece: String,
    pub to_exit: String,
    pub source_section: String,
    pub source_corridor: String,
    pub source_edge: String,
    pub source_edges: Vec<String>,
    pub traversal_refs: Vec<PhysicalTraversalRef>,
    pub source_ref: String,
    pub traversal: String,
    pub required_item: Option<String>,
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_points: Vec<GeometryPoint>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PieceContentRequirement {
    pub id: String,
    pub piece_id: String,
    pub source_ref: String,
    pub kind: String,
    pub label: String,
    pub required_socket: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PieceShapeMatchReport {
    pub kind: String,
    pub schema_version: u32,
    pub match_id: String,
    pub plan_id: String,
    pub catalog_id: String,
    pub seed: u64,
    pub alternative_attempt: u32,
    pub source_plan_ref: String,
    pub source_catalog_ref: String,
    pub ok: bool,
    pub unmatched_count: usize,
    pub matches: Vec<MatchedPiece>,
    pub rejections: Vec<ShapeMatchRejection>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchedPiece {
    pub piece_id: String,
    pub requirement_kind: String,
    pub shape_id: String,
    pub transform: String,
    pub score: i32,
    pub candidate_rank: usize,
    pub candidate_count: usize,
    pub source_requirement_ref: String,
    pub exit_map: Vec<MatchedExit>,
    pub socket_map: Vec<MatchedSocket>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchedExit {
    pub requirement_exit_id: String,
    pub catalog_exit_id: String,
    pub x: i32,
    pub y: i32,
    pub direction: String,
    pub width: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchedSocket {
    pub required_socket: String,
    pub catalog_socket_id: String,
    pub kind: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeMatchRejection {
    pub piece_id: String,
    pub shape_id: String,
    pub transform: Option<String>,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiecePlacement {
    pub kind: String,
    pub schema_version: u32,
    pub placement_id: String,
    pub plan_id: String,
    pub catalog_id: String,
    pub match_id: String,
    #[serde(default)]
    pub corridor_realization: CorridorRealization,
    pub source_plan_ref: String,
    pub source_catalog_ref: String,
    pub source_match_ref: String,
    pub cell_size: i32,
    pub grid_connectivity: GridConnectivity,
    pub placement_policy: PiecePlacementPolicy,
    pub realization_search: PieceRealizationSearchEvidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_search: Option<CatalogSearchEvidence>,
    pub instances: Vec<PieceInstance>,
    pub glued_exits: Vec<GluedExit>,
    pub gate_portals: Vec<GatePortal>,
    pub occupied_cells: Vec<PlacementCellRef>,
    pub connection_cells: Vec<PlacementCellRef>,
    pub reserved_cells: Vec<PlacementCellRef>,
    pub dangling_exits: Vec<DanglingExit>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PieceRealizationSearchEvidence {
    pub realization_scale_tier: u32,
    pub realization_attempts: u32,
    pub route_order_attempt: u32,
    pub route_attempts: u32,
    #[serde(default)]
    pub route_decisions: u32,
    #[serde(default)]
    pub route_backtracks: u32,
    #[serde(default)]
    pub route_path_alternatives: u32,
    #[serde(default)]
    pub route_repairs: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_blocking_owners: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_budget_exhausted: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSearchEvidence {
    pub schema_version: u32,
    pub max_decisions: u32,
    pub max_backtracks: u32,
    pub max_chain_expansions_per_section: u32,
    pub max_room_origin_alternatives: u32,
    pub max_room_rotation_alternatives: u32,
    pub decisions: u32,
    pub backtracks: u32,
    pub chain_expansions: u32,
    pub room_origin_attempts: u32,
    pub room_rotation_attempts: u32,
    pub selected: Vec<CatalogPlacementDecision>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPlacementDecision {
    pub piece_id: String,
    pub shape_id: String,
    pub transform: String,
    pub candidate_rank: u32,
    pub candidate_count: u32,
    pub origin: GridCell,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_bounds: Option<CatalogGridBounds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_constraint: Option<CatalogLaneConstraint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGridBounds {
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogLaneConstraint {
    pub source_hint: String,
    pub from: GridCell,
    pub to: GridCell,
    pub envelope_cells: i32,
    pub bounds: CatalogGridBounds,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PieceInstance {
    pub instance_id: String,
    pub piece_id: String,
    pub requirement_kind: String,
    pub role: String,
    pub shape_id: String,
    pub transform: String,
    pub origin: GridCell,
    pub occupied_cells: Vec<GridCell>,
    pub reserved_cells: Vec<GridCell>,
    pub exit_map: Vec<MatchedExit>,
    pub feature_placements: Vec<MatchedSocket>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scene_placements: Vec<ScenePlacement>,
    pub source_requirement_ref: String,
    pub source_refs: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GluedExit {
    pub id: String,
    pub link_id: String,
    pub from_instance: String,
    pub from_exit: String,
    pub from_cell: GridCell,
    pub from_direction: String,
    pub from_width: i32,
    pub to_instance: String,
    pub to_exit: String,
    pub to_cell: GridCell,
    pub to_direction: String,
    pub to_width: i32,
    pub source_section: String,
    pub source_corridor: String,
    pub source_edge: String,
    pub source_edges: Vec<String>,
    pub traversal_refs: Vec<PhysicalTraversalRef>,
    pub source_ref: String,
    pub traversal: String,
    pub required_item: Option<String>,
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_points: Vec<GeometryPoint>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatePortal {
    pub id: String,
    pub source_section: String,
    pub source_edge: String,
    pub source_edges: Vec<String>,
    pub traversal_refs: Vec<PhysicalTraversalRef>,
    pub source_corridor: String,
    pub link_id: String,
    pub from_piece: String,
    pub from_instance: String,
    pub to_piece: String,
    pub to_instance: String,
    pub cells: Vec<GridCell>,
    pub orientation: String,
    pub width: i32,
    pub traversal: String,
    pub required_item: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltFlowValidationReport {
    pub kind: String,
    pub schema_version: u32,
    pub validation_id: String,
    pub candidate_id: String,
    pub geometry_id: String,
    pub plan_id: String,
    pub placement_id: String,
    pub candidate_ref: String,
    pub geometry_ref: String,
    pub piece_plan_ref: String,
    pub piece_placement_ref: String,
    pub walkable_projection: BuiltWalkableProjection,
    pub progression: Vec<BuiltFlowProgressionStep>,
    pub portal_count: usize,
    pub ok: bool,
    pub fatal_count: usize,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltWalkableProjection {
    pub source: String,
    pub cell_count: usize,
    pub projection_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltFlowProgressionStep {
    pub step: usize,
    pub items: Vec<String>,
    pub reachable_nodes: Vec<String>,
    pub reachable_edges: Vec<String>,
    pub open_portals: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacementCellRef {
    pub instance_id: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DanglingExit {
    pub instance_id: String,
    pub exit_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedConnector {
    pub source_connector: String,
    pub reason: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HtmlPreviewArtifact {
    pub kind: String,
    pub schema_version: u32,
    pub preview_id: String,
    pub geometry_ref: String,
    pub validation_ref: String,
    pub html_ref: String,
    pub screenshot_hint: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ShapeCatalog {
    pub kind: String,
    pub schema_version: u32,
    pub catalog_id: String,
    pub cell_size: i32,
    #[serde(default)]
    pub placement_policy: PiecePlacementPolicy,
    #[serde(default)]
    pub catalog_search_policy: CatalogSearchPolicy,
    pub shapes: Vec<CatalogShape>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSearchPolicy {
    pub schema_version: u32,
    pub max_decisions: u32,
    pub max_backtracks: u32,
    pub max_chain_expansions_per_section: u32,
    pub max_room_origin_alternatives: u32,
    pub max_room_rotation_alternatives: u32,
}

impl Default for CatalogSearchPolicy {
    fn default() -> Self {
        Self {
            schema_version: 1,
            max_decisions: 50_000,
            max_backtracks: 10_000,
            max_chain_expansions_per_section: 4_096,
            max_room_origin_alternatives: 8,
            max_room_rotation_alternatives: 4,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiecePlacementPolicy {
    pub schema_version: u32,
    pub minimum_clearance_cells: i32,
    pub contact_policy: PieceContactPolicy,
    pub wall_thickness_cells: i32,
    pub doorway_width_cells: i32,
    pub preserve_piece_boundaries: bool,
}

impl Default for PiecePlacementPolicy {
    fn default() -> Self {
        Self {
            schema_version: 1,
            minimum_clearance_cells: 3,
            contact_policy: PieceContactPolicy::GluedExitsOnly,
            wall_thickness_cells: 1,
            doorway_width_cells: 1,
            preserve_piece_boundaries: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PieceContactPolicy {
    #[default]
    GluedExitsOnly,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct CatalogShape {
    pub shape_id: String,
    pub label: String,
    pub piece_kinds: Vec<String>,
    pub footprint: Vec<GridCell>,
    #[serde(default)]
    pub reserved_cells: Vec<GridCell>,
    pub exits: Vec<CatalogExit>,
    pub allowed_transforms: Vec<String>,
    #[serde(default)]
    pub feature_sockets: Vec<FeatureSocket>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scene_sockets: Vec<SceneSocket>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct GridCell {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct CatalogExit {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub direction: String,
    pub width: i32,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct FeatureSocket {
    pub id: String,
    pub kind: String,
    pub x: i32,
    pub y: i32,
    pub tags: Vec<String>,
}

/// Asset-agnostic scene content attached to a catalog shape.
///
/// Procgen owns the deterministic placement of this symbolic content. A
/// downstream product resolves `content_id` values and light presentation;
/// this contract never contains asset URLs or renderer handles.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SceneSocket {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub facing: SceneFacing,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub content: SceneSocketContent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SceneSocketContent {
    Prop {
        content_id: String,
    },
    PointLight {
        color_rgb: String,
        intensity_milli: u32,
        range_cells: u32,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneFacing {
    North,
    East,
    South,
    West,
}

impl SceneFacing {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::North => "north",
            Self::East => "east",
            Self::South => "south",
            Self::West => "west",
        }
    }
}

/// Absolute, transformed scene placement emitted for one piece instance.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScenePlacement {
    pub id: String,
    pub instance_id: String,
    pub source_socket_id: String,
    pub x: i32,
    pub y: i32,
    pub facing: SceneFacing,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub content: SceneSocketContent,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogInspectionReport {
    pub kind: String,
    pub schema_version: u32,
    pub catalog_id: String,
    pub catalog_ref: String,
    pub shape_count: usize,
    pub placement_policy: PiecePlacementPolicy,
    pub catalog_search_policy: CatalogSearchPolicy,
    pub piece_kinds: Vec<String>,
    pub feature_sockets: Vec<String>,
    #[serde(default)]
    pub scene_socket_kinds: Vec<String>,
    pub exit_directions: Vec<String>,
    pub transforms: Vec<String>,
    pub shapes: Vec<CatalogShapeSummary>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogShapeSummary {
    pub shape_id: String,
    pub piece_kinds: Vec<String>,
    pub footprint_cells: usize,
    pub reserved_cells: usize,
    pub exit_count: usize,
    pub feature_socket_kinds: Vec<String>,
    #[serde(default)]
    pub scene_socket_count: usize,
    pub allowed_transforms: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedArtifact {
    pub kind: String,
    pub schema_version: u32,
    pub artifact_id: String,
    pub candidate_hash: String,
    pub layout_hash: String,
    pub validation_ref: String,
    pub score_ref: String,
    pub candidate: Candidate,
    pub layout: LayoutArtifact,
    pub score_summary: ScoreReport,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionReport {
    pub kind: String,
    pub schema_version: u32,
    pub batch_id: String,
    pub profile_id: String,
    pub profile_ref: String,
    pub seed: u64,
    pub requested_count: usize,
    pub generated_count: usize,
    pub accepted: Vec<SelectionEntry>,
    pub rejected: Vec<SelectionRejection>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionEntry {
    pub candidate_id: String,
    pub profile_sequence: String,
    pub topology_fingerprint: String,
    pub duplicate_of: Option<String>,
    pub budget_checks: Vec<BudgetCheck>,
    pub budget_penalty: f64,
    pub selection_score: f64,
    pub artifact_ref: String,
    pub validation_ref: String,
    pub score_ref: String,
    pub layout_ref: String,
    pub analysis_ref: String,
    pub compatible_rules_ref: String,
    pub spatial_intent_ref: String,
    pub intermediate_breakdown_ref: String,
    pub intermediate_validation_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical_connection_plan_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry_validation_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html_preview_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape_catalog_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_inspection_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piece_plan_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape_match_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piece_placement_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piece_placement_validation_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub built_flow_validation_ref: Option<String>,
    pub overall: f64,
    pub metrics: BTreeMap<String, f64>,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionRejection {
    pub candidate_id: String,
    pub profile_sequence: String,
    pub candidate_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical_connection_plan_ref: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchProfile {
    pub kind: String,
    pub schema_version: u32,
    pub profile_id: String,
    pub description: String,
    pub budgets: Option<IntentBudget>,
    pub sequences: Vec<BatchProfileSequence>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetCheck {
    pub code: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Clone, Debug)]
pub struct IntermediateArtifactRefs {
    pub analysis_ref: String,
    pub compatible_rules_ref: String,
    pub spatial_intent_ref: String,
    pub intermediate_breakdown_ref: String,
    pub intermediate_validation_ref: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchProfileSequence {
    pub label: String,
    pub rules: Vec<GraphRule>,
}
