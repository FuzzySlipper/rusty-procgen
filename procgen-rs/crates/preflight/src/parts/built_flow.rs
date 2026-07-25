fn build_validate_flow_command(args: BuildValidateFlowArgs) -> Result<(), String> {
    let candidate = read_flow_candidate(&args.candidate)?;
    let geometry: Geometry2dArtifact = read_json(&args.geometry)?;
    let plan: PieceBuildPlan = read_json(&args.piece_plan)?;
    let placement: PiecePlacement = read_json(&args.piece_placement)?;
    let report = validate_built_flow(&candidate, &geometry, &plan, &placement, &args);
    write_json(&args.out, &report)?;
    if !report.ok {
        return Err(format!(
            "built flow validation failed with {} fatal diagnostic(s); see {}",
            report.fatal_count,
            args.out.display()
        ));
    }
    Ok(())
}

fn read_flow_candidate(path: &Path) -> Result<Candidate, String> {
    let value: JsonValue = read_json(path)?;
    let candidate_value = value.get("candidate").cloned().unwrap_or(value);
    serde_json::from_value(candidate_value)
        .map_err(|error| format!("failed to read candidate graph from {}: {error}", path.display()))
}

fn validate_built_flow(
    candidate: &Candidate,
    geometry: &Geometry2dArtifact,
    plan: &PieceBuildPlan,
    placement: &PiecePlacement,
    args: &BuildValidateFlowArgs,
) -> BuiltFlowValidationReport {
    let mut diagnostics = Vec::new();
    validate_flow_identity(candidate, geometry, plan, placement, &mut diagnostics);
    validate_source_edge_chains(candidate, geometry, plan, placement, &mut diagnostics);
    validate_gate_portals(candidate, placement, &mut diagnostics);
    validate_physical_routes(placement, &mut diagnostics);
    let progression = validate_item_progression(candidate, placement, &mut diagnostics);

    let walkable_cells = placement_walkable_cells(placement);
    let walkable_serializable = walkable_cells
        .iter()
        .map(|(x, y)| GridCell { x: *x, y: *y })
        .collect::<Vec<_>>();
    let fatal_count = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Fatal)
        .count();
    BuiltFlowValidationReport {
        kind: "asha_procgen.validation.built_flow.v1".to_owned(),
        schema_version: 1,
        validation_id: format!("built_flow_validation.{}", placement.placement_id),
        candidate_id: candidate.candidate_id.clone(),
        geometry_id: geometry.geometry_id.clone(),
        plan_id: plan.plan_id.clone(),
        placement_id: placement.placement_id.clone(),
        candidate_ref: display_path(&args.candidate),
        geometry_ref: display_path(&args.geometry),
        piece_plan_ref: display_path(&args.piece_plan),
        piece_placement_ref: display_path(&args.piece_placement),
        walkable_projection: BuiltWalkableProjection {
            source: "piecePlacement.occupiedCells+connectionCells minus closed gatePortal.cells"
                .to_owned(),
            cell_count: walkable_cells.len(),
            projection_hash: hash_json(&walkable_serializable)
                .unwrap_or_else(|_| "hash_error".to_owned()),
        },
        progression,
        portal_count: placement.gate_portals.len(),
        ok: fatal_count == 0,
        fatal_count,
        diagnostics,
    }
}

fn validate_flow_identity(
    candidate: &Candidate,
    geometry: &Geometry2dArtifact,
    plan: &PieceBuildPlan,
    placement: &PiecePlacement,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if geometry.candidate_id != candidate.candidate_id {
        diagnostics.push(fatal(
            "built_flow_candidate_geometry_mismatch",
            None,
            None,
            format!(
                "Geometry candidate {} does not match {}.",
                geometry.candidate_id, candidate.candidate_id
            ),
        ));
    }
    if plan.candidate_id != candidate.candidate_id || plan.geometry_id != geometry.geometry_id {
        diagnostics.push(fatal(
            "built_flow_plan_input_mismatch",
            None,
            None,
            "Piece plan does not identify the supplied candidate and geometry.",
        ));
    }
    if placement.plan_id != plan.plan_id {
        diagnostics.push(fatal(
            "built_flow_placement_plan_mismatch",
            None,
            None,
            format!(
                "Placement plan {} does not match {}.",
                placement.plan_id, plan.plan_id
            ),
        ));
    }
    if placement.corridor_realization != plan.corridor_realization {
        diagnostics.push(fatal(
            "built_flow_corridor_realization_mismatch",
            None,
            None,
            "Piece placement corridor realization mode does not match its piece plan.",
        ));
    }
}

fn validate_source_edge_chains(
    candidate: &Candidate,
    geometry: &Geometry2dArtifact,
    plan: &PieceBuildPlan,
    placement: &PiecePlacement,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source_edges = candidate
        .graph
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect::<BTreeMap<_, _>>();
    let mut corridors_by_edge: BTreeMap<&str, Vec<&GeometryCorridor>> = BTreeMap::new();
    for corridor in &geometry.corridors {
        for source_edge in &corridor.source_edges {
            corridors_by_edge.entry(source_edge.as_str()).or_default().push(corridor);
            if !source_edges.contains_key(source_edge.as_str()) {
                diagnostics.push(fatal(
                    "built_flow_extra_geometry_corridor",
                    None,
                    Some(source_edge.as_str()),
                    format!("Geometry corridor {} has no source graph edge {}.", corridor.id, source_edge),
                ));
            }
        }
    }
    let mut links_by_edge: BTreeMap<&str, Vec<&PieceLink>> = BTreeMap::new();
    for link in &plan.links {
        for source_edge in &link.source_edges {
            links_by_edge.entry(source_edge.as_str()).or_default().push(link);
            if !source_edges.contains_key(source_edge.as_str()) {
                diagnostics.push(fatal(
                    "built_flow_extra_piece_link",
                    None,
                    Some(source_edge.as_str()),
                    format!("Piece link {} has no source graph edge {}.", link.id, source_edge),
                ));
            }
        }
    }
    let glued_by_link = placement
        .glued_exits
        .iter()
        .map(|glued| (glued.link_id.as_str(), glued))
        .collect::<BTreeMap<_, _>>();
    let mut used_exits: BTreeMap<(&str, &str), &str> = BTreeMap::new();

    for edge in &candidate.graph.edges {
        let corridors = corridors_by_edge
            .get(edge.id.as_str())
            .cloned()
            .unwrap_or_default();
        if corridors.len() != 1 {
            diagnostics.push(fatal(
                "built_flow_geometry_corridor_count",
                None,
                Some(edge.id.as_str()),
                format!(
                    "Source edge {} requires exactly one geometry corridor; found {}.",
                    edge.id,
                    corridors.len()
                ),
            ));
            continue;
        }
        let corridor = corridors[0];
        let links = links_by_edge
            .get(edge.id.as_str())
            .cloned()
            .unwrap_or_default();
        if links.is_empty() {
            diagnostics.push(fatal(
                "built_flow_piece_chain_missing",
                None,
                Some(edge.id.as_str()),
                format!("Source edge {} has no piece-link chain.", edge.id),
            ));
            continue;
        }
        for (index, link) in links.iter().enumerate() {
            let traversal_ref = link
                .traversal_refs
                .iter()
                .find(|reference| reference.edge_id == edge.id);
            if link.source_corridor != corridor.id
                || link.source_section != corridor.physical_section
                || !link.source_edges.contains(&edge.id)
                || match plan.corridor_realization {
                    CorridorRealization::Catalog => !link.route_points.is_empty(),
                    CorridorRealization::Hybrid => {
                        catalog_link_route_mismatch(corridor, &links, index)
                    }
                    CorridorRealization::Procedural => link.route_points != corridor.points,
                }
                || traversal_ref.is_none_or(|reference| {
                    reference.traversal != edge.traversal.as_str()
                        || reference.required_item != edge.required_item
                })
            {
                diagnostics.push(fatal(
                    "built_flow_link_provenance_mismatch",
                    None,
                    Some(edge.id.as_str()),
                    format!(
                        "Piece link {} does not preserve corridor, traversal, and required-item provenance.",
                        link.id
                    ),
                ));
            }
            if index > 0 && links[index - 1].to_piece != link.from_piece {
                diagnostics.push(fatal(
                    "built_flow_piece_chain_disconnected",
                    None,
                    Some(edge.id.as_str()),
                    format!(
                        "Piece-link chain breaks between {} and {}.",
                        links[index - 1].id, link.id
                    ),
                ));
            }
            let Some(glued) = glued_by_link.get(link.id.as_str()).copied() else {
                diagnostics.push(fatal(
                    "built_flow_glued_join_missing",
                    None,
                    Some(edge.id.as_str()),
                    format!("Piece link {} has no glued join.", link.id),
                ));
                continue;
            };
            if !glued.source_edges.contains(&edge.id)
                || glued.source_section != corridor.physical_section
                || glued.source_corridor != corridor.id
                || glued.from_exit != link.from_exit
                || glued.to_exit != link.to_exit
                || glued.route_points != link.route_points
            {
                diagnostics.push(fatal(
                    "built_flow_glued_join_mismatch",
                    None,
                    Some(edge.id.as_str()),
                    format!("Glued join {} does not match piece link {}.", glued.id, link.id),
                ));
            }
            if edge.id == link.source_edge {
                for endpoint in [
                    (glued.from_instance.as_str(), glued.from_exit.as_str()),
                    (glued.to_instance.as_str(), glued.to_exit.as_str()),
                ] {
                    if let Some(first_link) = used_exits.insert(endpoint, link.id.as_str()) {
                        diagnostics.push(fatal(
                            "built_flow_instance_exit_reused",
                            None,
                            Some(edge.id.as_str()),
                            format!(
                                "Instance exit {}:{} is reused by {} and {} without shared-junction semantics.",
                                endpoint.0, endpoint.1, first_link, link.id
                            ),
                        ));
                    }
                }
            }
        }
    }
    if placement.glued_exits.len() != plan.links.len() {
        diagnostics.push(fatal(
            "built_flow_glued_join_count",
            None,
            None,
            format!(
                "Placement has {} glued joins for {} piece links.",
                placement.glued_exits.len(),
                plan.links.len()
            ),
        ));
    }
}

fn validate_gate_portals(
    candidate: &Candidate,
    placement: &PiecePlacement,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let edges = candidate
        .graph
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect::<BTreeMap<_, _>>();
    let mut portals_by_edge: BTreeMap<&str, Vec<&GatePortal>> = BTreeMap::new();
    let walkable = placement_walkable_cells(placement);
    for portal in &placement.gate_portals {
        for source_edge in &portal.source_edges {
            portals_by_edge.entry(source_edge.as_str()).or_default().push(portal);
            let Some(edge) = edges.get(source_edge.as_str()).copied() else {
                diagnostics.push(fatal(
                    "built_flow_extra_gate_portal",
                    None,
                    Some(source_edge.as_str()),
                    format!("Portal {} has no source edge {}.", portal.id, source_edge),
                ));
                continue;
            };
            let traversal_ref = portal
                .traversal_refs
                .iter()
                .find(|reference| reference.edge_id == edge.id);
            if traversal_ref.is_none_or(|reference| {
                reference.traversal != edge.traversal.as_str()
                    || reference.required_item != edge.required_item
            }) {
                diagnostics.push(fatal(
                    "built_flow_gate_portal_mismatch",
                    None,
                    Some(edge.id.as_str()),
                    format!("Portal {} does not preserve source edge traversal fields.", portal.id),
                ));
            }
        }
        if portal.cells.is_empty() || portal.width <= 0 {
            diagnostics.push(fatal(
                "built_flow_gate_portal_mismatch",
                None,
                Some(portal.source_edge.as_str()),
                format!("Portal {} does not preserve source gate fields.", portal.id),
            ));
        }
        for cell in &portal.cells {
            if !walkable.contains(&(cell.x, cell.y)) {
                diagnostics.push(fatal(
                    "built_flow_gate_portal_not_walkable",
                    None,
                    Some(portal.source_edge.as_str()),
                    format!(
                        "Portal {} cell {},{} is not in the presentation walkable projection.",
                        portal.id, cell.x, cell.y
                    ),
                ));
            }
        }
    }
    for edge in &candidate.graph.edges {
        let count = portals_by_edge
            .get(edge.id.as_str())
            .map(|portals| portals.len())
            .unwrap_or(0);
        if count != 1 {
            diagnostics.push(fatal(
                "built_flow_gate_portal_count",
                None,
                Some(edge.id.as_str()),
                format!("Source edge {} requires exactly one portal; found {}.", edge.id, count),
            ));
        }
    }
}

fn validate_physical_routes(placement: &PiecePlacement, diagnostics: &mut Vec<Diagnostic>) {
    let section_instances = collect_catalog_section_instances(placement);
    let occupied_by_cell = placement
        .occupied_cells
        .iter()
        .map(|cell| ((cell.x, cell.y), cell.instance_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    for glued in &placement.glued_exits {
        if placement.corridor_realization == CorridorRealization::Catalog {
            let direct = glued.from_cell.x.abs_diff(glued.to_cell.x)
                + glued.from_cell.y.abs_diff(glued.to_cell.y)
                == 1
                && occupied_by_cell
                    .get(&(glued.from_cell.x, glued.from_cell.y))
                    .is_some_and(|owner| *owner == glued.to_instance)
                && occupied_by_cell
                    .get(&(glued.to_cell.x, glued.to_cell.y))
                    .is_some_and(|owner| *owner == glued.from_instance)
                && opposite_direction(glued.from_direction.as_str()) == glued.to_direction;
            if !direct {
                diagnostics.push(fatal(
                    "built_flow_catalog_direct_glue_invalid",
                    None,
                    Some(glued.source_edge.as_str()),
                    format!(
                        "Pure catalog join {} is not a direct opposing exit-to-exit glue.",
                        glued.id
                    ),
                ));
            }
            continue;
        }
        let owner = format!("connection.{}", slugify_label(glued.id.as_str()));
        let mut cells = placement
            .connection_cells
            .iter()
            .filter(|cell| cell.instance_id == owner)
            .map(|cell| (cell.x, cell.y))
            .collect::<BTreeSet<_>>();
        if placement.corridor_realization == CorridorRealization::Hybrid {
            if let Some(instances) = section_instances.get(glued.source_section.as_str()) {
                cells.extend(
                    placement
                        .occupied_cells
                        .iter()
                        .filter(|cell| instances.contains(&cell.instance_id))
                        .map(|cell| (cell.x, cell.y)),
                );
            }
        }
        if !cells.contains(&(glued.from_cell.x, glued.from_cell.y))
            || !cells.contains(&(glued.to_cell.x, glued.to_cell.y))
            || !cells_connected(&cells, placement.grid_connectivity)
        {
            diagnostics.push(fatal(
                "built_flow_physical_route_disconnected",
                None,
                Some(glued.source_edge.as_str()),
                format!("Glued join {} lacks one connected physical route.", glued.id),
            ));
        }
    }
}

fn cells_connected(cells: &BTreeSet<(i32, i32)>, connectivity: GridConnectivity) -> bool {
    let Some(start) = cells.iter().next().copied() else {
        return false;
    };
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(cell) = queue.pop_front() {
        for neighbor in grid_neighbors(cell, connectivity) {
            if cells.contains(&neighbor) && seen.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }
    seen.len() == cells.len()
}

fn validate_item_progression(
    candidate: &Candidate,
    placement: &PiecePlacement,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<BuiltFlowProgressionStep> {
    let mut items = BTreeSet::new();
    let mut progression = Vec::new();
    let max_steps = candidate.graph.nodes.len() + 1;
    for step in 0..max_steps {
        let source_nodes = source_reachable_nodes(candidate, &items);
        let physical_nodes = physical_reachable_nodes(candidate, placement, &items);
        if source_nodes != physical_nodes {
            let missing = source_nodes
                .difference(&physical_nodes)
                .cloned()
                .collect::<Vec<_>>();
            let extra = physical_nodes
                .difference(&source_nodes)
                .cloned()
                .collect::<Vec<_>>();
            diagnostics.push(fatal(
                "built_flow_reachability_mismatch",
                None,
                None,
                format!(
                    "At progression step {} with items [{}], physical flood fill is missing [{}] and adds [{}].",
                    step,
                    items.iter().cloned().collect::<Vec<_>>().join(","),
                    missing.join(","),
                    extra.join(",")
                ),
            ));
        }
        let reachable_edges = candidate
            .graph
            .edges
            .iter()
            .filter(|edge| {
                source_nodes.contains(&edge.from) && edge_open_for_items(edge, &items)
            })
            .map(|edge| edge.id.clone())
            .collect::<Vec<_>>();
        let open_portals = placement
            .gate_portals
            .iter()
            .filter(|portal| portal_open_for_items(portal, &items))
            .map(|portal| portal.id.clone())
            .collect::<Vec<_>>();
        progression.push(BuiltFlowProgressionStep {
            step,
            items: items.iter().cloned().collect(),
            reachable_nodes: source_nodes.iter().cloned().collect(),
            reachable_edges,
            open_portals,
        });
        let before = items.len();
        for node in &candidate.graph.nodes {
            if source_nodes.contains(&node.id) {
                if let Some(item) = &node.grants_item {
                    items.insert(item.clone());
                }
            }
        }
        if items.len() == before {
            return progression;
        }
    }
    diagnostics.push(fatal(
        "built_flow_progression_did_not_converge",
        None,
        None,
        "Item-aware flow progression exceeded the node-count convergence bound.",
    ));
    progression
}

fn source_reachable_nodes(candidate: &Candidate, items: &BTreeSet<String>) -> BTreeSet<String> {
    let mut reachable = candidate
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Start)
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    loop {
        let before = reachable.len();
        for edge in &candidate.graph.edges {
            if reachable.contains(&edge.from) && edge_open_for_items(edge, items) {
                reachable.insert(edge.to.clone());
            }
        }
        if reachable.len() == before {
            return reachable;
        }
    }
}

fn edge_open_for_items(edge: &Edge, items: &BTreeSet<String>) -> bool {
    edge.required_item
        .as_ref()
        .map(|item| items.contains(item))
        .unwrap_or(true)
}

fn portal_open_for_items(portal: &GatePortal, items: &BTreeSet<String>) -> bool {
    portal
        .required_item
        .as_ref()
        .map(|item| items.contains(item))
        .unwrap_or(true)
}

fn catalog_link_route_mismatch(
    corridor: &GeometryCorridor,
    links: &[&PieceLink],
    index: usize,
) -> bool {
    let link = links[index];
    if link.route_points.len() < 2 {
        return true;
    }
    let distances = link
        .route_points
        .iter()
        .map(|point| corridor_distance_at_point(corridor, point))
        .collect::<Option<Vec<_>>>();
    let Some(distances) = distances else {
        return true;
    };
    if distances.windows(2).any(|pair| pair[0] > pair[1]) {
        return true;
    }
    if index == 0 && link.route_points.first() != corridor.points.first() {
        return true;
    }
    if index + 1 == links.len() && link.route_points.last() != corridor.points.last() {
        return true;
    }
    index > 0 && links[index - 1].route_points.last() != link.route_points.first()
}

fn physical_reachable_nodes(
    candidate: &Candidate,
    placement: &PiecePlacement,
    items: &BTreeSet<String>,
) -> BTreeSet<String> {
    let node_instances = placement
        .instances
        .iter()
        .flat_map(|instance| {
            instance.source_refs.iter().filter_map(move |source_ref| {
                source_ref
                    .strip_prefix("node:")
                    .map(|node| (node.to_owned(), instance))
            })
        })
        .collect::<BTreeMap<_, _>>();
    let mut walkable = placement_walkable_cells(placement);
    for portal in placement
        .gate_portals
        .iter()
        .filter(|portal| !portal_open_for_items(portal, items))
    {
        for cell in &portal.cells {
            walkable.remove(&(cell.x, cell.y));
        }
    }
    let mut queue = VecDeque::new();
    let mut seen = BTreeSet::new();
    for node in candidate
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Start)
    {
        if let Some(instance) = node_instances.get(node.id.as_str()) {
            for cell in &instance.occupied_cells {
                if walkable.contains(&(cell.x, cell.y)) && seen.insert((cell.x, cell.y)) {
                    queue.push_back((cell.x, cell.y));
                }
            }
        }
    }
    while let Some(cell) = queue.pop_front() {
        for neighbor in grid_neighbors(cell, placement.grid_connectivity) {
            if walkable.contains(&neighbor) && seen.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }
    node_instances
        .iter()
        .filter(|(_, instance)| {
            instance
                .occupied_cells
                .iter()
                .any(|cell| seen.contains(&(cell.x, cell.y)))
        })
        .map(|(node, _)| node.clone())
        .collect()
}

fn placement_walkable_cells(placement: &PiecePlacement) -> BTreeSet<(i32, i32)> {
    placement
        .occupied_cells
        .iter()
        .chain(placement.connection_cells.iter())
        .map(|cell| (cell.x, cell.y))
        .collect()
}
