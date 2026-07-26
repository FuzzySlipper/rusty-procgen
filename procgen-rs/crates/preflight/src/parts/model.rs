#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SeedIntent {
    kind: String,
    id: String,
    title: String,
    target_dimension: String,
    desired_patterns: Vec<String>,
    notes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    kind: String,
    schema_version: u32,
    candidate_id: String,
    seed: u64,
    dimension_model: String,
    source_intent: Option<String>,
    provenance: Vec<ProvenanceStep>,
    graph: IntentGraph,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProvenanceStep {
    step: u32,
    command: String,
    seed: Option<u64>,
    summary: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct IntentGraph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Node {
    id: String,
    kind: NodeKind,
    label: String,
    tags: Vec<String>,
    grants_item: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum NodeKind {
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
    fn as_str(self) -> &'static str {
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
struct Edge {
    id: String,
    from: String,
    to: String,
    kind: EdgeKind,
    traversal: TraversalKind,
    required_item: Option<String>,
    tags: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum EdgeKind {
    CriticalPath,
    KeyBranch,
    OptionalBranch,
    Shortcut,
    SecretBypass,
}

impl EdgeKind {
    fn as_str(self) -> &'static str {
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
enum TraversalKind {
    Open,
    Locked,
    OneWayReturn,
    Hidden,
}

impl TraversalKind {
    fn as_str(self) -> &'static str {
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
struct Receipt {
    kind: String,
    schema_version: u32,
    command: String,
    status: String,
    seed: Option<u64>,
    input_hash: Option<String>,
    output_hash: Option<String>,
    output_ref: Option<String>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Diagnostic {
    code: String,
    severity: Severity,
    node: Option<String>,
    edge: Option<String>,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    repair_hint: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Severity {
    Info,
    Warning,
    Fatal,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidationReport {
    kind: String,
    schema_version: u32,
    state_hash: String,
    ok: bool,
    fatal_count: usize,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScoreReport {
    kind: String,
    schema_version: u32,
    state_hash: String,
    overall: f64,
    metrics: BTreeMap<String, f64>,
    notes: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuleMetadataReport {
    kind: String,
    schema_version: u32,
    rules: Vec<RuleMetadata>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuleMetadata {
    id: String,
    intent: String,
    required_patterns: Vec<String>,
    duplicate_markers: Vec<String>,
    emitted_node_tags: Vec<String>,
    emitted_edge_tags: Vec<String>,
    compatibility_hints: Vec<String>,
    repair_hints: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphSummaryReport {
    kind: String,
    schema_version: u32,
    candidate_id: String,
    state_hash: String,
    validation_ok: bool,
    fatal_count: usize,
    score_overall: f64,
    metrics: BTreeMap<String, f64>,
    node_count: usize,
    edge_count: usize,
    tags: Vec<String>,
    locked_items: Vec<String>,
    dead_ends: Vec<String>,
    provenance_tail: Vec<ProvenanceStep>,
    nodes: Vec<NodeSummary>,
    edges: Vec<EdgeSummary>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RepairReport {
    kind: String,
    schema_version: u32,
    candidate_id: String,
    state_hash: String,
    validation_ok: bool,
    fatal_count: usize,
    suggestions: Vec<RepairSuggestion>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RepairSuggestion {
    code: String,
    severity: Severity,
    node: Option<String>,
    edge: Option<String>,
    detail: String,
    repair_hint: Option<String>,
    suggested_actions: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphAnalysisReport {
    kind: String,
    schema_version: u32,
    candidate_id: String,
    state_hash: String,
    critical_path: Vec<String>,
    dominators: Vec<String>,
    optional_branches: Vec<BranchAnalysis>,
    lock_key_order: Vec<LockKeyAnalysis>,
    loop_signals: Vec<LoopSignal>,
    shortcut_bypass_risks: Vec<ShortcutRisk>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BranchAnalysis {
    edge_id: String,
    from: String,
    to: String,
    classification: String,
    rejoins_goal_route: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LockKeyAnalysis {
    edge_id: String,
    required_item: String,
    provider_node: Option<String>,
    provider_reachable_before_lock: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoopSignal {
    edge_id: String,
    signal: String,
    detail: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShortcutRisk {
    edge_id: String,
    risk: String,
    detail: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuleCompatibilityReport {
    kind: String,
    schema_version: u32,
    candidate_id: String,
    state_hash: String,
    rules: Vec<RuleCompatibility>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuleCompatibility {
    rule: String,
    status: String,
    reasons: Vec<String>,
    recommended_actions: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpatialIntentReport {
    kind: String,
    schema_version: u32,
    candidate_id: String,
    state_hash: String,
    analysis_ref: Option<String>,
    annotations: Vec<SpatialIntentAnnotation>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpatialIntentAnnotation {
    target_type: String,
    target_id: String,
    intents: Vec<String>,
    rationale: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct IntermediateBreakdown {
    kind: String,
    schema_version: u32,
    candidate_id: String,
    state_hash: String,
    annotation_ref: String,
    regions: Vec<IntermediateRegion>,
    connectors: Vec<IntermediateConnector>,
    constraints: Vec<IntermediateConstraint>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct IntermediateRegion {
    id: String,
    node_ids: Vec<String>,
    role: String,
    anchor_node: Option<String>,
    #[serde(default)]
    geometry_role: String,
    #[serde(default)]
    footprint_class: String,
    #[serde(default)]
    scale_band: String,
    #[serde(default)]
    anchor_quality: String,
    #[serde(default)]
    entrance_expectations: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct IntermediateConnector {
    id: String,
    edge_id: String,
    from_region: String,
    to_region: String,
    intents: Vec<String>,
    #[serde(default)]
    affordances: Vec<String>,
    #[serde(default)]
    traversal_hint: String,
    #[serde(default)]
    constraint_refs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct IntermediateConstraint {
    code: String,
    target: String,
    #[serde(default)]
    target_type: String,
    #[serde(default)]
    source_intents: Vec<String>,
    #[serde(default)]
    graph_refs: Vec<String>,
    detail: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicalConnectionPlan {
    kind: String,
    schema_version: u32,
    plan_id: String,
    candidate_id: String,
    source_candidate_ref: String,
    source_intermediate_ref: String,
    sections: Vec<PhysicalConnectionSection>,
    edge_mappings: Vec<PhysicalEdgeMapping>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicalConnectionSection {
    id: String,
    topology: String,
    terminal_regions: Vec<String>,
    source_connectors: Vec<String>,
    source_edges: Vec<String>,
    traversal_refs: Vec<PhysicalTraversalRef>,
    width: i32,
    semantic_tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicalTraversalRef {
    connector_id: String,
    edge_id: String,
    from_region: String,
    to_region: String,
    traversal: String,
    required_item: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicalEdgeMapping {
    edge_id: String,
    connector_id: String,
    section_id: String,
    from_region: String,
    to_region: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeSummary {
    id: String,
    kind: NodeKind,
    tags: Vec<String>,
    grants_item: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct EdgeSummary {
    id: String,
    from: String,
    to: String,
    kind: EdgeKind,
    traversal: TraversalKind,
    required_item: Option<String>,
    tags: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LayoutArtifact {
    kind: String,
    schema_version: u32,
    layout_id: String,
    candidate_id: String,
    seed: u64,
    rooms: Vec<LayoutRoom>,
    links: Vec<LayoutLink>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LayoutRoom {
    node_id: String,
    kind: NodeKind,
    label: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LayoutLink {
    edge_id: String,
    from_node: String,
    to_node: String,
    kind: EdgeKind,
    traversal: TraversalKind,
    required_item: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Geometry2dArtifact {
    kind: String,
    schema_version: u32,
    geometry_id: String,
    candidate_id: String,
    seed: u64,
    source_candidate_ref: String,
    source_intermediate_ref: String,
    source_connection_plan_ref: String,
    connection_plan_id: String,
    layout_policy: GeometryLayoutPolicy,
    layout_search: GeometryLayoutSearchEvidence,
    bounds: GeometryBounds,
    rooms: Vec<GeometryRoom>,
    corridors: Vec<GeometryCorridor>,
    contents: Vec<GeometryContent>,
    skipped_connectors: Vec<SkippedConnector>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometryLayoutPolicy {
    kind: String,
    schema_version: u32,
    initial_room_margin: i32,
    initial_column_gap: i32,
    initial_row_gap: i32,
    room_margin_growth: i32,
    column_gap_growth: i32,
    row_gap_growth: i32,
    max_spacing_tiers: u32,
    room_order_attempts_per_tier: u32,
    max_search_attempts: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometrySpacing {
    room_margin: i32,
    column_gap: i32,
    row_gap: i32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometryLayoutSearchEvidence {
    spacing_tier: u32,
    room_order_attempt: u32,
    port_order_attempt: u32,
    route_order_attempt: u32,
    search_attempts: u32,
    effective_spacing: GeometrySpacing,
    #[serde(default = "default_geometry_embedding_kind")]
    embedding_kind: String,
    #[serde(default)]
    embedding_id: String,
    #[serde(default)]
    embedding_faces: u32,
    #[serde(default)]
    embedding_target_faces: u32,
    #[serde(default)]
    embedding_search_steps: u32,
    #[serde(default)]
    route_decisions: u32,
    #[serde(default)]
    route_backtracks: u32,
    #[serde(default)]
    route_path_alternatives: u32,
    #[serde(default)]
    route_repairs: u32,
    #[serde(default)]
    route_grid_expansions: u32,
    #[serde(default)]
    route_path_expansion_exhaustions: u32,
    #[serde(default)]
    route_last_failed_section: String,
    #[serde(default)]
    route_blocking_owners: Vec<String>,
}

fn default_geometry_embedding_kind() -> String {
    "depth_columns".to_owned()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometryBounds {
    width: i32,
    height: i32,
    grid: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometryRoom {
    id: String,
    source_region: String,
    source_nodes: Vec<String>,
    role: String,
    geometry_role: String,
    footprint_class: String,
    rect: GeometryRect,
    ports: Vec<GeometryRoomPort>,
    style_tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometryRoomPort {
    id: String,
    section_id: String,
    side: String,
    point: GeometryPoint,
    width: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometryRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometryCorridor {
    id: String,
    physical_section: String,
    source_connector: String,
    source_edge: String,
    source_connectors: Vec<String>,
    source_edges: Vec<String>,
    traversal_refs: Vec<PhysicalTraversalRef>,
    from_room: String,
    to_room: String,
    traversal_hint: String,
    semantic_tags: Vec<String>,
    width: i32,
    from_port: String,
    to_port: String,
    points: Vec<GeometryPoint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometryPoint {
    x: i32,
    y: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeometryContent {
    id: String,
    room_id: String,
    source_ref: String,
    kind: String,
    label: String,
    tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PieceBuildPlan {
    kind: String,
    schema_version: u32,
    plan_id: String,
    candidate_id: String,
    geometry_id: String,
    #[serde(default)]
    corridor_realization: CorridorRealization,
    source_candidate_ref: String,
    source_intermediate_ref: String,
    source_geometry_ref: String,
    requirements: Vec<PieceRequirement>,
    links: Vec<PieceLink>,
    content_requirements: Vec<PieceContentRequirement>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PieceRequirement {
    piece_id: String,
    kind: String,
    role: String,
    source_refs: Vec<String>,
    required_exits: Vec<PieceExitRequirement>,
    required_sockets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    required_shape_tags: Vec<String>,
    tags: Vec<String>,
    placement_hints: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PieceExitRequirement {
    id: String,
    direction: String,
    width: i32,
    tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PieceLink {
    id: String,
    from_piece: String,
    from_exit: String,
    to_piece: String,
    to_exit: String,
    source_section: String,
    source_corridor: String,
    source_edge: String,
    source_edges: Vec<String>,
    traversal_refs: Vec<PhysicalTraversalRef>,
    source_ref: String,
    traversal: String,
    required_item: Option<String>,
    tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    route_points: Vec<GeometryPoint>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PieceContentRequirement {
    id: String,
    piece_id: String,
    source_ref: String,
    kind: String,
    label: String,
    required_socket: String,
    tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PieceShapeMatchReport {
    kind: String,
    schema_version: u32,
    match_id: String,
    plan_id: String,
    catalog_id: String,
    seed: u64,
    alternative_attempt: u32,
    source_plan_ref: String,
    source_catalog_ref: String,
    ok: bool,
    unmatched_count: usize,
    matches: Vec<MatchedPiece>,
    rejections: Vec<ShapeMatchRejection>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MatchedPiece {
    piece_id: String,
    requirement_kind: String,
    shape_id: String,
    transform: String,
    score: i32,
    candidate_rank: usize,
    candidate_count: usize,
    source_requirement_ref: String,
    exit_map: Vec<MatchedExit>,
    socket_map: Vec<MatchedSocket>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MatchedExit {
    requirement_exit_id: String,
    catalog_exit_id: String,
    x: i32,
    y: i32,
    direction: String,
    width: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MatchedSocket {
    required_socket: String,
    catalog_socket_id: String,
    kind: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShapeMatchRejection {
    piece_id: String,
    shape_id: String,
    transform: Option<String>,
    reasons: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PiecePlacement {
    kind: String,
    schema_version: u32,
    placement_id: String,
    plan_id: String,
    catalog_id: String,
    match_id: String,
    #[serde(default)]
    corridor_realization: CorridorRealization,
    source_plan_ref: String,
    source_catalog_ref: String,
    source_match_ref: String,
    cell_size: i32,
    grid_connectivity: GridConnectivity,
    placement_policy: PiecePlacementPolicy,
    realization_search: PieceRealizationSearchEvidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    catalog_search: Option<CatalogSearchEvidence>,
    instances: Vec<PieceInstance>,
    glued_exits: Vec<GluedExit>,
    gate_portals: Vec<GatePortal>,
    occupied_cells: Vec<PlacementCellRef>,
    connection_cells: Vec<PlacementCellRef>,
    reserved_cells: Vec<PlacementCellRef>,
    dangling_exits: Vec<DanglingExit>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PieceRealizationSearchEvidence {
    realization_scale_tier: u32,
    realization_attempts: u32,
    route_order_attempt: u32,
    route_attempts: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogSearchEvidence {
    schema_version: u32,
    max_decisions: u32,
    max_backtracks: u32,
    max_chain_expansions_per_section: u32,
    max_room_origin_alternatives: u32,
    max_room_rotation_alternatives: u32,
    decisions: u32,
    backtracks: u32,
    chain_expansions: u32,
    room_origin_attempts: u32,
    room_rotation_attempts: u32,
    selected: Vec<CatalogPlacementDecision>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogPlacementDecision {
    piece_id: String,
    shape_id: String,
    transform: String,
    candidate_rank: u32,
    candidate_count: u32,
    origin: GridCell,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin_bounds: Option<CatalogGridBounds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lane_constraint: Option<CatalogLaneConstraint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogGridBounds {
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogLaneConstraint {
    source_hint: String,
    from: GridCell,
    to: GridCell,
    envelope_cells: i32,
    bounds: CatalogGridBounds,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PieceInstance {
    instance_id: String,
    piece_id: String,
    requirement_kind: String,
    role: String,
    shape_id: String,
    transform: String,
    origin: GridCell,
    occupied_cells: Vec<GridCell>,
    reserved_cells: Vec<GridCell>,
    exit_map: Vec<MatchedExit>,
    feature_placements: Vec<MatchedSocket>,
    source_requirement_ref: String,
    source_refs: Vec<String>,
    tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GluedExit {
    id: String,
    link_id: String,
    from_instance: String,
    from_exit: String,
    from_cell: GridCell,
    from_direction: String,
    from_width: i32,
    to_instance: String,
    to_exit: String,
    to_cell: GridCell,
    to_direction: String,
    to_width: i32,
    source_section: String,
    source_corridor: String,
    source_edge: String,
    source_edges: Vec<String>,
    traversal_refs: Vec<PhysicalTraversalRef>,
    source_ref: String,
    traversal: String,
    required_item: Option<String>,
    tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    route_points: Vec<GeometryPoint>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GatePortal {
    id: String,
    source_section: String,
    source_edge: String,
    source_edges: Vec<String>,
    traversal_refs: Vec<PhysicalTraversalRef>,
    source_corridor: String,
    link_id: String,
    from_piece: String,
    from_instance: String,
    to_piece: String,
    to_instance: String,
    cells: Vec<GridCell>,
    orientation: String,
    width: i32,
    traversal: String,
    required_item: Option<String>,
    provenance: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuiltFlowValidationReport {
    kind: String,
    schema_version: u32,
    validation_id: String,
    candidate_id: String,
    geometry_id: String,
    plan_id: String,
    placement_id: String,
    candidate_ref: String,
    geometry_ref: String,
    piece_plan_ref: String,
    piece_placement_ref: String,
    walkable_projection: BuiltWalkableProjection,
    progression: Vec<BuiltFlowProgressionStep>,
    portal_count: usize,
    ok: bool,
    fatal_count: usize,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuiltWalkableProjection {
    source: String,
    cell_count: usize,
    projection_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuiltFlowProgressionStep {
    step: usize,
    items: Vec<String>,
    reachable_nodes: Vec<String>,
    reachable_edges: Vec<String>,
    open_portals: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlacementCellRef {
    instance_id: String,
    x: i32,
    y: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DanglingExit {
    instance_id: String,
    exit_id: String,
    reason: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkippedConnector {
    source_connector: String,
    reason: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct HtmlPreviewArtifact {
    kind: String,
    schema_version: u32,
    preview_id: String,
    geometry_ref: String,
    validation_ref: String,
    html_ref: String,
    screenshot_hint: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct ShapeCatalog {
    kind: String,
    schema_version: u32,
    catalog_id: String,
    cell_size: i32,
    #[serde(default)]
    placement_policy: PiecePlacementPolicy,
    #[serde(default)]
    catalog_search_policy: CatalogSearchPolicy,
    shapes: Vec<CatalogShape>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogSearchPolicy {
    schema_version: u32,
    max_decisions: u32,
    max_backtracks: u32,
    max_chain_expansions_per_section: u32,
    max_room_origin_alternatives: u32,
    max_room_rotation_alternatives: u32,
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
struct PiecePlacementPolicy {
    schema_version: u32,
    minimum_clearance_cells: i32,
    contact_policy: PieceContactPolicy,
    wall_thickness_cells: i32,
    doorway_width_cells: i32,
    preserve_piece_boundaries: bool,
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
enum PieceContactPolicy {
    #[default]
    GluedExitsOnly,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct CatalogShape {
    shape_id: String,
    label: String,
    piece_kinds: Vec<String>,
    footprint: Vec<GridCell>,
    #[serde(default)]
    reserved_cells: Vec<GridCell>,
    exits: Vec<CatalogExit>,
    allowed_transforms: Vec<String>,
    #[serde(default)]
    feature_sockets: Vec<FeatureSocket>,
    tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct GridCell {
    x: i32,
    y: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct CatalogExit {
    id: String,
    x: i32,
    y: i32,
    direction: String,
    width: i32,
    tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct FeatureSocket {
    id: String,
    kind: String,
    x: i32,
    y: i32,
    tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogInspectionReport {
    kind: String,
    schema_version: u32,
    catalog_id: String,
    catalog_ref: String,
    shape_count: usize,
    placement_policy: PiecePlacementPolicy,
    catalog_search_policy: CatalogSearchPolicy,
    piece_kinds: Vec<String>,
    feature_sockets: Vec<String>,
    exit_directions: Vec<String>,
    transforms: Vec<String>,
    shapes: Vec<CatalogShapeSummary>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogShapeSummary {
    shape_id: String,
    piece_kinds: Vec<String>,
    footprint_cells: usize,
    reserved_cells: usize,
    exit_count: usize,
    feature_socket_kinds: Vec<String>,
    allowed_transforms: Vec<String>,
    tags: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AcceptedArtifact {
    kind: String,
    schema_version: u32,
    artifact_id: String,
    candidate_hash: String,
    layout_hash: String,
    validation_ref: String,
    score_ref: String,
    candidate: Candidate,
    layout: LayoutArtifact,
    score_summary: ScoreReport,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectionReport {
    kind: String,
    schema_version: u32,
    batch_id: String,
    profile_id: String,
    profile_ref: String,
    seed: u64,
    requested_count: usize,
    generated_count: usize,
    accepted: Vec<SelectionEntry>,
    rejected: Vec<SelectionRejection>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectionEntry {
    candidate_id: String,
    profile_sequence: String,
    topology_fingerprint: String,
    duplicate_of: Option<String>,
    budget_checks: Vec<BudgetCheck>,
    budget_penalty: f64,
    selection_score: f64,
    artifact_ref: String,
    validation_ref: String,
    score_ref: String,
    layout_ref: String,
    analysis_ref: String,
    compatible_rules_ref: String,
    spatial_intent_ref: String,
    intermediate_breakdown_ref: String,
    intermediate_validation_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    physical_connection_plan_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    geometry_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    geometry_validation_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    html_preview_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    html_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shape_catalog_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    catalog_inspection_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    piece_plan_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shape_match_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    piece_placement_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    piece_placement_validation_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    built_flow_validation_ref: Option<String>,
    overall: f64,
    metrics: BTreeMap<String, f64>,
    tags: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectionRejection {
    candidate_id: String,
    profile_sequence: String,
    candidate_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    physical_connection_plan_ref: Option<String>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchProfile {
    kind: String,
    schema_version: u32,
    profile_id: String,
    description: String,
    budgets: Option<IntentBudget>,
    sequences: Vec<BatchProfileSequence>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BudgetCheck {
    code: String,
    ok: bool,
    detail: String,
}

#[derive(Clone, Debug)]
struct IntermediateArtifactRefs {
    analysis_ref: String,
    compatible_rules_ref: String,
    spatial_intent_ref: String,
    intermediate_breakdown_ref: String,
    intermediate_validation_ref: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchProfileSequence {
    label: String,
    rules: Vec<GraphRule>,
}
