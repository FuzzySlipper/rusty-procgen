fn build_emit_piece_plan_command(args: BuildEmitPiecePlanArgs) -> Result<(), String> {
    let candidate: Candidate = read_json(&args.candidate)?;
    let intermediate: IntermediateBreakdown = read_json(&args.intermediate)?;
    let geometry: Geometry2dArtifact = read_json(&args.geometry)?;
    let plan = emit_piece_build_plan(&candidate, &intermediate, &geometry, &args)?;
    write_json(&args.out, &plan)
}

fn emit_piece_build_plan(
    candidate: &Candidate,
    intermediate: &IntermediateBreakdown,
    geometry: &Geometry2dArtifact,
    args: &BuildEmitPiecePlanArgs,
) -> Result<PieceBuildPlan, String> {
    validate_piece_plan_inputs(candidate, intermediate, geometry)?;

    let regions_by_id = intermediate
        .regions
        .iter()
        .map(|region| (region.id.as_str(), region))
        .collect::<BTreeMap<_, _>>();
    let connectors_by_id = intermediate
        .connectors
        .iter()
        .map(|connector| (connector.id.as_str(), connector))
        .collect::<BTreeMap<_, _>>();
    let edges_by_id = candidate
        .graph
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect::<BTreeMap<_, _>>();
    let contents_by_room = geometry_contents_by_room(geometry);

    let mut requirements = Vec::new();
    let mut links = Vec::new();
    let mut content_requirements = Vec::new();
    let mut room_piece_ids = BTreeMap::new();

    for room in &geometry.rooms {
        let region = regions_by_id.get(room.source_region.as_str()).copied();
        let room_contents = contents_by_room
            .get(room.id.as_str())
            .cloned()
            .unwrap_or_default();
        let piece_id = piece_id_for_room(room);
        room_piece_ids.insert(room.id.as_str(), piece_id.clone());
        let required_exits = room_exit_requirements(room, geometry);
        let required_sockets = dedupe_strings(
            room_contents
                .iter()
                .map(|content| socket_for_content_kind(content.kind.as_str()))
                .collect(),
        );
        let mut tags = vec![
            room.role.clone(),
            room.geometry_role.clone(),
            room.footprint_class.clone(),
        ];
        tags.extend(room.style_tags.clone());
        tags.extend(room_contents.iter().map(|content| content.kind.clone()));
        if let Some(region) = region {
            tags.extend(region.entrance_expectations.clone());
        }

        requirements.push(PieceRequirement {
            piece_id: piece_id.clone(),
            kind: piece_kind_for_room(room, &room_contents),
            role: room.role.clone(),
            source_refs: room_source_refs(room),
            required_exits,
            required_sockets,
            required_shape_tags: Vec::new(),
            tags: dedupe_strings(tags),
            placement_hints: room_placement_hints(room, region),
        });

        for content in room_contents {
            content_requirements.push(PieceContentRequirement {
                id: format!(
                    "piece_content.{}.{}",
                    slugify_label(piece_id.as_str()),
                    slugify_label(content.kind.as_str())
                ),
                piece_id: piece_id.clone(),
                source_ref: content.source_ref.clone(),
                kind: content.kind.clone(),
                label: content.label.clone(),
                required_socket: socket_for_content_kind(content.kind.as_str()),
                tags: dedupe_strings(content.tags.clone()),
            });
        }
    }

    for corridor in &geometry.corridors {
        let Some(from_piece) = room_piece_ids.get(corridor.from_room.as_str()).cloned() else {
            continue;
        };
        let Some(to_piece) = room_piece_ids.get(corridor.to_room.as_str()).cloned() else {
            continue;
        };
        let connector = connectors_by_id
            .get(corridor.source_connector.as_str())
            .copied();
        let edge = edges_by_id.get(corridor.source_edge.as_str()).copied();
        match args.corridor_realization {
            CorridorRealization::Catalog => {
                let corridor_pieces = emit_corridor_piece_requirements(
                    corridor,
                    connector,
                    edge,
                    true,
                    &mut requirements,
                )?;
                link_pure_catalog_corridor(
                    corridor,
                    edge,
                    &from_piece,
                    &to_piece,
                    &corridor_pieces,
                    &mut links,
                );
                debug_assert!(!corridor_pieces.is_empty());
            }
            CorridorRealization::Hybrid => {
                let corridor_pieces = emit_corridor_piece_requirements(
                    corridor,
                    connector,
                    edge,
                    false,
                    &mut requirements,
                )?;
                link_hybrid_corridor(
                    corridor,
                    edge,
                    &from_piece,
                    &to_piece,
                    &mut links,
                );
                debug_assert!(!corridor_pieces.is_empty());
            }
            CorridorRealization::Procedural => {
                link_procedural_corridor(corridor, edge, &from_piece, &to_piece, &mut links);
            }
        }
    }

    Ok(PieceBuildPlan {
        kind: "asha_procgen.piece_build_plan.v1".to_owned(),
        schema_version: 1,
        plan_id: format!(
            "piece_plan.{}.{}",
            geometry.geometry_id,
            args.corridor_realization.as_str()
        ),
        candidate_id: candidate.candidate_id.clone(),
        geometry_id: geometry.geometry_id.clone(),
        corridor_realization: args.corridor_realization,
        source_candidate_ref: display_path(&args.candidate),
        source_intermediate_ref: display_path(&args.intermediate),
        source_geometry_ref: display_path(&args.geometry),
        requirements,
        links,
        content_requirements,
    })
}

fn validate_piece_plan_inputs(
    candidate: &Candidate,
    intermediate: &IntermediateBreakdown,
    geometry: &Geometry2dArtifact,
) -> Result<(), String> {
    if intermediate.candidate_id != candidate.candidate_id {
        return Err(format!(
            "intermediate candidate {} does not match candidate {}",
            intermediate.candidate_id, candidate.candidate_id
        ));
    }
    if geometry.candidate_id != candidate.candidate_id {
        return Err(format!(
            "geometry candidate {} does not match candidate {}",
            geometry.candidate_id, candidate.candidate_id
        ));
    }
    if geometry.kind != "asha_procgen.geometry_2d.v1" {
        return Err(format!(
            "piece plan requires geometry kind asha_procgen.geometry_2d.v1, got {}",
            geometry.kind
        ));
    }
    Ok(())
}

fn geometry_contents_by_room(
    geometry: &Geometry2dArtifact,
) -> BTreeMap<&str, Vec<&GeometryContent>> {
    let mut by_room: BTreeMap<&str, Vec<&GeometryContent>> = BTreeMap::new();
    for content in &geometry.contents {
        by_room
            .entry(content.room_id.as_str())
            .or_default()
            .push(content);
    }
    by_room
}

fn piece_id_for_room(room: &GeometryRoom) -> String {
    format!("piece.room.{}", slugify_label(room.id.as_str()))
}

fn piece_kind_for_room(room: &GeometryRoom, contents: &[&GeometryContent]) -> String {
    if room.role == "boss_gate" || room.geometry_role == "boss_threshold" {
        "boss".to_owned()
    } else if room.role == "gate" || room.geometry_role == "threshold" {
        "threshold".to_owned()
    } else if room.role == "pressure" || room.geometry_role == "pressure_lane" {
        "hazard".to_owned()
    } else if room.role == "reward" || room.geometry_role == "reward_pocket" {
        "reward".to_owned()
    } else if room.role == "secret"
        || room.geometry_role.contains("secret")
        || contents
            .iter()
            .any(|content| content.kind == "secret_route_marker")
    {
        "secret".to_owned()
    } else if room.role == "shortcut"
        || room.geometry_role.contains("shortcut")
        || contents
            .iter()
            .any(|content| content.kind == "shortcut_marker")
    {
        "shortcut".to_owned()
    } else if room.role == "resource"
        || room.geometry_role.contains("resource")
        || contents
        .iter()
        .any(|content| content.kind == "resource_clue")
    {
        "resource".to_owned()
    } else if contents.iter().any(|content| content.kind == "key_pickup") {
        "key".to_owned()
    } else {
        "room".to_owned()
    }
}

fn room_source_refs(room: &GeometryRoom) -> Vec<String> {
    let mut refs = vec![
        format!("geometryRoom:{}", room.id),
        format!("region:{}", room.source_region),
    ];
    refs.extend(
        room.source_nodes
            .iter()
            .map(|node_id| format!("node:{node_id}")),
    );
    refs
}

fn room_placement_hints(
    room: &GeometryRoom,
    region: Option<&IntermediateRegion>,
) -> Vec<String> {
    let mut hints = vec![
        format!("geometryRect:{}:{}:{}:{}", room.rect.x, room.rect.y, room.rect.width, room.rect.height),
        format!("footprintClass:{}", room.footprint_class),
    ];
    if let Some(region) = region {
        hints.push(format!("scaleBand:{}", region.scale_band));
        hints.push(format!("anchorQuality:{}", region.anchor_quality));
    }
    dedupe_strings(hints)
}

fn room_exit_requirements(
    room: &GeometryRoom,
    geometry: &Geometry2dArtifact,
) -> Vec<PieceExitRequirement> {
    let mut exits = Vec::new();
    for corridor in &geometry.corridors {
        if corridor.from_room == room.id {
            if let Some(direction) = corridor_endpoint_direction(corridor, true) {
                exits.push(PieceExitRequirement {
                    id: format!("exit.{}.{}", slugify_label(corridor.id.as_str()), direction),
                    direction,
                    width: corridor.width,
                    tags: dedupe_strings(corridor.semantic_tags.clone()),
                });
            }
        } else if corridor.to_room == room.id {
            if let Some(direction) = corridor_endpoint_direction(corridor, false) {
                exits.push(PieceExitRequirement {
                    id: format!("exit.{}.{}", slugify_label(corridor.id.as_str()), direction),
                    direction,
                    width: corridor.width,
                    tags: dedupe_strings(corridor.semantic_tags.clone()),
                });
            }
        }
    }
    exits
}

#[derive(Clone, Debug)]
struct CatalogRouteSegment {
    from: GeometryPoint,
    to: GeometryPoint,
    direction: String,
    target_cells: i32,
    direct_target_cells: i32,
}

#[derive(Clone, Debug)]
struct CatalogRoutePiece {
    piece_id: String,
    inbound_exit: String,
    outbound_exit: String,
}

const MAX_CATALOG_ROUTE_PIECES_PER_SECTION: usize = 64;
const CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL: i32 = 6;
const CATALOG_SMALL_BEND_ALLOWANCE_CELLS: i32 = 4;
const CATALOG_LARGE_BEND_THRESHOLD_CELLS: i32 = 16;
const CATALOG_TIGHT_BEND_THRESHOLD_CELLS: i32 = 3;
const PURE_CATALOG_TIGHT_BEND_AXIS_CELLS: i32 = 1;
const PURE_CATALOG_SMALL_BEND_AXIS_CELLS: i32 = 2;
const PURE_CATALOG_LARGE_BEND_AXIS_CELLS: i32 = 3;

fn emit_corridor_piece_requirements(
    corridor: &GeometryCorridor,
    connector: Option<&IntermediateConnector>,
    edge: Option<&Edge>,
    pure_catalog: bool,
    requirements: &mut Vec<PieceRequirement>,
) -> Result<Vec<CatalogRoutePiece>, String> {
    let segments = catalog_route_segments(corridor)?;
    let source_refs = corridor_source_refs(corridor, connector, edge);
    let base_tags = corridor_tags(corridor, connector, edge);
    let mut pieces = Vec::new();

    for (segment_index, segment) in segments.iter().enumerate() {
        let starts_at_bend = segment_index > 0;
        let ends_at_bend = segment_index + 1 < segments.len();
        let reserved_cells = if pure_catalog {
            let start_allowance = starts_at_bend
                .then(|| catalog_bend_axis_cells(&segments[segment_index - 1], segment))
                .unwrap_or(0);
            let end_allowance = ends_at_bend
                .then(|| catalog_bend_axis_cells(segment, &segments[segment_index + 1]))
                .unwrap_or(0);
            start_allowance + end_allowance
        } else {
            i32::from(starts_at_bend) * CATALOG_SMALL_BEND_ALLOWANCE_CELLS
                + i32::from(ends_at_bend) * CATALOG_SMALL_BEND_ALLOWANCE_CELLS
        };
        let segment_cells = if pure_catalog {
            segment.direct_target_cells
        } else {
            segment.target_cells
        };
        let uncovered_cells = segment_cells.saturating_sub(reserved_cells);
        let straight_spans = if pure_catalog {
            pure_catalog_straight_spans(uncovered_cells)
        } else {
            catalog_straight_spans(uncovered_cells)
        }
        .map_err(|remaining| {
            format!(
                "catalog corridor {} segment {} exceeds bounded piece coverage with {} cell(s) remaining",
                corridor.id,
                segment_index + 1,
                remaining
            )
        })?;
        for (tile_index, span) in straight_spans.iter().enumerate() {
            let anchor = interpolate_segment_anchor(
                segment,
                tile_index + 1,
                straight_spans.len() + 1,
            );
            pieces.push(push_catalog_route_piece(
                corridor,
                "corridor",
                format!("segment_{:02}.tile_{:02}", segment_index + 1, tile_index + 1),
                opposite_direction(segment.direction.as_str()).to_owned(),
                segment.direction.clone(),
                span.tag(),
                &anchor,
                Some(segment),
                &source_refs,
                &base_tags,
                requirements,
            ));
        }

        if let Some(next_segment) = segments.get(segment_index + 1) {
            let bend_size = if pure_catalog {
                catalog_bend_family(segment, next_segment)
            } else if segment.target_cells.min(next_segment.target_cells)
                >= CATALOG_LARGE_BEND_THRESHOLD_CELLS
            {
                "bend_large"
            } else {
                "bend_small"
            };
            pieces.push(push_catalog_route_piece(
                corridor,
                "bend",
                format!("turn_{:02}", segment_index + 1),
                opposite_direction(segment.direction.as_str()).to_owned(),
                next_segment.direction.clone(),
                bend_size,
                &segment.to,
                None,
                &source_refs,
                &base_tags,
                requirements,
            ));
        }
    }

    if pieces.len() > MAX_CATALOG_ROUTE_PIECES_PER_SECTION {
        return Err(format!(
            "catalog corridor {} requires {} pieces, exceeding bounded section limit {}",
            corridor.id,
            pieces.len(),
            MAX_CATALOG_ROUTE_PIECES_PER_SECTION
        ));
    }
    Ok(pieces)
}

fn catalog_bend_axis_cells(
    incoming: &CatalogRouteSegment,
    outgoing: &CatalogRouteSegment,
) -> i32 {
    match catalog_bend_family(incoming, outgoing) {
        "bend_large" => PURE_CATALOG_LARGE_BEND_AXIS_CELLS,
        "bend_tight" => PURE_CATALOG_TIGHT_BEND_AXIS_CELLS,
        _ => PURE_CATALOG_SMALL_BEND_AXIS_CELLS,
    }
}

fn catalog_bend_family(
    incoming: &CatalogRouteSegment,
    outgoing: &CatalogRouteSegment,
) -> &'static str {
    let shortest = incoming
        .direct_target_cells
        .min(outgoing.direct_target_cells);
    if shortest >= CATALOG_LARGE_BEND_THRESHOLD_CELLS {
        "bend_large"
    } else if shortest <= CATALOG_TIGHT_BEND_THRESHOLD_CELLS {
        "bend_tight"
    } else {
        "bend_small"
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CatalogStraightSpan {
    Short,
    Medium,
    Long,
}

impl CatalogStraightSpan {
    fn tag(self) -> &'static str {
        match self {
            Self::Short => "span_short",
            Self::Medium => "span_medium",
            Self::Long => "span_long",
        }
    }
}

fn catalog_straight_spans(
    mut target_cells: i32,
) -> Result<Vec<CatalogStraightSpan>, i32> {
    let mut spans = Vec::new();
    while target_cells > 0 && spans.len() < MAX_CATALOG_ROUTE_PIECES_PER_SECTION {
        let (span, covered_cells) = if target_cells >= 11 {
            (CatalogStraightSpan::Long, 13)
        } else if target_cells >= 8 {
            (CatalogStraightSpan::Medium, 9)
        } else {
            (CatalogStraightSpan::Short, 7)
        };
        spans.push(span);
        target_cells = target_cells.saturating_sub(covered_cells);
    }
    if target_cells > 0 {
        Err(target_cells)
    } else {
        Ok(spans)
    }
}

fn pure_catalog_straight_spans(
    mut target_cells: i32,
) -> Result<Vec<CatalogStraightSpan>, i32> {
    let mut spans = Vec::new();
    while target_cells > 0 && spans.len() < MAX_CATALOG_ROUTE_PIECES_PER_SECTION {
        let (span, covered_cells) = if target_cells >= 7 {
            (CatalogStraightSpan::Long, 7)
        } else if target_cells >= 3 {
            (CatalogStraightSpan::Medium, 3)
        } else {
            (CatalogStraightSpan::Short, 1)
        };
        spans.push(span);
        target_cells -= covered_cells;
    }
    if target_cells > 0 {
        Err(target_cells)
    } else {
        Ok(spans)
    }
}

fn catalog_route_segments(corridor: &GeometryCorridor) -> Result<Vec<CatalogRouteSegment>, String> {
    let mut points = Vec::new();
    for point in &corridor.points {
        if points.last() != Some(point) {
            points.push(point.clone());
        }
    }
    if points.len() < 2 {
        return Err(format!(
            "catalog corridor {} requires at least two distinct route points",
            corridor.id
        ));
    }
    let mut segments = Vec::new();
    for pair in points.windows(2) {
        let from = &pair[0];
        let to = &pair[1];
        if from.x != to.x && from.y != to.y {
            return Err(format!(
                "catalog corridor {} contains non-orthogonal segment {},{} -> {},{}",
                corridor.id, from.x, from.y, to.x, to.y
            ));
        }
        let Some(direction) = direction_between_points(from, to) else {
            continue;
        };
        let pixels = (to.x - from.x).abs() + (to.y - from.y).abs();
        let target_cells =
            (pixels + CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL - 1)
                / CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL;
        let direct_target_cells = target_cells
            + i32::from(pixels % CATALOG_ROUTE_PIXELS_PER_PLACEMENT_CELL == 0);
        segments.push(CatalogRouteSegment {
            from: from.clone(),
            to: to.clone(),
            direction,
            target_cells,
            direct_target_cells,
        });
    }
    if segments.is_empty() {
        return Err(format!(
            "catalog corridor {} has no realizable route segments",
            corridor.id
        ));
    }
    Ok(segments)
}

fn interpolate_segment_anchor(
    segment: &CatalogRouteSegment,
    numerator: usize,
    denominator: usize,
) -> GeometryPoint {
    let numerator = numerator as i64;
    let denominator = denominator.max(1) as i64;
    GeometryPoint {
        x: (i64::from(segment.from.x)
            + i64::from(segment.to.x - segment.from.x) * numerator / denominator)
            as i32,
        y: (i64::from(segment.from.y)
            + i64::from(segment.to.y - segment.from.y) * numerator / denominator)
            as i32,
    }
}

#[allow(clippy::too_many_arguments)]
fn push_catalog_route_piece(
    corridor: &GeometryCorridor,
    kind: &str,
    suffix: String,
    inbound_direction: String,
    outbound_direction: String,
    family_tag: &str,
    anchor: &GeometryPoint,
    segment: Option<&CatalogRouteSegment>,
    source_refs: &[String],
    base_tags: &[String],
    requirements: &mut Vec<PieceRequirement>,
) -> CatalogRoutePiece {
    let piece_id = format!(
        "piece.{}.{}.{}",
        kind,
        slugify_label(corridor.id.as_str()),
        suffix
    );
    let inbound_exit = format!("exit.{}.in", suffix);
    let outbound_exit = format!("exit.{}.out", suffix);
    let mut tags = base_tags.to_vec();
    tags.extend([
        if kind == "bend" { "bend" } else { "straight" }.to_owned(),
        family_tag.to_owned(),
    ]);
    let mut placement_hints = vec![format!("point:{}:{}", anchor.x, anchor.y)];
    if let Some(segment) = segment {
        placement_hints.push(format!(
            "segment:{}:{}:{}:{}",
            segment.from.x, segment.from.y, segment.to.x, segment.to.y
        ));
    }
    requirements.push(PieceRequirement {
        piece_id: piece_id.clone(),
        kind: kind.to_owned(),
        role: if kind == "bend" {
            "corridor_bend".to_owned()
        } else {
            "corridor_segment".to_owned()
        },
        source_refs: source_refs.to_vec(),
        required_exits: vec![
            PieceExitRequirement {
                id: inbound_exit.clone(),
                direction: inbound_direction,
                width: corridor.width,
                tags: base_tags.to_vec(),
            },
            PieceExitRequirement {
                id: outbound_exit.clone(),
                direction: outbound_direction,
                width: corridor.width,
                tags: base_tags.to_vec(),
            },
        ],
        required_sockets: Vec::new(),
        required_shape_tags: vec![
            "corridor".to_owned(),
            if kind == "bend" { "bend" } else { "straight" }.to_owned(),
            family_tag.to_owned(),
        ],
        tags: dedupe_strings(tags),
        placement_hints,
    });
    CatalogRoutePiece {
        piece_id,
        inbound_exit,
        outbound_exit,
    }
}

fn link_pure_catalog_corridor(
    corridor: &GeometryCorridor,
    edge: Option<&Edge>,
    from_room: &str,
    to_room: &str,
    pieces: &[CatalogRoutePiece],
    links: &mut Vec<PieceLink>,
) {
    let mut from_piece = from_room.to_owned();
    let mut from_exit = room_corridor_exit_id(corridor, true);
    for (index, piece) in pieces.iter().enumerate() {
        links.push(catalog_piece_link(
            corridor,
            edge,
            format!("piece_link.{}.catalog.{:02}", slugify_label(corridor.id.as_str()), index + 1),
            from_piece,
            from_exit,
            piece.piece_id.clone(),
            piece.inbound_exit.clone(),
            Vec::new(),
        ));
        from_piece = piece.piece_id.clone();
        from_exit = piece.outbound_exit.clone();
    }
    links.push(catalog_piece_link(
        corridor,
        edge,
        format!(
            "piece_link.{}.catalog.{:02}",
            slugify_label(corridor.id.as_str()),
            pieces.len() + 1
        ),
        from_piece,
        from_exit,
        to_room.to_owned(),
        room_corridor_exit_id(corridor, false),
        Vec::new(),
    ));
}

fn link_hybrid_corridor(
    corridor: &GeometryCorridor,
    edge: Option<&Edge>,
    from_piece: &str,
    to_piece: &str,
    links: &mut Vec<PieceLink>,
) {
    links.push(catalog_piece_link(
        corridor,
        edge,
        format!(
            "piece_link.{}.catalog",
            slugify_label(corridor.id.as_str())
        ),
        from_piece.to_owned(),
        room_corridor_exit_id(corridor, true),
        to_piece.to_owned(),
        room_corridor_exit_id(corridor, false),
        corridor.points.clone(),
    ));
}

#[allow(clippy::too_many_arguments)]
fn catalog_piece_link(
    corridor: &GeometryCorridor,
    edge: Option<&Edge>,
    id: String,
    from_piece: String,
    from_exit: String,
    to_piece: String,
    to_exit: String,
    route_points: Vec<GeometryPoint>,
) -> PieceLink {
    PieceLink {
        id,
        from_piece,
        from_exit,
        to_piece,
        to_exit,
        source_section: corridor.physical_section.clone(),
        source_corridor: corridor.id.clone(),
        source_edge: corridor.source_edge.clone(),
        source_edges: corridor.source_edges.clone(),
        traversal_refs: corridor.traversal_refs.clone(),
        source_ref: format!(
            "physicalSection:{};geometryCorridor:{};connector:{};edge:{}",
            corridor.physical_section,
            corridor.id,
            corridor.source_connector,
            corridor.source_edge
        ),
        traversal: edge
            .map(|source_edge| source_edge.traversal.as_str().to_owned())
            .unwrap_or_else(|| corridor.traversal_hint.clone()),
        required_item: edge.and_then(|source_edge| source_edge.required_item.clone()),
        tags: dedupe_strings(corridor.semantic_tags.clone()),
        route_points,
    }
}

fn corridor_distance_at_point(
    corridor: &GeometryCorridor,
    point: &GeometryPoint,
) -> Option<i64> {
    let mut distance = 0_i64;
    for pair in corridor.points.windows(2) {
        let from = &pair[0];
        let to = &pair[1];
        let on_segment = if from.x == to.x {
            point.x == from.x
                && point.y >= from.y.min(to.y)
                && point.y <= from.y.max(to.y)
        } else if from.y == to.y {
            point.y == from.y
                && point.x >= from.x.min(to.x)
                && point.x <= from.x.max(to.x)
        } else {
            false
        };
        if on_segment {
            return Some(
                distance
                    + i64::from((point.x - from.x).abs() + (point.y - from.y).abs()),
            );
        }
        distance += i64::from((to.x - from.x).abs() + (to.y - from.y).abs());
    }
    corridor
        .points
        .last()
        .filter(|last| *last == point)
        .map(|_| distance)
}

fn link_procedural_corridor(
    corridor: &GeometryCorridor,
    edge: Option<&Edge>,
    from_piece: &str,
    to_piece: &str,
    links: &mut Vec<PieceLink>,
) {
    links.push(PieceLink {
        id: format!(
            "piece_link.{}.procedural",
            slugify_label(corridor.id.as_str())
        ),
        from_piece: from_piece.to_owned(),
        from_exit: room_corridor_exit_id(corridor, true),
        to_piece: to_piece.to_owned(),
        to_exit: room_corridor_exit_id(corridor, false),
        source_section: corridor.physical_section.clone(),
        source_corridor: corridor.id.clone(),
        source_edge: corridor.source_edge.clone(),
        source_edges: corridor.source_edges.clone(),
        traversal_refs: corridor.traversal_refs.clone(),
        source_ref: format!(
            "physicalSection:{};geometryCorridor:{};connector:{};edge:{}",
            corridor.physical_section,
            corridor.id,
            corridor.source_connector,
            corridor.source_edge
        ),
        traversal: edge
            .map(|source_edge| source_edge.traversal.as_str().to_owned())
            .unwrap_or_else(|| corridor.traversal_hint.clone()),
        required_item: edge.and_then(|source_edge| source_edge.required_item.clone()),
        tags: dedupe_strings(corridor.semantic_tags.clone()),
        route_points: corridor.points.clone(),
    });
}

fn room_corridor_exit_id(corridor: &GeometryCorridor, start: bool) -> String {
    let direction = corridor_endpoint_direction(corridor, start)
        .unwrap_or_else(|| "unknown".to_owned());
    format!("exit.{}.{}", slugify_label(corridor.id.as_str()), direction)
}

fn corridor_source_refs(
    corridor: &GeometryCorridor,
    connector: Option<&IntermediateConnector>,
    edge: Option<&Edge>,
) -> Vec<String> {
    let mut refs = vec![
        format!("physicalSection:{}", corridor.physical_section),
        format!("geometryCorridor:{}", corridor.id),
        format!("room:{}", corridor.from_room),
        format!("room:{}", corridor.to_room),
    ];
    refs.extend(corridor.source_connectors.iter().map(|value| format!("connector:{value}")));
    refs.extend(corridor.source_edges.iter().map(|value| format!("edge:{value}")));
    if let Some(connector) = connector {
        refs.extend(
            connector
                .constraint_refs
                .iter()
                .map(|constraint| format!("constraint:{constraint}")),
        );
    }
    if let Some(edge) = edge {
        refs.push(format!("graphEdge:{}:{}", edge.from, edge.to));
    }
    dedupe_strings(refs)
}

fn corridor_tags(
    corridor: &GeometryCorridor,
    connector: Option<&IntermediateConnector>,
    edge: Option<&Edge>,
) -> Vec<String> {
    let mut tags = vec![
        corridor.traversal_hint.clone(),
        "explicit_corridor_piece".to_owned(),
    ];
    tags.extend(corridor.semantic_tags.clone());
    if let Some(connector) = connector {
        tags.extend(connector.intents.clone());
        tags.extend(connector.affordances.clone());
    }
    if let Some(edge) = edge {
        tags.push(edge.kind.as_str().to_owned());
        tags.push(edge.traversal.as_str().to_owned());
        tags.extend(edge.tags.clone());
        if let Some(required_item) = &edge.required_item {
            tags.push(format!("requires:{required_item}"));
        }
    }
    dedupe_strings(tags)
}

fn corridor_endpoint_direction(corridor: &GeometryCorridor, start: bool) -> Option<String> {
    if corridor.points.len() < 2 {
        return None;
    }
    if start {
        direction_between_points(&corridor.points[0], &corridor.points[1])
    } else {
        let last = corridor.points.len() - 1;
        direction_between_points(&corridor.points[last], &corridor.points[last - 1])
    }
}

fn direction_between_points(from: &GeometryPoint, to: &GeometryPoint) -> Option<String> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    if dx.abs() >= dy.abs() && dx != 0 {
        Some(if dx > 0 { "east" } else { "west" }.to_owned())
    } else if dy != 0 {
        Some(if dy > 0 { "south" } else { "north" }.to_owned())
    } else {
        None
    }
}

fn opposite_direction(direction: &str) -> &'static str {
    match direction {
        "north" => "south",
        "east" => "west",
        "south" => "north",
        "west" => "east",
        _ => "unknown",
    }
}

fn socket_for_content_kind(kind: &str) -> String {
    match kind {
        "boss_threshold" => "boss_space".to_owned(),
        "hazard" => "hazard_zone".to_owned(),
        "locked_gate" => "gate_line".to_owned(),
        "secret_route_marker" => "secret_marker".to_owned(),
        "start_marker" | "goal_marker" => "marker".to_owned(),
        other => other.to_owned(),
    }
}
