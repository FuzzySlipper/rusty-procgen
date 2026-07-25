fn build_assemble_command(args: BuildAssembleArgs) -> Result<(), String> {
    let catalog: ShapeCatalog = read_json(&args.catalog)?;
    let plan: PieceBuildPlan = read_json(&args.piece_plan)?;
    let shape_match: PieceShapeMatchReport = read_json(&args.shape_match)?;
    let placement = assemble_piece_placement(&catalog, &plan, &shape_match, &args)?;
    write_json(&args.out, &placement)
}

fn build_validate_placement_command(args: ReportOutArgs) -> Result<(), String> {
    let placement: PiecePlacement = read_json(&args.state)?;
    let report = validate_piece_placement(&placement);
    write_json(&args.out, &report)?;
    if !report.ok {
        return Err(format!(
            "piece placement validation failed with {} fatal diagnostic(s); see {}",
            report.fatal_count,
            args.out.display()
        ));
    }
    Ok(())
}

fn assemble_piece_placement(
    catalog: &ShapeCatalog,
    plan: &PieceBuildPlan,
    shape_match: &PieceShapeMatchReport,
    args: &BuildAssembleArgs,
) -> Result<PiecePlacement, String> {
    if plan.corridor_realization == CorridorRealization::Catalog {
        return assemble_pure_catalog_placement(catalog, plan, shape_match, args);
    }
    const REALIZATION_SCALE_MULTIPLIERS: [i32; 2] = [1, 2];
    let mut last_error = "no piece realization was attempted".to_owned();
    for (realization_scale_tier, scale_multiplier) in
        REALIZATION_SCALE_MULTIPLIERS.into_iter().enumerate()
    {
        match assemble_piece_placement_attempt(
            catalog,
            plan,
            shape_match,
            args,
            scale_multiplier,
        ) {
            Ok(mut placement) => {
                placement
                    .realization_search
                    .realization_scale_tier = realization_scale_tier as u32;
                placement.realization_search.realization_attempts =
                    realization_scale_tier as u32 + 1;
                return Ok(placement);
            }
            Err(error)
                if error.starts_with("piece route search exhausted")
                    || error.starts_with("no placement origin satisfies") =>
            {
                last_error = error;
            }
            Err(error) => return Err(error),
        }
    }
    Err(format!(
        "piece realization search exhausted after {} scale tier(s); last realization failure: {last_error}",
        REALIZATION_SCALE_MULTIPLIERS.len()
    ))
}

fn assemble_piece_placement_attempt(
    catalog: &ShapeCatalog,
    plan: &PieceBuildPlan,
    shape_match: &PieceShapeMatchReport,
    args: &BuildAssembleArgs,
    scale_multiplier: i32,
) -> Result<PiecePlacement, String> {
    let mut policy_diagnostics = Vec::new();
    validate_piece_placement_policy(&catalog.placement_policy, &mut policy_diagnostics);
    if policy_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Fatal)
    {
        return Err(format!(
            "shape catalog placement policy is invalid: {}",
            policy_diagnostics
                .iter()
                .map(|diagnostic| diagnostic.detail.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    if shape_match.plan_id != plan.plan_id {
        return Err(format!(
            "shape match plan {} does not match piece plan {}",
            shape_match.plan_id, plan.plan_id
        ));
    }
    if shape_match.catalog_id != catalog.catalog_id {
        return Err(format!(
            "shape match catalog {} does not match catalog {}",
            shape_match.catalog_id, catalog.catalog_id
        ));
    }
    if !shape_match.ok {
        return Err(format!(
            "shape match report {} has {} unmatched requirement(s)",
            shape_match.match_id, shape_match.unmatched_count
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

    let mut instances = Vec::new();
    let mut occupied_cells = Vec::new();
    let mut reserved_cells = Vec::new();
    let mut occupied_positions: BTreeMap<(i32, i32), String> = BTreeMap::new();
    let mut reserved_positions: BTreeSet<(i32, i32)> = BTreeSet::new();
    let mut exit_protected_positions: BTreeMap<(i32, i32), BTreeSet<String>> = BTreeMap::new();
    let mut ordered_matches = shape_match.matches.iter().collect::<Vec<_>>();
    ordered_matches.sort_by_key(|matched| match matched.requirement_kind.as_str() {
        "connector" => 1_u8,
        "corridor" | "bend" => 2_u8,
        _ => 0_u8,
    });
    for (index, matched) in ordered_matches.into_iter().enumerate() {
        let Some(requirement) = requirements.get(matched.piece_id.as_str()).copied() else {
            return Err(format!(
                "shape match references missing requirement {}",
                matched.piece_id
            ));
        };
        let Some(shape) = shapes.get(matched.shape_id.as_str()).copied() else {
            return Err(format!(
                "shape match references missing catalog shape {}",
                matched.shape_id
            ));
        };
        let instance_id = format!("instance.{}", slugify_label(matched.piece_id.as_str()));
        let allowed_contact_instances =
            catalog_section_contact_instances(plan, requirement, &instances);
        let desired_origin = catalog_route_piece_origin(plan, matched, requirement, &instances)
            .or_else(|| {
                linked_piece_origin(
                    plan,
                    matched,
                    requirement,
                    &instances,
                    &catalog.placement_policy,
                )
            })
        .unwrap_or_else(|| {
            scaled_desired_origin(
                desired_origin_for_requirement(requirement, index),
                &catalog.placement_policy,
                scale_multiplier,
            )
        });
        let origin = find_available_origin(
            shape,
            &matched.exit_map,
            matched.transform.as_str(),
            &desired_origin,
            &catalog.placement_policy,
            &occupied_positions,
            &reserved_positions,
            &exit_protected_positions,
            &allowed_contact_instances,
        )
        .ok_or_else(|| {
            format!(
                "no placement origin satisfies {} clearance cell(s) for {} near {},{}",
                catalog.placement_policy.minimum_clearance_cells,
                matched.piece_id,
                desired_origin.x,
                desired_origin.y
            )
        })?;
        let occupied = transform_cells(&shape.footprint, matched.transform.as_str(), &origin);
        let reserved = transform_cells(&shape.reserved_cells, matched.transform.as_str(), &origin);
        let exit_protection = exit_route_protection(
            &matched.exit_map,
            &origin,
            &occupied,
            &catalog.placement_policy,
        );
        for cell in &occupied {
            occupied_positions.insert((cell.x, cell.y), instance_id.clone());
        }
        for cell in &reserved {
            reserved_positions.insert((cell.x, cell.y));
        }
        for position in exit_protection {
            exit_protected_positions
                .entry(position)
                .or_default()
                .insert(instance_id.clone());
        }

        occupied_cells.extend(occupied.iter().map(|cell| PlacementCellRef {
            instance_id: instance_id.clone(),
            x: cell.x,
            y: cell.y,
        }));
        reserved_cells.extend(reserved.iter().map(|cell| PlacementCellRef {
            instance_id: instance_id.clone(),
            x: cell.x,
            y: cell.y,
        }));

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
        instances.push(PieceInstance {
            instance_id,
            piece_id: matched.piece_id.clone(),
            requirement_kind: matched.requirement_kind.clone(),
            role: requirement.role.clone(),
            shape_id: matched.shape_id.clone(),
            transform: matched.transform.clone(),
            origin,
            occupied_cells: occupied,
            reserved_cells: reserved,
            exit_map,
            feature_placements: matched.socket_map.clone(),
            source_requirement_ref: matched.source_requirement_ref.clone(),
            source_refs: requirement.source_refs.clone(),
            tags: requirement.tags.clone(),
        });
    }

    let mut placement = PiecePlacement {
        kind: "asha_procgen.piece_placement.v1".to_owned(),
        schema_version: 1,
        placement_id: format!("piece_placement.{}", shape_match.match_id),
        plan_id: plan.plan_id.clone(),
        catalog_id: catalog.catalog_id.clone(),
        match_id: shape_match.match_id.clone(),
        corridor_realization: plan.corridor_realization,
        source_plan_ref: display_path(&args.piece_plan),
        source_catalog_ref: display_path(&args.catalog),
        source_match_ref: display_path(&args.shape_match),
        cell_size: catalog.cell_size,
        grid_connectivity: args.connectivity,
        placement_policy: catalog.placement_policy.clone(),
        realization_search: PieceRealizationSearchEvidence {
            realization_scale_tier: (scale_multiplier - 1) as u32,
            ..PieceRealizationSearchEvidence::default()
        },
        catalog_search: None,
        instances,
        glued_exits: Vec::new(),
        gate_portals: Vec::new(),
        occupied_cells,
        connection_cells: Vec::new(),
        reserved_cells,
        dangling_exits: Vec::new(),
    };
    placement.glued_exits = derive_glued_exits(plan, &placement.instances)?;
    placement.gate_portals = derive_gate_portals(plan, &placement.glued_exits)?;
    if placement.corridor_realization == CorridorRealization::Catalog {
        placement.realization_search.route_attempts = 1;
    } else {
        let (connection_cells, route_search) = derive_connection_cells(&placement)?;
        placement.connection_cells = connection_cells;
        placement.realization_search.route_order_attempt = route_search.route_order_attempt;
        placement.realization_search.route_attempts = route_search.route_attempts;
    }
    Ok(placement)
}

fn catalog_section_contact_instances(
    plan: &PieceBuildPlan,
    requirement: &PieceRequirement,
    instances: &[PieceInstance],
) -> BTreeSet<String> {
    if !plan.corridor_realization.uses_catalog_pieces()
        || !matches!(requirement.kind.as_str(), "corridor" | "bend" | "junction")
    {
        return BTreeSet::new();
    }
    let Some(source_section) = requirement
        .source_refs
        .iter()
        .find_map(|reference| reference.strip_prefix("physicalSection:"))
    else {
        return BTreeSet::new();
    };
    let endpoint_pieces = plan
        .links
        .iter()
        .filter(|link| link.source_section == source_section)
        .flat_map(|link| [link.from_piece.as_str(), link.to_piece.as_str()])
        .collect::<BTreeSet<_>>();
    instances
        .iter()
        .filter(|instance| {
            endpoint_pieces.contains(instance.piece_id.as_str())
                || instance
                    .source_refs
                    .iter()
                    .any(|reference| reference == &format!("physicalSection:{source_section}"))
        })
        .map(|instance| instance.instance_id.clone())
        .collect()
}

fn catalog_route_piece_origin(
    plan: &PieceBuildPlan,
    matched: &MatchedPiece,
    requirement: &PieceRequirement,
    instances: &[PieceInstance],
) -> Option<GridCell> {
    if !plan.corridor_realization.uses_catalog_pieces()
        || !matches!(requirement.kind.as_str(), "corridor" | "bend" | "junction")
    {
        return None;
    }
    let source_section = requirement
        .source_refs
        .iter()
        .find_map(|reference| reference.strip_prefix("physicalSection:"))?;
    let links = plan
        .links
        .iter()
        .filter(|link| link.source_section == source_section)
        .collect::<Vec<_>>();
    let first_link = links.first()?;
    let last_link = links.last()?;
    let instances_by_piece = instances
        .iter()
        .map(|instance| (instance.piece_id.as_str(), instance))
        .collect::<BTreeMap<_, _>>();
    let from_room = instances_by_piece.get(first_link.from_piece.as_str())?;
    let to_room = instances_by_piece.get(last_link.to_piece.as_str())?;
    let from_exit = from_room
        .exit_map
        .iter()
        .find(|exit| exit.requirement_exit_id == first_link.from_exit)?;
    let to_exit = to_room
        .exit_map
        .iter()
        .find(|exit| exit.requirement_exit_id == last_link.to_exit)?;
    let mut route_points = Vec::new();
    for link in &links {
        for point in &link.route_points {
            if route_points.last() != Some(point) {
                route_points.push(point.clone());
            }
        }
    }
    let source_start = route_points.first()?;
    let source_end = route_points.last()?;
    let anchor = requirement.placement_hints.iter().find_map(|hint| {
        let values = parse_i32_parts(hint.strip_prefix("point:")?);
        (values.len() == 2).then(|| GeometryPoint {
            x: values[0],
            y: values[1],
        })
    })?;
    let target = map_source_point_to_target(
        &anchor,
        source_start,
        source_end,
        &GridCell {
            x: from_exit.x,
            y: from_exit.y,
        },
        &GridCell {
            x: to_exit.x,
            y: to_exit.y,
        },
    );
    let exit_count = matched.exit_map.len().max(1) as i32;
    let local_anchor_x = matched.exit_map.iter().map(|exit| exit.x).sum::<i32>() / exit_count;
    let local_anchor_y = matched.exit_map.iter().map(|exit| exit.y).sum::<i32>() / exit_count;
    Some(GridCell {
        x: target.x - local_anchor_x,
        y: target.y - local_anchor_y,
    })
}

fn map_source_point_to_target(
    point: &GeometryPoint,
    source_start: &GeometryPoint,
    source_end: &GeometryPoint,
    target_start: &GridCell,
    target_end: &GridCell,
) -> GridCell {
    let source_dx = source_end.x - source_start.x;
    let source_dy = source_end.y - source_start.y;
    let target_dx = target_end.x - target_start.x;
    let target_dy = target_end.y - target_start.y;
    GridCell {
        x: if source_dx == 0 {
            target_start.x + (point.x - source_start.x) / 8
        } else {
            target_start.x + rounded_ratio(point.x - source_start.x, target_dx, source_dx)
        },
        y: if source_dy == 0 {
            target_start.y + (point.y - source_start.y) / 8
        } else {
            target_start.y + rounded_ratio(point.y - source_start.y, target_dy, source_dy)
        },
    }
}

fn linked_piece_origin(
    plan: &PieceBuildPlan,
    matched: &MatchedPiece,
    requirement: &PieceRequirement,
    instances: &[PieceInstance],
    policy: &PiecePlacementPolicy,
) -> Option<GridCell> {
    if requirement.kind != "connector" {
        return None;
    }
    let instances_by_piece = instances
        .iter()
        .map(|instance| (instance.piece_id.as_str(), instance))
        .collect::<BTreeMap<_, _>>();
    let gap = policy.minimum_clearance_cells + policy.wall_thickness_cells + 1;
    let prefer_room_neighbor = requirement
        .placement_hints
        .iter()
        .any(|hint| hint == "glue:from_room" || hint == "glue:to_room");
    let mut anchors = Vec::new();
    for link in &plan.links {
        let (neighbor_piece, neighbor_exit_id, current_exit_id) =
            if link.to_piece == matched.piece_id {
                (
                    link.from_piece.as_str(),
                    link.from_exit.as_str(),
                    link.to_exit.as_str(),
                )
            } else if link.from_piece == matched.piece_id {
                (
                    link.to_piece.as_str(),
                    link.to_exit.as_str(),
                    link.from_exit.as_str(),
                )
            } else {
                continue;
            };
        let Some(neighbor) = instances_by_piece.get(neighbor_piece).copied() else {
            continue;
        };
        let Some(neighbor_exit) = neighbor
            .exit_map
            .iter()
            .find(|exit| exit.requirement_exit_id == neighbor_exit_id)
        else {
            continue;
        };
        let Some(current_exit) = matched
            .exit_map
            .iter()
            .find(|exit| exit.requirement_exit_id == current_exit_id)
        else {
            continue;
        };
        if opposite_direction(neighbor_exit.direction.as_str()) != current_exit.direction {
            continue;
        }
        let (direction_x, direction_y) = direction_vector(neighbor_exit.direction.as_str());
        let room_neighbor = neighbor
            .source_refs
            .iter()
            .any(|source_ref| source_ref.starts_with("geometryRoom:"));
        anchors.push((
            room_neighbor,
            link.id.as_str(),
            GridCell {
                x: neighbor_exit.x + direction_x * gap - current_exit.x,
                y: neighbor_exit.y + direction_y * gap - current_exit.y,
            },
        ));
    }
    anchors.sort_by(|left, right| {
        if prefer_room_neighbor {
            right.0.cmp(&left.0)
        } else {
            left.0.cmp(&right.0)
        }
        .then_with(|| left.1.cmp(right.1))
    });
    anchors.into_iter().next().map(|(_, _, origin)| origin)
}

fn scaled_desired_origin(
    origin: GridCell,
    policy: &PiecePlacementPolicy,
    scale_multiplier: i32,
) -> GridCell {
    let scale = (policy.minimum_clearance_cells + policy.wall_thickness_cells)
        .saturating_mul(scale_multiplier);
    GridCell {
        x: origin.x.saturating_mul(scale),
        y: origin.y.saturating_mul(scale),
    }
}

fn desired_origin_for_requirement(requirement: &PieceRequirement, index: usize) -> GridCell {
    const GEOMETRY_CELL_SIZE: i32 = 24;
    for hint in &requirement.placement_hints {
        if let Some(rest) = hint.strip_prefix("geometryRect:") {
            let values = parse_i32_parts(rest);
            if values.len() == 4 {
                return GridCell {
                    x: values[0] / GEOMETRY_CELL_SIZE,
                    y: values[1] / GEOMETRY_CELL_SIZE,
                };
            }
        }
        if let Some(rest) = hint.strip_prefix("segment:") {
            let values = parse_i32_parts(rest);
            if values.len() == 4 {
                return GridCell {
                    x: ((values[0] + values[2]) / 2) / GEOMETRY_CELL_SIZE,
                    y: ((values[1] + values[3]) / 2) / GEOMETRY_CELL_SIZE,
                };
            }
        }
        if let Some(rest) = hint.strip_prefix("bend:").or_else(|| hint.strip_prefix("point:")) {
            let values = parse_i32_parts(rest);
            if values.len() == 2 {
                return GridCell {
                    x: values[0] / GEOMETRY_CELL_SIZE,
                    y: values[1] / GEOMETRY_CELL_SIZE,
                };
            }
        }
    }
    GridCell {
        x: (index as i32 % 24) * 5,
        y: (index as i32 / 24) * 5,
    }
}

fn parse_i32_parts(value: &str) -> Vec<i32> {
    value
        .split(':')
        .filter_map(|part| part.parse::<i32>().ok())
        .collect()
}

fn find_available_origin(
    shape: &CatalogShape,
    exit_map: &[MatchedExit],
    transform: &str,
    desired_origin: &GridCell,
    policy: &PiecePlacementPolicy,
    occupied_positions: &BTreeMap<(i32, i32), String>,
    reserved_positions: &BTreeSet<(i32, i32)>,
    exit_protected_positions: &BTreeMap<(i32, i32), BTreeSet<String>>,
    allowed_contact_instances: &BTreeSet<String>,
) -> Option<GridCell> {
    for radius in 0_i32..=120 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue;
                }
                let origin = GridCell {
                    x: desired_origin.x + dx,
                    y: desired_origin.y + dy,
                };
                if origin_available(
                    shape,
                    exit_map,
                    transform,
                    &origin,
                    policy,
                    occupied_positions,
                    reserved_positions,
                    exit_protected_positions,
                    allowed_contact_instances,
                ) {
                    return Some(origin);
                }
            }
        }
    }
    None
}

fn origin_available(
    shape: &CatalogShape,
    exit_map: &[MatchedExit],
    transform: &str,
    origin: &GridCell,
    policy: &PiecePlacementPolicy,
    occupied_positions: &BTreeMap<(i32, i32), String>,
    reserved_positions: &BTreeSet<(i32, i32)>,
    exit_protected_positions: &BTreeMap<(i32, i32), BTreeSet<String>>,
    allowed_contact_instances: &BTreeSet<String>,
) -> bool {
    let occupied = transform_cells(&shape.footprint, transform, origin);
    let reserved = transform_cells(&shape.reserved_cells, transform, origin);
    let exit_protection = exit_route_protection(exit_map, origin, &occupied, policy);
    occupied.iter().all(|cell| {
        !occupied_positions.contains_key(&(cell.x, cell.y))
            && !reserved_positions.contains(&(cell.x, cell.y))
            && exit_protected_positions
                .get(&(cell.x, cell.y))
                .is_none_or(|owners| owners.iter().all(|owner| allowed_contact_instances.contains(owner)))
            && clearance_available(
                (cell.x, cell.y),
                policy.minimum_clearance_cells,
                occupied_positions,
                allowed_contact_instances,
            )
    }) && reserved.iter().all(|cell| {
        !occupied_positions.contains_key(&(cell.x, cell.y))
            && !reserved_positions.contains(&(cell.x, cell.y))
            && exit_protected_positions
                .get(&(cell.x, cell.y))
                .is_none_or(|owners| owners.iter().all(|owner| allowed_contact_instances.contains(owner)))
    }) && exit_protection.iter().all(|position| {
        occupied_positions
            .get(position)
            .is_none_or(|owner| allowed_contact_instances.contains(owner))
            && !reserved_positions.contains(position)
            && exit_protected_positions
                .get(position)
                .is_none_or(|owners| owners.iter().all(|owner| allowed_contact_instances.contains(owner)))
    })
}

fn exit_route_protection(
    exit_map: &[MatchedExit],
    origin: &GridCell,
    occupied: &[GridCell],
    policy: &PiecePlacementPolicy,
) -> BTreeSet<(i32, i32)> {
    let occupied = occupied
        .iter()
        .map(|cell| (cell.x, cell.y))
        .collect::<BTreeSet<_>>();
    let approach_length = policy.minimum_clearance_cells + policy.wall_thickness_cells;
    let mut protected = BTreeSet::new();
    for exit in exit_map {
        let exit_position = (exit.x + origin.x, exit.y + origin.y);
        let (direction_x, direction_y) = direction_vector(exit.direction.as_str());
        for step in 0..=approach_length {
            let lane = (
                exit_position.0 + direction_x * step,
                exit_position.1 + direction_y * step,
            );
            for dy in -policy.wall_thickness_cells..=policy.wall_thickness_cells {
                for dx in -policy.wall_thickness_cells..=policy.wall_thickness_cells {
                    if dx.abs() + dy.abs() > policy.wall_thickness_cells {
                        continue;
                    }
                    let position = (lane.0 + dx, lane.1 + dy);
                    if !occupied.contains(&position) {
                        protected.insert(position);
                    }
                }
            }
        }
    }
    protected
}

fn clearance_available(
    cell: (i32, i32),
    minimum_clearance_cells: i32,
    occupied_positions: &BTreeMap<(i32, i32), String>,
    allowed_contact_instances: &BTreeSet<String>,
) -> bool {
    for dy in -minimum_clearance_cells..=minimum_clearance_cells {
        for dx in -minimum_clearance_cells..=minimum_clearance_cells {
            if dx.abs() + dy.abs() > minimum_clearance_cells {
                continue;
            }
            if occupied_positions
                .get(&(cell.0 + dx, cell.1 + dy))
                .is_some_and(|owner| !allowed_contact_instances.contains(owner))
            {
                return false;
            }
        }
    }
    true
}

fn derive_glued_exits(
    plan: &PieceBuildPlan,
    instances: &[PieceInstance],
) -> Result<Vec<GluedExit>, String> {
    let instances_by_piece = instances
        .iter()
        .map(|instance| (instance.piece_id.as_str(), instance))
        .collect::<BTreeMap<_, _>>();
    let mut glued = Vec::new();
    let mut consumed: BTreeMap<(String, String), String> = BTreeMap::new();
    for link in &plan.links {
        let from = instances_by_piece
            .get(link.from_piece.as_str())
            .copied()
            .ok_or_else(|| format!("link {} references missing from piece {}", link.id, link.from_piece))?;
        let to = instances_by_piece
            .get(link.to_piece.as_str())
            .copied()
            .ok_or_else(|| format!("link {} references missing to piece {}", link.id, link.to_piece))?;
        let from_exit = required_instance_exit(from, link.from_exit.as_str(), link.id.as_str())?;
        let to_exit = required_instance_exit(to, link.to_exit.as_str(), link.id.as_str())?;
        if plan.corridor_realization == CorridorRealization::Catalog
            && link.route_points.is_empty()
            && opposite_direction(from_exit.direction.as_str()) != to_exit.direction
        {
            return Err(format!(
                "link {} exit directions do not oppose: {} {} vs {} {}",
                link.id, link.from_exit, from_exit.direction, link.to_exit, to_exit.direction
            ));
        }
        consume_instance_exit(&mut consumed, from, from_exit, link)?;
        consume_instance_exit(&mut consumed, to, to_exit, link)?;
        glued.push(GluedExit {
            id: format!("glue.{}", slugify_label(link.id.as_str())),
            link_id: link.id.clone(),
            from_instance: from.instance_id.clone(),
            from_exit: from_exit.requirement_exit_id.clone(),
            from_cell: GridCell {
                x: from_exit.x,
                y: from_exit.y,
            },
            from_direction: from_exit.direction.clone(),
            from_width: from_exit.width,
            to_instance: to.instance_id.clone(),
            to_exit: to_exit.requirement_exit_id.clone(),
            to_cell: GridCell {
                x: to_exit.x,
                y: to_exit.y,
            },
            to_direction: to_exit.direction.clone(),
            to_width: to_exit.width,
            source_section: link.source_section.clone(),
            source_corridor: link.source_corridor.clone(),
            source_edge: link.source_edge.clone(),
            source_edges: link.source_edges.clone(),
            traversal_refs: link.traversal_refs.clone(),
            source_ref: link.source_ref.clone(),
            traversal: link.traversal.clone(),
            required_item: link.required_item.clone(),
            tags: link.tags.clone(),
            route_points: link.route_points.clone(),
        });
    }
    Ok(glued)
}

fn required_instance_exit<'a>(
    instance: &'a PieceInstance,
    requirement_exit_id: &str,
    link_id: &str,
) -> Result<&'a MatchedExit, String> {
    instance
        .exit_map
        .iter()
        .find(|exit| exit.requirement_exit_id == requirement_exit_id)
        .ok_or_else(|| {
            format!(
                "link {} requires missing exit {} on instance {}",
                link_id, requirement_exit_id, instance.instance_id
            )
        })
}

fn consume_instance_exit(
    consumed: &mut BTreeMap<(String, String), String>,
    instance: &PieceInstance,
    exit: &MatchedExit,
    link: &PieceLink,
) -> Result<(), String> {
    let key = (instance.instance_id.clone(), exit.requirement_exit_id.clone());
    if let Some(first_link) = consumed.insert(key, link.id.clone()) {
        return Err(format!(
            "instance exit {}:{} is reused by links {} and {}; shared portals require explicit junction semantics",
            instance.instance_id, exit.requirement_exit_id, first_link, link.id
        ));
    }
    Ok(())
}

fn derive_gate_portals(
    plan: &PieceBuildPlan,
    glued_exits: &[GluedExit],
) -> Result<Vec<GatePortal>, String> {
    let mut first_link_by_section: BTreeMap<&str, &PieceLink> = BTreeMap::new();
    for link in &plan.links {
        first_link_by_section.entry(link.source_section.as_str()).or_insert(link);
    }
    let glued_by_link = glued_exits
        .iter()
        .map(|glued| (glued.link_id.as_str(), glued))
        .collect::<BTreeMap<_, _>>();
    let mut portals = Vec::new();
    for (source_section, link) in first_link_by_section {
        let glued = glued_by_link.get(link.id.as_str()).copied().ok_or_else(|| {
            format!("physical section {} has no glued portal link {}", source_section, link.id)
        })?;
        portals.push(GatePortal {
            id: format!("gate_portal.{}", slugify_label(source_section)),
            source_section: source_section.to_owned(),
            source_edge: link.source_edge.clone(),
            source_edges: link.source_edges.clone(),
            traversal_refs: link.traversal_refs.clone(),
            source_corridor: link.source_corridor.clone(),
            link_id: link.id.clone(),
            from_piece: link.from_piece.clone(),
            from_instance: glued.from_instance.clone(),
            to_piece: link.to_piece.clone(),
            to_instance: glued.to_instance.clone(),
            cells: vec![glued.from_cell.clone()],
            orientation: glued.from_direction.clone(),
            width: glued.from_width,
            traversal: link.traversal.clone(),
            required_item: link.required_item.clone(),
            provenance: vec![
                format!("physicalSection:{}", source_section),
                format!("edges:{}", link.source_edges.join(",")),
                format!("geometryCorridor:{}", link.source_corridor),
                format!("pieceLink:{}", link.id),
                format!("gluedExit:{}", glued.id),
            ],
        });
    }
    Ok(portals)
}

type SectionRoomEndpoints =
    BTreeMap<String, BTreeMap<String, Vec<(GridCell, String)>>>;
type RoutedSections = BTreeMap<(i32, i32), BTreeSet<String>>;

fn collect_section_room_endpoints(placement: &PiecePlacement) -> SectionRoomEndpoints {
    let room_instances = placement
        .instances
        .iter()
        .filter(|instance| {
            instance
                .source_refs
                .iter()
                .any(|reference| reference.starts_with("geometryRoom:"))
        })
        .map(|instance| instance.instance_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut section_room_endpoints = SectionRoomEndpoints::new();
    for glued in &placement.glued_exits {
        for (instance, cell, direction) in [
            (
                glued.from_instance.as_str(),
                &glued.from_cell,
                glued.from_direction.as_str(),
            ),
            (
                glued.to_instance.as_str(),
                &glued.to_cell,
                glued.to_direction.as_str(),
            ),
        ] {
            if room_instances.contains(instance) {
                section_room_endpoints
                    .entry(glued.source_section.clone())
                    .or_default()
                    .entry(instance.to_owned())
                    .or_default()
                    .push((cell.clone(), direction.to_owned()));
            }
        }
    }
    section_room_endpoints
}

const PIECE_ROUTE_ORDER_COUNT: u32 = 4;

fn derive_connection_cells(
    placement: &PiecePlacement,
) -> Result<(Vec<PlacementCellRef>, PieceRealizationSearchEvidence), String> {
    let mut base_order = placement.glued_exits.iter().collect::<Vec<_>>();
    let mut orders = Vec::new();
    orders.push(base_order.clone());

    let mut reversed = base_order.clone();
    reversed.reverse();
    orders.push(reversed);

    base_order.sort_by(|left, right| {
        right
            .from_cell
            .x
            .abs_diff(right.to_cell.x)
            .saturating_add(right.from_cell.y.abs_diff(right.to_cell.y))
            .cmp(
                &left
                    .from_cell
                    .x
                    .abs_diff(left.to_cell.x)
                    .saturating_add(left.from_cell.y.abs_diff(left.to_cell.y)),
            )
            .then_with(|| left.id.cmp(&right.id))
    });
    orders.push(base_order.clone());
    base_order.reverse();
    orders.push(base_order);

    let mut last_error = "no piece route order was attempted".to_owned();
    let mut route_attempts = 0_u32;
    for (route_order_attempt, order) in orders
        .into_iter()
        .take(PIECE_ROUTE_ORDER_COUNT as usize)
        .enumerate()
    {
        route_attempts += 1;
        match try_derive_connection_cells(placement, &order) {
            Ok(cells) => {
                return Ok((
                    cells,
                    PieceRealizationSearchEvidence {
                        realization_scale_tier: 0,
                        realization_attempts: 0,
                        route_order_attempt: route_order_attempt as u32,
                        route_attempts,
                    },
                ));
            }
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "piece route search exhausted after {route_attempts} deterministic order attempt(s); last route failure: {last_error}"
    ))
}

fn try_derive_connection_cells(
    placement: &PiecePlacement,
    glued_order: &[&GluedExit],
) -> Result<Vec<PlacementCellRef>, String> {
    let instances = placement
        .instances
        .iter()
        .map(|instance| (instance.instance_id.as_str(), instance))
        .collect::<BTreeMap<_, _>>();
    let occupied_by_cell = placement
        .occupied_cells
        .iter()
        .map(|cell| ((cell.x, cell.y), cell.instance_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let reserved = placement
        .reserved_cells
        .iter()
        .map(|cell| (cell.x, cell.y))
        .collect::<BTreeSet<_>>();
    let section_room_endpoints = collect_section_room_endpoints(placement);
    let section_instances = collect_catalog_section_instances(placement);
    let no_section_instances = BTreeSet::new();
    let bounds = placement_route_bounds(placement);
    let mut cells = Vec::new();
    let mut routed_sections = RoutedSections::new();
    for glued in glued_order {
        let Some(from) = instances.get(glued.from_instance.as_str()) else {
            continue;
        };
        let Some(to) = instances.get(glued.to_instance.as_str()) else {
            continue;
        };
        let instance_id = format!("connection.{}", slugify_label(glued.id.as_str()));
        let same_section_instances = section_instances
            .get(glued.source_section.as_str())
            .unwrap_or(&no_section_instances);
        let bridge = route_connection_for_mode(
            placement,
            glued,
            &occupied_by_cell,
            &reserved,
            &routed_sections,
            &section_room_endpoints,
            same_section_instances,
            bounds,
        );
        let Some(mut bridge) = bridge else {
            let without_routed_sections = route_connection_for_mode(
                placement,
                glued,
                &occupied_by_cell,
                &reserved,
                &RoutedSections::new(),
                &section_room_endpoints,
                same_section_instances,
                bounds,
            )
            .is_some();
            return Err(format!(
                "no clearance-safe connection route exists for glued exit {} between {} at {},{} ({}) and {} at {},{} ({}); {}",
                glued.id,
                from.instance_id,
                glued.from_cell.x,
                glued.from_cell.y,
                glued.from_direction,
                to.instance_id,
                glued.to_cell.x,
                glued.to_cell.y,
                glued.to_direction,
                if without_routed_sections {
                    "blocked by previously routed physical sections"
                } else {
                    "piece occupancy or reservations block every route"
                },
            ));
        };
        if placement.corridor_realization == CorridorRealization::Hybrid {
            bridge = stitch_catalog_section_pieces(
                placement,
                glued,
                bridge,
                &occupied_by_cell,
                &reserved,
                &routed_sections,
                &section_room_endpoints,
                same_section_instances,
            )
            .ok_or_else(|| {
                format!(
                    "catalog physical section {} could not connect every planned prefab to its route",
                    glued.source_section
                )
            })?;
        }
        for cell in bridge {
            routed_sections
                .entry((cell.x, cell.y))
                .or_default()
                .insert(glued.source_section.clone());
            if occupied_by_cell
                .get(&(cell.x, cell.y))
                .is_some_and(|owner| same_section_instances.contains(*owner))
            {
                continue;
            }
            cells.push(PlacementCellRef {
                instance_id: instance_id.clone(),
                x: cell.x,
                y: cell.y,
            });
        }
    }
    Ok(cells)
}

#[allow(clippy::too_many_arguments)]
fn stitch_catalog_section_pieces(
    placement: &PiecePlacement,
    glued: &GluedExit,
    mut bridge: Vec<GridCell>,
    occupied_by_cell: &BTreeMap<(i32, i32), &str>,
    reserved: &BTreeSet<(i32, i32)>,
    routed_sections: &RoutedSections,
    section_room_endpoints: &SectionRoomEndpoints,
    same_section_instances: &BTreeSet<String>,
) -> Option<Vec<GridCell>> {
    let waypoints = map_geometry_lane_to_room_exits(glued)?;
    let lane = rasterize_orthogonal_waypoints(&waypoints)?;
    let lane_positions = lane
        .iter()
        .map(|cell| (cell.x, cell.y))
        .collect::<BTreeSet<_>>();
    let envelope = placement.placement_policy.minimum_clearance_cells * 2
        + placement.placement_policy.wall_thickness_cells;
    let lane_distances = build_lane_distance_map(&lane_positions, envelope);
    let mut connected = bridge
        .iter()
        .map(|cell| (cell.x, cell.y))
        .collect::<BTreeSet<_>>();
    for instance_id in same_section_instances {
        let starts = occupied_by_cell
            .iter()
            .filter(|(_, owner)| **owner == instance_id)
            .map(|(position, _)| *position)
            .collect::<BTreeSet<_>>();
        if starts.is_empty()
            || starts.iter().any(|position| {
                connected.contains(position)
                    || grid_neighbors(*position, GridConnectivity::FourWay)
                        .iter()
                        .any(|neighbor| connected.contains(neighbor))
            })
        {
            connected.extend(starts);
            continue;
        }
        let stitch = route_catalog_stitch(
            placement,
            glued,
            &starts,
            &connected,
            occupied_by_cell,
            reserved,
            routed_sections,
            section_room_endpoints,
            same_section_instances,
            &lane_distances,
        )?;
        for cell in stitch {
            let position = (cell.x, cell.y);
            if connected.insert(position) {
                bridge.push(cell);
            }
        }
        connected.extend(starts);
    }
    Some(bridge)
}

#[allow(clippy::too_many_arguments)]
fn route_catalog_stitch(
    placement: &PiecePlacement,
    glued: &GluedExit,
    starts: &BTreeSet<(i32, i32)>,
    connected: &BTreeSet<(i32, i32)>,
    occupied_by_cell: &BTreeMap<(i32, i32), &str>,
    reserved: &BTreeSet<(i32, i32)>,
    routed_sections: &RoutedSections,
    section_room_endpoints: &SectionRoomEndpoints,
    same_section_instances: &BTreeSet<String>,
    lane_distances: &BTreeMap<(i32, i32), u32>,
) -> Option<Vec<GridCell>> {
    let min_x = connected.iter().map(|position| position.0).min()?;
    let max_x = connected.iter().map(|position| position.0).max()?;
    let min_y = connected.iter().map(|position| position.1).min()?;
    let max_y = connected.iter().map(|position| position.1).max()?;
    let distance_to_connected_bounds = |position: (i32, i32)| {
        let dx = if position.0 < min_x {
            position.0.abs_diff(min_x)
        } else if position.0 > max_x {
            position.0.abs_diff(max_x)
        } else {
            0
        };
        let dy = if position.1 < min_y {
            position.1.abs_diff(min_y)
        } else if position.1 > max_y {
            position.1.abs_diff(max_y)
        } else {
            0
        };
        dx + dy
    };
    let mut frontier = BinaryHeap::new();
    let mut best_cost = HashMap::new();
    let mut previous = HashMap::new();
    for start in starts {
        let heuristic = distance_to_connected_bounds(*start);
        frontier.push(Reverse((heuristic, 0_u32, start.0, start.1)));
        best_cost.insert(*start, 0_u32);
    }
    let mut target = None;
    while let Some(Reverse((_, cost, x, y))) = frontier.pop() {
        let position = (x, y);
        if cost > best_cost.get(&position).copied().unwrap_or(u32::MAX) {
            continue;
        }
        if connected.contains(&position) {
            target = Some(position);
            break;
        }
        for neighbor in grid_neighbors(position, GridConnectivity::FourWay) {
            let Some(lane_distance) = lane_distances.get(&neighbor).copied() else {
                continue;
            };
            if !connected.contains(&neighbor)
                && !bridge_position_available(
                    neighbor,
                    glued.from_instance.as_str(),
                    glued.to_instance.as_str(),
                    &glued.from_cell,
                    glued.from_direction.as_str(),
                    &glued.to_cell,
                    glued.to_direction.as_str(),
                    occupied_by_cell,
                    reserved,
                    placement.placement_policy.wall_thickness_cells,
                    placement.placement_policy.minimum_clearance_cells,
                    routed_sections,
                    section_room_endpoints,
                    glued.source_section.as_str(),
                    same_section_instances,
                )
            {
                continue;
            }
            let next_cost = cost.saturating_add(1 + lane_distance);
            if next_cost >= best_cost.get(&neighbor).copied().unwrap_or(u32::MAX) {
                continue;
            }
            best_cost.insert(neighbor, next_cost);
            previous.insert(neighbor, position);
            let priority = next_cost.saturating_add(distance_to_connected_bounds(neighbor));
            frontier.push(Reverse((priority, next_cost, neighbor.0, neighbor.1)));
        }
    }
    let mut cursor = target?;
    let mut route = vec![GridCell {
        x: cursor.0,
        y: cursor.1,
    }];
    while !starts.contains(&cursor) {
        cursor = *previous.get(&cursor)?;
        route.push(GridCell {
            x: cursor.0,
            y: cursor.1,
        });
    }
    route.reverse();
    Some(route)
}

fn collect_catalog_section_instances(
    placement: &PiecePlacement,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut sections = BTreeMap::new();
    for instance in &placement.instances {
        for source_ref in &instance.source_refs {
            if let Some(section) = source_ref.strip_prefix("physicalSection:") {
                sections
                    .entry(section.to_owned())
                    .or_insert_with(BTreeSet::new)
                    .insert(instance.instance_id.clone());
            }
        }
    }
    sections
}

fn collect_catalog_section_contact_instances(
    placement: &PiecePlacement,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut sections = collect_catalog_section_instances(placement);
    for glued in &placement.glued_exits {
        let instances = sections.entry(glued.source_section.clone()).or_default();
        instances.insert(glued.from_instance.clone());
        instances.insert(glued.to_instance.clone());
    }
    sections
}

fn route_connection_for_mode(
    placement: &PiecePlacement,
    glued: &GluedExit,
    occupied_by_cell: &BTreeMap<(i32, i32), &str>,
    reserved: &BTreeSet<(i32, i32)>,
    routed_sections: &RoutedSections,
    section_room_endpoints: &SectionRoomEndpoints,
    same_section_instances: &BTreeSet<String>,
    bounds: (i32, i32, i32, i32),
) -> Option<Vec<GridCell>> {
    if !glued.route_points.is_empty() {
        realize_planned_lane_connection(
            glued,
            occupied_by_cell,
            reserved,
            placement.placement_policy.wall_thickness_cells,
            placement.placement_policy.minimum_clearance_cells,
            routed_sections,
            section_room_endpoints,
            same_section_instances,
        )
    } else {
        route_instance_connection(
            glued,
            placement.grid_connectivity,
            occupied_by_cell,
            reserved,
            placement.placement_policy.wall_thickness_cells,
            placement.placement_policy.minimum_clearance_cells,
            routed_sections,
            section_room_endpoints,
            same_section_instances,
            bounds,
        )
    }
}

fn realize_planned_lane_connection(
    glued: &GluedExit,
    occupied_by_cell: &BTreeMap<(i32, i32), &str>,
    reserved: &BTreeSet<(i32, i32)>,
    wall_clearance: i32,
    corridor_clearance: i32,
    routed_sections: &RoutedSections,
    section_room_endpoints: &SectionRoomEndpoints,
    same_section_instances: &BTreeSet<String>,
) -> Option<Vec<GridCell>> {
    let waypoints = map_geometry_lane_to_room_exits(glued)?;
    let lane = rasterize_orthogonal_waypoints(&waypoints)?;
    let lane_positions = lane
        .iter()
        .map(|cell| (cell.x, cell.y))
        .collect::<BTreeSet<_>>();
    let envelope = corridor_clearance * 2 + wall_clearance;
    let lane_distances = build_lane_distance_map(&lane_positions, envelope);
    route_bridge_cells_in_lane(
        glued,
        occupied_by_cell,
        reserved,
        wall_clearance,
        corridor_clearance,
        routed_sections,
        section_room_endpoints,
        same_section_instances,
        &lane_distances,
    )
}

fn build_lane_distance_map(
    lane_positions: &BTreeSet<(i32, i32)>,
    envelope: i32,
) -> BTreeMap<(i32, i32), u32> {
    let mut distances = BTreeMap::new();
    for lane in lane_positions {
        for dy in -envelope..=envelope {
            for dx in -envelope..=envelope {
                let distance = dx.abs() + dy.abs();
                if distance > envelope {
                    continue;
                }
                distances
                    .entry((lane.0 + dx, lane.1 + dy))
                    .and_modify(|existing: &mut u32| {
                        *existing = (*existing).min(distance as u32);
                    })
                    .or_insert(distance as u32);
            }
        }
    }
    distances
}

fn map_geometry_lane_to_room_exits(glued: &GluedExit) -> Option<Vec<GridCell>> {
    let source_start = glued.route_points.first()?;
    let source_end = glued.route_points.last()?;
    if glued.route_points.len() < 2 {
        return None;
    }
    let source_dx = source_end.x - source_start.x;
    let source_dy = source_end.y - source_start.y;
    let target_dx = glued.to_cell.x - glued.from_cell.x;
    let target_dy = glued.to_cell.y - glued.from_cell.y;
    let mut waypoints = Vec::new();
    for point in &glued.route_points {
        let x = if source_dx == 0 {
            glued.from_cell.x + (point.x - source_start.x) / 8
        } else {
            glued.from_cell.x
                + rounded_ratio(point.x - source_start.x, target_dx, source_dx)
        };
        let y = if source_dy == 0 {
            glued.from_cell.y + (point.y - source_start.y) / 8
        } else {
            glued.from_cell.y
                + rounded_ratio(point.y - source_start.y, target_dy, source_dy)
        };
        let cell = GridCell { x, y };
        if waypoints.last() != Some(&cell) {
            waypoints.push(cell);
        }
    }
    waypoints[0] = glued.from_cell.clone();
    let last = waypoints.len() - 1;
    waypoints[last] = glued.to_cell.clone();
    normalize_orthogonal_waypoints(&mut waypoints, glued);
    Some(waypoints)
}

fn rounded_ratio(value: i32, numerator: i32, denominator: i32) -> i32 {
    let product = i64::from(value) * i64::from(numerator);
    let denominator = i64::from(denominator);
    let quotient = product / denominator;
    let remainder = product % denominator;
    let rounded = if remainder.abs() * 2 >= denominator.abs() {
        quotient
            + if product.signum() == denominator.signum() {
                1
            } else {
                -1
            }
    } else {
        quotient
    };
    rounded.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn normalize_orthogonal_waypoints(waypoints: &mut Vec<GridCell>, glued: &GluedExit) {
    let mut normalized = vec![waypoints[0].clone()];
    for (index, to) in waypoints.iter().enumerate().skip(1) {
        let from = normalized.last().expect("normalized has a start");
        if from.x != to.x && from.y != to.y {
            let source_from = &glued.route_points[index - 1];
            let source_to = &glued.route_points[index];
            let bend = if source_from.y == source_to.y {
                GridCell {
                    x: to.x,
                    y: from.y,
                }
            } else {
                GridCell {
                    x: from.x,
                    y: to.y,
                }
            };
            if normalized.last() != Some(&bend) {
                normalized.push(bend);
            }
        }
        if normalized.last() != Some(to) {
            normalized.push(to.clone());
        }
    }
    *waypoints = normalized;
}

fn rasterize_orthogonal_waypoints(waypoints: &[GridCell]) -> Option<Vec<GridCell>> {
    let first = waypoints.first()?.clone();
    let mut cells = vec![first];
    for pair in waypoints.windows(2) {
        let from = &pair[0];
        let to = &pair[1];
        if from.x != to.x && from.y != to.y {
            return None;
        }
        let step_x = (to.x - from.x).signum();
        let step_y = (to.y - from.y).signum();
        let mut cursor = from.clone();
        while cursor != *to {
            cursor.x += step_x;
            cursor.y += step_y;
            if cells.last() != Some(&cursor) {
                cells.push(cursor.clone());
            }
        }
    }
    Some(cells)
}

#[allow(clippy::too_many_arguments)]
fn route_bridge_cells_in_lane(
    glued: &GluedExit,
    occupied_by_cell: &BTreeMap<(i32, i32), &str>,
    reserved: &BTreeSet<(i32, i32)>,
    wall_clearance: i32,
    corridor_clearance: i32,
    routed_sections: &RoutedSections,
    section_room_endpoints: &SectionRoomEndpoints,
    same_section_instances: &BTreeSet<String>,
    lane_distances: &BTreeMap<(i32, i32), u32>,
) -> Option<Vec<GridCell>> {
    let start = (glued.from_cell.x, glued.from_cell.y);
    let end = (glued.to_cell.x, glued.to_cell.y);
    let mut queue = VecDeque::from([start]);
    let mut previous = BTreeMap::new();
    let mut seen = BTreeSet::from([start]);
    while let Some(position) = queue.pop_front() {
        if position == end {
            break;
        }
        let mut neighbors = grid_neighbors(position, GridConnectivity::FourWay);
        neighbors.sort_by_key(|neighbor| {
            lane_distances
                .get(neighbor)
                .copied()
                .unwrap_or(u32::MAX)
                .saturating_mul(1_000)
                .saturating_add(neighbor.0.abs_diff(end.0) + neighbor.1.abs_diff(end.1))
        });
        for neighbor in neighbors {
            if !seen.insert(neighbor) || (neighbor != end && !lane_distances.contains_key(&neighbor))
            {
                continue;
            }
            if neighbor != end
                && !bridge_position_available(
                    neighbor,
                    glued.from_instance.as_str(),
                    glued.to_instance.as_str(),
                    &glued.from_cell,
                    glued.from_direction.as_str(),
                    &glued.to_cell,
                    glued.to_direction.as_str(),
                    occupied_by_cell,
                    reserved,
                    wall_clearance,
                    corridor_clearance,
                    routed_sections,
                    section_room_endpoints,
                    glued.source_section.as_str(),
                    same_section_instances,
                )
            {
                continue;
            }
            previous.insert(neighbor, position);
            queue.push_back(neighbor);
        }
    }
    if !seen.contains(&end) {
        return None;
    }
    let mut path = vec![GridCell { x: end.0, y: end.1 }];
    let mut cursor = end;
    while cursor != start {
        cursor = *previous.get(&cursor)?;
        path.push(GridCell {
            x: cursor.0,
            y: cursor.1,
        });
    }
    path.reverse();
    Some(path)
}

fn distance_to_lane(
    position: (i32, i32),
    lane_positions: &BTreeSet<(i32, i32)>,
) -> u32 {
    lane_positions
        .iter()
        .map(|lane| position.0.abs_diff(lane.0) + position.1.abs_diff(lane.1))
        .min()
        .unwrap_or(u32::MAX)
}

fn route_instance_connection(
    glued: &GluedExit,
    connectivity: GridConnectivity,
    occupied_by_cell: &BTreeMap<(i32, i32), &str>,
    reserved: &BTreeSet<(i32, i32)>,
    wall_clearance: i32,
    corridor_clearance: i32,
    routed_sections: &RoutedSections,
    section_room_endpoints: &SectionRoomEndpoints,
    same_section_instances: &BTreeSet<String>,
    bounds: (i32, i32, i32, i32),
) -> Option<Vec<GridCell>> {
    route_bridge_cells(
        &glued.from_cell,
        &glued.to_cell,
        glued.from_instance.as_str(),
        glued.to_instance.as_str(),
        glued.from_direction.as_str(),
        glued.to_direction.as_str(),
        connectivity,
        occupied_by_cell,
        reserved,
        wall_clearance,
        corridor_clearance,
        routed_sections,
        section_room_endpoints,
        glued.source_section.as_str(),
        same_section_instances,
        bounds,
    )
}

fn placement_route_bounds(placement: &PiecePlacement) -> (i32, i32, i32, i32) {
    let min_x = placement
        .occupied_cells
        .iter()
        .map(|cell| cell.x)
        .min()
        .unwrap_or(0)
        - 80;
    let max_x = placement
        .occupied_cells
        .iter()
        .map(|cell| cell.x)
        .max()
        .unwrap_or(0)
        + 80;
    let min_y = placement
        .occupied_cells
        .iter()
        .map(|cell| cell.y)
        .min()
        .unwrap_or(0)
        - 80;
    let max_y = placement
        .occupied_cells
        .iter()
        .map(|cell| cell.y)
        .max()
        .unwrap_or(0)
        + 80;
    (min_x, max_x, min_y, max_y)
}

fn route_bridge_cells(
    start: &GridCell,
    end: &GridCell,
    from_instance: &str,
    to_instance: &str,
    from_direction: &str,
    to_direction: &str,
    connectivity: GridConnectivity,
    occupied_by_cell: &BTreeMap<(i32, i32), &str>,
    reserved: &BTreeSet<(i32, i32)>,
    wall_clearance: i32,
    corridor_clearance: i32,
    routed_sections: &RoutedSections,
    section_room_endpoints: &SectionRoomEndpoints,
    source_section: &str,
    same_section_instances: &BTreeSet<String>,
    bounds: (i32, i32, i32, i32),
) -> Option<Vec<GridCell>> {
    let start_position = (start.x, start.y);
    let end_position = (end.x, end.y);
    let mut queue = VecDeque::new();
    let mut previous: BTreeMap<(i32, i32), (i32, i32)> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    queue.push_back(start_position);
    seen.insert(start_position);
    while let Some(position) = queue.pop_front() {
        if position == end_position {
            break;
        }
        for neighbor in grid_neighbors(position, connectivity) {
            if !position_in_bounds(neighbor, bounds) || !seen.insert(neighbor) {
                continue;
            }
            if neighbor != end_position
                && !bridge_position_available(
                    neighbor,
                    from_instance,
                    to_instance,
                    start,
                    from_direction,
                    end,
                    to_direction,
                    occupied_by_cell,
                    reserved,
                    wall_clearance,
                    corridor_clearance,
                    routed_sections,
                    section_room_endpoints,
                    source_section,
                    same_section_instances,
                )
            {
                continue;
            }
            previous.insert(neighbor, position);
            queue.push_back(neighbor);
        }
    }
    if !seen.contains(&end_position) {
        return None;
    }
    let mut path = vec![GridCell {
        x: end_position.0,
        y: end_position.1,
    }];
    let mut cursor = end_position;
    while cursor != start_position {
        cursor = *previous.get(&cursor)?;
        path.push(GridCell {
            x: cursor.0,
            y: cursor.1,
        });
    }
    path.reverse();
    Some(path)
}

fn position_in_bounds(position: (i32, i32), bounds: (i32, i32, i32, i32)) -> bool {
    position.0 >= bounds.0 && position.0 <= bounds.1 && position.1 >= bounds.2 && position.1 <= bounds.3
}

fn bridge_position_available(
    position: (i32, i32),
    from_instance: &str,
    to_instance: &str,
    from_exit: &GridCell,
    from_direction: &str,
    to_exit: &GridCell,
    to_direction: &str,
    occupied_by_cell: &BTreeMap<(i32, i32), &str>,
    reserved: &BTreeSet<(i32, i32)>,
    wall_clearance: i32,
    corridor_clearance: i32,
    routed_sections: &RoutedSections,
    section_room_endpoints: &SectionRoomEndpoints,
    source_section: &str,
    same_section_instances: &BTreeSet<String>,
) -> bool {
    if let Some(owner) = occupied_by_cell.get(&position) {
        if !same_section_instances.contains(*owner) {
            return false;
        }
    }
    if reserved.contains(&position) {
        return false;
    }
    for dy in -wall_clearance..=wall_clearance {
        for dx in -wall_clearance..=wall_clearance {
            if dx.abs() + dy.abs() > wall_clearance {
                continue;
            }
            if let Some(owner) = occupied_by_cell.get(&(position.0 + dx, position.1 + dy)) {
                let allowed = if *owner == from_instance {
                    endpoint_tunnel_contains(position, from_exit, from_direction, wall_clearance)
                } else if *owner == to_instance {
                    endpoint_tunnel_contains(position, to_exit, to_direction, wall_clearance)
                } else if same_section_instances.contains(*owner) {
                    true
                } else {
                    false
                };
                if !allowed {
                    return false;
                }
            }
        }
    }
    for dy in -corridor_clearance..=corridor_clearance {
        for dx in -corridor_clearance..=corridor_clearance {
            if dx.abs() + dy.abs() > corridor_clearance {
                continue;
            }
            let nearby = (position.0 + dx, position.1 + dy);
            if let Some(other_sections) = routed_sections.get(&nearby) {
                for other_section in other_sections {
                    if other_section != source_section
                        && !connection_contact_at_shared_room(
                            source_section,
                            other_section,
                            position,
                            nearby,
                            section_room_endpoints,
                            shared_room_approach_length(
                                corridor_clearance,
                                wall_clearance,
                            ),
                        )
                    {
                        return false;
                    }
                }
            }
        }
    }
    true
}

fn endpoint_tunnel_contains(
    position: (i32, i32),
    exit: &GridCell,
    direction: &str,
    wall_clearance: i32,
) -> bool {
    let (dx, dy) = direction_vector(direction);
    (0..wall_clearance)
        .any(|step| position == (exit.x + dx * step, exit.y + dy * step))
}

fn direction_vector(direction: &str) -> (i32, i32) {
    match direction {
        "north" => (0, -1),
        "east" => (1, 0),
        "south" => (0, 1),
        "west" => (-1, 0),
        _ => (0, 0),
    }
}

fn validate_piece_placement(placement: &PiecePlacement) -> ValidationReport {
    let mut diagnostics = Vec::new();
    if placement.kind != "asha_procgen.piece_placement.v1" {
        diagnostics.push(fatal(
            "piece_placement_kind_invalid",
            None,
            None,
            "Placement artifact kind must be asha_procgen.piece_placement.v1.",
        ));
    }
    validate_piece_placement_policy(&placement.placement_policy, &mut diagnostics);
    validate_corridor_realization(placement, &mut diagnostics);
    if placement.realization_search.realization_attempts == 0
        || placement.realization_search.realization_attempts > 2
        || placement.realization_search.realization_scale_tier
            >= placement.realization_search.realization_attempts
        || placement.realization_search.route_attempts == 0
        || placement.realization_search.route_attempts > PIECE_ROUTE_ORDER_COUNT
        || placement.realization_search.route_order_attempt
            >= placement.realization_search.route_attempts
    {
        diagnostics.push(fatal(
            "piece_realization_search_evidence_invalid",
            None,
            None,
            "Piece realization search evidence exceeds its scale-tier or deterministic route-order bounds.",
        ));
    }
    validate_placement_cells(placement, &mut diagnostics);
    validate_placement_links(placement, &mut diagnostics);
    validate_placement_unplanned_contacts(placement, &mut diagnostics);
    validate_placement_reachability(placement, &mut diagnostics);
    validate_placement_grid_reachability(placement, &mut diagnostics);
    let fatal_count = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Fatal)
        .count();
    ValidationReport {
        kind: "asha_procgen.validation.piece_placement.v1".to_owned(),
        schema_version: 1,
        state_hash: hash_json(placement).unwrap_or_else(|_| "hash_error".to_owned()),
        ok: fatal_count == 0,
        fatal_count,
        diagnostics,
    }
}

fn validate_corridor_realization(
    placement: &PiecePlacement,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match placement.corridor_realization {
        CorridorRealization::Catalog => {
            if !placement.connection_cells.is_empty() {
                diagnostics.push(fatal(
                    "piece_catalog_has_procedural_connection_cells",
                    None,
                    None,
                    "Pure catalog realization must contain zero generated connection cells.",
                ));
            }
            let Some(search) = placement.catalog_search.as_ref() else {
                diagnostics.push(fatal(
                    "piece_catalog_search_evidence_missing",
                    None,
                    None,
                    "Pure catalog realization must record its bounded placement-search evidence.",
                ));
                return;
            };
            if search.schema_version != 1
                || search.max_decisions == 0
                || search.max_backtracks == 0
                || search.max_chain_expansions_per_section == 0
                || search.max_room_origin_alternatives == 0
                || search.max_room_rotation_alternatives == 0
                || search.decisions > search.max_decisions
                || search.backtracks > search.max_backtracks
                || search.selected.len() != placement.instances.len()
            {
                diagnostics.push(fatal(
                    "piece_catalog_search_evidence_invalid",
                    None,
                    None,
                    "Pure catalog placement-search evidence is missing decisions or exceeds a declared deterministic budget.",
                ));
            }
            let instances = placement
                .instances
                .iter()
                .map(|instance| (instance.piece_id.as_str(), instance))
                .collect::<BTreeMap<_, _>>();
            let mut selected_pieces = BTreeSet::new();
            for decision in &search.selected {
                if !selected_pieces.insert(decision.piece_id.as_str()) {
                    diagnostics.push(fatal(
                        "piece_catalog_decision_duplicate",
                        Some(decision.piece_id.as_str()),
                        None,
                        "Pure catalog search evidence must contain one decision per placed piece.",
                    ));
                }
                let Some(instance) = instances.get(decision.piece_id.as_str()) else {
                    diagnostics.push(fatal(
                        "piece_catalog_decision_instance_missing",
                        Some(decision.piece_id.as_str()),
                        None,
                        "Pure catalog search evidence references a piece with no placed instance.",
                    ));
                    continue;
                };
                if decision.shape_id != instance.shape_id
                    || decision.transform != instance.transform
                    || decision.origin != instance.origin
                {
                    diagnostics.push(fatal(
                        "piece_catalog_decision_readback_mismatch",
                        Some(decision.piece_id.as_str()),
                        None,
                        "Pure catalog decision shape, transform, and origin must match the placed instance.",
                    ));
                }
                if decision.origin_bounds.as_ref().is_some_and(|bounds| {
                    decision.origin.x < bounds.min_x
                        || decision.origin.x > bounds.max_x
                        || decision.origin.y < bounds.min_y
                        || decision.origin.y > bounds.max_y
                }) {
                    diagnostics.push(fatal(
                        "piece_catalog_origin_bounds_invalid",
                        Some(decision.piece_id.as_str()),
                        None,
                        "Pure catalog room origin falls outside its serialized geometry bounds.",
                    ));
                }
                if let Some(lane) = decision.lane_constraint.as_ref() {
                    let expected_bounds = CatalogGridBounds {
                        min_x: lane.from.x.min(lane.to.x) - lane.envelope_cells,
                        max_x: lane.from.x.max(lane.to.x) + lane.envelope_cells,
                        min_y: lane.from.y.min(lane.to.y) - lane.envelope_cells,
                        max_y: lane.from.y.max(lane.to.y) + lane.envelope_cells,
                    };
                    if lane.envelope_cells < 0 || lane.bounds != expected_bounds {
                        diagnostics.push(fatal(
                            "piece_catalog_lane_evidence_invalid",
                            Some(decision.piece_id.as_str()),
                            None,
                            "Pure catalog lane evidence must preserve a non-negative envelope and its derived bounds.",
                        ));
                    }
                    if instance.occupied_cells.iter().any(|cell| {
                        grid_distance_to_segment(cell, &lane.from, &lane.to)
                            > lane.envelope_cells
                    }) {
                        diagnostics.push(fatal(
                            "piece_catalog_lane_envelope_invalid",
                            Some(decision.piece_id.as_str()),
                            None,
                            "Pure catalog prefab cells fall outside the serialized corridor lane envelope.",
                        ));
                    }
                }
            }
            for instance in &placement.instances {
                if !selected_pieces.contains(instance.piece_id.as_str()) {
                    diagnostics.push(fatal(
                        "piece_catalog_instance_decision_missing",
                        Some(instance.piece_id.as_str()),
                        None,
                        "Every pure catalog instance must have matching bounded-search decision evidence.",
                    ));
                }
            }
            for glued in &placement.glued_exits {
                if !pure_catalog_glue_is_direct(glued, &placement.occupied_cells) {
                    diagnostics.push(fatal(
                        "piece_catalog_direct_glue_invalid",
                        None,
                        Some(glued.source_edge.as_str()),
                        format!(
                            "Pure catalog glue {} is not an exact adjacent prefab-port join.",
                            glued.id
                        ),
                    ));
                }
            }
            validate_pure_catalog_contacts(placement, diagnostics);
        }
        CorridorRealization::Hybrid => {
            if placement.catalog_search.is_some() {
                diagnostics.push(fatal(
                    "piece_hybrid_catalog_search_evidence_unexpected",
                    None,
                    None,
                    "Hybrid realization must not claim pure-catalog placement-search evidence.",
                ));
            }
            if placement
                .glued_exits
                .iter()
                .any(|glued| glued.route_points.len() < 2)
            {
                diagnostics.push(fatal(
                    "piece_hybrid_corridor_lane_missing",
                    None,
                    None,
                    "Every hybrid corridor join must preserve its planned route slice.",
                ));
            }
        }
        CorridorRealization::Procedural => {
            if placement.catalog_search.is_some() {
                diagnostics.push(fatal(
                    "piece_procedural_catalog_search_evidence_unexpected",
                    None,
                    None,
                    "Procedural realization must not claim pure-catalog placement-search evidence.",
                ));
            }
            let corridor_instances = placement.instances.iter().filter(|instance| {
                matches!(
                    instance.requirement_kind.as_str(),
                    "connector" | "corridor" | "bend" | "junction"
                )
            });
            if corridor_instances.count() > 0 {
                diagnostics.push(fatal(
                    "piece_procedural_corridor_has_catalog_instance",
                    None,
                    None,
                    "Procedural corridor realization must not place connector, corridor, bend, or junction prefab instances.",
                ));
            }
            if placement
                .glued_exits
                .iter()
                .any(|glued| glued.route_points.len() < 2)
            {
                diagnostics.push(fatal(
                    "piece_procedural_corridor_lane_missing",
                    None,
                    None,
                    "Every procedural physical section must preserve a geometry lane with at least two points.",
                ));
            }
        }
    }
}

fn validate_pure_catalog_contacts(
    placement: &PiecePlacement,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let occupied = placement
        .occupied_cells
        .iter()
        .map(|cell| ((cell.x, cell.y), cell.instance_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let glued_pairs = placement
        .glued_exits
        .iter()
        .flat_map(|glued| {
            [
                (
                    glued.from_instance.as_str(),
                    glued.to_instance.as_str(),
                ),
                (
                    glued.to_instance.as_str(),
                    glued.from_instance.as_str(),
                ),
            ]
        })
        .collect::<BTreeSet<_>>();
    let mut reported = BTreeSet::new();
    for (position, owner) in &occupied {
        for neighbor in [(position.0 + 1, position.1), (position.0, position.1 + 1)] {
            let Some(other) = occupied.get(&neighbor) else {
                continue;
            };
            if owner == other || glued_pairs.contains(&(*owner, *other)) {
                continue;
            }
            let pair = sorted_pair(owner, other);
            if reported.insert(pair.clone()) {
                diagnostics.push(fatal(
                    "piece_catalog_unplanned_contact",
                    None,
                    None,
                    format!(
                        "Pure catalog instances {} and {} touch without an explicit glued-exit link.",
                        pair.0, pair.1
                    ),
                ));
            }
        }
    }
}

fn validate_placement_unplanned_contacts(
    placement: &PiecePlacement,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let occupied_by_cell = placement
        .occupied_cells
        .iter()
        .map(|cell| ((cell.x, cell.y), cell.instance_id.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut reported_pairs = BTreeSet::new();
    let section_instances = collect_catalog_section_contact_instances(placement);
    let clearance = placement.placement_policy.minimum_clearance_cells;
    for cell in &placement.occupied_cells {
        for dy in -clearance..=clearance {
            for dx in -clearance..=clearance {
                let distance = dx.abs() + dy.abs();
                if distance == 0 || distance > clearance {
                    continue;
                }
                let neighbor = (cell.x + dx, cell.y + dy);
                let Some(other_instance) = occupied_by_cell.get(&neighbor) else {
                    continue;
                };
                if *other_instance == cell.instance_id {
                    continue;
                }
                if placement.corridor_realization.uses_catalog_pieces()
                    && section_instances.values().any(|instances| {
                        instances.contains(&cell.instance_id)
                            && instances.contains(*other_instance)
                    })
                {
                    continue;
                }
                let pair = sorted_pair(cell.instance_id.as_str(), other_instance);
                if !reported_pairs.insert(pair.clone()) {
                    continue;
                }
                diagnostics.push(fatal(
                    "piece_minimum_clearance_violated",
                    None,
                    None,
                    format!(
                        "Occupied pieces violate minimum clearance {}: {} at {},{} is distance {} from {} at {},{}; declared links must use routed connection cells.",
                        clearance,
                        cell.instance_id,
                        cell.x,
                        cell.y,
                        distance,
                        other_instance,
                        neighbor.0,
                        neighbor.1
                    ),
                ));
            }
        }
    }
}

fn sorted_pair(left: &str, right: &str) -> (String, String) {
    if left <= right {
        (left.to_owned(), right.to_owned())
    } else {
        (right.to_owned(), left.to_owned())
    }
}

fn validate_placement_cells(placement: &PiecePlacement, diagnostics: &mut Vec<Diagnostic>) {
    let mut occupied_by_cell: BTreeMap<(i32, i32), &str> = BTreeMap::new();
    for cell in &placement.occupied_cells {
        if let Some(existing) = occupied_by_cell.insert((cell.x, cell.y), cell.instance_id.as_str()) {
            diagnostics.push(fatal(
                "piece_occupied_cell_overlap",
                None,
                None,
                format!(
                    "Occupied cell {},{} is shared by {} and {}.",
                    cell.x, cell.y, existing, cell.instance_id
                ),
            ));
        }
    }
    let mut reserved_by_cell: BTreeMap<(i32, i32), &str> = BTreeMap::new();
    for cell in &placement.reserved_cells {
        if let Some(occupier) = occupied_by_cell.get(&(cell.x, cell.y)) {
            if *occupier != cell.instance_id {
                diagnostics.push(fatal(
                    "piece_reserved_cell_conflict",
                    None,
                    None,
                    format!(
                        "Reserved cell {},{} for {} is occupied by {}.",
                        cell.x, cell.y, cell.instance_id, occupier
                    ),
                ));
            }
        }
        if let Some(existing) = reserved_by_cell.insert((cell.x, cell.y), cell.instance_id.as_str()) {
            if existing != cell.instance_id {
                diagnostics.push(fatal(
                    "piece_reserved_cell_overlap",
                    None,
                    None,
                    format!(
                        "Reserved cell {},{} is shared by {} and {}.",
                        cell.x, cell.y, existing, cell.instance_id
                    ),
                ));
            }
        }
    }
    let mut connection_by_cell: BTreeMap<(i32, i32), BTreeSet<&str>> = BTreeMap::new();
    let declared_connection_owners = placement
        .glued_exits
        .iter()
        .map(|glued| format!("connection.{}", slugify_label(glued.id.as_str())))
        .collect::<BTreeSet<_>>();
    let connection_specs = placement
        .glued_exits
        .iter()
        .map(|glued| {
            (
                format!("connection.{}", slugify_label(glued.id.as_str())),
                glued,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let section_room_endpoints = collect_section_room_endpoints(placement);
    let section_instances = collect_catalog_section_instances(placement);
    let mut connection_by_owner: BTreeMap<&str, BTreeSet<(i32, i32)>> = BTreeMap::new();
    let mut used_connection_owners = BTreeSet::new();
    for cell in &placement.connection_cells {
        if !declared_connection_owners.contains(&cell.instance_id) {
            diagnostics.push(fatal(
                "piece_connection_owner_undeclared",
                None,
                None,
                format!(
                    "Connection cell {},{} uses undeclared owner {}.",
                    cell.x, cell.y, cell.instance_id
                ),
            ));
        }
        used_connection_owners.insert(cell.instance_id.as_str());
        if let Some(occupier) = occupied_by_cell.get(&(cell.x, cell.y)) {
            diagnostics.push(fatal(
                "piece_connection_cell_occupied",
                None,
                None,
                format!(
                    "Connection cell {},{} for {} crosses occupied piece {}.",
                    cell.x, cell.y, cell.instance_id, occupier
                ),
            ));
        }
        let connection_spec = connection_specs.get(&cell.instance_id).copied();
        if let Some(reserver) = reserved_by_cell.get(&(cell.x, cell.y)) {
            let declared_endpoint_reservation = connection_spec
                .map(|glued| {
                    ((cell.x == glued.from_cell.x && cell.y == glued.from_cell.y)
                        && *reserver == glued.from_instance)
                        || ((cell.x == glued.to_cell.x && cell.y == glued.to_cell.y)
                            && *reserver == glued.to_instance)
                })
                .unwrap_or(false);
            if !declared_endpoint_reservation {
            diagnostics.push(fatal(
                "piece_connection_cell_reserved",
                None,
                None,
                format!(
                    "Connection cell {},{} for {} crosses reservation {}.",
                    cell.x, cell.y, cell.instance_id, reserver
                ),
            ));
            }
        }
        if let Some(glued) = connection_spec {
            let wall_clearance = placement.placement_policy.wall_thickness_cells;
            'clearance: for dy in -wall_clearance..=wall_clearance {
                for dx in -wall_clearance..=wall_clearance {
                    if dx.abs() + dy.abs() > wall_clearance {
                        continue;
                    }
                    let Some(occupier) = occupied_by_cell.get(&(cell.x + dx, cell.y + dy)) else {
                        continue;
                    };
                    let position = (cell.x, cell.y);
                    let allowed = if *occupier == glued.from_instance {
                        endpoint_tunnel_contains(
                            position,
                            &glued.from_cell,
                            glued.from_direction.as_str(),
                            wall_clearance,
                        )
                    } else if *occupier == glued.to_instance {
                        endpoint_tunnel_contains(
                            position,
                            &glued.to_cell,
                            glued.to_direction.as_str(),
                            wall_clearance,
                        )
                    } else if placement.corridor_realization.uses_catalog_pieces()
                        && section_instances
                            .get(glued.source_section.as_str())
                            .is_some_and(|instances| instances.contains(*occupier))
                    {
                        true
                    } else {
                        false
                    };
                    if !allowed {
                        diagnostics.push(fatal(
                            "piece_connection_wall_clearance_violated",
                            None,
                            None,
                            format!(
                                "Connection cell {},{} for {} enters the wall clearance of unrelated piece {}.",
                                cell.x, cell.y, cell.instance_id, occupier
                            ),
                        ));
                        break 'clearance;
                    }
                }
            }
        }
        let owners = connection_by_cell.entry((cell.x, cell.y)).or_default();
        if !owners.insert(cell.instance_id.as_str()) {
            diagnostics.push(fatal(
                "piece_connection_cell_duplicate",
                None,
                None,
                format!(
                    "Connection cell {},{} is repeated for {}.",
                    cell.x, cell.y, cell.instance_id
                ),
            ));
        }
        connection_by_owner
            .entry(cell.instance_id.as_str())
            .or_default()
            .insert((cell.x, cell.y));
    }
    let corridor_clearance = placement.placement_policy.minimum_clearance_cells.max(0);
    let mut reported_section_conflicts = BTreeSet::new();
    for (position, owners) in &connection_by_cell {
        for owner in owners {
            let Some(glued) = connection_specs.get(*owner).copied() else {
                continue;
            };
            for dy in -corridor_clearance..=corridor_clearance {
                for dx in -corridor_clearance..=corridor_clearance {
                    if dx.abs() + dy.abs() > corridor_clearance {
                        continue;
                    }
                    let nearby = (position.0 + dx, position.1 + dy);
                    let Some(nearby_owners) = connection_by_cell.get(&nearby) else {
                        continue;
                    };
                    for nearby_owner in nearby_owners {
                        let Some(nearby_glued) = connection_specs.get(*nearby_owner).copied() else {
                            continue;
                        };
                        if glued.source_section == nearby_glued.source_section {
                            continue;
                        }
                        if connection_contact_at_shared_room(
                            glued.source_section.as_str(),
                            nearby_glued.source_section.as_str(),
                            *position,
                            nearby,
                            &section_room_endpoints,
                            shared_room_approach_length(
                                corridor_clearance,
                                placement.placement_policy.wall_thickness_cells,
                            ),
                        ) {
                            continue;
                        }
                        let section_pair = sorted_pair(
                            glued.source_section.as_str(),
                            nearby_glued.source_section.as_str(),
                        );
                        if !reported_section_conflicts.insert(section_pair.clone()) {
                            continue;
                        }
                        diagnostics.push(fatal(
                            "piece_connection_section_clearance_violated",
                            None,
                            None,
                            format!(
                                "Physical connection sections {} and {} overlap or come within {} cell(s) at {},{} and {},{}.",
                                section_pair.0,
                                section_pair.1,
                                corridor_clearance,
                                position.0,
                                position.1,
                                nearby.0,
                                nearby.1
                            ),
                        ));
                    }
                }
            }
        }
    }
    if placement.corridor_realization != CorridorRealization::Catalog {
    for owner in declared_connection_owners {
        if !used_connection_owners.contains(owner.as_str()) {
            diagnostics.push(fatal(
                "piece_glued_exit_route_missing",
                None,
                None,
                format!("Declared glued exit {owner} has no routed connection cells."),
            ));
            continue;
        }
        let Some(glued) = connection_specs.get(owner.as_str()).copied() else {
            continue;
        };
        let owner_cells = connection_by_owner
            .get(owner.as_str())
            .cloned()
            .unwrap_or_default();
        let from_position = (glued.from_cell.x, glued.from_cell.y);
        let to_position = (glued.to_cell.x, glued.to_cell.y);
        if !owner_cells.contains(&from_position) || !owner_cells.contains(&to_position) {
            diagnostics.push(fatal(
                "piece_connection_exit_endpoint_missing",
                None,
                None,
                format!(
                    "Connection {} must include declared exit cells {},{} and {},{}.",
                    owner,
                    from_position.0,
                    from_position.1,
                    to_position.0,
                    to_position.1
                ),
            ));
            continue;
        }
        let mut traversable = owner_cells.clone();
        if placement.corridor_realization.uses_catalog_pieces() {
            if let Some(instances) = section_instances.get(glued.source_section.as_str()) {
                traversable.extend(
                    placement
                        .occupied_cells
                        .iter()
                        .filter(|cell| instances.contains(&cell.instance_id))
                        .map(|cell| (cell.x, cell.y)),
                );
            }
        }
        let mut reachable = BTreeSet::from([from_position]);
        let mut queue = VecDeque::from([from_position]);
        while let Some(position) = queue.pop_front() {
            for neighbor in grid_neighbors(position, placement.grid_connectivity) {
                if traversable.contains(&neighbor) && reachable.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
        if !reachable.contains(&to_position)
            || owner_cells.iter().any(|cell| !reachable.contains(cell))
        {
            diagnostics.push(fatal(
                "piece_connection_route_disconnected",
                None,
                None,
                format!("Connection {owner} is not one connected declared-exit route."),
            ));
        }
        if !glued.route_points.is_empty() {
            let lane = map_geometry_lane_to_room_exits(glued)
                .and_then(|waypoints| rasterize_orthogonal_waypoints(&waypoints))
                .unwrap_or_default()
                .into_iter()
                .map(|cell| (cell.x, cell.y))
                .collect::<BTreeSet<_>>();
            let envelope = placement.placement_policy.minimum_clearance_cells * 2
                + placement.placement_policy.wall_thickness_cells;
            if lane.is_empty()
                || owner_cells
                    .iter()
                    .any(|cell| distance_to_lane(*cell, &lane) > envelope as u32)
            {
                diagnostics.push(fatal(
                    if placement.corridor_realization == CorridorRealization::Procedural {
                        "piece_procedural_corridor_left_geometry_lane"
                    } else {
                        "piece_catalog_corridor_left_planned_lane"
                    },
                    None,
                    Some(glued.source_edge.as_str()),
                    format!(
                        "Connection {owner} leaves its planned geometry lane envelope."
                    ),
                ));
            }
        }
    }
    }
}

fn connection_contact_at_shared_room(
    left_section: &str,
    right_section: &str,
    left_position: (i32, i32),
    right_position: (i32, i32),
    section_room_endpoints: &SectionRoomEndpoints,
    approach_length: i32,
) -> bool {
    let Some(left_rooms) = section_room_endpoints.get(left_section) else {
        return false;
    };
    let Some(right_rooms) = section_room_endpoints.get(right_section) else {
        return false;
    };
    left_rooms.iter().any(|(room, left_endpoints)| {
        let Some(right_endpoints) = right_rooms.get(room) else {
            return false;
        };
        left_endpoints.iter().any(|(cell, direction)| {
            endpoint_approach_contains(
                left_position,
                cell,
                direction.as_str(),
                approach_length,
            )
        }) && right_endpoints.iter().any(|(cell, direction)| {
            endpoint_approach_contains(
                right_position,
                cell,
                direction.as_str(),
                approach_length,
            )
        })
    })
}

fn shared_room_approach_length(
    minimum_clearance: i32,
    wall_thickness: i32,
) -> i32 {
    // Adjacent room exits may fan out through the protected doorway approach and
    // one wall buffer. Both section cells must independently remain in this
    // bounded region; sharing the room alone never grants a route-wide exemption.
    minimum_clearance + wall_thickness * 2
}

fn endpoint_approach_contains(
    position: (i32, i32),
    exit: &GridCell,
    direction: &str,
    approach_length: i32,
) -> bool {
    let (direction_x, direction_y) = direction_vector(direction);
    if direction_x == 0 && direction_y == 0 {
        return false;
    }
    let relative_x = position.0 - exit.x;
    let relative_y = position.1 - exit.y;
    let forward = relative_x * direction_x + relative_y * direction_y;
    let distance = relative_x.abs() + relative_y.abs();
    forward >= 0 && distance <= approach_length
}

fn validate_placement_links(placement: &PiecePlacement, diagnostics: &mut Vec<Diagnostic>) {
    let instances = placement
        .instances
        .iter()
        .map(|instance| (instance.instance_id.as_str(), instance))
        .collect::<BTreeMap<_, _>>();
    let mut glued_ids = BTreeSet::new();
    let mut consumed_exits: BTreeMap<(&str, &str), &str> = BTreeMap::new();
    for glued in &placement.glued_exits {
        if !glued_ids.insert(glued.id.as_str()) {
            diagnostics.push(fatal(
                "piece_glued_exit_duplicate",
                None,
                None,
                format!("Duplicate glued exit id {}.", glued.id),
            ));
        }
        for endpoint in [
            (glued.from_instance.as_str(), glued.from_exit.as_str()),
            (glued.to_instance.as_str(), glued.to_exit.as_str()),
        ] {
            if let Some(first) = consumed_exits.insert(endpoint, glued.id.as_str()) {
                diagnostics.push(fatal(
                    "piece_instance_exit_reused",
                    None,
                    Some(glued.source_edge.as_str()),
                    format!(
                        "Instance exit {}:{} is consumed by glued exits {} and {}.",
                        endpoint.0, endpoint.1, first, glued.id
                    ),
                ));
            }
        }
        let Some(from) = instances.get(glued.from_instance.as_str()) else {
            diagnostics.push(fatal(
                "piece_glued_exit_instance_missing",
                None,
                None,
                format!("Glued exit {} references missing from instance.", glued.id),
            ));
            continue;
        };
        let Some(to) = instances.get(glued.to_instance.as_str()) else {
            diagnostics.push(fatal(
                "piece_glued_exit_instance_missing",
                None,
                None,
                format!("Glued exit {} references missing to instance.", glued.id),
            ));
            continue;
        };
        let Some(from_exit) = from
            .exit_map
            .iter()
            .find(|exit| exit.requirement_exit_id == glued.from_exit)
        else {
            diagnostics.push(fatal(
                "piece_glued_exit_endpoint_missing",
                None,
                None,
                format!("Glued exit {} references missing from exit.", glued.id),
            ));
            continue;
        };
        let Some(to_exit) = to
            .exit_map
            .iter()
            .find(|exit| exit.requirement_exit_id == glued.to_exit)
        else {
            diagnostics.push(fatal(
                "piece_glued_exit_endpoint_missing",
                None,
                None,
                format!("Glued exit {} references missing to exit.", glued.id),
            ));
            continue;
        };
        if glued.from_cell.x != from_exit.x
            || glued.from_cell.y != from_exit.y
            || glued.from_direction != from_exit.direction
            || glued.from_width != from_exit.width
            || glued.to_cell.x != to_exit.x
            || glued.to_cell.y != to_exit.y
            || glued.to_direction != to_exit.direction
            || glued.to_width != to_exit.width
        {
            diagnostics.push(fatal(
                "piece_glued_exit_metadata_mismatch",
                None,
                None,
                format!(
                    "Glued exit {} endpoint metadata does not match its placed transformed exits.",
                    glued.id
                ),
            ));
        }
        validate_instance_exit_geometry(from, from_exit, glued.id.as_str(), diagnostics);
        validate_instance_exit_geometry(to, to_exit, glued.id.as_str(), diagnostics);
        if glued.route_points.is_empty()
            && opposite_direction(from_exit.direction.as_str()) != to_exit.direction
        {
            diagnostics.push(fatal(
                "piece_glued_exit_incompatible",
                None,
                None,
                format!(
                    "Glued exit {} joins {} to {}, which are not opposite directions.",
                    glued.id, from_exit.direction, to_exit.direction
                ),
            ));
        }
    }
    let glued_by_link = placement
        .glued_exits
        .iter()
        .map(|glued| (glued.link_id.as_str(), glued))
        .collect::<BTreeMap<_, _>>();
    let traversable_portal_cells = placement
        .connection_cells
        .iter()
        .chain(
            placement
                .occupied_cells
                .iter()
                .filter(|_| placement.corridor_realization == CorridorRealization::Catalog),
        )
        .map(|cell| (cell.x, cell.y))
        .collect::<BTreeSet<_>>();
    let mut portal_ids = BTreeSet::new();
    let mut portal_edges = BTreeSet::new();
    for portal in &placement.gate_portals {
        if !portal_ids.insert(portal.id.as_str()) || !portal_edges.insert(portal.source_edge.as_str()) {
            diagnostics.push(fatal(
                "piece_gate_portal_duplicate",
                None,
                Some(portal.source_edge.as_str()),
                format!("Gate portal {} duplicates a portal id or source edge.", portal.id),
            ));
        }
        let Some(glued) = glued_by_link.get(portal.link_id.as_str()).copied() else {
            diagnostics.push(fatal(
                "piece_gate_portal_link_missing",
                None,
                Some(portal.source_edge.as_str()),
                format!("Gate portal {} references missing link {}.", portal.id, portal.link_id),
            ));
            continue;
        };
        if portal.source_edge != glued.source_edge
            || portal.source_corridor != glued.source_corridor
            || portal.from_instance != glued.from_instance
            || portal.to_instance != glued.to_instance
            || portal.orientation != glued.from_direction
            || portal.width != glued.from_width
            || portal.traversal != glued.traversal
            || portal.required_item != glued.required_item
            || portal.cells.is_empty()
        {
            diagnostics.push(fatal(
                "piece_gate_portal_metadata_mismatch",
                None,
                Some(portal.source_edge.as_str()),
                format!("Gate portal {} does not match its controlling glued exit.", portal.id),
            ));
        }
        for cell in &portal.cells {
            if !traversable_portal_cells.contains(&(cell.x, cell.y)) {
                diagnostics.push(fatal(
                    "piece_gate_portal_cell_missing",
                    None,
                    Some(portal.source_edge.as_str()),
                    format!(
                        "Gate portal {} cell {},{} is not part of its realized traversal.",
                        portal.id, cell.x, cell.y
                    ),
                ));
            }
        }
    }
    for dangling in &placement.dangling_exits {
        diagnostics.push(fatal(
            "piece_required_exit_dangling",
            None,
            None,
            format!(
                "Instance {} has dangling required exit {} ({})",
                dangling.instance_id, dangling.exit_id, dangling.reason
            ),
        ));
    }
}

fn validate_instance_exit_geometry(
    instance: &PieceInstance,
    exit: &MatchedExit,
    glued_id: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (dx, dy) = direction_vector(exit.direction.as_str());
    let inside = (exit.x - dx, exit.y - dy);
    if !instance
        .occupied_cells
        .iter()
        .any(|cell| (cell.x, cell.y) == inside)
    {
        diagnostics.push(fatal(
            "piece_glued_exit_not_on_boundary",
            None,
            None,
            format!(
                "Glued exit {} uses {} exit {} at {},{}, which is not outside its declared boundary direction {}.",
                glued_id,
                instance.instance_id,
                exit.requirement_exit_id,
                exit.x,
                exit.y,
                exit.direction
            ),
        ));
    }
}

fn validate_placement_reachability(placement: &PiecePlacement, diagnostics: &mut Vec<Diagnostic>) {
    let starts = placement
        .instances
        .iter()
        .filter(|instance| instance.tags.iter().any(|tag| tag == "start_marker" || tag == "start"))
        .map(|instance| instance.instance_id.as_str())
        .collect::<Vec<_>>();
    let goals = placement
        .instances
        .iter()
        .filter(|instance| instance.tags.iter().any(|tag| tag == "goal_marker" || tag == "goal"))
        .map(|instance| instance.instance_id.as_str())
        .collect::<BTreeSet<_>>();
    if starts.is_empty() || goals.is_empty() {
        return;
    }
    let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for glued in &placement.glued_exits {
        adjacency
            .entry(glued.from_instance.as_str())
            .or_default()
            .push(glued.to_instance.as_str());
        adjacency
            .entry(glued.to_instance.as_str())
            .or_default()
            .push(glued.from_instance.as_str());
    }
    let mut queue = VecDeque::new();
    let mut seen = BTreeSet::new();
    for start in starts {
        queue.push_back(start);
        seen.insert(start);
    }
    while let Some(instance) = queue.pop_front() {
        if goals.contains(instance) {
            return;
        }
        for next in adjacency.get(instance).into_iter().flatten() {
            if seen.insert(*next) {
                queue.push_back(*next);
            }
        }
    }
    diagnostics.push(fatal(
        "piece_goal_unreachable",
        None,
        None,
        "No glued-exit path reaches a goal instance from a start instance.",
    ));
}

fn validate_placement_grid_reachability(
    placement: &PiecePlacement,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let grid_cells = placement
        .occupied_cells
        .iter()
        .chain(placement.connection_cells.iter())
        .map(|cell| (cell.x, cell.y))
        .collect::<BTreeSet<_>>();
    if grid_cells.is_empty() {
        return;
    }
    let start = placement
        .instances
        .iter()
        .find(|instance| instance.tags.iter().any(|tag| tag == "start_marker" || tag == "start"))
        .and_then(|instance| instance.occupied_cells.first())
        .or_else(|| {
            placement
                .instances
                .iter()
                .find(|instance| !instance.occupied_cells.is_empty())
                .and_then(|instance| instance.occupied_cells.first())
        });
    let Some(start) = start else {
        return;
    };
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::new();
    if grid_cells.contains(&(start.x, start.y)) {
        seen.insert((start.x, start.y));
        queue.push_back((start.x, start.y));
    }
    while let Some(cell) = queue.pop_front() {
        for neighbor in grid_neighbors(cell, placement.grid_connectivity) {
            if grid_cells.contains(&neighbor) && seen.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }
    for instance in &placement.instances {
        if instance.occupied_cells.is_empty() {
            continue;
        }
        if !instance
            .occupied_cells
            .iter()
            .any(|cell| seen.contains(&(cell.x, cell.y)))
        {
            diagnostics.push(fatal(
                "piece_grid_instance_unreachable",
                None,
                None,
                format!(
                    "Instance {} is not physically reachable on the {:?} placement grid.",
                    instance.instance_id, placement.grid_connectivity
                ),
            ));
        }
    }
}

fn grid_neighbors(cell: (i32, i32), connectivity: GridConnectivity) -> Vec<(i32, i32)> {
    let mut neighbors = vec![
        (cell.0 + 1, cell.1),
        (cell.0 - 1, cell.1),
        (cell.0, cell.1 + 1),
        (cell.0, cell.1 - 1),
    ];
    if connectivity == GridConnectivity::EightWay {
        neighbors.extend([
            (cell.0 + 1, cell.1 + 1),
            (cell.0 + 1, cell.1 - 1),
            (cell.0 - 1, cell.1 + 1),
            (cell.0 - 1, cell.1 - 1),
        ]);
    }
    neighbors
}

fn transform_cells(cells: &[GridCell], transform: &str, origin: &GridCell) -> Vec<GridCell> {
    let mut transformed = cells
        .iter()
        .map(|cell| {
            let (x, y) = transform_cell(cell.x, cell.y, transform);
            GridCell { x, y }
        })
        .collect::<Vec<_>>();
    let min_x = transformed.iter().map(|cell| cell.x).min().unwrap_or(0);
    let min_y = transformed.iter().map(|cell| cell.y).min().unwrap_or(0);
    for cell in &mut transformed {
        cell.x = cell.x - min_x + origin.x;
        cell.y = cell.y - min_y + origin.y;
    }
    transformed
}

fn transform_cell(x: i32, y: i32, transform: &str) -> (i32, i32) {
    match transform {
        "rotate90" => (-y, x),
        "rotate180" => (-x, -y),
        "rotate270" => (y, -x),
        "mirrorX" => (-x, y),
        "mirrorY" => (x, -y),
        _ => (x, y),
    }
}
