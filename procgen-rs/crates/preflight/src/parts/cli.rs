
#[derive(Parser)]
#[command(name = "asha-procgen")]
#[command(about = "Deterministic dungeon procgen CLI workbench")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check sibling ASHA engine checkout posture.
    Preflight(PreflightArgs),
    /// Create a minimal candidate from a seed intent.
    Init(InitArgs),
    /// Mutate or summarize intent graphs.
    Graph(GraphCommand),
    /// Analyze graph topology.
    Analyze(AnalyzeCommand),
    /// Add pre-geometry annotations.
    Annotate(AnnotateCommand),
    /// Emit or validate intermediate layout breakdowns.
    Breakdown(BreakdownCommand),
    /// Emit or validate concrete geometry artifacts.
    Geometry(GeometryCommand),
    /// Expand geometry into explicit catalog-piece requirements.
    Build(BuildCommand),
    /// Render generated artifacts into standalone previews.
    Preview(PreviewCommand),
    /// Validate candidates.
    Validate(ValidateCommand),
    /// Suggest repair actions for invalid or warning-heavy candidates.
    Repair(RepairCommand),
    /// Score candidates.
    Score(ScoreCommand),
    /// Embed candidates into inspectable layouts.
    Embed(EmbedCommand),
    /// Accept a validated candidate/layout as an artifact.
    Accept(AcceptArgs),
    /// Produce the first deterministic sample run.
    Baseline(BaselineArgs),
    /// Generate a deterministic batch and selection report.
    Batch(BatchCommand),
}

#[derive(Args)]
struct PreflightArgs {
    #[arg(default_value = ".")]
    repo_root: PathBuf,
}

#[derive(Args)]
struct InitArgs {
    #[arg(long)]
    intent: PathBuf,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    receipt: PathBuf,
    #[arg(long)]
    transcript: Option<PathBuf>,
}

#[derive(Args)]
struct GraphCommand {
    #[command(subcommand)]
    command: GraphSubcommand,
}

#[derive(Subcommand)]
enum GraphSubcommand {
    ApplyRule(ApplyRuleArgs),
    CompatibleRules(ReportOutArgs),
    Fork(ForkArgs),
    Rules(RuleMetadataArgs),
    Summarize(SummarizeArgs),
}

#[derive(Args)]
struct ApplyRuleArgs {
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    rule: GraphRule,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    receipt: PathBuf,
    #[arg(long)]
    transcript: Option<PathBuf>,
}

#[derive(Args)]
struct ForkArgs {
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    label: String,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    receipt: PathBuf,
    #[arg(long)]
    transcript: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[value(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
enum GraphRule {
    LockKeyLoop,
    OptionalTreasureDetour,
    OneWayShortcut,
    SecretBypass,
    HubSpokeCluster,
    NestedLockKeyChain,
    HazardResourceTradeoff,
    BossPreparationLoop,
    GatedTreasureBranch,
    BranchMergeShortcut,
}

impl GraphRule {
    fn as_str(self) -> &'static str {
        match self {
            GraphRule::LockKeyLoop => "lock_key_loop",
            GraphRule::OptionalTreasureDetour => "optional_treasure_detour",
            GraphRule::OneWayShortcut => "one_way_shortcut",
            GraphRule::SecretBypass => "secret_bypass",
            GraphRule::HubSpokeCluster => "hub_spoke_cluster",
            GraphRule::NestedLockKeyChain => "nested_lock_key_chain",
            GraphRule::HazardResourceTradeoff => "hazard_resource_tradeoff",
            GraphRule::BossPreparationLoop => "boss_preparation_loop",
            GraphRule::GatedTreasureBranch => "gated_treasure_branch",
            GraphRule::BranchMergeShortcut => "branch_merge_shortcut",
        }
    }
}

#[derive(Args)]
struct ValidateCommand {
    #[command(subcommand)]
    command: ValidateSubcommand,
}

#[derive(Subcommand)]
enum ValidateSubcommand {
    Graph(ReportOutArgs),
}

#[derive(Args)]
struct RepairCommand {
    #[command(subcommand)]
    command: RepairSubcommand,
}

#[derive(Subcommand)]
enum RepairSubcommand {
    Apply(RepairApplyArgs),
    Suggest(ReportOutArgs),
}

#[derive(Args)]
struct RepairApplyArgs {
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    action: RepairAction,
    #[arg(long)]
    target: Option<String>,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    receipt: PathBuf,
    #[arg(long)]
    transcript: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[value(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
enum RepairAction {
    AddRejoinEdge,
    RemoveOrphanNode,
}

impl RepairAction {
    fn as_str(self) -> &'static str {
        match self {
            RepairAction::AddRejoinEdge => "add_rejoin_edge",
            RepairAction::RemoveOrphanNode => "remove_orphan_node",
        }
    }
}

#[derive(Args)]
struct AnalyzeCommand {
    #[command(subcommand)]
    command: AnalyzeSubcommand,
}

#[derive(Subcommand)]
enum AnalyzeSubcommand {
    Graph(ReportOutArgs),
}

#[derive(Args)]
struct AnnotateCommand {
    #[command(subcommand)]
    command: AnnotateSubcommand,
}

#[derive(Subcommand)]
enum AnnotateSubcommand {
    SpatialIntent(AnnotateSpatialIntentArgs),
}

#[derive(Args)]
struct AnnotateSpatialIntentArgs {
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    analysis: Option<PathBuf>,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Args)]
struct BreakdownCommand {
    #[command(subcommand)]
    command: BreakdownSubcommand,
}

#[derive(Subcommand)]
enum BreakdownSubcommand {
    Emit(BreakdownEmitArgs),
    Validate(ReportOutArgs),
}

#[derive(Args)]
struct BreakdownEmitArgs {
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    annotations: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Args)]
struct GeometryCommand {
    #[command(subcommand)]
    command: GeometrySubcommand,
}

#[derive(Subcommand)]
enum GeometrySubcommand {
    #[command(name = "plan-connections")]
    PlanConnections(PhysicalConnectionPlanArgs),
    #[command(name = "emit-2d")]
    Emit2d(GeometryEmit2dArgs),
    #[command(name = "validate-2d")]
    Validate2d(ReportOutArgs),
}

#[derive(Args)]
struct PhysicalConnectionPlanArgs {
    #[arg(long)]
    candidate: PathBuf,
    #[arg(long)]
    intermediate: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Args)]
struct GeometryEmit2dArgs {
    #[arg(long)]
    candidate: PathBuf,
    #[arg(long)]
    intermediate: PathBuf,
    #[arg(long = "connection-plan")]
    connection_plan: PathBuf,
    #[arg(long = "layout-policy")]
    layout_policy: Option<PathBuf>,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Args)]
struct BuildCommand {
    #[command(subcommand)]
    command: BuildSubcommand,
}

#[derive(Subcommand)]
enum BuildSubcommand {
    Catalog(BuildCatalogCommand),
    #[command(name = "emit-piece-plan")]
    EmitPiecePlan(BuildEmitPiecePlanArgs),
    #[command(name = "match-shapes")]
    MatchShapes(BuildMatchShapesArgs),
    Assemble(BuildAssembleArgs),
    #[command(name = "validate-placement")]
    ValidatePlacement(ReportOutArgs),
    #[command(name = "validate-flow")]
    ValidateFlow(BuildValidateFlowArgs),
}

#[derive(Args)]
struct BuildCatalogCommand {
    #[command(subcommand)]
    command: BuildCatalogSubcommand,
}

#[derive(Subcommand)]
enum BuildCatalogSubcommand {
    Inspect(BuildCatalogInspectArgs),
}

#[derive(Args)]
struct BuildCatalogInspectArgs {
    #[arg(long)]
    catalog: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Args)]
struct BuildEmitPiecePlanArgs {
    #[arg(long)]
    candidate: PathBuf,
    #[arg(long)]
    intermediate: PathBuf,
    #[arg(long)]
    geometry: PathBuf,
    #[arg(long, value_enum, default_value_t = CorridorRealization::Hybrid)]
    corridor_realization: CorridorRealization,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[value(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
enum CorridorRealization {
    Catalog,
    #[default]
    Hybrid,
    Procedural,
}

impl CorridorRealization {
    fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::Hybrid => "hybrid",
            Self::Procedural => "procedural",
        }
    }

    fn uses_catalog_pieces(self) -> bool {
        matches!(self, Self::Catalog | Self::Hybrid)
    }
}

#[derive(Args)]
struct BuildMatchShapesArgs {
    #[arg(long)]
    catalog: PathBuf,
    #[arg(long = "piece-plan")]
    piece_plan: PathBuf,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Args)]
struct BuildAssembleArgs {
    #[arg(long)]
    catalog: PathBuf,
    #[arg(long = "piece-plan")]
    piece_plan: PathBuf,
    #[arg(long = "shape-match")]
    shape_match: PathBuf,
    #[arg(long, value_enum, default_value_t = GridConnectivity::FourWay)]
    connectivity: GridConnectivity,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Args)]
struct BuildValidateFlowArgs {
    #[arg(long)]
    candidate: PathBuf,
    #[arg(long)]
    geometry: PathBuf,
    #[arg(long = "piece-plan")]
    piece_plan: PathBuf,
    #[arg(long = "piece-placement")]
    piece_placement: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
enum GridConnectivity {
    FourWay,
    EightWay,
}

#[derive(Args)]
struct PreviewCommand {
    #[command(subcommand)]
    command: PreviewSubcommand,
}

#[derive(Subcommand)]
enum PreviewSubcommand {
    Html(PreviewHtmlArgs),
}

#[derive(Args)]
struct PreviewHtmlArgs {
    #[arg(long)]
    geometry: PathBuf,
    #[arg(long)]
    validation: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    allow_invalid: bool,
}

#[derive(Args)]
struct ScoreCommand {
    #[command(subcommand)]
    command: ScoreSubcommand,
}

#[derive(Subcommand)]
enum ScoreSubcommand {
    Graph(ReportOutArgs),
}

#[derive(Args)]
struct EmbedCommand {
    #[command(subcommand)]
    command: EmbedSubcommand,
}

#[derive(Subcommand)]
enum EmbedSubcommand {
    #[command(name = "2d")]
    TwoD(Embed2dArgs),
}

#[derive(Args)]
struct StateArg {
    #[arg(long)]
    state: PathBuf,
}

#[derive(Args)]
struct RuleMetadataArgs {
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct SummarizeArgs {
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct ReportOutArgs {
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

#[derive(Args)]
struct Embed2dArgs {
    #[arg(long)]
    state: PathBuf,
    #[arg(long)]
    seed: u64,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    receipt: PathBuf,
    #[arg(long)]
    transcript: Option<PathBuf>,
}

#[derive(Args)]
struct AcceptArgs {
    #[arg(long)]
    candidate: PathBuf,
    #[arg(long)]
    layout: PathBuf,
    #[arg(long)]
    validation: PathBuf,
    #[arg(long)]
    score: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    receipt: PathBuf,
    #[arg(long)]
    transcript: Option<PathBuf>,
}

#[derive(Args)]
struct BaselineArgs {
    #[arg(long)]
    out_dir: PathBuf,
    #[arg(long, default_value_t = 4103)]
    seed: u64,
}

#[derive(Args)]
struct BatchCommand {
    #[command(subcommand)]
    command: BatchSubcommand,
}

#[derive(Subcommand)]
enum BatchSubcommand {
    Generate(BatchGenerateArgs),
}

#[derive(Args)]
struct BatchGenerateArgs {
    #[arg(long)]
    out_dir: PathBuf,
    #[arg(long)]
    profile: Option<PathBuf>,
    #[arg(long, default_value_t = 5201)]
    seed: u64,
    #[arg(long, default_value_t = 10)]
    count: usize,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct IntentBudget {
    max_locked_edges: Option<usize>,
    min_optional_branches: Option<usize>,
    require_hub: Option<bool>,
    require_boss: Option<bool>,
    max_dead_ends: Option<usize>,
}

const DEFAULT_BATCH_PROFILE: &str = "fixtures/batch-profiles/v2-sample.json";
#[allow(dead_code)]
const DEFAULT_SHAPE_CATALOG: &str = "fixtures/shape-catalogs/2d-basic.json";
