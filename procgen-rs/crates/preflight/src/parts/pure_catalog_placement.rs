const PURE_CATALOG_EXHAUSTION_MARKER: &str = "evidence=";

#[derive(Clone, Default)]
struct PureCatalogPlacementState {
    instances: Vec<PieceInstance>,
    occupied_cells: Vec<PlacementCellRef>,
    reserved_cells: Vec<PlacementCellRef>,
    occupied_positions: BTreeMap<(i32, i32), String>,
    reserved_positions: BTreeSet<(i32, i32)>,
    exit_protected_positions: BTreeMap<(i32, i32), BTreeSet<String>>,
    decisions: Vec<CatalogPlacementDecision>,
}

#[derive(Default)]
struct PureCatalogSearchCounters {
    decisions: u32,
    backtracks: u32,
    chain_expansions: u32,
    room_origin_attempts: u32,
    room_rotation_attempts: u32,
    section_expansions: BTreeMap<String, u32>,
    last_failure: Option<PureCatalogFailureEvidence>,
    geometry_failure: Option<PureCatalogFailureEvidence>,
}

#[derive(Clone, Default)]
struct PureCatalogPlacementConstraints {
    origin_bounds: Option<CatalogGridBounds>,
    lane_constraint: Option<CatalogLaneConstraint>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PureCatalogFailureEvidence {
    reason: String,
    detail: String,
    piece_id: String,
    requirement_kind: String,
    shape_id: Option<String>,
    transform: Option<String>,
    required_endpoints: Vec<PureCatalogEndpointEvidence>,
    candidate_endpoints: Vec<PureCatalogEndpointEvidence>,
    fixed_port: Option<PureCatalogFixedPortEvidence>,
    origin_bounds: Option<CatalogGridBounds>,
    lane_envelope: Option<CatalogLaneConstraint>,
    exhausted_families: Vec<String>,
    candidate_count: u32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PureCatalogEndpointEvidence {
    id: String,
    direction: String,
    x: Option<i32>,
    y: Option<i32>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PureCatalogFixedPortEvidence {
    neighbor_piece_id: String,
    neighbor_exit_id: String,
    cell: GridCell,
    direction: String,
    required_opposite_direction: String,
    offset_from_envelope_anchor: Option<GridCell>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PureCatalogExhaustionEvidence {
    kind: String,
    schema_version: u32,
    failure: PureCatalogFailureEvidence,
    budgets: PureCatalogBudgetEvidence,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PureCatalogBudgetEvidence {
    max_decisions: u32,
    decisions: u32,
    max_backtracks: u32,
    backtracks: u32,
    max_chain_expansions_per_section: u32,
    chain_expansions: u32,
}

fn assemble_pure_catalog_placement(
    catalog: &ShapeCatalog,
    plan: &PieceBuildPlan,
    shape_match: &PieceShapeMatchReport,
    args: &BuildAssembleArgs,
) -> Result<PiecePlacement, String> {
    if plan.corridor_realization != CorridorRealization::Catalog {
        return Err("pure catalog assembler requires catalog realization mode".to_owned());
    }
    if shape_match.plan_id != plan.plan_id || shape_match.catalog_id != catalog.catalog_id {
        return Err("pure catalog shape domain does not match its plan and catalog".to_owned());
    }
    if !shape_match.ok {
        return Err(format!(
            "pure catalog coverage rejected {} unmatched requirement(s) in {}: {}",
            shape_match.unmatched_count,
            shape_match.match_id,
            shape_match
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.detail.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    let mut policy_diagnostics = Vec::new();
    validate_catalog_search_policy(&catalog.catalog_search_policy, &mut policy_diagnostics);
    if !policy_diagnostics.is_empty() {
        return Err(format!(
            "shape catalog search policy is invalid: {}",
            policy_diagnostics
                .iter()
                .map(|diagnostic| diagnostic.detail.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }

    let requirements = plan
        .requirements
        .iter()
        .map(|requirement| (requirement.piece_id.as_str(), requirement))
        .collect::<BTreeMap<_, _>>();
    let shapes = catalog
        .shapes
        .iter()
        .map(|shape| (shape.shape_id.as_str(), shape))
        .collect::<BTreeMap<_, _>>();
    let mut domains = BTreeMap::new();
    for requirement in &plan.requirements {
        let mut candidates =
            pure_catalog_match_candidates(catalog, requirement, plan, shape_match.seed);
        if is_room_requirement(requirement) {
            let mut rotations = BTreeSet::new();
            candidates.retain(|candidate| {
                rotations.contains(candidate.transform.as_str())
                    || (rotations.len()
                        < catalog.catalog_search_policy.max_room_rotation_alternatives as usize
                        && rotations.insert(candidate.transform.clone()))
            });
        }
        if candidates.is_empty() {
            return Err(format!(
                "pure catalog coverage missing for {} ({}) with exits [{}]",
                requirement.piece_id,
                requirement.kind,
                requirement
                    .required_exits
                    .iter()
                    .map(|exit| format!("{}:{}", exit.id, exit.direction))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        domains.insert(requirement.piece_id.as_str(), candidates);
    }

    let mut counters = PureCatalogSearchCounters::default();
    let state = search_pure_catalog_placement(
        catalog,
        plan,
        &requirements,
        &shapes,
        &domains,
        PureCatalogPlacementState::default(),
        &mut counters,
    )
    .ok_or_else(|| {
        let failure = counters
            .geometry_failure
            .clone()
            .or_else(|| counters.last_failure.clone())
            .unwrap_or_else(|| {
                pure_catalog_generic_failure(
                    plan.requirements.first(),
                    "no_solution",
                    "No exact exit-anchored placement satisfied the plan.",
                )
            });
        let evidence = PureCatalogExhaustionEvidence {
            kind: "asha_procgen.pure_catalog_exhaustion.v1".to_owned(),
            schema_version: 1,
            failure,
            budgets: PureCatalogBudgetEvidence {
                max_decisions: catalog.catalog_search_policy.max_decisions,
                decisions: counters.decisions,
                max_backtracks: catalog.catalog_search_policy.max_backtracks,
                backtracks: counters.backtracks,
                max_chain_expansions_per_section: catalog
                    .catalog_search_policy
                    .max_chain_expansions_per_section,
                chain_expansions: counters.chain_expansions,
            },
        };
        let serialized = serde_json::to_string(&evidence)
            .unwrap_or_else(|_| "{\"kind\":\"serialization_error\"}".to_owned());
        format!(
            "pure catalog search exhausted after {} decision(s), {} backtrack(s), and {} chain expansion(s); {}{}",
            counters.decisions,
            counters.backtracks,
            counters.chain_expansions,
            PURE_CATALOG_EXHAUSTION_MARKER,
            serialized,
        )
    })?;

    let mut placement = PiecePlacement {
        kind: "asha_procgen.piece_placement.v1".to_owned(),
        schema_version: 1,
        placement_id: format!("piece_placement.{}.pure_catalog", shape_match.match_id),
        plan_id: plan.plan_id.clone(),
        catalog_id: catalog.catalog_id.clone(),
        match_id: shape_match.match_id.clone(),
        corridor_realization: CorridorRealization::Catalog,
        source_plan_ref: display_path(&args.piece_plan),
        source_catalog_ref: display_path(&args.catalog),
        source_match_ref: display_path(&args.shape_match),
        cell_size: catalog.cell_size,
        grid_connectivity: args.connectivity,
        placement_policy: catalog.placement_policy.clone(),
        realization_search: PieceRealizationSearchEvidence {
            realization_scale_tier: 0,
            realization_attempts: 1,
            route_order_attempt: 0,
            route_attempts: 1,
        },
        catalog_search: Some(CatalogSearchEvidence {
            schema_version: 1,
            max_decisions: catalog.catalog_search_policy.max_decisions,
            max_backtracks: catalog.catalog_search_policy.max_backtracks,
            max_chain_expansions_per_section: catalog
                .catalog_search_policy
                .max_chain_expansions_per_section,
            max_room_origin_alternatives: catalog
                .catalog_search_policy
                .max_room_origin_alternatives,
            max_room_rotation_alternatives: catalog
                .catalog_search_policy
                .max_room_rotation_alternatives,
            decisions: counters.decisions,
            backtracks: counters.backtracks,
            chain_expansions: counters.chain_expansions,
            room_origin_attempts: counters.room_origin_attempts,
            room_rotation_attempts: counters.room_rotation_attempts,
            selected: state.decisions,
        }),
        instances: state.instances,
        glued_exits: Vec::new(),
        gate_portals: Vec::new(),
        occupied_cells: state.occupied_cells,
        connection_cells: Vec::new(),
        reserved_cells: state.reserved_cells,
        dangling_exits: Vec::new(),
    };
    placement.glued_exits = derive_glued_exits(plan, &placement.instances)?;
    if placement
        .glued_exits
        .iter()
        .any(|glued| !pure_catalog_glue_is_direct(glued, &placement.occupied_cells))
    {
        return Err(
            "pure catalog search produced a non-direct glued exit; this is a search invariant bug"
                .to_owned(),
        );
    }
    placement.gate_portals = derive_gate_portals(plan, &placement.glued_exits)?;
    Ok(placement)
}

fn consume_pure_catalog_decision(
    catalog: &ShapeCatalog,
    requirement: &PieceRequirement,
    candidate: &MatchedPiece,
    counters: &mut PureCatalogSearchCounters,
) -> bool {
    if counters.decisions >= catalog.catalog_search_policy.max_decisions {
        counters.last_failure = Some(pure_catalog_failure_evidence(
            requirement,
            Some(candidate),
            &PureCatalogPlacementConstraints::default(),
            None,
            "decision_budget_exhausted",
            "The search reached its hard decision budget before another candidate could be tried.",
        ));
        return false;
    }
    counters.decisions += 1;
    true
}

fn pure_catalog_constraints(
    requirement: &PieceRequirement,
    candidate: &MatchedPiece,
    shape: &CatalogShape,
    policy: &PiecePlacementPolicy,
) -> PureCatalogPlacementConstraints {
    let origin_bounds = requirement
        .placement_hints
        .iter()
        .find_map(|hint| parse_geometry_rect(hint))
        .map(|(x, y, width, height)| {
            let transformed = transform_cells(
                &shape.footprint,
                candidate.transform.as_str(),
                &GridCell { x: 0, y: 0 },
            );
            let footprint_width = transformed.iter().map(|cell| cell.x).max().unwrap_or(0) + 1;
            let footprint_height = transformed.iter().map(|cell| cell.y).max().unwrap_or(0) + 1;
            let min_x = x.div_euclid(CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL);
            let min_y = y.div_euclid(CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL);
            let max_exclusive_x =
                div_ceil_i32(x + width, CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL);
            let max_exclusive_y =
                div_ceil_i32(y + height, CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL);
            CatalogGridBounds {
                min_x,
                max_x: max_exclusive_x - footprint_width,
                min_y,
                max_y: max_exclusive_y - footprint_height,
            }
        });
    let lane_hint = requirement
        .placement_hints
        .iter()
        .find_map(|hint| parse_geometry_segment(hint).map(|segment| (hint.clone(), segment)))
        .or_else(|| {
            requirement.placement_hints.iter().find_map(|hint| {
                parse_geometry_point(hint)
                    .map(|point| (hint.clone(), (point.0, point.1, point.0, point.1)))
            })
        });
    let lane_constraint = lane_hint.map(|(source_hint, (from_x, from_y, to_x, to_y))| {
        let from = GridCell {
            x: from_x.div_euclid(CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL),
            y: from_y.div_euclid(CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL),
        };
        let to = GridCell {
            x: to_x.div_euclid(CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL),
            y: to_y.div_euclid(CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL),
        };
        let envelope_cells =
            policy.minimum_clearance_cells * 2 + policy.wall_thickness_cells;
        CatalogLaneConstraint {
            source_hint,
            bounds: CatalogGridBounds {
                min_x: from.x.min(to.x) - envelope_cells,
                max_x: from.x.max(to.x) + envelope_cells,
                min_y: from.y.min(to.y) - envelope_cells,
                max_y: from.y.max(to.y) + envelope_cells,
            },
            from,
            to,
            envelope_cells,
        }
    });
    PureCatalogPlacementConstraints {
        origin_bounds,
        lane_constraint,
    }
}

fn parse_geometry_rect(hint: &str) -> Option<(i32, i32, i32, i32)> {
    let values = hint.strip_prefix("geometryRect:")?;
    let mut parts = values.split(':').map(str::parse::<i32>);
    let parsed = (
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    );
    (parts.next().is_none() && parsed.2 > 0 && parsed.3 > 0).then_some(parsed)
}

fn parse_geometry_point(hint: &str) -> Option<(i32, i32)> {
    let values = hint.strip_prefix("point:")?;
    let mut parts = values.split(':').map(str::parse::<i32>);
    let parsed = (parts.next()?.ok()?, parts.next()?.ok()?);
    (parts.next().is_none()).then_some(parsed)
}

fn parse_geometry_segment(hint: &str) -> Option<(i32, i32, i32, i32)> {
    let values = hint.strip_prefix("segment:")?;
    let mut parts = values.split(':').map(str::parse::<i32>);
    let parsed = (
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    );
    (parts.next().is_none()).then_some(parsed)
}

fn div_ceil_i32(value: i32, divisor: i32) -> i32 {
    let quotient = value.div_euclid(divisor);
    quotient + i32::from(value.rem_euclid(divisor) != 0)
}

fn pure_catalog_origin_candidates(
    constraints: &PureCatalogPlacementConstraints,
    max_alternatives: u32,
) -> Vec<GridCell> {
    let Some(bounds) = constraints.origin_bounds.as_ref() else {
        return vec![GridCell { x: 0, y: 0 }];
    };
    let center_x_twice = bounds.min_x + bounds.max_x;
    let center_y_twice = bounds.min_y + bounds.max_y;
    let mut origins = (bounds.min_y..=bounds.max_y)
        .flat_map(|y| (bounds.min_x..=bounds.max_x).map(move |x| GridCell { x, y }))
        .collect::<Vec<_>>();
    origins.sort_by_key(|origin| {
        (
            (origin.x * 2 - center_x_twice).abs() + (origin.y * 2 - center_y_twice).abs(),
            origin.y,
            origin.x,
        )
    });
    origins.truncate(max_alternatives as usize);
    origins
}

fn pure_catalog_origin_satisfies_constraints(
    shape: &CatalogShape,
    transform: &str,
    origin: &GridCell,
    constraints: &PureCatalogPlacementConstraints,
) -> bool {
    if constraints.origin_bounds.as_ref().is_some_and(|bounds| {
        origin.x < bounds.min_x
            || origin.x > bounds.max_x
            || origin.y < bounds.min_y
            || origin.y > bounds.max_y
    }) {
        return false;
    }
    let occupied = transform_cells(&shape.footprint, transform, origin);
    constraints.lane_constraint.as_ref().is_none_or(|lane| {
        occupied
            .iter()
            .all(|cell| grid_distance_to_segment(cell, &lane.from, &lane.to) <= lane.envelope_cells)
    })
}

fn grid_distance_to_segment(cell: &GridCell, from: &GridCell, to: &GridCell) -> i32 {
    if from.x == to.x {
        (cell.x - from.x).abs()
            + if cell.y < from.y.min(to.y) {
                from.y.min(to.y) - cell.y
            } else if cell.y > from.y.max(to.y) {
                cell.y - from.y.max(to.y)
            } else {
                0
            }
    } else if from.y == to.y {
        (cell.y - from.y).abs()
            + if cell.x < from.x.min(to.x) {
                from.x.min(to.x) - cell.x
            } else if cell.x > from.x.max(to.x) {
                cell.x - from.x.max(to.x)
            } else {
                0
            }
    } else {
        (cell.x - from.x).abs().min((cell.x - to.x).abs())
            + (cell.y - from.y).abs().min((cell.y - to.y).abs())
    }
}

fn pure_catalog_generic_failure(
    requirement: Option<&PieceRequirement>,
    reason: &str,
    detail: &str,
) -> PureCatalogFailureEvidence {
    let Some(requirement) = requirement else {
        return PureCatalogFailureEvidence {
            reason: reason.to_owned(),
            detail: detail.to_owned(),
            piece_id: "unknown".to_owned(),
            requirement_kind: "unknown".to_owned(),
            shape_id: None,
            transform: None,
            required_endpoints: Vec::new(),
            candidate_endpoints: Vec::new(),
            fixed_port: None,
            origin_bounds: None,
            lane_envelope: None,
            exhausted_families: Vec::new(),
            candidate_count: 0,
        };
    };
    pure_catalog_failure_evidence(
        requirement,
        None,
        &PureCatalogPlacementConstraints::default(),
        None,
        reason,
        detail,
    )
}

fn pure_catalog_failure_evidence(
    requirement: &PieceRequirement,
    candidate: Option<&MatchedPiece>,
    constraints: &PureCatalogPlacementConstraints,
    fixed_neighbor: Option<(&PieceLink, &PieceInstance)>,
    reason: &str,
    detail: &str,
) -> PureCatalogFailureEvidence {
    let fixed_port = fixed_neighbor.and_then(|(link, neighbor)| {
        let neighbor_exit_id = if link.from_piece == requirement.piece_id {
            link.to_exit.as_str()
        } else {
            link.from_exit.as_str()
        };
        neighbor
            .exit_map
            .iter()
            .find(|exit| exit.requirement_exit_id == neighbor_exit_id)
            .map(|exit| {
                let anchor = constraints
                    .lane_constraint
                    .as_ref()
                    .map(|lane| lane.from.clone());
                PureCatalogFixedPortEvidence {
                    neighbor_piece_id: neighbor.piece_id.clone(),
                    neighbor_exit_id: exit.requirement_exit_id.clone(),
                    cell: GridCell {
                        x: exit.x,
                        y: exit.y,
                    },
                    direction: exit.direction.clone(),
                    required_opposite_direction: opposite_direction(exit.direction.as_str())
                        .to_owned(),
                    offset_from_envelope_anchor: anchor.map(|anchor| GridCell {
                        x: exit.x - anchor.x,
                        y: exit.y - anchor.y,
                    }),
                }
            })
    });
    let mut exhausted_families = requirement.required_shape_tags.clone();
    exhausted_families.push(requirement.kind.clone());
    exhausted_families.sort();
    exhausted_families.dedup();
    PureCatalogFailureEvidence {
        reason: reason.to_owned(),
        detail: detail.to_owned(),
        piece_id: requirement.piece_id.clone(),
        requirement_kind: requirement.kind.clone(),
        shape_id: candidate.map(|candidate| candidate.shape_id.clone()),
        transform: candidate.map(|candidate| candidate.transform.clone()),
        required_endpoints: requirement
            .required_exits
            .iter()
            .map(|exit| PureCatalogEndpointEvidence {
                id: exit.id.clone(),
                direction: exit.direction.clone(),
                x: None,
                y: None,
            })
            .collect(),
        candidate_endpoints: candidate
            .into_iter()
            .flat_map(|candidate| candidate.exit_map.iter())
            .map(|exit| PureCatalogEndpointEvidence {
                id: exit.requirement_exit_id.clone(),
                direction: exit.direction.clone(),
                x: Some(exit.x),
                y: Some(exit.y),
            })
            .collect(),
        fixed_port,
        origin_bounds: constraints.origin_bounds.clone(),
        lane_envelope: constraints.lane_constraint.clone(),
        exhausted_families,
        candidate_count: candidate.map_or(0, |candidate| candidate.candidate_count as u32),
    }
}

#[allow(clippy::too_many_arguments)]
fn search_pure_catalog_placement(
    catalog: &ShapeCatalog,
    plan: &PieceBuildPlan,
    requirements: &BTreeMap<&str, &PieceRequirement>,
    shapes: &BTreeMap<&str, &CatalogShape>,
    domains: &BTreeMap<&str, Vec<MatchedPiece>>,
    state: PureCatalogPlacementState,
    counters: &mut PureCatalogSearchCounters,
) -> Option<PureCatalogPlacementState> {
    if state.instances.len() == plan.requirements.len() {
        return pure_catalog_all_links_direct(plan, &state).then_some(state);
    }

    let placed = state
        .instances
        .iter()
        .map(|instance| (instance.piece_id.as_str(), instance))
        .collect::<BTreeMap<_, _>>();
    let next = select_pure_catalog_frontier(plan, requirements, domains, &placed)?;
    let requirement = requirements.get(next).copied()?;
    let linked_placed = plan
        .links
        .iter()
        .filter_map(|link| {
            if link.from_piece == next {
                placed.get(link.to_piece.as_str()).map(|instance| (link, *instance))
            } else if link.to_piece == next {
                placed.get(link.from_piece.as_str()).map(|instance| (link, *instance))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    for candidate in domains.get(next)? {
        let shape = shapes.get(candidate.shape_id.as_str()).copied()?;
        let constraints =
            pure_catalog_constraints(requirement, candidate, shape, &catalog.placement_policy);
        let origins = if let Some((link, neighbor)) = linked_placed.first() {
            let Some(origin) = pure_catalog_anchored_origin(link, next, candidate, neighbor) else {
                counters.last_failure = Some(pure_catalog_failure_evidence(
                    requirement,
                    Some(candidate),
                    &constraints,
                    Some((*link, *neighbor)),
                    "incompatible_fixed_port",
                    "The candidate has no exit orientation compatible with its fixed neighbor port.",
                ));
                continue;
            };
            vec![origin]
        } else {
            pure_catalog_origin_candidates(
                &constraints,
                catalog.catalog_search_policy.max_room_origin_alternatives,
            )
        };
        if origins.is_empty() {
            counters.last_failure = Some(pure_catalog_failure_evidence(
                requirement,
                Some(candidate),
                &constraints,
                linked_placed.first().map(|(link, neighbor)| (*link, *neighbor)),
                "origin_zone_empty",
                "The serialized geometry zone cannot contain the transformed prefab footprint.",
            ));
            continue;
        }

        if is_room_requirement(requirement) {
            counters.room_rotation_attempts += 1;
        }
        for origin in origins {
            if !consume_pure_catalog_decision(catalog, requirement, candidate, counters) {
                return None;
            }
            if is_room_requirement(requirement) {
                counters.room_origin_attempts += 1;
            } else if let Some(section) = requirement_physical_section(requirement) {
                let expansions = counters.section_expansions.entry(section.to_owned()).or_default();
                if *expansions
                    >= catalog
                        .catalog_search_policy
                        .max_chain_expansions_per_section
                {
                    counters.last_failure = Some(pure_catalog_failure_evidence(
                        requirement,
                        Some(candidate),
                        &constraints,
                        linked_placed.first().map(|(link, neighbor)| (*link, *neighbor)),
                        "section_expansion_budget_exhausted",
                        "The physical section exhausted its catalog-chain expansion budget.",
                    ));
                    return None;
                }
                *expansions += 1;
                counters.chain_expansions += 1;
            }
            if !pure_catalog_origin_satisfies_constraints(
                shape,
                candidate.transform.as_str(),
                &origin,
                &constraints,
            ) {
                let failure = pure_catalog_failure_evidence(
                    requirement,
                    Some(candidate),
                    &constraints,
                    linked_placed.first().map(|(link, neighbor)| (*link, *neighbor)),
                    "geometry_constraint_rejected",
                    "The fixed-port origin falls outside the serialized room bounds or corridor lane envelope.",
                );
                counters.geometry_failure = Some(failure.clone());
                counters.last_failure = Some(failure);
                continue;
            }
            let mut allowed_contact_instances = linked_placed
                .iter()
                .map(|(_, instance)| instance.instance_id.clone())
                .collect::<BTreeSet<_>>();
            let linked_sections = plan
                .links
                .iter()
                .filter(|link| link.from_piece == next || link.to_piece == next)
                .map(|link| link.source_section.as_str())
                .collect::<BTreeSet<_>>();
            for instance in &state.instances {
                if plan.links.iter().any(|link| {
                    linked_sections.contains(link.source_section.as_str())
                        && (link.from_piece == instance.piece_id
                            || link.to_piece == instance.piece_id)
                }) {
                    allowed_contact_instances.insert(instance.instance_id.clone());
                }
            }
            if !origin_available(
                shape,
                &candidate.exit_map,
                candidate.transform.as_str(),
                &origin,
                &catalog.placement_policy,
                &state.occupied_positions,
                &state.reserved_positions,
                &state.exit_protected_positions,
                &allowed_contact_instances,
            ) {
                counters.last_failure = Some(pure_catalog_failure_evidence(
                    requirement,
                    Some(candidate),
                    &constraints,
                    linked_placed.first().map(|(link, neighbor)| (*link, *neighbor)),
                    "occupancy_rejected",
                    "The candidate violates occupancy, clearance, or a protected prefab port.",
                ));
                continue;
            }
            let mut next_state = state.clone();
            add_pure_catalog_instance(
                requirement,
                candidate,
                shape,
                origin,
                &constraints,
                &catalog.placement_policy,
                &mut next_state,
            );
            if pure_catalog_has_unplanned_contact(plan, &next_state) {
                counters.last_failure = Some(pure_catalog_failure_evidence(
                    requirement,
                    Some(candidate),
                    &constraints,
                    linked_placed.first().map(|(link, neighbor)| (*link, *neighbor)),
                    "undeclared_contact",
                    "The candidate creates an undeclared prefab-to-prefab contact.",
                ));
                continue;
            }
            if !pure_catalog_placed_links_direct(plan, &next_state) {
                counters.last_failure = Some(pure_catalog_failure_evidence(
                    requirement,
                    Some(candidate),
                    &constraints,
                    linked_placed.first().map(|(link, neighbor)| (*link, *neighbor)),
                    "direct_glue_rejected",
                    "The candidate cannot directly glue every already-placed neighbor.",
                ));
                continue;
            }
            if let Some(result) = search_pure_catalog_placement(
                catalog,
                plan,
                requirements,
                shapes,
                domains,
                next_state,
                counters,
            ) {
                return Some(result);
            }
            if counters.backtracks >= catalog.catalog_search_policy.max_backtracks {
                counters.last_failure = Some(pure_catalog_failure_evidence(
                    requirement,
                    Some(candidate),
                    &constraints,
                    linked_placed.first().map(|(link, neighbor)| (*link, *neighbor)),
                    "backtrack_budget_exhausted",
                    "The search reached its hard backtrack budget before another sibling could be explored.",
                ));
                return None;
            }
            counters.backtracks += 1;
        }
    }
    None
}

fn pure_catalog_has_unplanned_contact(
    plan: &PieceBuildPlan,
    state: &PureCatalogPlacementState,
) -> bool {
    let pieces_by_instance = state
        .instances
        .iter()
        .map(|instance| (instance.instance_id.as_str(), instance.piece_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    state.occupied_positions.iter().any(|(position, owner)| {
        [(position.0 + 1, position.1), (position.0, position.1 + 1)]
            .into_iter()
            .filter_map(|neighbor| state.occupied_positions.get(&neighbor))
            .any(|other_owner| {
                if owner == other_owner {
                    return false;
                }
                let Some(owner_piece) = pieces_by_instance.get(owner.as_str()) else {
                    return true;
                };
                let Some(other_piece) = pieces_by_instance.get(other_owner.as_str()) else {
                    return true;
                };
                !plan.links.iter().any(|link| {
                    (link.from_piece == *owner_piece && link.to_piece == *other_piece)
                        || (link.from_piece == *other_piece
                            && link.to_piece == *owner_piece)
                })
            })
    })
}

fn select_pure_catalog_frontier<'a>(
    plan: &'a PieceBuildPlan,
    requirements: &BTreeMap<&'a str, &PieceRequirement>,
    domains: &BTreeMap<&str, Vec<MatchedPiece>>,
    placed: &BTreeMap<&str, &PieceInstance>,
) -> Option<&'a str> {
    if placed.is_empty() {
        if let Some(start) = plan.requirements.iter().find(|requirement| {
            requirement.role == "start"
                || requirement.tags.iter().any(|tag| tag == "start")
        }) {
            return Some(start.piece_id.as_str());
        }
    }
    plan.requirements
        .iter()
        .filter(|requirement| !placed.contains_key(requirement.piece_id.as_str()))
        .map(|requirement| {
            let placed_neighbors = plan
                .links
                .iter()
                .filter(|link| {
                    link.from_piece == requirement.piece_id
                        && placed.contains_key(link.to_piece.as_str())
                        || link.to_piece == requirement.piece_id
                            && placed.contains_key(link.from_piece.as_str())
                })
                .count();
            let start_rank = if requirement.role == "start"
                || requirement.tags.iter().any(|tag| tag == "start")
            {
                0_u8
            } else if is_room_requirement(requirement) {
                1
            } else {
                2
            };
            (
                requirement.piece_id.as_str(),
                placed_neighbors,
                domains
                    .get(requirement.piece_id.as_str())
                    .map(Vec::len)
                    .unwrap_or(usize::MAX),
                start_rank,
            )
        })
        .filter(|(_, placed_neighbors, _, _)| placed.is_empty() || *placed_neighbors > 0)
        .min_by(|left, right| {
            right
                .1
                .cmp(&left.1)
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.3.cmp(&right.3))
                .then_with(|| left.0.cmp(right.0))
        })
        .map(|candidate| candidate.0)
        .or_else(|| {
            requirements
                .keys()
                .find(|piece_id| !placed.contains_key(**piece_id))
                .copied()
        })
}

fn pure_catalog_anchored_origin(
    link: &PieceLink,
    new_piece_id: &str,
    candidate: &MatchedPiece,
    neighbor: &PieceInstance,
) -> Option<GridCell> {
    let (neighbor_exit_id, new_exit_id) = if link.from_piece == new_piece_id {
        (link.to_exit.as_str(), link.from_exit.as_str())
    } else {
        (link.from_exit.as_str(), link.to_exit.as_str())
    };
    let neighbor_exit = neighbor
        .exit_map
        .iter()
        .find(|exit| exit.requirement_exit_id == neighbor_exit_id)?;
    let new_exit = candidate
        .exit_map
        .iter()
        .find(|exit| exit.requirement_exit_id == new_exit_id)?;
    if opposite_direction(neighbor_exit.direction.as_str()) != new_exit.direction {
        return None;
    }
    let (dx, dy) = direction_vector(neighbor_exit.direction.as_str());
    Some(GridCell {
        x: neighbor_exit.x - dx - new_exit.x,
        y: neighbor_exit.y - dy - new_exit.y,
    })
}

fn add_pure_catalog_instance(
    requirement: &PieceRequirement,
    candidate: &MatchedPiece,
    shape: &CatalogShape,
    origin: GridCell,
    constraints: &PureCatalogPlacementConstraints,
    policy: &PiecePlacementPolicy,
    state: &mut PureCatalogPlacementState,
) {
    let instance_id = format!("instance.{}", slugify_label(candidate.piece_id.as_str()));
    let occupied = transform_cells(&shape.footprint, candidate.transform.as_str(), &origin);
    let reserved = transform_cells(&shape.reserved_cells, candidate.transform.as_str(), &origin);
    let exit_protection =
        exit_route_protection(&candidate.exit_map, &origin, &occupied, policy);
    for cell in &occupied {
        state
            .occupied_positions
            .insert((cell.x, cell.y), instance_id.clone());
        state.occupied_cells.push(PlacementCellRef {
            instance_id: instance_id.clone(),
            x: cell.x,
            y: cell.y,
        });
    }
    for cell in &reserved {
        state.reserved_positions.insert((cell.x, cell.y));
        state.reserved_cells.push(PlacementCellRef {
            instance_id: instance_id.clone(),
            x: cell.x,
            y: cell.y,
        });
    }
    for position in exit_protection {
        state
            .exit_protected_positions
            .entry(position)
            .or_default()
            .insert(instance_id.clone());
    }
    let exit_map = candidate
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
    state.instances.push(PieceInstance {
        instance_id,
        piece_id: candidate.piece_id.clone(),
        requirement_kind: candidate.requirement_kind.clone(),
        role: requirement.role.clone(),
        shape_id: candidate.shape_id.clone(),
        transform: candidate.transform.clone(),
        origin: origin.clone(),
        occupied_cells: occupied,
        reserved_cells: reserved,
        exit_map,
        feature_placements: candidate.socket_map.clone(),
        source_requirement_ref: candidate.source_requirement_ref.clone(),
        source_refs: requirement.source_refs.clone(),
        tags: requirement.tags.clone(),
    });
    state.decisions.push(CatalogPlacementDecision {
        piece_id: candidate.piece_id.clone(),
        shape_id: candidate.shape_id.clone(),
        transform: candidate.transform.clone(),
        candidate_rank: candidate.candidate_rank as u32,
        candidate_count: candidate.candidate_count as u32,
        origin,
        origin_bounds: constraints.origin_bounds.clone(),
        lane_constraint: constraints.lane_constraint.clone(),
    });
}

fn pure_catalog_placed_links_direct(
    plan: &PieceBuildPlan,
    state: &PureCatalogPlacementState,
) -> bool {
    let instances = state
        .instances
        .iter()
        .map(|instance| (instance.piece_id.as_str(), instance))
        .collect::<BTreeMap<_, _>>();
    plan.links.iter().all(|link| {
        let (Some(from), Some(to)) = (
            instances.get(link.from_piece.as_str()),
            instances.get(link.to_piece.as_str()),
        ) else {
            return true;
        };
        pure_catalog_link_is_direct(link, from, to, &state.occupied_positions)
    })
}

fn pure_catalog_all_links_direct(
    plan: &PieceBuildPlan,
    state: &PureCatalogPlacementState,
) -> bool {
    pure_catalog_placed_links_direct(plan, state)
        && plan.links.iter().all(|link| {
            state
                .instances
                .iter()
                .any(|instance| instance.piece_id == link.from_piece)
                && state
                    .instances
                    .iter()
                    .any(|instance| instance.piece_id == link.to_piece)
        })
}

fn pure_catalog_link_is_direct(
    link: &PieceLink,
    from: &PieceInstance,
    to: &PieceInstance,
    occupied: &BTreeMap<(i32, i32), String>,
) -> bool {
    let Some(from_exit) = from
        .exit_map
        .iter()
        .find(|exit| exit.requirement_exit_id == link.from_exit)
    else {
        return false;
    };
    let Some(to_exit) = to
        .exit_map
        .iter()
        .find(|exit| exit.requirement_exit_id == link.to_exit)
    else {
        return false;
    };
    from_exit.x.abs_diff(to_exit.x) + from_exit.y.abs_diff(to_exit.y) == 1
        && opposite_direction(from_exit.direction.as_str()) == to_exit.direction
        && occupied
            .get(&(from_exit.x, from_exit.y))
            .is_some_and(|owner| owner == &to.instance_id)
        && occupied
            .get(&(to_exit.x, to_exit.y))
            .is_some_and(|owner| owner == &from.instance_id)
}

fn pure_catalog_glue_is_direct(
    glued: &GluedExit,
    occupied_cells: &[PlacementCellRef],
) -> bool {
    let occupied = occupied_cells
        .iter()
        .map(|cell| ((cell.x, cell.y), cell.instance_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    glued.from_cell.x.abs_diff(glued.to_cell.x)
        + glued.from_cell.y.abs_diff(glued.to_cell.y)
        == 1
        && opposite_direction(glued.from_direction.as_str()) == glued.to_direction
        && occupied
            .get(&(glued.from_cell.x, glued.from_cell.y))
            .is_some_and(|owner| *owner == glued.to_instance)
        && occupied
            .get(&(glued.to_cell.x, glued.to_cell.y))
            .is_some_and(|owner| *owner == glued.from_instance)
}

fn is_room_requirement(requirement: &PieceRequirement) -> bool {
    !matches!(
        requirement.kind.as_str(),
        "connector" | "corridor" | "bend" | "junction"
    )
}

fn requirement_physical_section(requirement: &PieceRequirement) -> Option<&str> {
    requirement
        .source_refs
        .iter()
        .find_map(|reference| reference.strip_prefix("physicalSection:"))
}
