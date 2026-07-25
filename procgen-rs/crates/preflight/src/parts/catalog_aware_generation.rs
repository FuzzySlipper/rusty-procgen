#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogAwareGenerationPolicy {
    kind: String,
    schema_version: u32,
    max_generation_attempts: u32,
    initial_room_slack_cells: i32,
    room_slack_growth_cells: i32,
    max_room_candidates: u32,
    max_routing_states_per_section: u32,
    route_margin_cells: i32,
    guide_distance_weight: u32,
    turn_penalty: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogAwareAttemptEvidence {
    attempt: u32,
    room_slack_cells: i32,
    classification: String,
    stage: String,
    detail: String,
    rooms_placed: usize,
    sections_routed: usize,
    routing_states: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogAwareGenerationResult {
    kind: String,
    schema_version: u32,
    ok: bool,
    candidate_id: String,
    policy: CatalogAwareGenerationPolicy,
    attempts: Vec<CatalogAwareAttemptEvidence>,
    selected_attempt: Option<u32>,
    exhausted_classification: Option<String>,
    geometry: Option<Geometry2dArtifact>,
    geometry_validation: Option<ValidationReport>,
    piece_plan: Option<PieceBuildPlan>,
    shape_match: Option<PieceShapeMatchReport>,
    placement: Option<PiecePlacement>,
    placement_validation: Option<ValidationReport>,
    built_flow_validation: Option<BuiltFlowValidationReport>,
}

#[derive(Clone)]
struct CatalogRoomSelection {
    requirement: PieceRequirement,
    matched: MatchedPiece,
    shape: CatalogShape,
    origin: GridCell,
}

#[derive(Clone)]
struct CatalogSectionTerminal {
    requirement: PieceRequirement,
    exit_id: String,
}

#[derive(Clone)]
struct CatalogSectionSpec {
    section: String,
    source_corridor: String,
    template_link: PieceLink,
    left: CatalogSectionTerminal,
    right: CatalogSectionTerminal,
    guide: Vec<GridCell>,
}

#[derive(Clone)]
struct CatalogRoutedSection {
    spec: CatalogSectionSpec,
    cells: Vec<GridCell>,
}

enum CatalogRouteSearch {
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

fn build_realize_catalog_aware_command(args: BuildRealizeCatalogAwareArgs) -> Result<(), String> {
    let candidate = read_flow_candidate(&args.candidate)?;
    let geometry: Geometry2dArtifact = read_json(&args.geometry)?;
    let source_plan: PieceBuildPlan = read_json(&args.piece_plan)?;
    let catalog: ShapeCatalog = read_json(&args.catalog)?;
    let policy: CatalogAwareGenerationPolicy = read_json(&args.policy)?;
    validate_catalog_aware_policy(&policy)?;
    if source_plan.corridor_realization != CorridorRealization::Catalog {
        return Err("catalog-aware generation requires a catalog piece plan".to_owned());
    }
    let mut result = CatalogAwareGenerationResult {
        kind: "asha_procgen.catalog_aware_generation.v1".to_owned(),
        schema_version: 1,
        ok: false,
        candidate_id: candidate.candidate_id.clone(),
        policy: policy.clone(),
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
    for attempt in 0..policy.max_generation_attempts {
        let slack = policy.initial_room_slack_cells.saturating_add(
            policy
                .room_slack_growth_cells
                .saturating_mul(i32::try_from(attempt).unwrap_or(i32::MAX)),
        );
        match realize_catalog_aware_attempt(
            &candidate,
            &geometry,
            &source_plan,
            &catalog,
            &policy,
            &args,
            attempt,
            slack,
        ) {
            Ok((
                realized_geometry,
                geometry_validation,
                plan,
                shape_match,
                placement,
                placement_validation,
                built_flow_validation,
                routing_states,
            )) => {
                result.ok = true;
                result.selected_attempt = Some(attempt);
                result.attempts.push(CatalogAwareAttemptEvidence {
                    attempt,
                    room_slack_cells: slack,
                    classification: "success".to_owned(),
                    stage: "complete".to_owned(),
                    detail: "Catalog rooms and direct-glue corridor chains passed all validators."
                        .to_owned(),
                    rooms_placed: placement
                        .instances
                        .iter()
                        .filter(|instance| is_catalog_room_kind(instance.requirement_kind.as_str()))
                        .count(),
                    sections_routed: plan
                        .links
                        .iter()
                        .map(|link| link.source_section.as_str())
                        .collect::<BTreeSet<_>>()
                        .len(),
                    routing_states,
                });
                result.geometry = Some(realized_geometry);
                result.geometry_validation = Some(geometry_validation);
                result.piece_plan = Some(plan);
                result.shape_match = Some(shape_match);
                result.placement = Some(placement);
                result.placement_validation = Some(placement_validation);
                result.built_flow_validation = Some(built_flow_validation);
                break;
            }
            Err(failure) => {
                final_classification = failure.classification.clone();
                result.attempts.push(CatalogAwareAttemptEvidence {
                    attempt,
                    room_slack_cells: slack,
                    classification: failure.classification,
                    stage: failure.stage,
                    detail: failure.detail,
                    rooms_placed: failure.rooms_placed,
                    sections_routed: failure.sections_routed,
                    routing_states: failure.routing_states,
                });
            }
        }
    }
    if !result.ok {
        result.exhausted_classification = Some(final_classification);
    }
    write_json(&args.out, &result)
}

struct CatalogAwareFailure {
    classification: String,
    stage: String,
    detail: String,
    rooms_placed: usize,
    sections_routed: usize,
    routing_states: u32,
}

#[allow(clippy::type_complexity)]
fn realize_catalog_aware_attempt(
    candidate: &Candidate,
    source_geometry: &Geometry2dArtifact,
    source_plan: &PieceBuildPlan,
    catalog: &ShapeCatalog,
    policy: &CatalogAwareGenerationPolicy,
    args: &BuildRealizeCatalogAwareArgs,
    attempt: u32,
    room_slack_cells: i32,
) -> Result<
    (
        Geometry2dArtifact,
        ValidationReport,
        PieceBuildPlan,
        PieceShapeMatchReport,
        PiecePlacement,
        ValidationReport,
        BuiltFlowValidationReport,
        u32,
    ),
    CatalogAwareFailure,
> {
    let room_requirements = source_plan
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
            catalog,
            source_plan,
            &requirement,
            args.seed,
            policy.max_room_candidates,
        );
        let Some(matched) = candidates
            .get(attempt as usize % candidates.len().max(1))
            .cloned()
        else {
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
        let shape = catalog
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
            room_slack_cells,
        );
        let room_occupied = transform_cells(&shape.footprint, matched.transform.as_str(), &origin);
        let room_reserved =
            transform_cells(&shape.reserved_cells, matched.transform.as_str(), &origin);
        if room_occupied
            .iter()
            .any(|cell| occupied.contains_key(&(cell.x, cell.y)))
        {
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
        rooms.push(CatalogRoomSelection {
            requirement,
            matched,
            shape,
            origin,
        });
    }
    let sections = catalog_section_specs(source_plan, source_geometry)?;
    let room_by_piece = rooms
        .iter()
        .map(|room| (room.requirement.piece_id.as_str(), room))
        .collect::<BTreeMap<_, _>>();
    let bounds = catalog_route_bounds(source_geometry, policy.route_margin_cells);
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
            catalog.placement_policy.minimum_clearance_cells,
            &bounds,
            policy,
        );
        let (cells, states) = match route {
            CatalogRouteSearch::Found {
                cells,
                states_visited,
            } => (cells, states_visited),
            CatalogRouteSearch::NoPath { states_visited } => {
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
                        policy.max_routing_states_per_section,
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
        source_geometry,
        source_plan,
        catalog,
        &rooms,
        &routed,
        args,
    )?;
    normalize_catalog_geometry_bounds(&mut geometry);
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
        candidate: args.candidate.clone(),
        geometry: args.geometry.clone(),
        piece_plan: args.piece_plan.clone(),
        piece_placement: args.out.clone(),
        out: args.out.clone(),
    };
    let built_flow_validation =
        validate_built_flow(candidate, &geometry, &plan, &placement, &flow_args);
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
    Ok((
        geometry,
        geometry_validation,
        plan,
        shape_match,
        placement,
        placement_validation,
        built_flow_validation,
        total_states,
    ))
}

fn validate_catalog_aware_policy(policy: &CatalogAwareGenerationPolicy) -> Result<(), String> {
    if policy.kind != "asha_procgen.catalog_aware_generation_policy.v1"
        || policy.schema_version != 1
    {
        return Err("unsupported catalog-aware generation policy".to_owned());
    }
    if policy.max_generation_attempts == 0 || policy.max_generation_attempts > 16 {
        return Err("maxGenerationAttempts must be from 1 through 16".to_owned());
    }
    if policy.initial_room_slack_cells < 0
        || policy.room_slack_growth_cells < 0
        || policy.initial_room_slack_cells.saturating_add(
            policy.room_slack_growth_cells.saturating_mul(
                i32::try_from(policy.max_generation_attempts - 1).unwrap_or(i32::MAX),
            ),
        ) > 128
    {
        return Err("catalog-aware room slack must remain from 0 through 128 cells".to_owned());
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
    Ok(())
}

fn catalog_exact_room_candidates(
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

fn catalog_room_origin(
    requirement: &PieceRequirement,
    shape: &CatalogShape,
    transform: &str,
    slack_cells: i32,
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
    let min_x = x.div_euclid(CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL) - slack_cells;
    let min_y = y.div_euclid(CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL) - slack_cells;
    let zone_width_cells =
        div_ceil_i32(zone_width, CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL) + slack_cells * 2;
    let zone_height_cells =
        div_ceil_i32(zone_height, CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL) + slack_cells * 2;
    GridCell {
        x: min_x + (zone_width_cells - width).max(0) / 2,
        y: min_y + (zone_height_cells - height).max(0) / 2,
    }
}

fn catalog_section_specs(
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

fn geometry_guide_cells(corridor: &GeometryCorridor) -> Vec<GridCell> {
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

fn catalog_room_exit_cell(
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

fn catalog_route_bounds(geometry: &Geometry2dArtifact, margin: i32) -> CatalogGridBounds {
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
fn route_catalog_section(
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

fn catalog_route_cell_blocked(
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

#[allow(clippy::type_complexity)]
fn materialize_catalog_composition(
    source_geometry: &Geometry2dArtifact,
    source_plan: &PieceBuildPlan,
    catalog: &ShapeCatalog,
    rooms: &[CatalogRoomSelection],
    routed: &[CatalogRoutedSection],
    args: &BuildRealizeCatalogAwareArgs,
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
                args.seed,
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
        kind: "asha_procgen.piece_shape_match.v1".to_owned(),
        schema_version: 1,
        match_id: format!(
            "piece_shape_match.{}.{}.catalog_aware",
            plan.plan_id, args.seed
        ),
        plan_id: plan.plan_id.clone(),
        catalog_id: catalog.catalog_id.clone(),
        seed: args.seed,
        alternative_attempt: 0,
        source_plan_ref: display_path(&args.piece_plan),
        source_catalog_ref: display_path(&args.catalog),
        ok: true,
        unmatched_count: 0,
        matches: matched_pieces,
        rejections: Vec::new(),
        diagnostics: Vec::new(),
    };
    let placement = PiecePlacement {
        kind: "asha_procgen.piece_placement.v1".to_owned(),
        schema_version: 1,
        placement_id: format!("piece_placement.{}.catalog_aware", plan.plan_id),
        plan_id: plan.plan_id.clone(),
        catalog_id: catalog.catalog_id.clone(),
        match_id: shape_match.match_id.clone(),
        corridor_realization: CorridorRealization::Catalog,
        source_plan_ref: display_path(&args.piece_plan),
        source_catalog_ref: display_path(&args.catalog),
        source_match_ref: format!("{}:catalog-aware", display_path(&args.out)),
        cell_size: catalog.cell_size,
        grid_connectivity: GridConnectivity::FourWay,
        placement_policy: catalog.placement_policy.clone(),
        realization_search: PieceRealizationSearchEvidence {
            realization_scale_tier: 0,
            realization_attempts: 1,
            route_order_attempt: 0,
            route_attempts: 1,
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

fn catalog_room_exit<'a>(
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

fn catalog_route_piece(
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
                tags: vec!["catalog_route".to_owned()],
            },
            PieceExitRequirement {
                id: "exit.catalog_route.out".to_owned(),
                direction: forward_direction.to_owned(),
                width: 12,
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

fn catalog_piece_instance(
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

fn append_instance_cells(
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

fn catalog_decision(matched: &MatchedPiece, origin: GridCell) -> CatalogPlacementDecision {
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

fn align_geometry_room_to_catalog(geometry: &mut Geometry2dArtifact, room: &CatalogRoomSelection) {
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

fn catalog_exit_geometry_point(exit: &MatchedExit, origin: &GridCell) -> GeometryPoint {
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

fn align_geometry_corridor_to_catalog(
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

fn append_orthogonal_geometry_point(points: &mut Vec<GeometryPoint>, point: GeometryPoint) {
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

fn validate_catalog_geometry_segments(geometry: &Geometry2dArtifact) -> Result<(), String> {
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

fn normalize_catalog_geometry_bounds(geometry: &mut Geometry2dArtifact) {
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

fn direction_between(from: &GridCell, to: &GridCell) -> &'static str {
    match (to.x - from.x, to.y - from.y) {
        (0, -1) => "north",
        (1, 0) => "east",
        (0, 1) => "south",
        (-1, 0) => "west",
        _ => "unknown",
    }
}

fn is_catalog_room_kind(kind: &str) -> bool {
    !matches!(kind, "connector" | "corridor" | "bend" | "junction")
}

fn catalog_generation_failure(
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

fn catalog_validation_failure(
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
