#[allow(unused_imports)]
use crate::*;

pub(crate) fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Init(args) => init_candidate(args),
        Command::Graph(command) => match command.command {
            GraphSubcommand::ApplyRule(args) => apply_rule(args),
            GraphSubcommand::CompatibleRules(args) => compatible_rules_command(args),
            GraphSubcommand::Fork(args) => fork_command(args),
            GraphSubcommand::Rules(args) => graph_rules_command(args),
            GraphSubcommand::Summarize(args) => summarize_candidate(args),
        },
        Command::Analyze(command) => match command.command {
            AnalyzeSubcommand::Graph(args) => analyze_graph_command(args),
        },
        Command::Annotate(command) => match command.command {
            AnnotateSubcommand::SpatialIntent(args) => annotate_spatial_intent_command(args),
        },
        Command::Breakdown(command) => match command.command {
            BreakdownSubcommand::Emit(args) => breakdown_emit_command(args),
            BreakdownSubcommand::Validate(args) => breakdown_validate_command(args),
        },
        Command::Geometry(command) => match command.command {
            GeometrySubcommand::PlanConnections(args) => physical_connection_plan_command(args),
            GeometrySubcommand::Emit2d(args) => geometry_emit_2d_command(args),
            GeometrySubcommand::Validate2d(args) => geometry_validate_2d_command(args),
        },
        Command::Build(command) => match command.command {
            BuildSubcommand::Catalog(command) => match command.command {
                BuildCatalogSubcommand::Inspect(args) => build_catalog_inspect_command(args),
            },
            BuildSubcommand::RealizeCatalogAware(args) => build_realize_catalog_aware_command(args),
            BuildSubcommand::EmitPiecePlan(args) => build_emit_piece_plan_command(args),
            BuildSubcommand::MatchShapes(args) => build_match_shapes_command(args),
            BuildSubcommand::Assemble(args) => build_assemble_command(args),
            BuildSubcommand::ValidatePlacement(args) => build_validate_placement_command(args),
            BuildSubcommand::ValidateFlow(args) => build_validate_flow_command(args),
        },
        Command::Preview(command) => match command.command {
            PreviewSubcommand::Html(args) => preview_html_command(args),
        },
        Command::Validate(command) => match command.command {
            ValidateSubcommand::Graph(args) => validate_graph_command(args),
        },
        Command::Repair(command) => match command.command {
            RepairSubcommand::Apply(args) => repair_apply_command(args),
            RepairSubcommand::Suggest(args) => repair_suggest_command(args),
        },
        Command::Score(command) => match command.command {
            ScoreSubcommand::Graph(args) => score_graph_command(args),
        },
        Command::Embed(command) => match command.command {
            EmbedSubcommand::TwoD(args) => embed_2d_command(args),
        },
        Command::Accept(args) => accept_command(args),
        Command::Baseline(args) => baseline_command(args),
        Command::Batch(command) => match command.command {
            BatchSubcommand::Generate(args) => batch_generate_command(args),
        },
    }
}
