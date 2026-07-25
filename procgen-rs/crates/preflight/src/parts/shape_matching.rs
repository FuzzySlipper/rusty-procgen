fn build_match_shapes_command(args: BuildMatchShapesArgs) -> Result<(), String> {
    let catalog: ShapeCatalog = read_json(&args.catalog)?;
    let plan: PieceBuildPlan = read_json(&args.piece_plan)?;
    let report = match_shapes(&catalog, &plan, &args);
    write_json(&args.out, &report)
}

fn match_shapes(
    catalog: &ShapeCatalog,
    plan: &PieceBuildPlan,
    args: &BuildMatchShapesArgs,
) -> PieceShapeMatchReport {
    match_shapes_with_attempt(catalog, plan, args, 0)
}

fn match_shapes_with_attempt(
    catalog: &ShapeCatalog,
    plan: &PieceBuildPlan,
    args: &BuildMatchShapesArgs,
    alternative_attempt: u32,
) -> PieceShapeMatchReport {
    let mut matches = Vec::new();
    let mut rejections = Vec::new();
    let mut diagnostics = Vec::new();

    let alternative_requirement = if alternative_attempt == 0 || plan.requirements.is_empty() {
        None
    } else {
        Some(
            geometry_layout_order_key(
                plan.plan_id.as_str(),
                args.seed,
                u64::from(alternative_attempt),
            ) as usize
                % plan.requirements.len(),
        )
    };
    for (requirement_index, requirement) in plan.requirements.iter().enumerate() {
        let candidate_rank = usize::from(alternative_requirement == Some(requirement_index));
        let result = if plan.corridor_realization == CorridorRealization::Catalog {
            let candidates =
                pure_catalog_match_candidates(catalog, requirement, plan, args.seed);
            let exact =
                match_requirement(catalog, requirement, plan, args.seed, candidate_rank);
            RequirementMatchResult {
                selected: candidates
                    .get(candidate_rank)
                    .or_else(|| candidates.first())
                    .cloned(),
                rejections: exact.rejections,
            }
        } else {
            match_requirement(catalog, requirement, plan, args.seed, candidate_rank)
        };
        rejections.extend(result.rejections);
        if let Some(piece_match) = result.selected {
            matches.push(piece_match);
        } else {
            diagnostics.push(Diagnostic {
                code: "shape_match_missing".to_owned(),
                severity: Severity::Fatal,
                node: None,
                edge: None,
                detail: format!(
                    "No catalog shape matched piece requirement {} ({})",
                    requirement.piece_id, requirement.kind
                ),
                repair_hint: Some("Add a compatible catalog shape or relax the piece requirement.".to_owned()),
            });
        }
    }

    let unmatched_count = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "shape_match_missing")
        .count();
    PieceShapeMatchReport {
        kind: "asha_procgen.piece_shape_match.v1".to_owned(),
        schema_version: 1,
        match_id: format!(
            "piece_shape_match.{}.{}.alternative_{}",
            plan.plan_id, args.seed, alternative_attempt
        ),
        plan_id: plan.plan_id.clone(),
        catalog_id: catalog.catalog_id.clone(),
        seed: args.seed,
        alternative_attempt,
        source_plan_ref: display_path(&args.piece_plan),
        source_catalog_ref: display_path(&args.catalog),
        ok: unmatched_count == 0,
        unmatched_count,
        matches,
        rejections,
        diagnostics,
    }
}

struct RequirementMatchResult {
    selected: Option<MatchedPiece>,
    rejections: Vec<ShapeMatchRejection>,
}

#[derive(Clone, Debug)]
struct CandidateShapeMatch {
    matched_piece: MatchedPiece,
    tie_key: u64,
}

fn pure_catalog_match_candidates(
    catalog: &ShapeCatalog,
    requirement: &PieceRequirement,
    plan: &PieceBuildPlan,
    seed: u64,
) -> Vec<MatchedPiece> {
    let mut candidates = Vec::new();
    let mut signatures = BTreeSet::new();
    for shape in &catalog.shapes {
        if !static_shape_rejection_reasons(shape, requirement).is_empty() {
            continue;
        }
        for transform in &shape.allowed_transforms {
            if !matches!(
                transform.as_str(),
                "identity" | "rotate90" | "rotate180" | "rotate270"
            ) {
                continue;
            }
            let transformed_exits = transformed_catalog_exits(shape, transform);
            let direction_offsets: &[u8] = if is_room_requirement(requirement) {
                &[0, 1, 3, 2]
            } else {
                &[0]
            };
            for direction_offset in direction_offsets {
                let Some(exit_map) =
                    match_exits_with_direction_offset(
                        requirement,
                        &transformed_exits,
                        *direction_offset,
                    )
                else {
                    continue;
                };
                let socket_map = match_sockets(requirement, shape);
                let signature =
                    pure_catalog_transform_signature(shape, transform, &exit_map);
                if !signatures.insert(signature) {
                    continue;
                }
                let offset = i32::from(*direction_offset);
                let semantic_facing_preference = 48 - offset.min(4 - offset) * 12;
                candidates.push(CandidateShapeMatch {
                    matched_piece: MatchedPiece {
                        piece_id: requirement.piece_id.clone(),
                        requirement_kind: requirement.kind.clone(),
                        shape_id: shape.shape_id.clone(),
                        transform: transform.clone(),
                        score: shape_match_score(
                            shape,
                            requirement,
                            &exit_map,
                            &socket_map,
                        ) + semantic_facing_preference,
                        candidate_rank: 0,
                        candidate_count: 0,
                        source_requirement_ref: format!(
                            "piecePlan:{};requirement:{};semanticFacingOffset:{}",
                            plan.plan_id, requirement.piece_id, direction_offset
                        ),
                        exit_map,
                        socket_map,
                    },
                    tie_key: stable_match_tie_key(seed, requirement, shape, transform)
                        ^ u64::from(*direction_offset),
                });
            }
        }
    }
    candidates.sort_by(|left, right| {
        right
            .matched_piece
            .score
            .cmp(&left.matched_piece.score)
            .then_with(|| left.tie_key.cmp(&right.tie_key))
            .then_with(|| left.matched_piece.shape_id.cmp(&right.matched_piece.shape_id))
            .then_with(|| left.matched_piece.transform.cmp(&right.matched_piece.transform))
    });
    let candidate_count = candidates.len();
    candidates
        .into_iter()
        .enumerate()
        .map(|(rank, mut candidate)| {
            candidate.matched_piece.candidate_rank = rank;
            candidate.matched_piece.candidate_count = candidate_count;
            candidate.matched_piece
        })
        .collect()
}

fn match_exits_with_direction_offset(
    requirement: &PieceRequirement,
    transformed_exits: &[CatalogExit],
    direction_offset: u8,
) -> Option<Vec<MatchedExit>> {
    let mut mapped = Vec::new();
    let mut used = BTreeSet::new();
    let mut required_exits = requirement.required_exits.iter().collect::<Vec<_>>();
    required_exits.sort_by(|left, right| left.id.cmp(&right.id));
    for required_exit in required_exits {
        let preferred_direction =
            rotate_direction_steps(required_exit.direction.as_str(), direction_offset);
        let candidate = transformed_exits
            .iter()
            .enumerate()
            .filter(|(index, exit)| {
                !used.contains(index)
                    && exit.direction == preferred_direction
                    && exit_width_compatible(required_exit.width, exit.width)
            })
            .min_by(|(_, left), (_, right)| left.id.cmp(&right.id));
        let Some((index, catalog_exit)) = candidate else {
            return None;
        };
        used.insert(index);
        mapped.push(MatchedExit {
            requirement_exit_id: required_exit.id.clone(),
            catalog_exit_id: catalog_exit.id.clone(),
            x: catalog_exit.x,
            y: catalog_exit.y,
            direction: catalog_exit.direction.clone(),
            width: catalog_exit.width,
        });
    }
    Some(mapped)
}

fn rotate_direction_steps(direction: &str, steps: u8) -> &'static str {
    let directions = ["north", "east", "south", "west"];
    let index = directions
        .iter()
        .position(|candidate| *candidate == direction)
        .unwrap_or(0);
    directions[(index + usize::from(steps)) % directions.len()]
}

fn pure_catalog_transform_signature(
    shape: &CatalogShape,
    transform: &str,
    exit_map: &[MatchedExit],
) -> String {
    let transformed = shape
        .footprint
        .iter()
        .map(|cell| transform_cell(cell.x, cell.y, transform))
        .collect::<Vec<_>>();
    let min_x = transformed.iter().map(|cell| cell.0).min().unwrap_or(0);
    let min_y = transformed.iter().map(|cell| cell.1).min().unwrap_or(0);
    let mut footprint = transformed
        .into_iter()
        .map(|(x, y)| (x - min_x, y - min_y))
        .collect::<Vec<_>>();
    footprint.sort();
    let mut exits = exit_map
        .iter()
        .map(|exit| {
            (
                exit.requirement_exit_id.as_str(),
                exit.x,
                exit.y,
                exit.direction.as_str(),
                exit.width,
            )
        })
        .collect::<Vec<_>>();
    exits.sort();
    format!("{}:{footprint:?}:{exits:?}", shape.shape_id)
}

fn match_requirement(
    catalog: &ShapeCatalog,
    requirement: &PieceRequirement,
    plan: &PieceBuildPlan,
    seed: u64,
    candidate_rank: usize,
) -> RequirementMatchResult {
    let mut candidates = Vec::new();
    let mut rejections = Vec::new();
    for shape in &catalog.shapes {
        let static_reasons = static_shape_rejection_reasons(shape, requirement);
        if !static_reasons.is_empty() {
            rejections.push(ShapeMatchRejection {
                piece_id: requirement.piece_id.clone(),
                shape_id: shape.shape_id.clone(),
                transform: None,
                reasons: static_reasons,
            });
            continue;
        }

        for transform in &shape.allowed_transforms {
            let transformed_exits = transformed_catalog_exits(shape, transform);
            let exit_map = match_exits(requirement, &transformed_exits);
            let Some(exit_map) = exit_map else {
                rejections.push(ShapeMatchRejection {
                    piece_id: requirement.piece_id.clone(),
                    shape_id: shape.shape_id.clone(),
                    transform: Some(transform.clone()),
                    reasons: vec![exit_rejection_reason(shape, requirement, &transformed_exits)],
                });
                continue;
            };
            let socket_map = match_sockets(requirement, shape);
            let score = shape_match_score(shape, requirement, &exit_map, &socket_map);
            let tie_key = stable_match_tie_key(seed, requirement, shape, transform);
            candidates.push(CandidateShapeMatch {
                matched_piece: MatchedPiece {
                    piece_id: requirement.piece_id.clone(),
                    requirement_kind: requirement.kind.clone(),
                    shape_id: shape.shape_id.clone(),
                    transform: transform.clone(),
                    score,
                    candidate_rank: 0,
                    candidate_count: 0,
                    source_requirement_ref: format!(
                        "piecePlan:{};requirement:{}",
                        plan.plan_id, requirement.piece_id
                    ),
                    exit_map,
                    socket_map,
                },
                tie_key,
            });
        }
    }

    candidates.sort_by(|left, right| {
        right
            .matched_piece
            .score
            .cmp(&left.matched_piece.score)
            .then_with(|| left.tie_key.cmp(&right.tie_key))
            .then_with(|| {
                left.matched_piece
                    .shape_id
                    .cmp(&right.matched_piece.shape_id)
            })
            .then_with(|| {
                left.matched_piece
                    .transform
                    .cmp(&right.matched_piece.transform)
            })
    });
    let candidate_count = candidates.len();
    let selected_rank = candidate_rank.min(candidate_count.saturating_sub(1));
    RequirementMatchResult {
        selected: candidates.into_iter().nth(selected_rank).map(|mut candidate| {
            candidate.matched_piece.candidate_rank = selected_rank;
            candidate.matched_piece.candidate_count = candidate_count;
            candidate.matched_piece
        }),
        rejections,
    }
}

fn static_shape_rejection_reasons(
    shape: &CatalogShape,
    requirement: &PieceRequirement,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if !shape
        .piece_kinds
        .iter()
        .any(|kind| kind == requirement.kind.as_str())
    {
        reasons.push(format!("kind_mismatch: need {}", requirement.kind));
    }
    let missing_sockets = requirement
        .required_sockets
        .iter()
        .filter(|socket| {
            !shape
                .feature_sockets
                .iter()
                .any(|feature_socket| feature_socket.kind == socket.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();
    if !missing_sockets.is_empty() {
        reasons.push(format!("missing_sockets: {}", missing_sockets.join(",")));
    }
    let missing_shape_tags = requirement
        .required_shape_tags
        .iter()
        .filter(|required_tag| !shape.tags.contains(required_tag))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_shape_tags.is_empty() {
        reasons.push(format!(
            "missing_shape_tags: {}",
            missing_shape_tags.join(",")
        ));
    }
    let required_exits = &requirement.required_exits;
    if shape.exits.len() < required_exits.len() {
        reasons.push(format!(
            "exit_count_mismatch: need at least {} got {}",
            required_exits.len(),
            shape.exits.len()
        ));
    }
    reasons
}

fn transformed_catalog_exits(shape: &CatalogShape, transform: &str) -> Vec<CatalogExit> {
    let transformed_footprint = shape
        .footprint
        .iter()
        .map(|cell| transform_cell(cell.x, cell.y, transform))
        .collect::<Vec<_>>();
    let min_x = transformed_footprint
        .iter()
        .map(|cell| cell.0)
        .min()
        .unwrap_or(0);
    let min_y = transformed_footprint
        .iter()
        .map(|cell| cell.1)
        .min()
        .unwrap_or(0);
    shape
        .exits
        .iter()
        .map(|exit| {
            let (x, y) = transform_cell(exit.x, exit.y, transform);
            CatalogExit {
                id: exit.id.clone(),
                x: x - min_x,
                y: y - min_y,
                direction: transform_direction(exit.direction.as_str(), transform).to_owned(),
                width: exit.width,
                tags: exit.tags.clone(),
            }
        })
        .collect()
}

fn transform_direction(direction: &str, transform: &str) -> &'static str {
    let steps = match transform {
        "identity" => 0,
        "rotate90" => 1,
        "rotate180" => 2,
        "rotate270" => 3,
        "mirrorX" => {
            return match direction {
                "east" => "west",
                "west" => "east",
                "north" => "north",
                "south" => "south",
                _ => "unknown",
            };
        }
        "mirrorY" => {
            return match direction {
                "north" => "south",
                "south" => "north",
                "east" => "east",
                "west" => "west",
                _ => "unknown",
            };
        }
        _ => 0,
    };
    let directions = ["north", "east", "south", "west"];
    let index = directions
        .iter()
        .position(|candidate| *candidate == direction)
        .unwrap_or(0);
    directions[(index + steps) % directions.len()]
}

fn match_exits(
    requirement: &PieceRequirement,
    transformed_exits: &[CatalogExit],
) -> Option<Vec<MatchedExit>> {
    let mut mapped = Vec::new();
    let mut used = BTreeSet::new();
    let mut required_exits = requirement.required_exits.iter().collect::<Vec<_>>();
    required_exits.sort_by(|left, right| left.id.cmp(&right.id));
    for required_exit in required_exits {
        let candidate = transformed_exits
            .iter()
            .enumerate()
            .filter(|(index, exit)| {
                !used.contains(index)
                    && exit.direction == required_exit.direction
                    && exit_width_compatible(required_exit.width, exit.width)
            })
            .min_by(|(_, left), (_, right)| left.id.cmp(&right.id));
        let Some((index, catalog_exit)) = candidate else {
            return None;
        };
        used.insert(index);
        mapped.push(MatchedExit {
            requirement_exit_id: required_exit.id.clone(),
            catalog_exit_id: catalog_exit.id.clone(),
            x: catalog_exit.x,
            y: catalog_exit.y,
            direction: catalog_exit.direction.clone(),
            width: catalog_exit.width,
        });
    }
    Some(mapped)
}

fn exit_width_compatible(required_width: i32, catalog_width: i32) -> bool {
    required_width > 0 && catalog_width > 0
}

fn exit_rejection_reason(
    shape: &CatalogShape,
    requirement: &PieceRequirement,
    transformed_exits: &[CatalogExit],
) -> String {
    let available = transformed_exits
        .iter()
        .map(|exit| format!("{}:{}", exit.id, exit.direction))
        .collect::<Vec<_>>()
        .join(",");
    let required = requirement
        .required_exits
        .iter()
        .map(|exit| format!("{}:{}", exit.id, exit.direction))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "exit_compatibility_mismatch: shape {} required [{}] available [{}]",
        shape.shape_id, required, available
    )
}

fn match_sockets(requirement: &PieceRequirement, shape: &CatalogShape) -> Vec<MatchedSocket> {
    requirement
        .required_sockets
        .iter()
        .filter_map(|required_socket| {
            shape
                .feature_sockets
                .iter()
                .find(|socket| socket.kind == *required_socket)
                .map(|socket| MatchedSocket {
                    required_socket: required_socket.clone(),
                    catalog_socket_id: socket.id.clone(),
                    kind: socket.kind.clone(),
                })
        })
        .collect()
}

fn shape_match_score(
    shape: &CatalogShape,
    requirement: &PieceRequirement,
    exit_map: &[MatchedExit],
    socket_map: &[MatchedSocket],
) -> i32 {
    let mut score = 0;
    score += 1000;
    score += (exit_map.len() as i32) * 20;
    score += (socket_map.len() as i32) * 25;
    score -= ((shape.exits.len() as i32) - (requirement.required_exits.len() as i32))
        .abs()
        * 5;
    score += shape
        .tags
        .iter()
        .filter(|tag| requirement.tags.contains(tag))
        .count() as i32
        * 4;
    score
}

fn stable_match_tie_key(
    seed: u64,
    requirement: &PieceRequirement,
    shape: &CatalogShape,
    transform: &str,
) -> u64 {
    let input = format!(
        "{}:{}:{}:{}",
        seed, requirement.piece_id, shape.shape_id, transform
    );
    fnv1a64(input.as_bytes())
}
