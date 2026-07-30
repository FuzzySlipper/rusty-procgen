#[allow(unused_imports)]
use crate::*;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAwareGenerationPolicy {
    pub kind: String,
    pub schema_version: u32,
    pub max_generation_attempts: u32,
    pub initial_room_compaction_cells: i32,
    pub room_compaction_growth_cells: i32,
    pub max_room_candidates: u32,
    pub max_routing_states_per_section: u32,
    pub route_margin_cells: i32,
    pub guide_distance_weight: u32,
    pub turn_penalty: u32,
    pub outcome_constraints: CatalogAwareOutcomeConstraints,
    pub outcome_preferences: CatalogAwareOutcomePreferences,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAwareOutcomeConstraints {
    pub max_placement_width_cells: u64,
    pub max_placement_height_cells: u64,
    pub max_placement_area_cells: u64,
    pub max_routed_catalog_cells: u64,
}

impl Default for CatalogAwareOutcomeConstraints {
    fn default() -> Self {
        Self {
            max_placement_width_cells: 4_096,
            max_placement_height_cells: 4_096,
            max_placement_area_cells: 16_777_216,
            max_routed_catalog_cells: 1_048_576,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogAwareOutcomeMetric {
    PlacementSpan,
    PlacementArea,
    RoutedCatalogCells,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAwareOutcomePreferences {
    pub primary_metric: CatalogAwareOutcomeMetric,
    pub preferred_maximum: u64,
}

impl Default for CatalogAwareOutcomePreferences {
    fn default() -> Self {
        Self {
            primary_metric: CatalogAwareOutcomeMetric::PlacementSpan,
            preferred_maximum: 286,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAwareOutcomeMetrics {
    pub placement_width_cells: u64,
    pub placement_height_cells: u64,
    pub placement_span_cells: u64,
    pub placement_area_cells: u64,
    pub routed_catalog_cells: u64,
    pub route_bends: u64,
    pub routing_states: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAwareConstraintMiss {
    pub metric: String,
    pub actual: u64,
    pub limit: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAwareOutcomeComparison {
    pub incumbent_attempt: Option<u32>,
    pub ordering: String,
    pub decisive_metric: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAwareOutcomeEvaluation {
    pub metrics: CatalogAwareOutcomeMetrics,
    pub constraint_misses: Vec<CatalogAwareConstraintMiss>,
    pub admissible: bool,
    pub preference_satisfied: bool,
    pub comparison: CatalogAwareOutcomeComparison,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAwareAttemptEvidence {
    pub attempt: u32,
    pub room_compaction_cells: i32,
    pub classification: String,
    pub stage: String,
    pub detail: String,
    pub rooms_placed: usize,
    pub sections_routed: usize,
    pub routing_states: u32,
    pub outcome: Option<CatalogAwareOutcomeEvaluation>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAwareGenerationResult {
    pub kind: String,
    pub schema_version: u32,
    pub ok: bool,
    pub candidate_id: String,
    pub policy: CatalogAwareGenerationPolicy,
    pub attempts: Vec<CatalogAwareAttemptEvidence>,
    pub selected_attempt: Option<u32>,
    pub exhausted_classification: Option<String>,
    pub geometry: Option<Geometry2dArtifact>,
    pub geometry_validation: Option<ValidationReport>,
    pub piece_plan: Option<PieceBuildPlan>,
    pub shape_match: Option<PieceShapeMatchReport>,
    pub placement: Option<PiecePlacement>,
    pub placement_validation: Option<ValidationReport>,
    pub built_flow_validation: Option<BuiltFlowValidationReport>,
}

/// Stable artifact labels used by the filesystem-free catalog-aware runner.
///
/// The labels are copied into provenance fields only. The runner never opens
/// or writes the referenced paths.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogAwareGenerationProvenance {
    pub candidate_ref: String,
    pub geometry_ref: String,
    pub piece_plan_ref: String,
    pub catalog_ref: String,
    pub result_ref: String,
}

/// Borrowed inputs for one deterministic catalog-aware generation run.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CatalogAwareGenerationInput<'a> {
    pub candidate: &'a Candidate,
    pub source_geometry: &'a Geometry2dArtifact,
    pub source_plan: &'a PieceBuildPlan,
    pub catalog: &'a ShapeCatalog,
    pub policy: &'a CatalogAwareGenerationPolicy,
    pub provenance: &'a CatalogAwareGenerationProvenance,
    pub seed: u64,
}

/// Complete accepted value produced by one catalog-aware attempt.
#[derive(Debug)]
pub(crate) struct CatalogAwareAttemptOutcome {
    pub(crate) geometry: Geometry2dArtifact,
    pub(crate) geometry_validation: ValidationReport,
    pub(crate) piece_plan: PieceBuildPlan,
    pub(crate) shape_match: PieceShapeMatchReport,
    pub(crate) placement: PiecePlacement,
    pub(crate) placement_validation: ValidationReport,
    pub(crate) built_flow_validation: BuiltFlowValidationReport,
    pub(crate) routing_states: u32,
    pub(crate) routed_catalog_cells: u64,
    pub(crate) route_bends: u64,
}

#[derive(Clone)]
pub(crate) struct CatalogRoomSelection {
    requirement: PieceRequirement,
    matched: MatchedPiece,
    shape: CatalogShape,
    origin: GridCell,
}

#[derive(Clone)]
pub(crate) struct CatalogSectionTerminal {
    requirement: PieceRequirement,
    exit_id: String,
}

#[derive(Clone)]
pub(crate) struct CatalogSectionSpec {
    section: String,
    source_corridor: String,
    template_link: PieceLink,
    left: CatalogSectionTerminal,
    right: CatalogSectionTerminal,
    guide: Vec<GridCell>,
}

#[derive(Clone)]
pub(crate) struct CatalogRoutedSection {
    spec: CatalogSectionSpec,
    cells: Vec<GridCell>,
}

pub(crate) enum CatalogRouteSearch {
    Found {
        cells: Vec<GridCell>,
        states_visited: u32,
    },
    NoPath {
        states_visited: u32,
    },
    BudgetExhausted {
        states_visited: u32,
    },
}

pub(crate) fn build_realize_catalog_aware_command(
    args: BuildRealizeCatalogAwareArgs,
) -> Result<(), String> {
    let candidate = read_flow_candidate(&args.candidate)?;
    let geometry: Geometry2dArtifact = read_json(&args.geometry)?;
    let source_plan: PieceBuildPlan = read_json(&args.piece_plan)?;
    let catalog: ShapeCatalog = read_json(&args.catalog)?;
    let policy: CatalogAwareGenerationPolicy = read_json(&args.policy)?;
    let provenance = CatalogAwareGenerationProvenance {
        candidate_ref: display_path(&args.candidate),
        geometry_ref: display_path(&args.geometry),
        piece_plan_ref: display_path(&args.piece_plan),
        catalog_ref: display_path(&args.catalog),
        result_ref: display_path(&args.out),
    };
    let input = CatalogAwareGenerationInput {
        candidate: &candidate,
        source_geometry: &geometry,
        source_plan: &source_plan,
        catalog: &catalog,
        policy: &policy,
        provenance: &provenance,
        seed: args.seed,
    };
    if let Some(trace_out) = args.trace_out.as_deref() {
        if trace_out == args.out {
            return Err("--trace-out must differ from --out".to_owned());
        }
        let run = run_catalog_aware_generation_traced(
            input,
            CatalogGenerationTraceLimits {
                max_events: args.trace_max_events,
                max_event_body_bytes: args.trace_max_event_body_bytes,
                max_visual_cells: args.trace_max_visual_cells,
            },
        )
        .map_err(|error| error.to_string())?;
        write_json_pair_atomic(&args.out, &run.result, trace_out, &run.trace)
    } else {
        let result = run_catalog_aware_generation(input)?;
        write_json(&args.out, &result)
    }
}

/// Run bounded catalog-aware generation without filesystem access.
pub(crate) fn run_catalog_aware_generation(
    input: CatalogAwareGenerationInput<'_>,
) -> Result<CatalogAwareGenerationResult, String> {
    let mut recorder = CatalogGenerationTraceRecorder::disabled();
    run_catalog_aware_generation_recording(input, &mut recorder).map_err(|error| match error {
        CatalogAwareRunError::Generation(detail) => detail,
        CatalogAwareRunError::Trace(error) => error.to_string(),
    })
}

pub(crate) fn run_catalog_aware_generation_traced(
    input: CatalogAwareGenerationInput<'_>,
    limits: CatalogGenerationTraceLimits,
) -> Result<CatalogAwareGenerationRun, CatalogGenerationTraceError> {
    let run = record_catalog_aware_generation_trace(input, limits)?;
    replay_catalog_generation_trace(
        &run.trace,
        &run.result,
        CatalogGenerationTraceRequest {
            candidate: input.candidate,
            source_geometry: input.source_geometry,
            source_plan: input.source_plan,
            catalog: input.catalog,
            generation_policy: input.policy,
            provenance: input.provenance,
            seed: input.seed,
            trace_limits: run.trace.limits.clone(),
        },
    )?;
    Ok(run)
}

pub(crate) fn record_catalog_aware_generation_trace(
    input: CatalogAwareGenerationInput<'_>,
    limits: CatalogGenerationTraceLimits,
) -> Result<CatalogAwareGenerationRun, CatalogGenerationTraceError> {
    let mut recorder = CatalogGenerationTraceRecorder::new(input, limits)?;
    let result =
        run_catalog_aware_generation_recording(input, &mut recorder).map_err(
            |error| match error {
                CatalogAwareRunError::Generation(detail) => CatalogGenerationTraceError::new(
                    "catalog_generation_rejected",
                    detail,
                    None,
                    None,
                ),
                CatalogAwareRunError::Trace(error) => error,
            },
        )?;
    let trace = recorder.finish(&result)?;
    Ok(CatalogAwareGenerationRun { result, trace })
}

enum CatalogAwareRunError {
    Generation(String),
    Trace(CatalogGenerationTraceError),
}

fn run_catalog_aware_generation_recording(
    input: CatalogAwareGenerationInput<'_>,
    recorder: &mut CatalogGenerationTraceRecorder,
) -> Result<CatalogAwareGenerationResult, CatalogAwareRunError> {
    validate_catalog_aware_policy(input.policy).map_err(CatalogAwareRunError::Generation)?;
    if input.source_plan.corridor_realization != CorridorRealization::Catalog {
        return Err(CatalogAwareRunError::Generation(
            "catalog-aware generation requires a catalog piece plan".to_owned(),
        ));
    }
    let mut result = CatalogAwareGenerationResult {
        kind: "rusty_procgen.catalog_aware_generation.v2".to_owned(),
        schema_version: 2,
        ok: false,
        candidate_id: input.candidate.candidate_id.clone(),
        policy: input.policy.clone(),
        attempts: Vec::new(),
        selected_attempt: None,
        exhausted_classification: None,
        geometry: None,
        geometry_validation: None,
        piece_plan: None,
        shape_match: None,
        placement: None,
        placement_validation: None,
        built_flow_validation: None,
    };
    let mut final_classification = "generation_infeasibility".to_owned();
    let mut constraint_rejected = false;
    let mut best: Option<(u32, CatalogAwareAttemptOutcome, CatalogAwareOutcomeMetrics)> = None;
    for attempt in 0..input.policy.max_generation_attempts {
        let compaction = input.policy.initial_room_compaction_cells.saturating_add(
            input
                .policy
                .room_compaction_growth_cells
                .saturating_mul(i32::try_from(attempt).unwrap_or(i32::MAX)),
        );
        trace_record_or_error(
            recorder,
            Some(attempt),
            CatalogGenerationTraceEventBody::AttemptStarted {
                room_compaction_cells: compaction,
            },
        )
        .map_err(CatalogAwareRunError::Trace)?;
        let attempt_result = realize_catalog_aware_attempt(input, attempt, compaction, recorder);
        if let Some(error) = recorder.error() {
            return Err(CatalogAwareRunError::Trace(error));
        }
        match attempt_result {
            Ok(outcome) => {
                let metrics = catalog_outcome_metrics(&outcome).map_err(|detail| {
                    CatalogAwareRunError::Generation(format!(
                        "catalog outcome metric calculation failed: {detail}"
                    ))
                })?;
                let constraint_misses =
                    catalog_outcome_constraint_misses(&metrics, &input.policy.outcome_constraints);
                let admissible = constraint_misses.is_empty();
                let incumbent_attempt = best.as_ref().map(|(attempt, _, _)| *attempt);
                let (ordering, decisive_metric, replaces_incumbent) = if !admissible {
                    constraint_rejected = true;
                    (
                        "constraint_miss".to_owned(),
                        constraint_misses.first().map_or_else(
                            || "outcome_constraints".to_owned(),
                            |miss| miss.metric.clone(),
                        ),
                        false,
                    )
                } else if let Some((_, _, incumbent_metrics)) = best.as_ref() {
                    let (ordering, decisive_metric) = catalog_outcome_ordering(
                        &metrics,
                        incumbent_metrics,
                        input.policy.outcome_preferences.primary_metric,
                    );
                    (
                        if ordering == Ordering::Less {
                            "new_incumbent".to_owned()
                        } else {
                            "incumbent_retained".to_owned()
                        },
                        decisive_metric.to_owned(),
                        ordering == Ordering::Less,
                    )
                } else {
                    (
                        "first_admissible".to_owned(),
                        catalog_primary_metric_name(
                            input.policy.outcome_preferences.primary_metric,
                        )
                        .to_owned(),
                        true,
                    )
                };
                let evaluation = CatalogAwareOutcomeEvaluation {
                    metrics: metrics.clone(),
                    constraint_misses,
                    admissible,
                    preference_satisfied: admissible
                        && catalog_outcome_metric_value(
                            &metrics,
                            catalog_primary_metric_name(
                                input.policy.outcome_preferences.primary_metric,
                            ),
                        ) <= input.policy.outcome_preferences.preferred_maximum,
                    comparison: CatalogAwareOutcomeComparison {
                        incumbent_attempt,
                        ordering,
                        decisive_metric,
                    },
                };
                trace_record_or_error(
                    recorder,
                    Some(attempt),
                    CatalogGenerationTraceEventBody::OutcomeEvaluated {
                        evaluation: evaluation.clone(),
                    },
                )
                .map_err(CatalogAwareRunError::Trace)?;
                let evidence = CatalogAwareAttemptEvidence {
                    attempt,
                    room_compaction_cells: compaction,
                    classification: if admissible {
                        "admissible".to_owned()
                    } else {
                        "outcome_constraint_miss".to_owned()
                    },
                    stage: if admissible {
                        "outcome_selection".to_owned()
                    } else {
                        "outcome_constraints".to_owned()
                    },
                    detail: if admissible {
                        format!(
                            "Validated catalog outcome was compared by {}.",
                            catalog_primary_metric_name(
                                input.policy.outcome_preferences.primary_metric,
                            ),
                        )
                    } else {
                        format!(
                            "Validated catalog outcome exceeded {} hard constraint(s).",
                            evaluation.constraint_misses.len(),
                        )
                    },
                    rooms_placed: outcome
                        .placement
                        .instances
                        .iter()
                        .filter(|instance| is_catalog_room_kind(instance.requirement_kind.as_str()))
                        .count(),
                    sections_routed: outcome
                        .piece_plan
                        .links
                        .iter()
                        .map(|link| link.source_section.as_str())
                        .collect::<BTreeSet<_>>()
                        .len(),
                    routing_states: outcome.routing_states,
                    outcome: Some(evaluation),
                };
                trace_record_or_error(
                    recorder,
                    Some(attempt),
                    CatalogGenerationTraceEventBody::AttemptFinished {
                        classification: evidence.classification.clone(),
                        stage: evidence.stage.clone(),
                        detail: evidence.detail.clone(),
                        rooms_placed: evidence.rooms_placed,
                        sections_routed: evidence.sections_routed,
                        routing_states: evidence.routing_states,
                    },
                )
                .map_err(CatalogAwareRunError::Trace)?;
                result.attempts.push(evidence);
                if replaces_incumbent {
                    best = Some((attempt, outcome, metrics));
                }
                if result
                    .attempts
                    .last()
                    .and_then(|attempt| attempt.outcome.as_ref())
                    .is_some_and(|evaluation| evaluation.preference_satisfied)
                {
                    break;
                }
            }
            Err(failure) => {
                final_classification = failure.classification.clone();
                let evidence = CatalogAwareAttemptEvidence {
                    attempt,
                    room_compaction_cells: compaction,
                    classification: failure.classification,
                    stage: failure.stage,
                    detail: failure.detail,
                    rooms_placed: failure.rooms_placed,
                    sections_routed: failure.sections_routed,
                    routing_states: failure.routing_states,
                    outcome: None,
                };
                trace_record_or_error(
                    recorder,
                    Some(attempt),
                    CatalogGenerationTraceEventBody::AttemptFinished {
                        classification: evidence.classification.clone(),
                        stage: evidence.stage.clone(),
                        detail: evidence.detail.clone(),
                        rooms_placed: evidence.rooms_placed,
                        sections_routed: evidence.sections_routed,
                        routing_states: evidence.routing_states,
                    },
                )
                .map_err(CatalogAwareRunError::Trace)?;
                result.attempts.push(evidence);
            }
        }
    }
    if let Some((attempt, outcome, _)) = best {
        result.ok = true;
        result.selected_attempt = Some(attempt);
        result.geometry = Some(outcome.geometry);
        result.geometry_validation = Some(outcome.geometry_validation);
        result.piece_plan = Some(outcome.piece_plan);
        result.shape_match = Some(outcome.shape_match);
        result.placement = Some(outcome.placement);
        result.placement_validation = Some(outcome.placement_validation);
        result.built_flow_validation = Some(outcome.built_flow_validation);
    } else {
        result.exhausted_classification = Some(if constraint_rejected {
            "outcome_constraint_miss".to_owned()
        } else {
            final_classification
        });
    }
    Ok(result)
}

fn catalog_outcome_metrics(
    outcome: &CatalogAwareAttemptOutcome,
) -> Result<CatalogAwareOutcomeMetrics, String> {
    let (placement_width_cells, placement_height_cells) =
        if outcome.placement.occupied_cells.is_empty() {
            (0, 0)
        } else {
            let min_x = outcome
                .placement
                .occupied_cells
                .iter()
                .map(|cell| cell.x)
                .min()
                .ok_or_else(|| "placement has no minimum x".to_owned())?;
            let max_x = outcome
                .placement
                .occupied_cells
                .iter()
                .map(|cell| cell.x)
                .max()
                .ok_or_else(|| "placement has no maximum x".to_owned())?;
            let min_y = outcome
                .placement
                .occupied_cells
                .iter()
                .map(|cell| cell.y)
                .min()
                .ok_or_else(|| "placement has no minimum y".to_owned())?;
            let max_y = outcome
                .placement
                .occupied_cells
                .iter()
                .map(|cell| cell.y)
                .max()
                .ok_or_else(|| "placement has no maximum y".to_owned())?;
            (
                u64::try_from(i64::from(max_x) - i64::from(min_x) + 1)
                    .map_err(|_| "placement width is negative".to_owned())?,
                u64::try_from(i64::from(max_y) - i64::from(min_y) + 1)
                    .map_err(|_| "placement height is negative".to_owned())?,
            )
        };
    catalog_outcome_metrics_from_parts(
        placement_width_cells,
        placement_height_cells,
        outcome.routed_catalog_cells,
        outcome.route_bends,
        u64::from(outcome.routing_states),
    )
}

pub(crate) fn catalog_outcome_metrics_from_parts(
    placement_width_cells: u64,
    placement_height_cells: u64,
    routed_catalog_cells: u64,
    route_bends: u64,
    routing_states: u64,
) -> Result<CatalogAwareOutcomeMetrics, String> {
    let placement_span_cells = placement_width_cells
        .checked_add(placement_height_cells)
        .ok_or_else(|| "placement span arithmetic overflowed".to_owned())?;
    let placement_area_cells = placement_width_cells
        .checked_mul(placement_height_cells)
        .ok_or_else(|| "placement area arithmetic overflowed".to_owned())?;
    Ok(CatalogAwareOutcomeMetrics {
        placement_width_cells,
        placement_height_cells,
        placement_span_cells,
        placement_area_cells,
        routed_catalog_cells,
        route_bends,
        routing_states,
    })
}

pub(crate) fn catalog_outcome_constraint_misses(
    metrics: &CatalogAwareOutcomeMetrics,
    constraints: &CatalogAwareOutcomeConstraints,
) -> Vec<CatalogAwareConstraintMiss> {
    [
        (
            "placement_width_cells",
            metrics.placement_width_cells,
            constraints.max_placement_width_cells,
        ),
        (
            "placement_height_cells",
            metrics.placement_height_cells,
            constraints.max_placement_height_cells,
        ),
        (
            "placement_area_cells",
            metrics.placement_area_cells,
            constraints.max_placement_area_cells,
        ),
        (
            "routed_catalog_cells",
            metrics.routed_catalog_cells,
            constraints.max_routed_catalog_cells,
        ),
    ]
    .into_iter()
    .filter(|(_, actual, limit)| actual > limit)
    .map(|(metric, actual, limit)| CatalogAwareConstraintMiss {
        metric: metric.to_owned(),
        actual,
        limit,
    })
    .collect()
}

pub(crate) fn catalog_outcome_ordering(
    contender: &CatalogAwareOutcomeMetrics,
    incumbent: &CatalogAwareOutcomeMetrics,
    primary: CatalogAwareOutcomeMetric,
) -> (Ordering, &'static str) {
    let order = match primary {
        CatalogAwareOutcomeMetric::PlacementSpan => [
            "placement_span_cells",
            "placement_area_cells",
            "routed_catalog_cells",
            "route_bends",
            "routing_states",
        ],
        CatalogAwareOutcomeMetric::PlacementArea => [
            "placement_area_cells",
            "placement_span_cells",
            "routed_catalog_cells",
            "route_bends",
            "routing_states",
        ],
        CatalogAwareOutcomeMetric::RoutedCatalogCells => [
            "routed_catalog_cells",
            "placement_span_cells",
            "placement_area_cells",
            "route_bends",
            "routing_states",
        ],
    };
    for metric in order {
        let ordering = catalog_outcome_metric_value(contender, metric)
            .cmp(&catalog_outcome_metric_value(incumbent, metric));
        if ordering != Ordering::Equal {
            return (ordering, metric);
        }
    }
    (Ordering::Equal, "attempt_order")
}

pub(crate) fn catalog_outcome_metric_value(
    metrics: &CatalogAwareOutcomeMetrics,
    metric: &str,
) -> u64 {
    match metric {
        "placement_span_cells" => metrics.placement_span_cells,
        "placement_area_cells" => metrics.placement_area_cells,
        "routed_catalog_cells" => metrics.routed_catalog_cells,
        "route_bends" => metrics.route_bends,
        "routing_states" => metrics.routing_states,
        _ => 0,
    }
}

pub(crate) fn catalog_primary_metric_name(metric: CatalogAwareOutcomeMetric) -> &'static str {
    match metric {
        CatalogAwareOutcomeMetric::PlacementSpan => "placement_span_cells",
        CatalogAwareOutcomeMetric::PlacementArea => "placement_area_cells",
        CatalogAwareOutcomeMetric::RoutedCatalogCells => "routed_catalog_cells",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CatalogAwareFailure {
    pub(crate) classification: String,
    pub(crate) stage: String,
    pub(crate) detail: String,
    pub(crate) rooms_placed: usize,
    pub(crate) sections_routed: usize,
    pub(crate) routing_states: u32,
}

pub(crate) fn realize_catalog_aware_attempt(
    input: CatalogAwareGenerationInput<'_>,
    attempt: u32,
    room_compaction_cells: i32,
    recorder: &mut CatalogGenerationTraceRecorder,
) -> Result<CatalogAwareAttemptOutcome, CatalogAwareFailure> {
    let room_requirements = input
        .source_plan
        .requirements
        .iter()
        .filter(|requirement| is_room_requirement(requirement))
        .cloned()
        .collect::<Vec<_>>();
    let mut rooms = Vec::new();
    let mut occupied = BTreeMap::<(i32, i32), String>::new();
    let mut reserved = BTreeSet::<(i32, i32)>::new();
    for requirement in room_requirements {
        let candidates = catalog_exact_room_candidates(
            input.catalog,
            input.source_plan,
            &requirement,
            input.seed,
            input.policy.max_room_candidates,
        );
        if !recorder.record(
            Some(attempt),
            CatalogGenerationTraceEventBody::RoomDomainEvaluated {
                piece_id: requirement.piece_id.clone(),
                requirement_kind: requirement.kind.clone(),
                candidates: candidates
                    .iter()
                    .map(|candidate| CatalogGenerationTraceRoomCandidate {
                        shape_id: candidate.shape_id.clone(),
                        transform: candidate.transform.clone(),
                        score: candidate.score,
                        rank: candidate.candidate_rank,
                    })
                    .collect(),
            },
        ) {
            return Err(catalog_generation_failure(
                "trace_recording",
                "Trace quota rejected a room-domain event.".to_owned(),
                rooms.len(),
                0,
                0,
            ));
        }
        // Attempts vary compact placement while preserving the deterministic
        // best exact catalog shape. Cycling through lower-ranked shapes made
        // later attempts a different content choice instead of a controlled
        // realization comparison.
        let Some(matched) = candidates.first().cloned() else {
            return Err(CatalogAwareFailure {
                classification: "catalog_coverage_gap".to_owned(),
                stage: "room_domains".to_owned(),
                detail: format!(
                    "No exact-facing catalog room matched {} ({}) with exits [{}].",
                    requirement.piece_id,
                    requirement.kind,
                    requirement
                        .required_exits
                        .iter()
                        .map(|exit| format!("{}:{}", exit.id, exit.direction))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                rooms_placed: rooms.len(),
                sections_routed: 0,
                routing_states: 0,
            });
        };
        let shape = input
            .catalog
            .shapes
            .iter()
            .find(|shape| shape.shape_id == matched.shape_id)
            .cloned()
            .ok_or_else(|| CatalogAwareFailure {
                classification: "catalog_coverage_gap".to_owned(),
                stage: "room_domains".to_owned(),
                detail: format!(
                    "Matched room shape {} is absent from the catalog.",
                    matched.shape_id
                ),
                rooms_placed: rooms.len(),
                sections_routed: 0,
                routing_states: 0,
            })?;
        let origin = catalog_room_origin(
            &requirement,
            &shape,
            matched.transform.as_str(),
            room_compaction_cells,
            &GridCell {
                x: div_ceil_i32(
                    input.source_geometry.bounds.width,
                    CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL,
                ) / 2,
                y: div_ceil_i32(
                    input.source_geometry.bounds.height,
                    CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL,
                ) / 2,
            },
        );
        let room_occupied = transform_cells(&shape.footprint, matched.transform.as_str(), &origin);
        let room_reserved =
            transform_cells(&shape.reserved_cells, matched.transform.as_str(), &origin);
        let conflicting_cells = room_occupied
            .iter()
            .filter(|cell| occupied.contains_key(&(cell.x, cell.y)))
            .cloned()
            .collect::<Vec<_>>();
        if !conflicting_cells.is_empty() {
            if !recorder.record(
                Some(attempt),
                CatalogGenerationTraceEventBody::RoomConflict {
                    piece_id: requirement.piece_id.clone(),
                    conflicting_cells,
                },
            ) {
                return Err(catalog_generation_failure(
                    "trace_recording",
                    "Trace quota rejected a room-conflict event.".to_owned(),
                    rooms.len(),
                    0,
                    0,
                ));
            }
            return Err(CatalogAwareFailure {
                classification: "generation_infeasibility".to_owned(),
                stage: "room_placement".to_owned(),
                detail: format!(
                    "Catalog room {} overlaps an earlier room.",
                    requirement.piece_id
                ),
                rooms_placed: rooms.len(),
                sections_routed: 0,
                routing_states: 0,
            });
        }
        for cell in &room_occupied {
            occupied.insert((cell.x, cell.y), requirement.piece_id.clone());
        }
        for cell in room_reserved {
            reserved.insert((cell.x, cell.y));
        }
        if !recorder.record(
            Some(attempt),
            CatalogGenerationTraceEventBody::RoomPlaced {
                placement: CatalogGenerationTraceRoomPlacement {
                    piece_id: requirement.piece_id.clone(),
                    requirement_kind: requirement.kind.clone(),
                    shape_id: matched.shape_id.clone(),
                    transform: matched.transform.clone(),
                    origin: origin.clone(),
                    occupied_cells: room_occupied.clone(),
                    reserved_cells: transform_cells(
                        &shape.reserved_cells,
                        matched.transform.as_str(),
                        &origin,
                    ),
                },
            },
        ) {
            return Err(catalog_generation_failure(
                "trace_recording",
                "Trace quota rejected a room-placement event.".to_owned(),
                rooms.len(),
                0,
                0,
            ));
        }
        rooms.push(CatalogRoomSelection {
            requirement,
            matched,
            shape,
            origin,
        });
    }
    let sections = catalog_section_specs(input.source_plan, input.source_geometry)?;
    let room_by_piece = rooms
        .iter()
        .map(|room| (room.requirement.piece_id.as_str(), room))
        .collect::<BTreeMap<_, _>>();
    let bounds = catalog_route_bounds(input.source_geometry, input.policy.route_margin_cells);
    let mut routed = Vec::new();
    let mut total_states = 0_u32;
    let mut sections_by_length = sections;
    sections_by_length.sort_by(|left, right| {
        right
            .guide
            .len()
            .cmp(&left.guide.len())
            .then_with(|| left.section.cmp(&right.section))
    });
    for spec in sections_by_length {
        let Some(left_room) = room_by_piece
            .get(spec.left.requirement.piece_id.as_str())
            .copied()
        else {
            return Err(catalog_generation_failure(
                "section_routing",
                format!("Section {} has no placed left room.", spec.section),
                rooms.len(),
                routed.len(),
                total_states,
            ));
        };
        let Some(right_room) = room_by_piece
            .get(spec.right.requirement.piece_id.as_str())
            .copied()
        else {
            return Err(catalog_generation_failure(
                "section_routing",
                format!("Section {} has no placed right room.", spec.section),
                rooms.len(),
                routed.len(),
                total_states,
            ));
        };
        let start = catalog_room_exit_cell(left_room, spec.left.exit_id.as_str())?;
        let goal = catalog_room_exit_cell(right_room, spec.right.exit_id.as_str())?;
        if !recorder.record(
            Some(attempt),
            CatalogGenerationTraceEventBody::SectionRoutingStarted {
                section_id: spec.section.clone(),
                start: start.clone(),
                goal: goal.clone(),
                guide: spec.guide.clone(),
                bounds: bounds.clone(),
            },
        ) {
            return Err(catalog_generation_failure(
                "trace_recording",
                "Trace quota rejected a section-routing start event.".to_owned(),
                rooms.len(),
                routed.len(),
                total_states,
            ));
        }
        let route = route_catalog_section(
            &start,
            &goal,
            &spec.guide,
            &occupied,
            &reserved,
            &[
                left_room.requirement.piece_id.as_str(),
                right_room.requirement.piece_id.as_str(),
            ],
            input.catalog.placement_policy.minimum_clearance_cells,
            &bounds,
            input.policy,
        );
        let (cells, states) = match route {
            CatalogRouteSearch::Found {
                cells,
                states_visited,
            } => {
                if !recorder.record(
                    Some(attempt),
                    CatalogGenerationTraceEventBody::SectionRoutingFinished {
                        section_id: spec.section.clone(),
                        status: "found".to_owned(),
                        cells: cells.clone(),
                        states_visited,
                    },
                ) {
                    return Err(catalog_generation_failure(
                        "trace_recording",
                        "Trace quota rejected a successful section-routing event.".to_owned(),
                        rooms.len(),
                        routed.len(),
                        total_states,
                    ));
                }
                (cells, states_visited)
            }
            CatalogRouteSearch::NoPath { states_visited } => {
                if !recorder.record(
                    Some(attempt),
                    CatalogGenerationTraceEventBody::SectionRoutingFinished {
                        section_id: spec.section.clone(),
                        status: "no_path".to_owned(),
                        cells: Vec::new(),
                        states_visited,
                    },
                ) {
                    return Err(catalog_generation_failure(
                        "trace_recording",
                        "Trace quota rejected a failed section-routing event.".to_owned(),
                        rooms.len(),
                        routed.len(),
                        total_states,
                    ));
                }
                return Err(CatalogAwareFailure {
                    classification: "generation_infeasibility".to_owned(),
                    stage: "section_routing".to_owned(),
                    detail: format!(
                        "No catalog-cell route satisfied section {} from {},{} to {},{} in bounds {},{}..{},{}.",
                        spec.section,
                        start.x,
                        start.y,
                        goal.x,
                        goal.y,
                        bounds.min_x,
                        bounds.min_y,
                        bounds.max_x,
                        bounds.max_y,
                    ),
                    rooms_placed: rooms.len(),
                    sections_routed: routed.len(),
                    routing_states: total_states.saturating_add(states_visited),
                });
            }
            CatalogRouteSearch::BudgetExhausted { states_visited } => {
                if !recorder.record(
                    Some(attempt),
                    CatalogGenerationTraceEventBody::SectionRoutingFinished {
                        section_id: spec.section.clone(),
                        status: "budget_exhausted".to_owned(),
                        cells: Vec::new(),
                        states_visited,
                    },
                ) {
                    return Err(catalog_generation_failure(
                        "trace_recording",
                        "Trace quota rejected a budget-exhausted routing event.".to_owned(),
                        rooms.len(),
                        routed.len(),
                        total_states,
                    ));
                }
                return Err(CatalogAwareFailure {
                    classification: "search_budget_exhaustion".to_owned(),
                    stage: "section_routing".to_owned(),
                    detail: format!(
                        "Catalog-cell routing for section {} from {},{} to {},{} exhausted its {}-state budget.",
                        spec.section,
                        start.x,
                        start.y,
                        goal.x,
                        goal.y,
                        input.policy.max_routing_states_per_section,
                    ),
                    rooms_placed: rooms.len(),
                    sections_routed: routed.len(),
                    routing_states: total_states.saturating_add(states_visited),
                });
            }
        };
        total_states = total_states.saturating_add(states);
        for (index, cell) in cells.iter().enumerate() {
            occupied.insert(
                (cell.x, cell.y),
                format!("catalog-route:{}:{index}", spec.section),
            );
        }
        routed.push(CatalogRoutedSection { spec, cells });
    }
    let (mut geometry, plan, shape_match, mut placement) = materialize_catalog_composition(
        input.source_geometry,
        input.source_plan,
        input.catalog,
        &rooms,
        &routed,
        input.seed,
        input.provenance,
    )?;
    normalize_catalog_geometry_bounds(&mut geometry);
    refresh_geometry_compactness_evidence(&mut geometry);
    validate_catalog_geometry_segments(&geometry).map_err(|detail| {
        catalog_generation_failure(
            "geometry_materialization",
            detail,
            rooms.len(),
            routed.len(),
            total_states,
        )
    })?;
    let geometry_validation = validate_geometry_2d(&geometry);
    if !recorder.record(
        Some(attempt),
        CatalogGenerationTraceEventBody::ValidationCompleted {
            stage: "geometry_validation".to_owned(),
            ok: geometry_validation.ok,
            subject_hash: geometry_validation.state_hash.clone(),
            diagnostic_codes: geometry_validation
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.clone())
                .collect(),
        },
    ) {
        return Err(catalog_generation_failure(
            "trace_recording",
            "Trace quota rejected a geometry-validation event.".to_owned(),
            rooms.len(),
            routed.len(),
            total_states,
        ));
    }
    if !geometry_validation.ok {
        return Err(catalog_validation_failure(
            "geometry_validation",
            &geometry_validation,
            rooms.len(),
            routed.len(),
            total_states,
        ));
    }
    placement.glued_exits = derive_glued_exits(&plan, &placement.instances).map_err(|detail| {
        catalog_generation_failure(
            "direct_glue",
            detail,
            rooms.len(),
            routed.len(),
            total_states,
        )
    })?;
    if placement
        .glued_exits
        .iter()
        .any(|glued| !pure_catalog_glue_is_direct(glued, &placement.occupied_cells))
    {
        return Err(catalog_generation_failure(
            "direct_glue",
            "A catalog-aware link is not a direct occupied-cell glue.".to_owned(),
            rooms.len(),
            routed.len(),
            total_states,
        ));
    }
    placement.gate_portals =
        derive_gate_portals(&plan, &placement.glued_exits).map_err(|detail| {
            catalog_generation_failure(
                "gate_portals",
                detail,
                rooms.len(),
                routed.len(),
                total_states,
            )
        })?;
    let placement_validation = validate_piece_placement(&placement);
    if !recorder.record(
        Some(attempt),
        CatalogGenerationTraceEventBody::ValidationCompleted {
            stage: "placement_validation".to_owned(),
            ok: placement_validation.ok,
            subject_hash: placement_validation.state_hash.clone(),
            diagnostic_codes: placement_validation
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.clone())
                .collect(),
        },
    ) {
        return Err(catalog_generation_failure(
            "trace_recording",
            "Trace quota rejected a placement-validation event.".to_owned(),
            rooms.len(),
            routed.len(),
            total_states,
        ));
    }
    if !placement_validation.ok {
        return Err(catalog_validation_failure(
            "placement_validation",
            &placement_validation,
            rooms.len(),
            routed.len(),
            total_states,
        ));
    }
    let flow_args = BuildValidateFlowArgs {
        candidate: PathBuf::from(&input.provenance.candidate_ref),
        geometry: PathBuf::from(&input.provenance.geometry_ref),
        piece_plan: PathBuf::from(&input.provenance.piece_plan_ref),
        piece_placement: PathBuf::from(&input.provenance.result_ref),
        out: PathBuf::from(&input.provenance.result_ref),
    };
    let built_flow_validation =
        validate_built_flow(input.candidate, &geometry, &plan, &placement, &flow_args);
    let built_flow_hash = if recorder.is_active() {
        hash_json(&built_flow_validation).unwrap_or_else(|_| "hash_error".to_owned())
    } else {
        String::new()
    };
    if !recorder.record(
        Some(attempt),
        CatalogGenerationTraceEventBody::ValidationCompleted {
            stage: "built_flow_validation".to_owned(),
            ok: built_flow_validation.ok,
            subject_hash: built_flow_hash,
            diagnostic_codes: built_flow_validation
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.clone())
                .collect(),
        },
    ) {
        return Err(catalog_generation_failure(
            "trace_recording",
            "Trace quota rejected a built-flow-validation event.".to_owned(),
            rooms.len(),
            routed.len(),
            total_states,
        ));
    }
    if !built_flow_validation.ok {
        return Err(CatalogAwareFailure {
            classification: "generation_infeasibility".to_owned(),
            stage: "built_flow_validation".to_owned(),
            detail: built_flow_validation
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.detail.as_str())
                .collect::<Vec<_>>()
                .join("; "),
            rooms_placed: rooms.len(),
            sections_routed: routed.len(),
            routing_states: total_states,
        });
    }
    let routed_catalog_cells = routed.iter().try_fold(0_u64, |total, route| {
        let cells = u64::try_from(route.cells.len()).ok()?;
        total.checked_add(cells)
    });
    let route_bends = routed.iter().try_fold(0_u64, |total, route| {
        total.checked_add(catalog_route_bends(&route.cells)?)
    });
    let (Some(routed_catalog_cells), Some(route_bends)) = (routed_catalog_cells, route_bends)
    else {
        return Err(catalog_generation_failure(
            "outcome_metrics",
            "Catalog outcome route metrics overflowed.".to_owned(),
            rooms.len(),
            routed.len(),
            total_states,
        ));
    };
    Ok(CatalogAwareAttemptOutcome {
        geometry,
        geometry_validation,
        piece_plan: plan,
        shape_match,
        placement,
        placement_validation,
        built_flow_validation,
        routing_states: total_states,
        routed_catalog_cells,
        route_bends,
    })
}

pub(crate) fn catalog_route_bends(cells: &[GridCell]) -> Option<u64> {
    cells.windows(3).try_fold(0_u64, |total, points| {
        let first = &points[0];
        let middle = &points[1];
        let last = &points[2];
        let prior = (
            middle.x.checked_sub(first.x)?,
            middle.y.checked_sub(first.y)?,
        );
        let next = (last.x.checked_sub(middle.x)?, last.y.checked_sub(middle.y)?);
        total.checked_add(u64::from(prior != next))
    })
}

pub(crate) fn validate_catalog_aware_policy(
    policy: &CatalogAwareGenerationPolicy,
) -> Result<(), String> {
    if policy.kind != "rusty_procgen.catalog_aware_generation_policy.v2"
        || policy.schema_version != 2
    {
        return Err("unsupported catalog-aware generation policy".to_owned());
    }
    if policy.max_generation_attempts == 0 || policy.max_generation_attempts > 16 {
        return Err("maxGenerationAttempts must be from 1 through 16".to_owned());
    }
    if policy.initial_room_compaction_cells < 0
        || policy.room_compaction_growth_cells < 0
        || policy.initial_room_compaction_cells.saturating_add(
            policy.room_compaction_growth_cells.saturating_mul(
                i32::try_from(policy.max_generation_attempts - 1).unwrap_or(i32::MAX),
            ),
        ) > 128
    {
        return Err(
            "catalog-aware room compaction must remain from 0 through 128 cells".to_owned(),
        );
    }
    if policy.max_room_candidates == 0 || policy.max_room_candidates > 64 {
        return Err("maxRoomCandidates must be from 1 through 64".to_owned());
    }
    if policy.max_routing_states_per_section < 100
        || policy.max_routing_states_per_section > 1_000_000
    {
        return Err("maxRoutingStatesPerSection must be from 100 through 1000000".to_owned());
    }
    if policy.route_margin_cells < 8 || policy.route_margin_cells > 256 {
        return Err("routeMarginCells must be from 8 through 256".to_owned());
    }
    for (name, value) in [
        (
            "maxPlacementWidthCells",
            policy.outcome_constraints.max_placement_width_cells,
        ),
        (
            "maxPlacementHeightCells",
            policy.outcome_constraints.max_placement_height_cells,
        ),
    ] {
        if value == 0 || value > 4_294_967_296 {
            return Err(format!("{name} must be from 1 through 4294967296"));
        }
    }
    if policy.outcome_constraints.max_placement_area_cells == 0
        || policy.outcome_constraints.max_placement_area_cells > 9_007_199_254_740_991
    {
        return Err("maxPlacementAreaCells must be from 1 through 9007199254740991".to_owned());
    }
    if policy.outcome_constraints.max_routed_catalog_cells == 0
        || policy.outcome_constraints.max_routed_catalog_cells > 1_048_576
    {
        return Err("maxRoutedCatalogCells must be from 1 through 1048576".to_owned());
    }
    if policy.outcome_preferences.preferred_maximum == 0
        || policy.outcome_preferences.preferred_maximum > 9_007_199_254_740_991
    {
        return Err("preferredMaximum must be from 1 through 9007199254740991".to_owned());
    }
    Ok(())
}

pub(crate) fn catalog_exact_room_candidates(
    catalog: &ShapeCatalog,
    plan: &PieceBuildPlan,
    requirement: &PieceRequirement,
    seed: u64,
    max_candidates: u32,
) -> Vec<MatchedPiece> {
    pure_catalog_match_candidates(catalog, requirement, plan, seed)
        .into_iter()
        .filter(|candidate| {
            candidate.exit_map.iter().all(|mapped| {
                requirement
                    .required_exits
                    .iter()
                    .find(|required| required.id == mapped.requirement_exit_id)
                    .is_some_and(|required| required.direction == mapped.direction)
            })
        })
        .take(max_candidates as usize)
        .collect()
}

pub(crate) fn catalog_room_origin(
    requirement: &PieceRequirement,
    shape: &CatalogShape,
    transform: &str,
    compaction_cells: i32,
    compaction_target: &GridCell,
) -> GridCell {
    let transformed = transform_cells(
        shape.footprint.as_slice(),
        transform,
        &GridCell { x: 0, y: 0 },
    );
    let width = transformed.iter().map(|cell| cell.x).max().unwrap_or(0) + 1;
    let height = transformed.iter().map(|cell| cell.y).max().unwrap_or(0) + 1;
    let (x, y, zone_width, zone_height) = requirement
        .placement_hints
        .iter()
        .find_map(|hint| parse_geometry_rect(hint))
        .unwrap_or((
            0,
            0,
            width * CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL,
            height * CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL,
        ));
    let zone_width_cells = div_ceil_i32(zone_width, CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL);
    let zone_height_cells = div_ceil_i32(zone_height, CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL);
    let base = GridCell {
        x: x.div_euclid(CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL)
            + (zone_width_cells - width).max(0) / 2,
        y: y.div_euclid(CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL)
            + (zone_height_cells - height).max(0) / 2,
    };
    let room_center = GridCell {
        x: base.x.saturating_add(width / 2),
        y: base.y.saturating_add(height / 2),
    };
    GridCell {
        x: base.x.saturating_add(
            compaction_target
                .x
                .saturating_sub(room_center.x)
                .signum()
                .saturating_mul(compaction_cells),
        ),
        y: base.y.saturating_add(
            compaction_target
                .y
                .saturating_sub(room_center.y)
                .signum()
                .saturating_mul(compaction_cells),
        ),
    }
}

pub(crate) fn catalog_section_specs(
    plan: &PieceBuildPlan,
    geometry: &Geometry2dArtifact,
) -> Result<Vec<CatalogSectionSpec>, CatalogAwareFailure> {
    let requirements = plan
        .requirements
        .iter()
        .map(|requirement| (requirement.piece_id.as_str(), requirement))
        .collect::<BTreeMap<_, _>>();
    let geometry_by_section = geometry
        .corridors
        .iter()
        .map(|corridor| (corridor.physical_section.as_str(), corridor))
        .collect::<BTreeMap<_, _>>();
    let mut links_by_section = BTreeMap::<String, Vec<&PieceLink>>::new();
    for link in &plan.links {
        links_by_section
            .entry(link.source_section.clone())
            .or_default()
            .push(link);
    }
    let mut sections = Vec::new();
    for (section, links) in links_by_section {
        let mut terminals = links
            .iter()
            .flat_map(|link| {
                [
                    (link.from_piece.as_str(), link.from_exit.as_str()),
                    (link.to_piece.as_str(), link.to_exit.as_str()),
                ]
            })
            .filter_map(|(piece, exit)| {
                let requirement = requirements.get(piece).copied()?;
                is_room_requirement(requirement).then_some(CatalogSectionTerminal {
                    requirement: requirement.clone(),
                    exit_id: exit.to_owned(),
                })
            })
            .collect::<Vec<_>>();
        terminals.sort_by(|left, right| left.requirement.piece_id.cmp(&right.requirement.piece_id));
        terminals.dedup_by(|left, right| left.requirement.piece_id == right.requirement.piece_id);
        if terminals.len() != 2 {
            return Err(catalog_generation_failure(
                "section_domains",
                format!("Section {section} does not have exactly two room terminals."),
                0,
                sections.len(),
                0,
            ));
        }
        let Some(template_link) = links.first().copied().cloned() else {
            continue;
        };
        let corridor = geometry_by_section
            .get(section.as_str())
            .copied()
            .ok_or_else(|| {
                catalog_generation_failure(
                    "section_domains",
                    format!("Section {section} has no geometry corridor."),
                    0,
                    sections.len(),
                    0,
                )
            })?;
        sections.push(CatalogSectionSpec {
            section,
            source_corridor: corridor.id.clone(),
            template_link,
            left: terminals[0].clone(),
            right: terminals[1].clone(),
            guide: geometry_guide_cells(corridor),
        });
    }
    Ok(sections)
}

pub(crate) fn geometry_guide_cells(corridor: &GeometryCorridor) -> Vec<GridCell> {
    let mut cells = Vec::new();
    for pair in corridor.points.windows(2) {
        let mut current = GridCell {
            x: pair[0]
                .x
                .div_euclid(CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL),
            y: pair[0]
                .y
                .div_euclid(CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL),
        };
        let target = GridCell {
            x: pair[1]
                .x
                .div_euclid(CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL),
            y: pair[1]
                .y
                .div_euclid(CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL),
        };
        cells.push(current.clone());
        while current != target {
            current.x += (target.x - current.x).signum();
            current.y += (target.y - current.y).signum();
            cells.push(current.clone());
        }
    }
    cells.sort_by_key(|cell| (cell.x, cell.y));
    cells.dedup();
    cells
}

pub(crate) fn catalog_room_exit_cell(
    room: &CatalogRoomSelection,
    exit_id: &str,
) -> Result<GridCell, CatalogAwareFailure> {
    room.matched
        .exit_map
        .iter()
        .find(|exit| exit.requirement_exit_id == exit_id)
        .map(|exit| GridCell {
            x: exit.x + room.origin.x,
            y: exit.y + room.origin.y,
        })
        .ok_or_else(|| {
            catalog_generation_failure(
                "section_domains",
                format!(
                    "Room {} has no matched exit {}.",
                    room.requirement.piece_id, exit_id
                ),
                0,
                0,
                0,
            )
        })
}

pub(crate) fn catalog_route_bounds(
    geometry: &Geometry2dArtifact,
    margin: i32,
) -> CatalogGridBounds {
    CatalogGridBounds {
        min_x: -margin,
        min_y: -margin,
        max_x: div_ceil_i32(
            geometry.bounds.width,
            CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL,
        ) + margin,
        max_y: div_ceil_i32(
            geometry.bounds.height,
            CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL,
        ) + margin,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn route_catalog_section(
    start: &GridCell,
    goal: &GridCell,
    guide: &[GridCell],
    occupied: &BTreeMap<(i32, i32), String>,
    reserved: &BTreeSet<(i32, i32)>,
    terminal_rooms: &[&str],
    minimum_clearance_cells: i32,
    bounds: &CatalogGridBounds,
    policy: &CatalogAwareGenerationPolicy,
) -> CatalogRouteSearch {
    type State = (i32, i32, u8);
    let start_state = (start.x, start.y, 4_u8);
    let mut frontier = BinaryHeap::new();
    frontier.push((Reverse((0_u64, 0_u64)), start_state));
    let mut costs = BTreeMap::<State, u64>::new();
    let mut previous = BTreeMap::<State, State>::new();
    costs.insert(start_state, 0);
    let mut visited = 0_u32;
    let mut final_state = None;
    while let Some((Reverse((_priority, cost)), state)) = frontier.pop() {
        if costs.get(&state).is_some_and(|known| *known != cost) {
            continue;
        }
        visited = visited.saturating_add(1);
        if visited > policy.max_routing_states_per_section {
            return CatalogRouteSearch::BudgetExhausted {
                states_visited: visited,
            };
        }
        if state.0 == goal.x && state.1 == goal.y {
            final_state = Some(state);
            break;
        }
        for (direction, (dx, dy)) in [(0_u8, (0, -1)), (1, (1, 0)), (2, (0, 1)), (3, (-1, 0))] {
            let next = GridCell {
                x: state.0 + dx,
                y: state.1 + dy,
            };
            if next.x < bounds.min_x
                || next.x > bounds.max_x
                || next.y < bounds.min_y
                || next.y > bounds.max_y
                || catalog_route_cell_blocked(
                    &next,
                    start,
                    goal,
                    occupied,
                    reserved,
                    terminal_rooms,
                    minimum_clearance_cells,
                )
            {
                continue;
            }
            let guide_distance = guide
                .iter()
                .map(|cell| next.x.abs_diff(cell.x) + next.y.abs_diff(cell.y))
                .min()
                .unwrap_or(0);
            let turn = u64::from(state.2 < 4 && state.2 != direction)
                .saturating_mul(u64::from(policy.turn_penalty));
            let step_cost = 1_u64
                .saturating_add(
                    u64::from(guide_distance)
                        .saturating_mul(u64::from(policy.guide_distance_weight)),
                )
                .saturating_add(turn);
            let next_state = (next.x, next.y, direction);
            let next_cost = cost.saturating_add(step_cost);
            if costs
                .get(&next_state)
                .is_none_or(|existing| next_cost < *existing)
            {
                costs.insert(next_state, next_cost);
                previous.insert(next_state, state);
                let heuristic = u64::from(next.x.abs_diff(goal.x) + next.y.abs_diff(goal.y));
                frontier.push((
                    Reverse((next_cost.saturating_add(heuristic), next_cost)),
                    next_state,
                ));
            }
        }
    }
    let Some(mut state) = final_state else {
        return CatalogRouteSearch::NoPath {
            states_visited: visited,
        };
    };
    let mut cells = vec![GridCell {
        x: state.0,
        y: state.1,
    }];
    while state != start_state {
        let Some(predecessor) = previous.get(&state) else {
            return CatalogRouteSearch::NoPath {
                states_visited: visited,
            };
        };
        state = *predecessor;
        cells.push(GridCell {
            x: state.0,
            y: state.1,
        });
    }
    cells.reverse();
    CatalogRouteSearch::Found {
        cells,
        states_visited: visited,
    }
}

pub(crate) fn catalog_route_cell_blocked(
    cell: &GridCell,
    start: &GridCell,
    goal: &GridCell,
    occupied: &BTreeMap<(i32, i32), String>,
    reserved: &BTreeSet<(i32, i32)>,
    terminal_rooms: &[&str],
    minimum_clearance_cells: i32,
) -> bool {
    if cell == start || cell == goal {
        return false;
    }
    if occupied.contains_key(&(cell.x, cell.y)) || reserved.contains(&(cell.x, cell.y)) {
        return true;
    }
    for dx in -minimum_clearance_cells..=minimum_clearance_cells {
        for dy in -minimum_clearance_cells..=minimum_clearance_cells {
            let distance = dx.abs() + dy.abs();
            if distance == 0 || distance > minimum_clearance_cells {
                continue;
            }
            if let Some(owner) = occupied.get(&(cell.x + dx, cell.y + dy)) {
                if distance == 1 || !terminal_rooms.contains(&owner.as_str()) {
                    return true;
                }
            }
        }
    }
    false
}

pub(crate) fn materialize_catalog_composition(
    source_geometry: &Geometry2dArtifact,
    source_plan: &PieceBuildPlan,
    catalog: &ShapeCatalog,
    rooms: &[CatalogRoomSelection],
    routed: &[CatalogRoutedSection],
    seed: u64,
    provenance: &CatalogAwareGenerationProvenance,
) -> Result<
    (
        Geometry2dArtifact,
        PieceBuildPlan,
        PieceShapeMatchReport,
        PiecePlacement,
    ),
    CatalogAwareFailure,
> {
    let mut geometry = source_geometry.clone();
    let mut plan = source_plan.clone();
    plan.requirements.retain(is_room_requirement);
    plan.links.clear();
    plan.content_requirements.retain(|content| {
        plan.requirements
            .iter()
            .any(|requirement| requirement.piece_id == content.piece_id)
    });
    let mut matched_pieces = rooms
        .iter()
        .map(|room| room.matched.clone())
        .collect::<Vec<_>>();
    let mut instances = Vec::new();
    let mut occupied_cells = Vec::new();
    let mut reserved_cells = Vec::new();
    let mut decisions = Vec::new();
    for room in rooms {
        instances.push(catalog_piece_instance(
            &room.requirement,
            &room.matched,
            &room.shape,
            room.origin.clone(),
        ));
        append_instance_cells(
            instances.last().expect("room instance"),
            &mut occupied_cells,
            &mut reserved_cells,
        );
        decisions.push(catalog_decision(&room.matched, room.origin.clone()));
        align_geometry_room_to_catalog(&mut geometry, room);
    }
    for section in routed {
        let left_room = rooms
            .iter()
            .find(|room| room.requirement.piece_id == section.spec.left.requirement.piece_id)
            .ok_or_else(|| {
                catalog_generation_failure(
                    "materialize",
                    "Missing left room.".to_owned(),
                    rooms.len(),
                    0,
                    0,
                )
            })?;
        let right_room = rooms
            .iter()
            .find(|room| room.requirement.piece_id == section.spec.right.requirement.piece_id)
            .ok_or_else(|| {
                catalog_generation_failure(
                    "materialize",
                    "Missing right room.".to_owned(),
                    rooms.len(),
                    0,
                    0,
                )
            })?;
        let left_exit = catalog_room_exit(left_room, section.spec.left.exit_id.as_str())?;
        let right_exit = catalog_room_exit(right_room, section.spec.right.exit_id.as_str())?;
        let mut previous_piece = section.spec.left.requirement.piece_id.clone();
        let mut previous_exit = section.spec.left.exit_id.clone();
        for (index, cell) in section.cells.iter().enumerate() {
            let back_direction = if index == 0 {
                opposite_direction(left_exit.direction.as_str())
            } else {
                direction_between(cell, &section.cells[index - 1])
            };
            let forward_direction = if index + 1 == section.cells.len() {
                opposite_direction(right_exit.direction.as_str())
            } else {
                direction_between(cell, &section.cells[index + 1])
            };
            let piece_id = format!(
                "piece.{}.catalog_route.{}.tile_{:03}",
                if opposite_direction(back_direction) == forward_direction {
                    "corridor"
                } else {
                    "bend"
                },
                slugify_label(section.spec.section.as_str()),
                index + 1
            );
            let (requirement, matched, shape) = catalog_route_piece(
                catalog,
                &piece_id,
                back_direction,
                forward_direction,
                &section.spec,
                seed,
            )?;
            let input_exit = requirement.required_exits[0].id.clone();
            let output_exit = requirement.required_exits[1].id.clone();
            plan.requirements.push(requirement.clone());
            matched_pieces.push(matched.clone());
            let instance = catalog_piece_instance(&requirement, &matched, &shape, cell.clone());
            append_instance_cells(&instance, &mut occupied_cells, &mut reserved_cells);
            instances.push(instance);
            decisions.push(catalog_decision(&matched, cell.clone()));
            let mut link = section.spec.template_link.clone();
            link.id = format!(
                "piece_link.{}.catalog_route.{:03}",
                slugify_label(section.spec.section.as_str()),
                index + 1
            );
            link.from_piece = previous_piece;
            link.from_exit = previous_exit;
            link.to_piece = piece_id.clone();
            link.to_exit = input_exit;
            link.route_points.clear();
            plan.links.push(link);
            previous_piece = piece_id;
            previous_exit = output_exit;
        }
        let mut final_link = section.spec.template_link.clone();
        final_link.id = format!(
            "piece_link.{}.catalog_route.out",
            slugify_label(section.spec.section.as_str())
        );
        final_link.from_piece = previous_piece;
        final_link.from_exit = previous_exit;
        final_link.to_piece = section.spec.right.requirement.piece_id.clone();
        final_link.to_exit = section.spec.right.exit_id.clone();
        final_link.route_points.clear();
        plan.links.push(final_link);
        align_geometry_corridor_to_catalog(
            &mut geometry,
            section,
            &left_room.origin,
            left_exit,
            &right_room.origin,
            right_exit,
        );
    }
    plan.requirements
        .sort_by(|left, right| left.piece_id.cmp(&right.piece_id));
    plan.links.sort_by(|left, right| left.id.cmp(&right.id));
    let shape_match = PieceShapeMatchReport {
        kind: "rusty_procgen.piece_shape_match.v1".to_owned(),
        schema_version: 1,
        match_id: format!("piece_shape_match.{}.{}.catalog_aware", plan.plan_id, seed),
        plan_id: plan.plan_id.clone(),
        catalog_id: catalog.catalog_id.clone(),
        seed,
        alternative_attempt: 0,
        source_plan_ref: provenance.piece_plan_ref.clone(),
        source_catalog_ref: provenance.catalog_ref.clone(),
        ok: true,
        unmatched_count: 0,
        matches: matched_pieces,
        rejections: Vec::new(),
        diagnostics: Vec::new(),
    };
    let placement = PiecePlacement {
        kind: "rusty_procgen.piece_placement.v1".to_owned(),
        schema_version: 1,
        placement_id: format!("piece_placement.{}.catalog_aware", plan.plan_id),
        plan_id: plan.plan_id.clone(),
        catalog_id: catalog.catalog_id.clone(),
        match_id: shape_match.match_id.clone(),
        corridor_realization: CorridorRealization::Catalog,
        source_plan_ref: provenance.piece_plan_ref.clone(),
        source_catalog_ref: provenance.catalog_ref.clone(),
        source_match_ref: format!("{}:catalog-aware", provenance.result_ref),
        cell_size: catalog.cell_size,
        grid_connectivity: GridConnectivity::FourWay,
        placement_policy: catalog.placement_policy.clone(),
        realization_search: PieceRealizationSearchEvidence {
            realization_scale_tier: 0,
            realization_attempts: 1,
            route_order_attempt: 0,
            route_attempts: 1,
            ..PieceRealizationSearchEvidence::default()
        },
        catalog_search: Some(CatalogSearchEvidence {
            schema_version: 1,
            max_decisions: 1_000_000,
            max_backtracks: 1,
            max_chain_expansions_per_section: 1_000_000,
            max_room_origin_alternatives: 1,
            max_room_rotation_alternatives: 4,
            decisions: decisions.len() as u32,
            backtracks: 0,
            chain_expansions: routed
                .iter()
                .map(|section| section.cells.len() as u32)
                .sum(),
            room_origin_attempts: rooms.len() as u32,
            room_rotation_attempts: rooms.len() as u32,
            selected: decisions,
        }),
        instances,
        glued_exits: Vec::new(),
        gate_portals: Vec::new(),
        occupied_cells,
        connection_cells: Vec::new(),
        reserved_cells,
        dangling_exits: Vec::new(),
    };
    Ok((geometry, plan, shape_match, placement))
}

pub(crate) fn catalog_room_exit<'a>(
    room: &'a CatalogRoomSelection,
    exit_id: &str,
) -> Result<&'a MatchedExit, CatalogAwareFailure> {
    room.matched
        .exit_map
        .iter()
        .find(|exit| exit.requirement_exit_id == exit_id)
        .ok_or_else(|| {
            catalog_generation_failure(
                "materialize",
                format!("Missing room exit {exit_id}."),
                0,
                0,
                0,
            )
        })
}

pub(crate) fn catalog_route_piece(
    catalog: &ShapeCatalog,
    piece_id: &str,
    back_direction: &str,
    forward_direction: &str,
    section: &CatalogSectionSpec,
    seed: u64,
) -> Result<(PieceRequirement, MatchedPiece, CatalogShape), CatalogAwareFailure> {
    if back_direction == forward_direction {
        return Err(catalog_generation_failure(
            "section_materialization",
            format!("Route {} doubles back through one cell.", section.section),
            0,
            0,
            0,
        ));
    }
    let kind = if opposite_direction(back_direction) == forward_direction {
        "corridor"
    } else {
        "bend"
    };
    let requirement = PieceRequirement {
        piece_id: piece_id.to_owned(),
        kind: kind.to_owned(),
        role: "catalog_route".to_owned(),
        source_refs: vec![
            format!("physicalSection:{}", section.section),
            format!("geometryCorridor:{}", section.source_corridor),
        ],
        required_exits: vec![
            PieceExitRequirement {
                id: "exit.catalog_route.in".to_owned(),
                direction: back_direction.to_owned(),
                width: 12,
                order: 0,
                tags: vec!["catalog_route".to_owned()],
            },
            PieceExitRequirement {
                id: "exit.catalog_route.out".to_owned(),
                direction: forward_direction.to_owned(),
                width: 12,
                order: 0,
                tags: vec!["catalog_route".to_owned()],
            },
        ],
        required_sockets: Vec::new(),
        required_shape_tags: Vec::new(),
        tags: vec!["catalog_route".to_owned(), kind.to_owned()],
        placement_hints: Vec::new(),
    };
    for shape in &catalog.shapes {
        if !shape
            .piece_kinds
            .iter()
            .any(|piece_kind| piece_kind == kind)
            || shape.footprint.len() != 1
        {
            continue;
        }
        for transform in &shape.allowed_transforms {
            let exits = transformed_catalog_exits(shape, transform);
            let back = exits.iter().find(|exit| exit.direction == back_direction);
            let forward = exits
                .iter()
                .find(|exit| exit.direction == forward_direction);
            let (Some(back), Some(forward)) = (back, forward) else {
                continue;
            };
            if back.id == forward.id {
                continue;
            }
            let exit_map = vec![
                MatchedExit {
                    requirement_exit_id: requirement.required_exits[0].id.clone(),
                    catalog_exit_id: back.id.clone(),
                    x: back.x,
                    y: back.y,
                    direction: back.direction.clone(),
                    width: back.width,
                },
                MatchedExit {
                    requirement_exit_id: requirement.required_exits[1].id.clone(),
                    catalog_exit_id: forward.id.clone(),
                    x: forward.x,
                    y: forward.y,
                    direction: forward.direction.clone(),
                    width: forward.width,
                },
            ];
            return Ok((
                requirement,
                MatchedPiece {
                    piece_id: piece_id.to_owned(),
                    requirement_kind: kind.to_owned(),
                    shape_id: shape.shape_id.clone(),
                    transform: transform.clone(),
                    score: 1_000,
                    candidate_rank: 0,
                    candidate_count: 1,
                    source_requirement_ref: format!(
                        "catalogAware:{}:{}:{}",
                        section.section, seed, piece_id
                    ),
                    exit_map,
                    socket_map: Vec::new(),
                },
                shape.clone(),
            ));
        }
    }
    Err(CatalogAwareFailure {
        classification: "catalog_coverage_gap".to_owned(),
        stage: "section_materialization".to_owned(),
        detail: format!(
            "No one-cell catalog {} supports {} -> {}.",
            kind, back_direction, forward_direction
        ),
        rooms_placed: 0,
        sections_routed: 0,
        routing_states: 0,
    })
}

pub(crate) fn catalog_piece_instance(
    requirement: &PieceRequirement,
    matched: &MatchedPiece,
    shape: &CatalogShape,
    origin: GridCell,
) -> PieceInstance {
    let occupied_cells = transform_cells(&shape.footprint, matched.transform.as_str(), &origin);
    let reserved_cells =
        transform_cells(&shape.reserved_cells, matched.transform.as_str(), &origin);
    let exit_map = matched
        .exit_map
        .iter()
        .map(|exit| MatchedExit {
            requirement_exit_id: exit.requirement_exit_id.clone(),
            catalog_exit_id: exit.catalog_exit_id.clone(),
            x: exit.x + origin.x,
            y: exit.y + origin.y,
            direction: exit.direction.clone(),
            width: exit.width,
        })
        .collect();
    PieceInstance {
        instance_id: format!("instance.{}", slugify_label(requirement.piece_id.as_str())),
        piece_id: requirement.piece_id.clone(),
        requirement_kind: requirement.kind.clone(),
        role: requirement.role.clone(),
        shape_id: shape.shape_id.clone(),
        transform: matched.transform.clone(),
        origin,
        occupied_cells,
        reserved_cells,
        exit_map,
        feature_placements: matched.socket_map.clone(),
        source_requirement_ref: matched.source_requirement_ref.clone(),
        source_refs: requirement.source_refs.clone(),
        tags: requirement.tags.clone(),
    }
}

pub(crate) fn append_instance_cells(
    instance: &PieceInstance,
    occupied: &mut Vec<PlacementCellRef>,
    reserved: &mut Vec<PlacementCellRef>,
) {
    for cell in &instance.occupied_cells {
        occupied.push(PlacementCellRef {
            instance_id: instance.instance_id.clone(),
            x: cell.x,
            y: cell.y,
        });
    }
    for cell in &instance.reserved_cells {
        reserved.push(PlacementCellRef {
            instance_id: instance.instance_id.clone(),
            x: cell.x,
            y: cell.y,
        });
    }
}

pub(crate) fn catalog_decision(
    matched: &MatchedPiece,
    origin: GridCell,
) -> CatalogPlacementDecision {
    CatalogPlacementDecision {
        piece_id: matched.piece_id.clone(),
        shape_id: matched.shape_id.clone(),
        transform: matched.transform.clone(),
        candidate_rank: matched.candidate_rank as u32,
        candidate_count: matched.candidate_count as u32,
        origin,
        origin_bounds: None,
        lane_constraint: None,
    }
}

pub(crate) fn align_geometry_room_to_catalog(
    geometry: &mut Geometry2dArtifact,
    room: &CatalogRoomSelection,
) {
    let Some(room_id) = room
        .requirement
        .source_refs
        .iter()
        .find_map(|source| source.strip_prefix("geometryRoom:"))
    else {
        return;
    };
    let transformed = transform_cells(
        &room.shape.footprint,
        room.matched.transform.as_str(),
        &room.origin,
    );
    let min_x = transformed
        .iter()
        .map(|cell| cell.x)
        .min()
        .unwrap_or(room.origin.x);
    let min_y = transformed
        .iter()
        .map(|cell| cell.y)
        .min()
        .unwrap_or(room.origin.y);
    let max_x = transformed
        .iter()
        .map(|cell| cell.x)
        .max()
        .unwrap_or(room.origin.x);
    let max_y = transformed
        .iter()
        .map(|cell| cell.y)
        .max()
        .unwrap_or(room.origin.y);
    if let Some(geometry_room) = geometry
        .rooms
        .iter_mut()
        .find(|candidate| candidate.id == room_id)
    {
        geometry_room.rect = GeometryRect {
            x: min_x * GEOMETRY_ROUTE_GRID,
            y: min_y * GEOMETRY_ROUTE_GRID,
            width: (max_x - min_x + 1) * GEOMETRY_ROUTE_GRID,
            height: (max_y - min_y + 1) * GEOMETRY_ROUTE_GRID,
        };
        for port in &mut geometry_room.ports {
            if let Some(exit) = room.matched.exit_map.iter().find(|exit| {
                exit.requirement_exit_id
                    .contains(slugify_label(port.section_id.as_str()).as_str())
            }) {
                port.side = exit.direction.clone();
                port.point = catalog_exit_geometry_point(exit, &room.origin);
            }
        }
    }
}

pub(crate) fn catalog_exit_geometry_point(exit: &MatchedExit, origin: &GridCell) -> GeometryPoint {
    let mut x = exit.x + origin.x;
    let mut y = exit.y + origin.y;
    if exit.direction == "west" {
        x += 1;
    }
    if exit.direction == "north" {
        y += 1;
    }
    GeometryPoint {
        x: x * GEOMETRY_ROUTE_GRID,
        y: y * GEOMETRY_ROUTE_GRID,
    }
}

pub(crate) fn align_geometry_corridor_to_catalog(
    geometry: &mut Geometry2dArtifact,
    section: &CatalogRoutedSection,
    left_origin: &GridCell,
    left_exit: &MatchedExit,
    right_origin: &GridCell,
    right_exit: &MatchedExit,
) {
    if let Some(corridor) = geometry
        .corridors
        .iter_mut()
        .find(|corridor| corridor.physical_section == section.spec.section)
    {
        let mut points = Vec::new();
        append_orthogonal_geometry_point(
            &mut points,
            catalog_exit_geometry_point(left_exit, left_origin),
        );
        for cell in &section.cells {
            append_orthogonal_geometry_point(
                &mut points,
                GeometryPoint {
                    x: cell.x * GEOMETRY_ROUTE_GRID,
                    y: cell.y * GEOMETRY_ROUTE_GRID,
                },
            );
        }
        append_orthogonal_geometry_point(
            &mut points,
            catalog_exit_geometry_point(right_exit, right_origin),
        );
        corridor.points = points;
    }
}

pub(crate) fn append_orthogonal_geometry_point(
    points: &mut Vec<GeometryPoint>,
    point: GeometryPoint,
) {
    let Some(previous) = points.last() else {
        points.push(point);
        return;
    };
    if previous.x == point.x && previous.y == point.y {
        return;
    }
    if previous.x != point.x && previous.y != point.y {
        points.push(GeometryPoint {
            x: point.x,
            y: previous.y,
        });
    }
    points.push(point);
}

pub(crate) fn validate_catalog_geometry_segments(
    geometry: &Geometry2dArtifact,
) -> Result<(), String> {
    for corridor in &geometry.corridors {
        for segment in corridor.points.windows(2) {
            let dx = segment[1].x - segment[0].x;
            let dy = segment[1].y - segment[0].y;
            if (dx != 0 && dy != 0)
                || dx.rem_euclid(GEOMETRY_ROUTE_GRID) != 0
                || dy.rem_euclid(GEOMETRY_ROUTE_GRID) != 0
            {
                return Err(format!(
                    "Catalog corridor {} contains a non-orthogonal grid segment {},{} -> {},{}.",
                    corridor.id, segment[0].x, segment[0].y, segment[1].x, segment[1].y
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn normalize_catalog_geometry_bounds(geometry: &mut Geometry2dArtifact) {
    let max_x = geometry
        .rooms
        .iter()
        .map(|room| room.rect.x + room.rect.width)
        .chain(
            geometry
                .corridors
                .iter()
                .flat_map(|corridor| corridor.points.iter().map(|point| point.x)),
        )
        .max()
        .unwrap_or(geometry.bounds.width);
    let max_y = geometry
        .rooms
        .iter()
        .map(|room| room.rect.y + room.rect.height)
        .chain(
            geometry
                .corridors
                .iter()
                .flat_map(|corridor| corridor.points.iter().map(|point| point.y)),
        )
        .max()
        .unwrap_or(geometry.bounds.height);
    geometry.bounds.width = align_geometry(max_x + 32, GEOMETRY_ROUTE_GRID);
    geometry.bounds.height = align_geometry(max_y + 32, GEOMETRY_ROUTE_GRID);
}

pub(crate) fn direction_between(from: &GridCell, to: &GridCell) -> &'static str {
    match (to.x - from.x, to.y - from.y) {
        (0, -1) => "north",
        (1, 0) => "east",
        (0, 1) => "south",
        (-1, 0) => "west",
        _ => "unknown",
    }
}

pub(crate) fn is_catalog_room_kind(kind: &str) -> bool {
    !matches!(kind, "connector" | "corridor" | "bend" | "junction")
}

pub(crate) fn catalog_generation_failure(
    stage: &str,
    detail: String,
    rooms_placed: usize,
    sections_routed: usize,
    routing_states: u32,
) -> CatalogAwareFailure {
    CatalogAwareFailure {
        classification: "generation_infeasibility".to_owned(),
        stage: stage.to_owned(),
        detail,
        rooms_placed,
        sections_routed,
        routing_states,
    }
}

pub(crate) fn catalog_validation_failure(
    stage: &str,
    report: &ValidationReport,
    rooms_placed: usize,
    sections_routed: usize,
    routing_states: u32,
) -> CatalogAwareFailure {
    CatalogAwareFailure {
        classification: "generation_infeasibility".to_owned(),
        stage: stage.to_owned(),
        detail: report
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.detail))
            .collect::<Vec<_>>()
            .join("; "),
        rooms_placed,
        sections_routed,
        routing_states,
    }
}
