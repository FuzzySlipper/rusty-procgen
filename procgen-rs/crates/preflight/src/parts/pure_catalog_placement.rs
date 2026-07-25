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
    last_failure: String,
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
            let mut origins_per_rotation = BTreeMap::<String, u32>::new();
            candidates.retain(|candidate| {
                let rotation_allowed = rotations.contains(candidate.transform.as_str())
                    || (rotations.len()
                        < catalog.catalog_search_policy.max_room_rotation_alternatives as usize
                        && rotations.insert(candidate.transform.clone()));
                if !rotation_allowed {
                    return false;
                }
                let origin_count = origins_per_rotation
                    .entry(candidate.transform.clone())
                    .or_default();
                if *origin_count
                    >= catalog
                        .catalog_search_policy
                        .max_room_origin_alternatives
                {
                    return false;
                }
                *origin_count += 1;
                true
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
        format!(
            "pure catalog search exhausted after {} decision(s), {} backtrack(s), and {} chain expansion(s); last failure: {}",
            counters.decisions,
            counters.backtracks,
            counters.chain_expansions,
            if counters.last_failure.is_empty() {
                "no exact exit-anchored placement satisfied the plan"
            } else {
                counters.last_failure.as_str()
            }
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
    if counters.decisions >= catalog.catalog_search_policy.max_decisions
        || counters.backtracks >= catalog.catalog_search_policy.max_backtracks
    {
        counters.last_failure = "global decision or backtrack budget exhausted".to_owned();
        return None;
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
        counters.decisions += 1;
        if is_room_requirement(requirement) {
            counters.room_rotation_attempts += 1;
            counters.room_origin_attempts += 1;
        } else if let Some(section) = requirement_physical_section(requirement) {
            let expansions = counters.section_expansions.entry(section.to_owned()).or_default();
            if *expansions
                >= catalog
                    .catalog_search_policy
                    .max_chain_expansions_per_section
            {
                counters.last_failure = format!(
                    "physical section {section} exhausted {} catalog-chain expansion(s)",
                    catalog
                        .catalog_search_policy
                        .max_chain_expansions_per_section
                );
                continue;
            }
            *expansions += 1;
            counters.chain_expansions += 1;
        }
        let shape = shapes.get(candidate.shape_id.as_str()).copied()?;
        let origin = if let Some((link, neighbor)) = linked_placed.first() {
            let Some(origin) = pure_catalog_anchored_origin(link, next, candidate, neighbor) else {
                counters.last_failure = format!(
                    "{} {} has no compatible exit orientation for {}",
                    candidate.shape_id, candidate.transform, next
                );
                continue;
            };
            origin
        } else {
            GridCell { x: 0, y: 0 }
        };
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
            counters.last_failure = format!(
                "{} {} at {},{} violates occupancy, clearance, or a protected port",
                candidate.shape_id, candidate.transform, origin.x, origin.y
            );
            continue;
        }
        let mut next_state = state.clone();
        add_pure_catalog_instance(
            requirement,
            candidate,
            shape,
            origin,
            &catalog.placement_policy,
            &mut next_state,
        );
        if pure_catalog_has_unplanned_contact(plan, &next_state) {
            counters.last_failure = format!(
                "{} {} creates an undeclared prefab-to-prefab contact",
                candidate.shape_id, candidate.transform
            );
            continue;
        }
        if !pure_catalog_placed_links_direct(plan, &next_state) {
            counters.last_failure = format!(
                "{} {} cannot directly glue every already-placed neighbor",
                candidate.shape_id, candidate.transform
            );
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
        counters.backtracks += 1;
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
