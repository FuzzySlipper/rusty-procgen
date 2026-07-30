#[allow(unused_imports)]
use crate::*;

#[derive(Parser)]
#[command(name = "rusty-procgen")]
#[command(about = "Deterministic dungeon procgen CLI workbench")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
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
pub(crate) struct InitArgs {
    #[arg(long)]
    pub(crate) intent: PathBuf,
    #[arg(long)]
    pub(crate) seed: u64,
    #[arg(long)]
    pub(crate) out: PathBuf,
    #[arg(long)]
    pub(crate) receipt: PathBuf,
    #[arg(long)]
    pub(crate) transcript: Option<PathBuf>,
}

#[derive(Args)]
pub(crate) struct GraphCommand {
    #[command(subcommand)]
    pub(crate) command: GraphSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum GraphSubcommand {
    ApplyRule(ApplyRuleArgs),
    CompatibleRules(ReportOutArgs),
    Fork(ForkArgs),
    Rules(RuleMetadataArgs),
    Summarize(SummarizeArgs),
}

#[derive(Args)]
pub(crate) struct ApplyRuleArgs {
    #[arg(long)]
    pub(crate) state: PathBuf,
    #[arg(long)]
    pub(crate) rule: GraphRule,
    #[arg(long)]
    pub(crate) seed: u64,
    #[arg(long)]
    pub(crate) out: PathBuf,
    #[arg(long)]
    pub(crate) receipt: PathBuf,
    #[arg(long)]
    pub(crate) transcript: Option<PathBuf>,
}

#[derive(Args)]
pub(crate) struct ForkArgs {
    #[arg(long)]
    pub(crate) state: PathBuf,
    #[arg(long)]
    pub(crate) label: String,
    #[arg(long)]
    pub(crate) seed: u64,
    #[arg(long)]
    pub(crate) out: PathBuf,
    #[arg(long)]
    pub(crate) receipt: PathBuf,
    #[arg(long)]
    pub(crate) transcript: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[value(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum GraphRule {
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
    pub fn as_str(self) -> &'static str {
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
pub(crate) struct ValidateCommand {
    #[command(subcommand)]
    pub(crate) command: ValidateSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum ValidateSubcommand {
    Graph(ReportOutArgs),
}

#[derive(Args)]
pub(crate) struct RepairCommand {
    #[command(subcommand)]
    pub(crate) command: RepairSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum RepairSubcommand {
    Apply(RepairApplyArgs),
    Suggest(ReportOutArgs),
}

#[derive(Args)]
pub(crate) struct RepairApplyArgs {
    #[arg(long)]
    pub(crate) state: PathBuf,
    #[arg(long)]
    pub(crate) action: RepairAction,
    #[arg(long)]
    pub(crate) target: Option<String>,
    #[arg(long)]
    pub(crate) seed: u64,
    #[arg(long)]
    pub(crate) out: PathBuf,
    #[arg(long)]
    pub(crate) receipt: PathBuf,
    #[arg(long)]
    pub(crate) transcript: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[value(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RepairAction {
    AddRejoinEdge,
    RemoveOrphanNode,
}

impl RepairAction {
    pub fn as_str(self) -> &'static str {
        match self {
            RepairAction::AddRejoinEdge => "add_rejoin_edge",
            RepairAction::RemoveOrphanNode => "remove_orphan_node",
        }
    }
}

#[derive(Args)]
pub(crate) struct AnalyzeCommand {
    #[command(subcommand)]
    pub(crate) command: AnalyzeSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum AnalyzeSubcommand {
    Graph(ReportOutArgs),
}

#[derive(Args)]
pub(crate) struct AnnotateCommand {
    #[command(subcommand)]
    pub(crate) command: AnnotateSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum AnnotateSubcommand {
    SpatialIntent(AnnotateSpatialIntentArgs),
}

#[derive(Args)]
pub(crate) struct AnnotateSpatialIntentArgs {
    #[arg(long)]
    pub(crate) state: PathBuf,
    #[arg(long)]
    pub(crate) analysis: Option<PathBuf>,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Args)]
pub(crate) struct BreakdownCommand {
    #[command(subcommand)]
    pub(crate) command: BreakdownSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum BreakdownSubcommand {
    Emit(BreakdownEmitArgs),
    Validate(ReportOutArgs),
}

#[derive(Args)]
pub(crate) struct BreakdownEmitArgs {
    #[arg(long)]
    pub(crate) state: PathBuf,
    #[arg(long)]
    pub(crate) annotations: PathBuf,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Args)]
pub(crate) struct GeometryCommand {
    #[command(subcommand)]
    pub(crate) command: GeometrySubcommand,
}

#[derive(Subcommand)]
pub(crate) enum GeometrySubcommand {
    #[command(name = "plan-connections")]
    PlanConnections(PhysicalConnectionPlanArgs),
    #[command(name = "emit-2d")]
    Emit2d(GeometryEmit2dArgs),
    #[command(name = "validate-2d")]
    Validate2d(ReportOutArgs),
}

#[derive(Args)]
pub(crate) struct PhysicalConnectionPlanArgs {
    #[arg(long)]
    pub(crate) candidate: PathBuf,
    #[arg(long)]
    pub(crate) intermediate: PathBuf,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Args)]
pub(crate) struct GeometryEmit2dArgs {
    #[arg(long)]
    pub(crate) candidate: PathBuf,
    #[arg(long)]
    pub(crate) intermediate: PathBuf,
    #[arg(long = "connection-plan")]
    pub(crate) connection_plan: PathBuf,
    #[arg(long = "layout-policy")]
    pub(crate) layout_policy: Option<PathBuf>,
    #[arg(long)]
    pub(crate) seed: u64,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Args)]
pub(crate) struct BuildCommand {
    #[command(subcommand)]
    pub(crate) command: BuildSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum BuildSubcommand {
    Catalog(BuildCatalogCommand),
    #[command(name = "realize-catalog-aware")]
    RealizeCatalogAware(BuildRealizeCatalogAwareArgs),
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
pub(crate) struct BuildRealizeCatalogAwareArgs {
    #[arg(long)]
    pub(crate) candidate: PathBuf,
    #[arg(long)]
    pub(crate) geometry: PathBuf,
    #[arg(long = "piece-plan")]
    pub(crate) piece_plan: PathBuf,
    #[arg(long)]
    pub(crate) catalog: PathBuf,
    #[arg(long)]
    pub(crate) policy: PathBuf,
    #[arg(long)]
    pub(crate) seed: u64,
    #[arg(long)]
    pub(crate) out: PathBuf,
    #[arg(long = "trace-out")]
    pub(crate) trace_out: Option<PathBuf>,
    #[arg(long = "trace-max-events", default_value_t = DEFAULT_CATALOG_TRACE_MAX_EVENTS)]
    pub(crate) trace_max_events: u32,
    #[arg(
        long = "trace-max-event-body-bytes",
        default_value_t = DEFAULT_CATALOG_TRACE_MAX_EVENT_BODY_BYTES
    )]
    pub(crate) trace_max_event_body_bytes: u64,
    #[arg(
        long = "trace-max-visual-cells",
        default_value_t = DEFAULT_CATALOG_TRACE_MAX_VISUAL_CELLS
    )]
    pub(crate) trace_max_visual_cells: u64,
}

#[derive(Args)]
pub(crate) struct BuildCatalogCommand {
    #[command(subcommand)]
    pub(crate) command: BuildCatalogSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum BuildCatalogSubcommand {
    Inspect(BuildCatalogInspectArgs),
}

#[derive(Args)]
pub(crate) struct BuildCatalogInspectArgs {
    #[arg(long)]
    pub(crate) catalog: PathBuf,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Args)]
pub(crate) struct BuildEmitPiecePlanArgs {
    #[arg(long)]
    pub(crate) candidate: PathBuf,
    #[arg(long)]
    pub(crate) intermediate: PathBuf,
    #[arg(long)]
    pub(crate) geometry: PathBuf,
    #[arg(long, value_enum, default_value_t = CorridorRealization::Hybrid)]
    pub(crate) corridor_realization: CorridorRealization,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[value(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum CorridorRealization {
    Catalog,
    #[default]
    Hybrid,
    Procedural,
}

impl CorridorRealization {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::Hybrid => "hybrid",
            Self::Procedural => "procedural",
        }
    }

    pub fn uses_catalog_pieces(self) -> bool {
        matches!(self, Self::Catalog | Self::Hybrid)
    }
}

#[derive(Args)]
pub(crate) struct BuildMatchShapesArgs {
    #[arg(long)]
    pub(crate) catalog: PathBuf,
    #[arg(long = "piece-plan")]
    pub(crate) piece_plan: PathBuf,
    #[arg(long)]
    pub(crate) seed: u64,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Args)]
pub(crate) struct BuildAssembleArgs {
    #[arg(long)]
    pub(crate) catalog: PathBuf,
    #[arg(long = "piece-plan")]
    pub(crate) piece_plan: PathBuf,
    #[arg(long = "shape-match")]
    pub(crate) shape_match: PathBuf,
    #[arg(long, value_enum, default_value_t = GridConnectivity::FourWay)]
    pub(crate) connectivity: GridConnectivity,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Args)]
pub(crate) struct BuildValidateFlowArgs {
    #[arg(long)]
    pub(crate) candidate: PathBuf,
    #[arg(long)]
    pub(crate) geometry: PathBuf,
    #[arg(long = "piece-plan")]
    pub(crate) piece_plan: PathBuf,
    #[arg(long = "piece-placement")]
    pub(crate) piece_placement: PathBuf,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum GridConnectivity {
    FourWay,
    EightWay,
}

#[derive(Args)]
pub(crate) struct PreviewCommand {
    #[command(subcommand)]
    pub(crate) command: PreviewSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum PreviewSubcommand {
    Html(PreviewHtmlArgs),
}

#[derive(Args)]
pub(crate) struct PreviewHtmlArgs {
    #[arg(long)]
    pub(crate) geometry: PathBuf,
    #[arg(long)]
    pub(crate) validation: PathBuf,
    #[arg(long)]
    pub(crate) out: PathBuf,
    #[arg(long)]
    pub(crate) allow_invalid: bool,
}

#[derive(Args)]
pub(crate) struct ScoreCommand {
    #[command(subcommand)]
    pub(crate) command: ScoreSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum ScoreSubcommand {
    Graph(ReportOutArgs),
}

#[derive(Args)]
pub(crate) struct EmbedCommand {
    #[command(subcommand)]
    pub(crate) command: EmbedSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum EmbedSubcommand {
    #[command(name = "2d")]
    TwoD(Embed2dArgs),
}

#[derive(Args)]
pub(crate) struct StateArg {
    #[arg(long)]
    pub(crate) state: PathBuf,
}

#[derive(Args)]
pub(crate) struct RuleMetadataArgs {
    #[arg(long)]
    pub(crate) out: Option<PathBuf>,
}

#[derive(Args)]
pub(crate) struct SummarizeArgs {
    #[arg(long)]
    pub(crate) state: PathBuf,
    #[arg(long)]
    pub(crate) json: bool,
    #[arg(long)]
    pub(crate) out: Option<PathBuf>,
}

#[derive(Args)]
pub(crate) struct ReportOutArgs {
    #[arg(long)]
    pub(crate) state: PathBuf,
    #[arg(long)]
    pub(crate) out: PathBuf,
}

#[derive(Args)]
pub(crate) struct Embed2dArgs {
    #[arg(long)]
    pub(crate) state: PathBuf,
    #[arg(long)]
    pub(crate) seed: u64,
    #[arg(long)]
    pub(crate) out: PathBuf,
    #[arg(long)]
    pub(crate) receipt: PathBuf,
    #[arg(long)]
    pub(crate) transcript: Option<PathBuf>,
}

#[derive(Args)]
pub(crate) struct AcceptArgs {
    #[arg(long)]
    pub(crate) candidate: PathBuf,
    #[arg(long)]
    pub(crate) layout: PathBuf,
    #[arg(long)]
    pub(crate) validation: PathBuf,
    #[arg(long)]
    pub(crate) score: PathBuf,
    #[arg(long)]
    pub(crate) out: PathBuf,
    #[arg(long)]
    pub(crate) receipt: PathBuf,
    #[arg(long)]
    pub(crate) transcript: Option<PathBuf>,
}

#[derive(Args)]
pub(crate) struct BaselineArgs {
    #[arg(long)]
    pub(crate) out_dir: PathBuf,
    #[arg(long, default_value_t = 4103)]
    pub(crate) seed: u64,
}

#[derive(Args)]
pub(crate) struct BatchCommand {
    #[command(subcommand)]
    pub(crate) command: BatchSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum BatchSubcommand {
    Generate(BatchGenerateArgs),
}

#[derive(Args)]
pub(crate) struct BatchGenerateArgs {
    #[arg(long)]
    pub(crate) out_dir: PathBuf,
    #[arg(long)]
    pub(crate) profile: Option<PathBuf>,
    #[arg(long, default_value_t = 5201)]
    pub(crate) seed: u64,
    #[arg(long, default_value_t = 10)]
    pub(crate) count: usize,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntentBudget {
    pub max_locked_edges: Option<usize>,
    pub min_optional_branches: Option<usize>,
    pub require_hub: Option<bool>,
    pub require_boss: Option<bool>,
    pub max_dead_ends: Option<usize>,
}

pub(crate) const DEFAULT_BATCH_PROFILE: &str = "fixtures/batch-profiles/v2-sample.json";
#[allow(dead_code)]
pub(crate) const DEFAULT_SHAPE_CATALOG: &str = "fixtures/shape-catalogs/2d-basic.json";
