fn physical_connection_plan_command(args: PhysicalConnectionPlanArgs) -> Result<(), String> {
    let candidate: Candidate = read_json(&args.candidate)?;
    let intermediate: IntermediateBreakdown = read_json(&args.intermediate)?;
    let plan = plan_physical_connections(&candidate, &intermediate, &args)?;
    write_json(&args.out, &plan)
}

fn plan_physical_connections(
    candidate: &Candidate,
    intermediate: &IntermediateBreakdown,
    args: &PhysicalConnectionPlanArgs,
) -> Result<PhysicalConnectionPlan, String> {
    if intermediate.candidate_id != candidate.candidate_id {
        return Err(format!(
            "intermediate candidate {} does not match candidate {}",
            intermediate.candidate_id, candidate.candidate_id
        ));
    }
    let edges = candidate
        .graph
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect::<BTreeMap<_, _>>();
    let mut grouped: BTreeMap<String, Vec<(&IntermediateConnector, &Edge)>> = BTreeMap::new();
    for connector in &intermediate.connectors {
        let edge = edges.get(connector.edge_id.as_str()).copied().ok_or_else(|| {
            format!("connector {} references missing edge {}", connector.id, connector.edge_id)
        })?;
        let mergeable_open = edge.traversal == TraversalKind::Open
            && edge.required_item.is_none()
            && connector.traversal_hint == "open";
        let key = if mergeable_open {
            let mut terminals = [connector.from_region.as_str(), connector.to_region.as_str()];
            terminals.sort();
            format!("open:{}:{}", terminals[0], terminals[1])
        } else {
            format!("edge:{}", connector.id)
        };
        grouped.entry(key).or_default().push((connector, edge));
    }

    let mut sections = Vec::new();
    let mut edge_mappings = Vec::new();
    for (group_key, members) in grouped {
        let mut terminal_regions = members
            .iter()
            .flat_map(|(connector, _)| [connector.from_region.clone(), connector.to_region.clone()])
            .collect::<Vec<_>>();
        terminal_regions.sort();
        terminal_regions.dedup();
        if terminal_regions.len() != 2 {
            return Err(format!(
                "physical connection group {group_key} requires exactly two terminals; found {}",
                terminal_regions.len()
            ));
        }
        let section_suffix = group_key
            .strip_prefix("edge:")
            .map(slugify_label)
            .unwrap_or_else(|| "open".to_owned());
        let section_id = format!(
            "section.{}.{}.{}",
            slugify_label(terminal_regions[0].as_str()),
            slugify_label(terminal_regions[1].as_str()),
            section_suffix
        );
        let mut source_connectors = Vec::new();
        let mut source_edges = Vec::new();
        let mut traversal_refs = Vec::new();
        let mut semantic_tags = Vec::new();
        let mut width = 0;
        for (connector, edge) in members {
            source_connectors.push(connector.id.clone());
            source_edges.push(edge.id.clone());
            semantic_tags.extend(corridor_semantic_tags(connector));
            width = width.max(corridor_width(connector));
            traversal_refs.push(PhysicalTraversalRef {
                connector_id: connector.id.clone(),
                edge_id: edge.id.clone(),
                from_region: connector.from_region.clone(),
                to_region: connector.to_region.clone(),
                traversal: edge.traversal.as_str().to_owned(),
                required_item: edge.required_item.clone(),
            });
            edge_mappings.push(PhysicalEdgeMapping {
                edge_id: edge.id.clone(),
                connector_id: connector.id.clone(),
                section_id: section_id.clone(),
                from_region: connector.from_region.clone(),
                to_region: connector.to_region.clone(),
            });
        }
        source_connectors.sort();
        source_edges.sort();
        traversal_refs.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
        sections.push(PhysicalConnectionSection {
            id: section_id,
            topology: "corridor_2".to_owned(),
            terminal_regions,
            source_connectors,
            source_edges,
            traversal_refs,
            width,
            semantic_tags: dedupe_strings(semantic_tags),
        });
    }
    sections.sort_by(|left, right| left.id.cmp(&right.id));
    edge_mappings.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    Ok(PhysicalConnectionPlan {
        kind: "rusty_procgen.physical_connection_plan.v1".to_owned(),
        schema_version: 1,
        plan_id: format!("physical_connections.{}", candidate.candidate_id),
        candidate_id: candidate.candidate_id.clone(),
        source_candidate_ref: display_path(&args.candidate),
        source_intermediate_ref: display_path(&args.intermediate),
        sections,
        edge_mappings,
    })
}

fn geometry_emit_2d_command(args: GeometryEmit2dArgs) -> Result<(), String> {
    let candidate: Candidate = read_json(&args.candidate)?;
    let intermediate: IntermediateBreakdown = read_json(&args.intermediate)?;
    let connection_plan: PhysicalConnectionPlan = read_json(&args.connection_plan)?;
    let geometry = emit_geometry_2d(&candidate, &intermediate, &connection_plan, &args, args.seed)?;
    write_json(&args.out, &geometry)
}

fn geometry_validate_2d_command(args: ReportOutArgs) -> Result<(), String> {
    let geometry: Geometry2dArtifact = read_json(&args.state)?;
    let report = validate_geometry_2d(&geometry);
    write_json(&args.out, &report)?;
    if report.ok {
        Ok(())
    } else {
        Err(format!(
            "2D geometry validation failed with {} fatal diagnostic(s); see {}",
            report.fatal_count,
            args.out.display()
        ))
    }
}

fn preview_html_command(args: PreviewHtmlArgs) -> Result<(), String> {
    let geometry: Geometry2dArtifact = read_json(&args.geometry)?;
    let validation: ValidationReport = read_json(&args.validation)?;
    validate_preview_inputs(&geometry, &validation, args.allow_invalid)?;
    let html = render_geometry_preview_html(
        &geometry,
        &validation,
        &display_path(&args.geometry),
        &display_path(&args.validation),
    );
    write_text(&args.out, &html)
}

fn validate_preview_inputs(
    geometry: &Geometry2dArtifact,
    validation: &ValidationReport,
    allow_invalid: bool,
) -> Result<(), String> {
    if validation.kind != "rusty_procgen.validation.geometry_2d.v1" {
        return Err(format!(
            "preview html requires geometry validation kind rusty_procgen.validation.geometry_2d.v1, got {}",
            validation.kind
        ));
    }
    let geometry_hash = hash_json(geometry)?;
    if validation.state_hash != geometry_hash {
        return Err("preview html validation hash does not match geometry input".to_owned());
    }
    if !validation.ok && !allow_invalid {
        return Err(format!(
            "preview html refused invalid geometry with {} fatal diagnostic(s); pass --allow-invalid to render diagnostics",
            validation.fatal_count
        ));
    }
    Ok(())
}

fn render_geometry_preview_html(
    geometry: &Geometry2dArtifact,
    validation: &ValidationReport,
    geometry_ref: &str,
    validation_ref: &str,
) -> String {
    let svg_width = geometry.bounds.width.max(320);
    let svg_height = geometry.bounds.height.max(240);
    let mut corridors = String::new();
    for corridor in &geometry.corridors {
        let points = corridor
            .points
            .iter()
            .map(|point| format!("{},{}", point.x, point.y))
            .collect::<Vec<_>>()
            .join(" ");
        corridors.push_str(&format!(
            r#"<polyline class="corridor corridor-{}" data-source-edge="{}" points="{}" stroke-width="{}" />
"#,
            css_token(&corridor.traversal_hint),
            escape_attr(&corridor.source_edge),
            escape_attr(&points),
            corridor.width.max(2)
        ));
    }

    let mut rooms = String::new();
    for room in &geometry.rooms {
        let fill = room_fill(room);
        rooms.push_str(&format!(
            r#"<g class="room room-{}" data-room-id="{}" data-role="{}">
  <rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}" />
  <text class="room-label" x="{}" y="{}">{}</text>
"#,
            css_token(&room.role),
            escape_attr(&room.id),
            escape_attr(&room.role),
            room.rect.x,
            room.rect.y,
            room.rect.width,
            room.rect.height,
            fill,
            room.rect.x + 10,
            room.rect.y + 20,
            escape_html(&room_label(room))
        ));
        for (index, content) in geometry
            .contents
            .iter()
            .filter(|content| content.room_id == room.id)
            .enumerate()
        {
            rooms.push_str(&format!(
                r#"  <text class="content-label content-{}" x="{}" y="{}">{}</text>
"#,
                css_token(&content.kind),
                room.rect.x + 10,
                room.rect.y + 38 + index as i32 * 14,
                escape_html(&content.label)
            ));
        }
        rooms.push_str("</g>\n");
    }

    let diagnostics = if validation.diagnostics.is_empty() {
        "<li>No diagnostics.</li>".to_owned()
    } else {
        validation
            .diagnostics
            .iter()
            .map(|diagnostic| {
                format!(
                    "<li><strong>{}</strong> [{}] {}</li>",
                    escape_html(&diagnostic.code),
                    escape_html(severity_label(diagnostic.severity)),
                    escape_html(&diagnostic.detail)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let legend_items = [
        ("Start/Goal", "#5fb3ff"),
        ("Gate/Boss", "#f0b35f"),
        ("Hazard", "#ff6b6b"),
        ("Reward/Key", "#7bd88f"),
        ("Secret/Shortcut", "#c792ea"),
        ("Standard", "#94a3b8"),
    ]
    .iter()
    .map(|(label, color)| {
        format!(
            r#"<li><span style="background:{}"></span>{}</li>"#,
            color,
            escape_html(label)
        )
    })
    .collect::<Vec<_>>()
    .join("\n");

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Rusty Procgen Dungeon Preview</title>
<style>
:root {{ color-scheme: dark; font-family: Inter, ui-sans-serif, system-ui, sans-serif; background: #0b0d10; color: #e8edf2; }}
body {{ margin: 0; background: #0b0d10; }}
main {{ min-height: 100vh; display: grid; grid-template-columns: minmax(0, 1fr) 320px; gap: 0; }}
.stage {{ overflow: auto; padding: 24px; background: #0f1318; }}
.panel {{ border-left: 1px solid #2b3440; padding: 20px; background: #121820; }}
h1 {{ margin: 0 0 12px; font-size: 20px; }}
h2 {{ margin: 20px 0 8px; font-size: 14px; color: #b7c4d3; text-transform: uppercase; }}
p, li {{ color: #c7d1dc; font-size: 13px; line-height: 1.45; }}
code {{ color: #f8d67a; overflow-wrap: anywhere; }}
svg {{ display: block; min-width: {}px; min-height: {}px; background: #151b22; border: 1px solid #2d3743; }}
.corridor {{ fill: none; stroke: #6e7f93; stroke-linecap: round; stroke-linejoin: round; opacity: 0.82; }}
.corridor-locked {{ stroke: #f0b35f; }}
.corridor-hidden {{ stroke: #c792ea; stroke-dasharray: 10 8; }}
.corridor-one-way-return {{ stroke: #5fb3ff; stroke-dasharray: 16 6; }}
.room rect {{ stroke: #d3deea; stroke-width: 1.5; }}
.room-label {{ fill: #f4f8fb; font-size: 13px; font-weight: 700; }}
.content-label {{ fill: #d6e0eb; font-size: 11px; }}
.legend {{ list-style: none; padding: 0; margin: 0; }}
.legend li {{ display: flex; align-items: center; gap: 8px; margin: 6px 0; }}
.legend span {{ display: inline-block; width: 12px; height: 12px; border-radius: 2px; }}
.status-ok {{ color: #7bd88f; }}
.status-bad {{ color: #ff6b6b; }}
@media (max-width: 900px) {{ main {{ grid-template-columns: 1fr; }} .panel {{ border-left: 0; border-top: 1px solid #2b3440; }} }}
</style>
</head>
<body data-preview-kind="rusty_procgen.html_preview.v1" data-kind="{}">
<main>
<section class="stage" aria-label="Dungeon floor plan">
<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="preview-title" viewBox="0 0 {} {}">
<title id="preview-title">Generated dungeon preview for {}</title>
<g class="corridors">
{}</g>
<g class="rooms">
{}</g>
</svg>
</section>
<aside class="panel">
<h1>Dungeon Preview</h1>
<p class="{}">Validation: {}</p>
<p>Geometry: <code>{}</code></p>
<p>Validation: <code>{}</code></p>
<p>Rooms: {} · Corridors: {} · Contents: {}</p>
<h2>Legend</h2>
<ul class="legend">
{}
</ul>
<h2>Diagnostics</h2>
<ul>
{}
</ul>
</aside>
</main>
</body>
</html>
"#,
        svg_width,
        svg_height,
        escape_attr(&geometry.kind),
        svg_width,
        svg_height,
        escape_html(&geometry.geometry_id),
        corridors,
        rooms,
        if validation.ok {
            "status-ok"
        } else {
            "status-bad"
        },
        if validation.ok { "ok" } else { "invalid" },
        escape_html(geometry_ref),
        escape_html(validation_ref),
        geometry.rooms.len(),
        geometry.corridors.len(),
        geometry.contents.len(),
        legend_items,
        diagnostics
    )
}

fn room_label(room: &GeometryRoom) -> String {
    if room.role == room.geometry_role {
        room.role.clone()
    } else {
        format!("{} / {}", room.role, room.geometry_role)
    }
}

fn room_fill(room: &GeometryRoom) -> &'static str {
    match room.role.as_str() {
        "start" | "goal" => "#1f5f89",
        "gate" | "boss_gate" => "#725124",
        "pressure" => "#733238",
        "reward" => "#245a38",
        "landmark_hub" => "#394762",
        _ if room.geometry_role.contains("secret") || room.geometry_role.contains("shortcut") => {
            "#563d72"
        }
        _ => "#2d3a47",
    }
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Fatal => "fatal",
    }
}

fn css_token(value: &str) -> String {
    let token = slugify_label(value).replace('_', "-");
    if token.is_empty() {
        "unknown".to_owned()
    } else {
        token
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(value: &str) -> String {
    escape_html(value).replace('"', "&quot;")
}

fn validate_geometry_2d(geometry: &Geometry2dArtifact) -> ValidationReport {
    let mut diagnostics = Vec::new();
    if geometry.kind != "rusty_procgen.geometry_2d.v1" {
        diagnostics.push(fatal(
            "geometry_kind_invalid",
            None,
            None,
            "Geometry artifact kind must be rusty_procgen.geometry_2d.v1.",
        ));
    }
    if geometry.bounds.width <= 0 || geometry.bounds.height <= 0 || geometry.bounds.grid <= 0 {
        diagnostics.push(fatal(
            "geometry_bounds_invalid",
            None,
            None,
            "Geometry bounds width, height, and grid must be positive.",
        ));
    }
    if geometry.source_connection_plan_ref.is_empty() || geometry.connection_plan_id.is_empty() {
        diagnostics.push(fatal(
            "geometry_connection_plan_ref_missing",
            None,
            None,
            "Geometry must identify the exact physical connection plan it projects.",
        ));
    }
    match validate_geometry_layout_policy(&geometry.layout_policy) {
        Err(error) => diagnostics.push(fatal(
            "geometry_layout_policy_invalid",
            None,
            None,
            error,
        )),
        Ok(()) => {
            let search = &geometry.layout_search;
            if search.spacing_tier >= geometry.layout_policy.max_spacing_tiers
                || search.room_order_attempt
                    >= geometry.layout_policy.room_order_attempts_per_tier
                || search.port_order_attempt >= GEOMETRY_PORT_ORDER_COUNT
                || search.route_order_attempt >= GEOMETRY_ROUTE_ORDER_COUNT
                || search.search_attempts == 0
                || search.search_attempts > geometry.layout_policy.max_search_attempts
            {
                diagnostics.push(fatal(
                    "geometry_layout_search_evidence_invalid",
                    None,
                    None,
                    "Geometry layout search evidence exceeds its policy tier, ordering, or attempt bounds.",
                ));
            } else if geometry_spacing_for_tier(
                &geometry.layout_policy,
                search.spacing_tier,
            )
            .is_ok_and(|expected| expected != search.effective_spacing)
            {
                diagnostics.push(fatal(
                    "geometry_layout_search_spacing_mismatch",
                    None,
                    None,
                    "Geometry effective spacing does not match its recorded policy tier.",
                ));
            }
        }
    }

    let mut rooms_by_id = BTreeMap::new();
    let mut rooms_by_region = BTreeMap::new();
    for room in &geometry.rooms {
        if room.id.is_empty() {
            diagnostics.push(fatal(
                "geometry_room_id_missing",
                room.source_nodes.first().map(String::as_str),
                None,
                "Room id must not be empty.",
            ));
            continue;
        }
        if rooms_by_id.insert(room.id.as_str(), room).is_some() {
            diagnostics.push(fatal(
                "geometry_room_duplicate",
                room.source_nodes.first().map(String::as_str),
                None,
                format!("Room id {} appears more than once.", room.id),
            ));
        }
        rooms_by_region.insert(room.source_region.as_str(), room);
        let mut room_port_positions = BTreeSet::new();
        let mut room_port_sections = BTreeSet::new();
        for port in &room.ports {
            if !geometry_point_on_rect_boundary(&port.point, &room.rect) {
                diagnostics.push(fatal(
                    "geometry_room_port_detached",
                    room.source_nodes.first().map(String::as_str),
                    None,
                    format!("Room port {} is not on room {} boundary.", port.id, room.id),
                ));
            }
            if !room_port_positions.insert((port.point.x, port.point.y)) {
                diagnostics.push(fatal(
                    "geometry_room_port_span_reused",
                    room.source_nodes.first().map(String::as_str),
                    None,
                    format!("Room {} reuses doorway position {},{}.", room.id, port.point.x, port.point.y),
                ));
            }
            if !room_port_sections.insert(port.section_id.as_str()) {
                diagnostics.push(fatal(
                    "geometry_room_port_section_duplicate",
                    room.source_nodes.first().map(String::as_str),
                    None,
                    format!("Room {} has duplicate ports for section {}.", room.id, port.section_id),
                ));
            }
        }
        if room.rect.width <= 0 || room.rect.height <= 0 {
            diagnostics.push(fatal(
                "geometry_room_rect_invalid",
                room.source_nodes.first().map(String::as_str),
                None,
                format!("Room {} has a non-positive rectangle.", room.id),
            ));
        }
        if room.rect.x < 0
            || room.rect.y < 0
            || room.rect.x + room.rect.width > geometry.bounds.width
            || room.rect.y + room.rect.height > geometry.bounds.height
        {
            diagnostics.push(fatal(
                "geometry_room_out_of_bounds",
                room.source_nodes.first().map(String::as_str),
                None,
                format!("Room {} extends outside geometry bounds.", room.id),
            ));
        }
    }
    for (index, left) in geometry.rooms.iter().enumerate() {
        for right in geometry.rooms.iter().skip(index + 1) {
            if geometry_rectangles_overlap(&left.rect, &right.rect) {
                diagnostics.push(fatal(
                    "geometry_room_overlap",
                    left.source_nodes.first().map(String::as_str),
                    None,
                    format!("Room {} overlaps {}.", left.id, right.id),
                ));
            }
        }
    }

    let mut represented_connectors = BTreeSet::new();
    let mut represented_edges = BTreeSet::new();
    let mut represented_sections = BTreeSet::new();
    let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    if geometry.rooms.len() > 1
        && geometry.corridors.is_empty()
        && geometry.skipped_connectors.is_empty()
    {
        diagnostics.push(fatal(
            "geometry_connector_coverage_missing",
            None,
            None,
            "Multi-room geometry must include routed corridors or explicit skipped connectors.",
        ));
    }
    for corridor in &geometry.corridors {
        if !represented_sections.insert(corridor.physical_section.as_str()) {
            diagnostics.push(fatal(
                "geometry_physical_section_duplicate",
                None,
                Some(corridor.id.as_str()),
                format!(
                    "Physical section {} is represented by more than one corridor.",
                    corridor.physical_section
                ),
            ));
        }
        if corridor.physical_section.is_empty()
            || corridor.source_connectors.is_empty()
            || corridor.source_edges.is_empty()
            || corridor.traversal_refs.is_empty()
        {
            diagnostics.push(fatal(
                "geometry_corridor_source_missing",
                None,
                Some(corridor.id.as_str()),
                "Corridor must preserve source connector and source edge refs.",
            ));
        } else {
            for connector in &corridor.source_connectors {
                if !represented_connectors.insert(connector.as_str()) {
                    diagnostics.push(fatal(
                        "geometry_corridor_duplicate_connector",
                        None,
                        Some(corridor.id.as_str()),
                        format!("Connector {connector} is represented by more than one physical section."),
                    ));
                }
            }
            for edge in &corridor.source_edges {
                if !represented_edges.insert(edge.as_str()) {
                    diagnostics.push(fatal(
                        "geometry_corridor_duplicate_edge",
                        None,
                        Some(corridor.id.as_str()),
                        format!("Source edge {edge} is mapped to more than one physical section."),
                    ));
                }
            }
            let traversal_edges = corridor
                .traversal_refs
                .iter()
                .map(|reference| reference.edge_id.as_str())
                .collect::<BTreeSet<_>>();
            let traversal_connectors = corridor
                .traversal_refs
                .iter()
                .map(|reference| reference.connector_id.as_str())
                .collect::<BTreeSet<_>>();
            if traversal_edges
                != corridor.source_edges.iter().map(String::as_str).collect::<BTreeSet<_>>()
                || traversal_connectors
                    != corridor
                        .source_connectors
                        .iter()
                        .map(String::as_str)
                        .collect::<BTreeSet<_>>()
            {
                diagnostics.push(fatal(
                    "geometry_corridor_traversal_mapping_mismatch",
                    None,
                    Some(corridor.id.as_str()),
                    "Corridor traversal refs must exactly cover its source connectors and edges.",
                ));
            }
        }
        let from_room = rooms_by_id.get(corridor.from_room.as_str()).copied();
        let to_room = rooms_by_id.get(corridor.to_room.as_str()).copied();
        if from_room.is_none() || to_room.is_none() {
            diagnostics.push(fatal(
                "geometry_corridor_room_missing",
                None,
                Some(corridor.id.as_str()),
                format!(
                    "Corridor {} references a missing room endpoint.",
                    corridor.id
                ),
            ));
        }
        if corridor.points.len() < 2 {
            diagnostics.push(fatal(
                "geometry_corridor_points_missing",
                None,
                Some(corridor.id.as_str()),
                format!("Corridor {} must have at least two points.", corridor.id),
            ));
        }
        if let (Some(from_room), Some(to_room), Some(first), Some(last)) = (
            from_room,
            to_room,
            corridor.points.first(),
            corridor.points.last(),
        ) {
            if !geometry_point_on_rect_boundary(first, &from_room.rect)
                || !geometry_point_on_rect_boundary(last, &to_room.rect)
            {
                diagnostics.push(fatal(
                    "geometry_corridor_endpoint_detached",
                    None,
                    Some(corridor.id.as_str()),
                    format!(
                        "Corridor {} endpoints must attach to source and target room bounds.",
                        corridor.id
                    ),
                ));
            }
            if !from_room
                .ports
                .iter()
                .any(|port| port.id == corridor.from_port && port.section_id == corridor.physical_section)
                || !to_room
                    .ports
                    .iter()
                    .any(|port| port.id == corridor.to_port && port.section_id == corridor.physical_section)
            {
                diagnostics.push(fatal(
                    "geometry_corridor_port_mismatch",
                    None,
                    Some(corridor.id.as_str()),
                    format!("Corridor {} does not identify its planned terminal ports.", corridor.id),
                ));
            }
            for traversal in &corridor.traversal_refs {
                let terminal_pair = sorted_pair(
                    traversal.from_region.as_str(),
                    traversal.to_region.as_str(),
                );
                let room_pair = sorted_pair(
                    from_room.source_region.as_str(),
                    to_room.source_region.as_str(),
                );
                if terminal_pair != room_pair {
                    diagnostics.push(fatal(
                        "geometry_corridor_terminal_mapping_mismatch",
                        None,
                        Some(traversal.edge_id.as_str()),
                        format!(
                            "Traversal {} does not terminate at corridor rooms {} and {}.",
                            traversal.edge_id, from_room.source_region, to_room.source_region
                        ),
                    ));
                }
                if let (Some(source), Some(target)) = (
                    rooms_by_region.get(traversal.from_region.as_str()),
                    rooms_by_region.get(traversal.to_region.as_str()),
                ) {
                    adjacency.entry(source.id.as_str()).or_default().push(target.id.as_str());
                }
            }
        }
        if corridor.traversal_refs.iter().any(|reference| reference.traversal == "locked")
            && !corridor
                .semantic_tags
                .iter()
                .any(|tag| tag == "locked_threshold")
        {
            diagnostics.push(fatal(
                "geometry_locked_semantics_missing",
                None,
                Some(corridor.id.as_str()),
                "Locked corridors must preserve locked_threshold semantics.",
            ));
        }
        if corridor.traversal_refs.iter().any(|reference| reference.traversal == "hidden")
            && !corridor
                .semantic_tags
                .iter()
                .any(|tag| tag == "hidden_route" || tag == "hidden_passage")
        {
            diagnostics.push(fatal(
                "geometry_hidden_semantics_missing",
                None,
                Some(corridor.id.as_str()),
                "Hidden corridors must preserve hidden route semantics.",
            ));
        }
        if corridor
            .semantic_tags
            .iter()
            .any(|tag| tag == "shortcut_link")
            && corridor.source_edge.is_empty()
        {
            diagnostics.push(fatal(
                "geometry_shortcut_source_missing",
                None,
                Some(corridor.id.as_str()),
                "Shortcut corridors must preserve source edge refs.",
            ));
        }
    }

    validate_exclusive_geometry_routes(geometry, &rooms_by_id, &mut diagnostics);

    let mut skipped_connectors = BTreeSet::new();
    for skipped in &geometry.skipped_connectors {
        if skipped.source_connector.is_empty() || skipped.reason.is_empty() {
            diagnostics.push(fatal(
                "geometry_skipped_connector_invalid",
                None,
                None,
                "Skipped connectors must include source connector and reason.",
            ));
        } else if !skipped_connectors.insert(skipped.source_connector.as_str()) {
            diagnostics.push(fatal(
                "geometry_skipped_connector_duplicate",
                None,
                Some(skipped.source_connector.as_str()),
                format!(
                    "Skipped connector {} appears more than once.",
                    skipped.source_connector
                ),
            ));
        }
        if represented_connectors.contains(skipped.source_connector.as_str()) {
            diagnostics.push(fatal(
                "geometry_connector_represented_and_skipped",
                None,
                Some(skipped.source_connector.as_str()),
                format!(
                    "Connector {} is both routed and skipped.",
                    skipped.source_connector
                ),
            ));
        }
    }

    validate_geometry_content_anchors(geometry, &rooms_by_id, &mut diagnostics);
    validate_geometry_reachability(geometry, &adjacency, &mut diagnostics);
    let compactness = geometry_compactness_score(
        &geometry.bounds,
        &geometry.rooms,
        &geometry.corridors,
        geometry.layout_search.embedding_id.as_str(),
    );
    if geometry.layout_search.valid_layout_candidates == 0
        || geometry
            .layout_search
            .compactness_portal_capacity_penalty
            != compactness.portal_capacity_penalty
        || geometry.layout_search.compactness_envelope_area != compactness.envelope_area
        || geometry
            .layout_search
            .compactness_corridor_centerline_length
            != compactness.corridor_centerline_length
        || geometry.layout_search.compactness_routed_shell_cost != compactness.routed_shell_cost
        || geometry.layout_search.compactness_bend_count
            != u32::try_from(compactness.bend_count).unwrap_or(u32::MAX)
    {
        diagnostics.push(fatal(
            "geometry_compactness_evidence_invalid",
            None,
            None,
            "Geometry compactness evidence must match the selected valid layout.",
        ));
    }

    let fatal_count = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Fatal)
        .count();
    ValidationReport {
        kind: "rusty_procgen.validation.geometry_2d.v1".to_owned(),
        schema_version: 1,
        state_hash: hash_json(geometry).unwrap_or_else(|_| "hash_error".to_owned()),
        ok: fatal_count == 0,
        fatal_count,
        diagnostics,
    }
}

fn validate_geometry_content_anchors(
    geometry: &Geometry2dArtifact,
    rooms_by_id: &BTreeMap<&str, &GeometryRoom>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut content_ids = BTreeSet::new();
    for content in &geometry.contents {
        if content.id.is_empty()
            || content.kind.is_empty()
            || content.label.is_empty()
            || content.source_ref.is_empty()
        {
            diagnostics.push(fatal(
                "geometry_content_metadata_missing",
                None,
                None,
                "Content annotations must include id, kind, label, and source ref.",
            ));
        } else if !content_ids.insert(content.id.as_str()) {
            diagnostics.push(fatal(
                "geometry_content_duplicate",
                None,
                None,
                format!("Content id {} appears more than once.", content.id),
            ));
        }
        if !rooms_by_id.contains_key(content.room_id.as_str()) {
            diagnostics.push(fatal(
                "geometry_content_room_missing",
                None,
                None,
                format!(
                    "Content {} references missing room {}.",
                    content.id, content.room_id
                ),
            ));
        }
    }
}

fn validate_geometry_reachability(
    geometry: &Geometry2dArtifact,
    adjacency: &BTreeMap<&str, Vec<&str>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let start_rooms = geometry
        .rooms
        .iter()
        .filter(|room| room.role == "start")
        .map(|room| room.id.as_str())
        .collect::<Vec<_>>();
    let goal_rooms = geometry
        .rooms
        .iter()
        .filter(|room| room.role == "goal")
        .map(|room| room.id.as_str())
        .collect::<BTreeSet<_>>();
    if start_rooms.is_empty() {
        diagnostics.push(fatal(
            "geometry_start_missing",
            Some("start"),
            None,
            "Geometry must include a start room.",
        ));
    }
    if goal_rooms.is_empty() {
        diagnostics.push(fatal(
            "geometry_goal_missing",
            Some("goal"),
            None,
            "Geometry must include a goal room.",
        ));
    }
    if start_rooms.is_empty() || goal_rooms.is_empty() {
        return;
    }

    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();
    for start_room in start_rooms {
        visited.insert(start_room);
        queue.push_back(start_room);
    }
    while let Some(room_id) = queue.pop_front() {
        if let Some(next_rooms) = adjacency.get(room_id) {
            for next_room in next_rooms {
                if visited.insert(*next_room) {
                    queue.push_back(*next_room);
                }
            }
        }
    }
    if !goal_rooms.iter().any(|goal| visited.contains(goal)) {
        diagnostics.push(fatal(
            "geometry_goal_unreachable",
            Some("goal"),
            None,
            "Goal room is not reachable from start through directed corridors.",
        ));
    }
}

fn geometry_rectangles_overlap(left: &GeometryRect, right: &GeometryRect) -> bool {
    left.x < right.x + right.width
        && left.x + left.width > right.x
        && left.y < right.y + right.height
        && left.y + left.height > right.y
}

fn geometry_point_on_rect_boundary(point: &GeometryPoint, rect: &GeometryRect) -> bool {
    let on_vertical = (point.x == rect.x || point.x == rect.x + rect.width)
        && point.y >= rect.y
        && point.y <= rect.y + rect.height;
    let on_horizontal = (point.y == rect.y || point.y == rect.y + rect.height)
        && point.x >= rect.x
        && point.x <= rect.x + rect.width;
    on_vertical || on_horizontal
}

fn validate_exclusive_geometry_routes(
    geometry: &Geometry2dArtifact,
    rooms_by_id: &BTreeMap<&str, &GeometryRoom>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cells = BTreeMap::<(i32, i32), (&str, i32)>::new();
    let mut route_cells = Vec::new();
    for corridor in &geometry.corridors {
        for point in rasterize_geometry_corridor(corridor) {
            for room in &geometry.rooms {
                if room.id != corridor.from_room
                    && room.id != corridor.to_room
                    && point.x > room.rect.x
                    && point.x < room.rect.x + room.rect.width
                    && point.y > room.rect.y
                    && point.y < room.rect.y + room.rect.height
                {
                    diagnostics.push(fatal(
                        "geometry_corridor_room_intrusion",
                        room.source_nodes.first().map(String::as_str),
                        Some(corridor.id.as_str()),
                        format!("Physical section {} enters unrelated room {}.", corridor.physical_section, room.id),
                    ));
                }
            }
            if let Some((other, _)) = cells.insert(
                (point.x, point.y),
                (corridor.physical_section.as_str(), corridor.width),
            ) {
                if other != corridor.physical_section {
                    diagnostics.push(fatal(
                        "geometry_physical_section_overlap",
                        None,
                        Some(corridor.id.as_str()),
                        format!("Physical sections {} and {} overlap at {},{}.", other, corridor.physical_section, point.x, point.y),
                    ));
                }
            }
            route_cells.push(((point.x, point.y), corridor.physical_section.as_str(), corridor.width));
        }
    }
    let mut reported = BTreeSet::new();
    for (position, section, width) in route_cells {
        let max_radius = align_geometry(width / 2 + 10 + GEOMETRY_CORRIDOR_SEPARATION, GEOMETRY_ROUTE_GRID)
            / GEOMETRY_ROUTE_GRID;
        for dy in -max_radius..=max_radius {
            for dx in -max_radius..=max_radius {
                let Some((other_section, other_width)) = cells.get(&(
                    position.0 + dx * GEOMETRY_ROUTE_GRID,
                    position.1 + dy * GEOMETRY_ROUTE_GRID,
                )) else {
                    continue;
                };
                if *other_section == section {
                    continue;
                }
                let distance = (dx.abs() + dy.abs()) * GEOMETRY_ROUTE_GRID;
                let required = width / 2 + *other_width / 2 + GEOMETRY_CORRIDOR_SEPARATION;
                if distance < required {
                    let pair = sorted_pair(section, other_section);
                    if reported.insert(pair.clone()) {
                        diagnostics.push(fatal(
                            "geometry_physical_section_contact",
                            None,
                            None,
                            format!("Unrelated physical sections {} and {} violate separation.", pair.0, pair.1),
                        ));
                    }
                }
            }
        }
    }
    let _ = rooms_by_id;
}

fn rasterize_geometry_corridor(corridor: &GeometryCorridor) -> Vec<GeometryPoint> {
    let mut cells = Vec::new();
    for segment in corridor.points.windows(2) {
        let from = &segment[0];
        let to = &segment[1];
        let dx = (to.x - from.x).signum() * GEOMETRY_ROUTE_GRID;
        let dy = (to.y - from.y).signum() * GEOMETRY_ROUTE_GRID;
        let mut cursor = (from.x, from.y);
        cells.push(GeometryPoint { x: cursor.0, y: cursor.1 });
        while cursor != (to.x, to.y) {
            cursor = (cursor.0 + dx, cursor.1 + dy);
            cells.push(GeometryPoint { x: cursor.0, y: cursor.1 });
        }
    }
    dedupe_points(cells)
}

const GEOMETRY_ROUTE_GRID: i32 = 8;
const GEOMETRY_PORT_MARGIN: i32 = 32;
const GEOMETRY_PORT_SPACING: i32 = 48;
const GEOMETRY_CORRIDOR_SEPARATION: i32 = 8;
const GEOMETRY_MAX_CORRIDOR_HALF_WIDTH: i32 = 10;
const GEOMETRY_ROUTE_ORDER_COUNT: u32 = 4;
const GEOMETRY_PORT_ORDER_COUNT: u32 = 2;
const GEOMETRY_PATH_ALTERNATIVES: u32 = 8;
const GEOMETRY_ROUTE_DECISION_BUDGET: u32 = 256;
const GEOMETRY_ROUTE_BACKTRACK_BUDGET: u32 = 128;
const GEOMETRY_PATH_EXPANSION_BUDGET: u32 = 4_096;
const GEOMETRY_CONFLICT_REPAIR_BUDGET: u32 = 2;

#[derive(Clone, Debug)]
struct PhysicalPortDemand {
    section_id: String,
    side: String,
    width: i32,
    opposite_order: i32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TopologyPoint {
    x: i32,
    y: i32,
}

#[derive(Clone, Debug)]
struct TopologyEmbedding {
    positions: BTreeMap<String, TopologyPoint>,
    embedding_id: String,
    faces: u32,
    target_faces: u32,
    search_steps: u32,
    terminal_bars: bool,
}

#[derive(Debug)]
struct GeometryPlacementResult {
    rooms: Vec<GeometryRoom>,
    corridors: Vec<GeometryCorridor>,
    bounds: GeometryBounds,
    search: GeometryLayoutSearchEvidence,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct GeometryCompactnessScore {
    portal_capacity_penalty: u32,
    envelope_area: i64,
    routed_shell_cost: i64,
    corridor_centerline_length: i64,
    bend_count: usize,
    envelope_span: i32,
    embedding_id: String,
}

#[derive(Clone, Debug, Default)]
struct PhysicalRouteSearchEvidence {
    decisions: u32,
    backtracks: u32,
    path_alternatives: u32,
    repairs: u32,
    deepest_routed: usize,
    blocking_owners: BTreeSet<String>,
    budget_exhausted: Option<&'static str>,
    grid_expansions: u32,
    path_expansion_exhaustions: u32,
    last_failed_section: String,
    last_failed_ports: String,
}

#[derive(Debug, Default)]
struct PhysicalPathAlternatives {
    paths: Vec<Vec<GeometryPoint>>,
    blocking_owners: BTreeSet<String>,
    grid_expansions: u32,
    expansion_exhaustions: u32,
}

#[derive(Debug)]
struct PreparedPhysicalSectionRoutes<'a> {
    section: &'a PhysicalConnectionSection,
    paths: Vec<Vec<GeometryPoint>>,
}

#[derive(Debug)]
enum GeometryPlacementAttemptError {
    Invalid(String),
    RoutesUnavailable {
        attempted_orders: u32,
        last_error: String,
    },
}

#[derive(Debug)]
enum PhysicalRouteAttemptError {
    Invalid(String),
    Unavailable(String),
}

fn emit_geometry_2d(
    candidate: &Candidate,
    intermediate: &IntermediateBreakdown,
    connection_plan: &PhysicalConnectionPlan,
    args: &GeometryEmit2dArgs,
    seed: u64,
) -> Result<Geometry2dArtifact, String> {
    if intermediate.candidate_id != candidate.candidate_id {
        return Err(format!(
            "intermediate candidate {} does not match candidate {}",
            intermediate.candidate_id, candidate.candidate_id
        ));
    }
    if connection_plan.candidate_id != candidate.candidate_id
        || connection_plan.kind != "rusty_procgen.physical_connection_plan.v1"
    {
        return Err("physical connection plan does not match the supplied candidate".to_owned());
    }
    let layout_policy = match &args.layout_policy {
        Some(path) => read_json(path)?,
        None => default_geometry_layout_policy(),
    };
    validate_geometry_layout_policy(&layout_policy)?;
    let region_specs = ordered_geometry_region_specs(candidate, intermediate);
    let placement = place_and_route_physical_geometry(
        &region_specs,
        connection_plan,
        seed,
        &layout_policy,
    )?;
    let contents = geometry_contents(candidate, intermediate, &placement.rooms);
    Ok(Geometry2dArtifact {
        kind: "rusty_procgen.geometry_2d.v1".to_owned(),
        schema_version: 1,
        geometry_id: format!("geometry.{}.{}", candidate.candidate_id, seed),
        candidate_id: candidate.candidate_id.clone(),
        seed,
        source_candidate_ref: display_path(&args.candidate),
        source_intermediate_ref: display_path(&args.intermediate),
        source_connection_plan_ref: display_path(&args.connection_plan),
        connection_plan_id: connection_plan.plan_id.clone(),
        layout_policy,
        layout_search: placement.search,
        bounds: placement.bounds,
        rooms: placement.rooms,
        corridors: placement.corridors,
        contents,
        skipped_connectors: Vec::new(),
    })
}

fn ordered_geometry_region_specs<'a>(
    candidate: &Candidate,
    intermediate: &'a IntermediateBreakdown,
) -> Vec<(usize, String, String, &'a IntermediateRegion)> {
    let depths = graph_depths(candidate);
    let mut region_specs = intermediate
        .regions
        .iter()
        .map(|region| {
            let depth = region
                .node_ids
                .iter()
                .filter_map(|node_id| depths.get(node_id.as_str()).copied())
                .min()
                .unwrap_or(0);
            (depth, region.role.clone(), region.id.clone(), region)
        })
        .collect::<Vec<_>>();
    region_specs.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    region_specs
}

fn default_geometry_layout_policy() -> GeometryLayoutPolicy {
    GeometryLayoutPolicy {
        kind: "rusty_procgen.geometry_layout_policy.v1".to_owned(),
        schema_version: 1,
        initial_room_margin: 96,
        initial_column_gap: 144,
        initial_row_gap: 64,
        room_margin_growth: 48,
        column_gap_growth: 72,
        row_gap_growth: 40,
        max_spacing_tiers: 5,
        room_order_attempts_per_tier: 4,
        max_search_attempts: 80,
    }
}

fn validate_geometry_layout_policy(policy: &GeometryLayoutPolicy) -> Result<(), String> {
    if policy.kind != "rusty_procgen.geometry_layout_policy.v1" || policy.schema_version != 1 {
        return Err("unsupported geometry layout policy; expected rusty_procgen.geometry_layout_policy.v1".to_owned());
    }
    for (label, value, minimum, maximum) in [
        ("initialRoomMargin", policy.initial_room_margin, 32, 1_024),
        ("initialColumnGap", policy.initial_column_gap, 32, 1_024),
        ("initialRowGap", policy.initial_row_gap, 32, 1_024),
        ("roomMarginGrowth", policy.room_margin_growth, 0, 512),
        ("columnGapGrowth", policy.column_gap_growth, 0, 512),
        ("rowGapGrowth", policy.row_gap_growth, 0, 512),
    ] {
        if value < minimum || value > maximum || value % GEOMETRY_ROUTE_GRID != 0 {
            return Err(format!(
                "geometry layout policy {label} must be a multiple of {GEOMETRY_ROUTE_GRID} from {minimum} through {maximum}"
            ));
        }
    }
    if policy.max_spacing_tiers == 0 || policy.max_spacing_tiers > 8 {
        return Err("geometry layout policy maxSpacingTiers must be from 1 through 8".to_owned());
    }
    if policy.room_order_attempts_per_tier == 0 || policy.room_order_attempts_per_tier > 32 {
        return Err(
            "geometry layout policy roomOrderAttemptsPerTier must be from 1 through 32".to_owned(),
        );
    }
    let available_attempts = policy
        .max_spacing_tiers
        .saturating_mul(policy.room_order_attempts_per_tier)
        .saturating_mul(GEOMETRY_ROUTE_ORDER_COUNT);
    if policy.max_search_attempts == 0 || policy.max_search_attempts > available_attempts {
        return Err(format!(
            "geometry layout policy maxSearchAttempts must be from 1 through {available_attempts}"
        ));
    }
    let final_tier = policy.max_spacing_tiers - 1;
    for (label, initial, growth) in [
        (
            "roomMargin",
            policy.initial_room_margin,
            policy.room_margin_growth,
        ),
        (
            "columnGap",
            policy.initial_column_gap,
            policy.column_gap_growth,
        ),
        (
            "rowGap",
            policy.initial_row_gap,
            policy.row_gap_growth,
        ),
    ] {
        let final_value = initial
            .checked_add(
                growth
                    .checked_mul(i32::try_from(final_tier).unwrap_or(i32::MAX))
                    .unwrap_or(i32::MAX),
            )
            .unwrap_or(i32::MAX);
        if final_value > 2_048 {
            return Err(format!(
                "geometry layout policy {label} exceeds 2048 units at the final tier"
            ));
        }
    }
    Ok(())
}

fn geometry_spacing_for_tier(
    policy: &GeometryLayoutPolicy,
    tier: u32,
) -> Result<GeometrySpacing, String> {
    let grow = |initial: i32, per_tier: i32| {
        initial.checked_add(
            per_tier
                .checked_mul(i32::try_from(tier).unwrap_or(i32::MAX))
                .unwrap_or(i32::MAX),
        )
    };
    let spacing = GeometrySpacing {
        room_margin: grow(policy.initial_room_margin, policy.room_margin_growth)
            .ok_or_else(|| "geometry layout policy room margin overflowed".to_owned())?,
        column_gap: grow(policy.initial_column_gap, policy.column_gap_growth)
            .ok_or_else(|| "geometry layout policy column gap overflowed".to_owned())?,
        row_gap: grow(policy.initial_row_gap, policy.row_gap_growth)
            .ok_or_else(|| "geometry layout policy row gap overflowed".to_owned())?,
    };
    if spacing.room_margin > 2_048 || spacing.column_gap > 2_048 || spacing.row_gap > 2_048 {
        return Err("geometry layout policy effective spacing exceeds 2048 units".to_owned());
    }
    Ok(spacing)
}

fn geometry_compactness_score(
    bounds: &GeometryBounds,
    rooms: &[GeometryRoom],
    corridors: &[GeometryCorridor],
    embedding_id: &str,
) -> GeometryCompactnessScore {
    let corridor_centerline_length = corridors
        .iter()
        .flat_map(|corridor| corridor.points.windows(2))
        .map(|pair| {
            i64::from((pair[1].x - pair[0].x).abs())
                .saturating_add(i64::from((pair[1].y - pair[0].y).abs()))
        })
        .fold(0_i64, i64::saturating_add);
    let routed_shell_cost = corridors
        .iter()
        .map(|corridor| {
            let length = corridor
                .points
                .windows(2)
                .map(|pair| {
                    i64::from((pair[1].x - pair[0].x).abs())
                        .saturating_add(i64::from((pair[1].y - pair[0].y).abs()))
                })
                .fold(0_i64, i64::saturating_add);
            length.saturating_mul(i64::from(corridor.width.max(1)))
        })
        .fold(0_i64, i64::saturating_add);
    let bend_count = corridors
        .iter()
        .map(|corridor| corridor.points.len().saturating_sub(2))
        .sum();
    GeometryCompactnessScore {
        portal_capacity_penalty: geometry_portal_capacity_penalty(rooms),
        envelope_area: i64::from(bounds.width).saturating_mul(i64::from(bounds.height)),
        routed_shell_cost,
        corridor_centerline_length,
        bend_count,
        envelope_span: bounds.width.max(bounds.height),
        embedding_id: embedding_id.to_owned(),
    }
}

fn refresh_geometry_compactness_evidence(geometry: &mut Geometry2dArtifact) {
    let compactness = geometry_compactness_score(
        &geometry.bounds,
        &geometry.rooms,
        &geometry.corridors,
        geometry.layout_search.embedding_id.as_str(),
    );
    geometry.layout_search.compactness_portal_capacity_penalty =
        compactness.portal_capacity_penalty;
    geometry.layout_search.compactness_envelope_area = compactness.envelope_area;
    geometry
        .layout_search
        .compactness_corridor_centerline_length = compactness.corridor_centerline_length;
    geometry.layout_search.compactness_routed_shell_cost = compactness.routed_shell_cost;
    geometry.layout_search.compactness_bend_count =
        u32::try_from(compactness.bend_count).unwrap_or(u32::MAX);
}

fn geometry_portal_capacity_penalty(rooms: &[GeometryRoom]) -> u32 {
    rooms
        .iter()
        .map(|room| {
            let mut counts = BTreeMap::<&str, usize>::new();
            for port in &room.ports {
                *counts.entry(port.side.as_str()).or_default() += 1;
            }
            portal_capacity_penalty_for_counts(&counts)
        })
        .fold(0_u32, u32::saturating_add)
}

fn portal_capacity_penalty_for_counts(counts: &BTreeMap<&str, usize>) -> u32 {
    if counts.values().copied().max().unwrap_or(0) < 3 {
        return 0;
    }
    let multi_sides = counts
        .iter()
        .filter(|(_, count)| **count >= 2)
        .map(|(side, _)| *side)
        .collect::<Vec<_>>();
    let mut penalty = 0_u32;
    for (index, left) in multi_sides.iter().enumerate() {
        for right in multi_sides.iter().skip(index + 1) {
            let opposite = matches!(
                (*left, *right),
                ("north", "south")
                    | ("south", "north")
                    | ("east", "west")
                    | ("west", "east")
            );
            if !opposite {
                penalty = penalty.saturating_add(1);
            }
        }
    }
    penalty
}

fn place_and_route_physical_geometry(
    base_specs: &[(usize, String, String, &IntermediateRegion)],
    connection_plan: &PhysicalConnectionPlan,
    seed: u64,
    policy: &GeometryLayoutPolicy,
) -> Result<GeometryPlacementResult, String> {
    let mut search_attempts = 0_u32;
    let mut last_error = "no physical route order was attempted".to_owned();
    let mut last_spacing = geometry_spacing_for_tier(policy, 0)?;
    let mut spacing_tiers_attempted = 0_u32;
    for spacing_tier in 0..policy.max_spacing_tiers {
        if search_attempts >= policy.max_search_attempts {
            break;
        }
        let spacing = geometry_spacing_for_tier(policy, spacing_tier)?;
        last_spacing = spacing.clone();
        spacing_tiers_attempted += 1;
        let mut best_valid = None::<(GeometryCompactnessScore, GeometryPlacementResult)>;
        let mut valid_layout_candidates = 0_u32;
        for embedding_phase in 0..2_u32 {
            for room_order_attempt in 0..policy.room_order_attempts_per_tier {
                if search_attempts >= policy.max_search_attempts {
                    break;
                }
                if (embedding_phase == 0 && room_order_attempt != 0)
                    || (embedding_phase != 0 && room_order_attempt == 0)
                {
                    continue;
                }
                let mut specs = base_specs.to_vec();
                specs.sort_by(|left, right| {
                    left.0.cmp(&right.0).then_with(|| {
                        if room_order_attempt == 0 {
                            left.1.cmp(&right.1).then_with(|| left.2.cmp(&right.2))
                        } else {
                            geometry_layout_order_key(
                                left.3.id.as_str(),
                                seed,
                                u64::from(spacing_tier)
                                    .saturating_mul(u64::from(policy.room_order_attempts_per_tier))
                                    .saturating_add(u64::from(room_order_attempt)),
                            )
                            .cmp(&geometry_layout_order_key(
                                right.3.id.as_str(),
                                seed,
                                u64::from(spacing_tier)
                                    .saturating_mul(u64::from(policy.room_order_attempts_per_tier))
                                    .saturating_add(u64::from(room_order_attempt)),
                            ))
                            .then_with(|| left.2.cmp(&right.2))
                        }
                    })
                });
                let topology_embedding = if embedding_phase != 1 {
                    None
                } else {
                    match find_topology_embedding(
                        connection_plan,
                        seed,
                        spacing_tier,
                        room_order_attempt,
                    ) {
                        Ok(embedding) => Some(embedding),
                        Err(error) => {
                            last_error = error;
                            continue;
                        }
                    }
                };
                let remaining_attempts = policy.max_search_attempts - search_attempts;
                let route_attempt_limit = if embedding_phase == 1 {
                    remaining_attempts.min(GEOMETRY_ROUTE_ORDER_COUNT)
                } else {
                    remaining_attempts.min(GEOMETRY_PORT_ORDER_COUNT)
                };
                match place_and_route_physical_geometry_attempt(
                    &specs,
                    connection_plan,
                    &spacing,
                    seed,
                    room_order_attempt,
                    route_attempt_limit,
                    topology_embedding.as_ref(),
                ) {
                Ok((
                    rooms,
                    corridors,
                    bounds,
                    port_order_attempt,
                    route_order_attempt,
                    attempted_orders,
                    route_evidence,
                )) => {
                    search_attempts += attempted_orders;
                    valid_layout_candidates = valid_layout_candidates.saturating_add(1);
                    let embedding_kind = topology_embedding
                        .as_ref()
                        .map(|_| "planar_rotation")
                        .unwrap_or("depth_columns")
                        .to_owned();
                    let embedding_id = topology_embedding
                        .as_ref()
                        .map(|embedding| embedding.embedding_id.clone())
                        .unwrap_or_else(|| "depth-columns.v1".to_owned());
                    let score = geometry_compactness_score(
                        &bounds,
                        &rooms,
                        &corridors,
                        embedding_id.as_str(),
                    );
                    let result = GeometryPlacementResult {
                        rooms,
                        corridors,
                        bounds,
                        search: GeometryLayoutSearchEvidence {
                            spacing_tier,
                            room_order_attempt,
                            port_order_attempt,
                            route_order_attempt,
                            search_attempts,
                            effective_spacing: spacing.clone(),
                            embedding_kind,
                            embedding_id,
                            embedding_faces: topology_embedding
                                .as_ref()
                                .map(|embedding| embedding.faces)
                                .unwrap_or(0),
                            embedding_target_faces: topology_embedding
                                .as_ref()
                                .map(|embedding| embedding.target_faces)
                                .unwrap_or(0),
                            embedding_search_steps: topology_embedding
                                .as_ref()
                                .map(|embedding| embedding.search_steps)
                                .unwrap_or(0),
                            route_decisions: route_evidence.decisions,
                            route_backtracks: route_evidence.backtracks,
                            route_path_alternatives: route_evidence.path_alternatives,
                            route_repairs: route_evidence.repairs,
                            route_grid_expansions: route_evidence.grid_expansions,
                            route_path_expansion_exhaustions: route_evidence
                                .path_expansion_exhaustions,
                            route_last_failed_section: route_evidence.last_failed_section.clone(),
                            route_blocking_owners: route_evidence
                                .blocking_owners
                                .iter()
                                .cloned()
                                .collect(),
                            valid_layout_candidates: 0,
                            compactness_portal_capacity_penalty: score
                                .portal_capacity_penalty,
                            compactness_envelope_area: score.envelope_area,
                            compactness_corridor_centerline_length: score
                                .corridor_centerline_length,
                            compactness_routed_shell_cost: score.routed_shell_cost,
                            compactness_bend_count: u32::try_from(score.bend_count)
                                .unwrap_or(u32::MAX),
                        },
                    };
                    if best_valid
                        .as_ref()
                        .is_none_or(|(best_score, _)| score < *best_score)
                    {
                        best_valid = Some((score, result));
                    }
                }
                    Err(GeometryPlacementAttemptError::Invalid(error)) => {
                        return Err(format!("invalid physical geometry plan: {error}"));
                    }
                    Err(GeometryPlacementAttemptError::RoutesUnavailable {
                        attempted_orders,
                        last_error: error,
                    }) => {
                        search_attempts += attempted_orders;
                        last_error = error;
                    }
                }
            }
        }
        if let Some((_score, mut result)) = best_valid {
            result.search.search_attempts = search_attempts;
            result.search.valid_layout_candidates = valid_layout_candidates;
            return Ok(result);
        }
    }
    Err(format!(
        "geometry search exhausted after {search_attempts} route attempt(s) across {} spacing tier(s), {} room order(s) per tier, {GEOMETRY_PORT_ORDER_COUNT} port allocation(s) per room order, and up to {} route order(s) per port allocation; initial spacing margin/column/row={}/{}/{}, final spacing={}/{}/{}; last route failure: {last_error}",
        spacing_tiers_attempted,
        policy.room_order_attempts_per_tier,
        GEOMETRY_ROUTE_ORDER_COUNT / GEOMETRY_PORT_ORDER_COUNT,
        policy.initial_room_margin,
        policy.initial_column_gap,
        policy.initial_row_gap,
        last_spacing.room_margin,
        last_spacing.column_gap,
        last_spacing.row_gap,
    ))
}

fn geometry_layout_order_key(id: &str, seed: u64, attempt: u64) -> u64 {
    let mut value = seed ^ attempt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for byte in id.bytes() {
        value ^= u64::from(byte);
        value = value.wrapping_mul(0x100_0000_01B3);
        value ^= value >> 29;
    }
    value
}

#[derive(Debug)]
struct TopologyRng {
    state: u64,
}

impl TopologyRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0xA076_1D64_78BD_642F,
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.state;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.state = value;
        value.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn index(&mut self, length: usize) -> usize {
        if length <= 1 {
            0
        } else {
            (self.next_u64() % length as u64) as usize
        }
    }

    fn shuffle<T>(&mut self, values: &mut [T]) {
        for index in (1..values.len()).rev() {
            let other = self.index(index + 1);
            values.swap(index, other);
        }
    }
}

fn find_topology_embedding(
    plan: &PhysicalConnectionPlan,
    seed: u64,
    spacing_tier: u32,
    room_order_attempt: u32,
) -> Result<TopologyEmbedding, String> {
    let mut rotation = BTreeMap::<String, Vec<String>>::new();
    let mut undirected_edges = BTreeSet::new();
    for section in &plan.sections {
        if section.terminal_regions.len() != 2 {
            return Err(format!(
                "topology embedding rejected section {} with {} terminal regions",
                section.id,
                section.terminal_regions.len()
            ));
        }
        let left = section.terminal_regions[0].clone();
        let right = section.terminal_regions[1].clone();
        if left == right {
            return Err(format!(
                "topology embedding rejected self-loop section {}",
                section.id
            ));
        }
        let edge = if left < right {
            (left.clone(), right.clone())
        } else {
            (right.clone(), left.clone())
        };
        if !undirected_edges.insert(edge) {
            return Err(format!(
                "topology embedding rejected duplicate physical terminals on {}",
                section.id
            ));
        }
        rotation.entry(left.clone()).or_default().push(right.clone());
        rotation.entry(right).or_default().push(left);
    }
    for neighbors in rotation.values_mut() {
        neighbors.sort();
    }
    if rotation.is_empty() {
        return Err("topology embedding rejected an empty physical graph".to_owned());
    }
    let vertex_count = u32::try_from(rotation.len()).unwrap_or(u32::MAX);
    let edge_count = u32::try_from(undirected_edges.len()).unwrap_or(u32::MAX);
    if vertex_count >= 3 && edge_count > vertex_count.saturating_mul(3).saturating_sub(6) {
        return Err(format!(
            "topology embedding necessary-condition failed: {edge_count} edges exceed 3V-6 for {vertex_count} regions"
        ));
    }
    let reachable = topology_reachable_regions(
        rotation.keys().next().expect("non-empty rotation"),
        &rotation,
    );
    if reachable.len() != rotation.len() {
        return Err(format!(
            "topology embedding rejected a disconnected physical graph: reached {} of {} regions",
            reachable.len(),
            rotation.len()
        ));
    }
    let target_faces = edge_count.saturating_sub(vertex_count).saturating_add(2);
    let nonce = u64::from(spacing_tier)
        .saturating_mul(1_000_003)
        .saturating_add(u64::from(room_order_attempt));
    let mut rng = TopologyRng::new(seed ^ nonce.rotate_left(17));
    for neighbors in rotation.values_mut() {
        rng.shuffle(neighbors);
    }
    let eligible = rotation
        .iter()
        .filter(|(_, neighbors)| neighbors.len() >= 3)
        .map(|(region, _)| region.clone())
        .collect::<Vec<_>>();
    let mut faces = topology_embedding_faces(&rotation)
        .map_err(|error| format!("topology embedding rotation invalid: {error}"))?;
    let max_steps = 4_096_u32;
    let mut search_steps = 0_u32;
    while faces.len() < target_faces as usize && search_steps < max_steps && !eligible.is_empty() {
        search_steps += 1;
        let region = &eligible[rng.index(eligible.len())];
        let (left, right) = {
            let neighbors = rotation
                .get_mut(region)
                .expect("eligible topology region should exist");
            let left = rng.index(neighbors.len());
            let mut right = rng.index(neighbors.len());
            if left == right {
                right = (right + 1) % neighbors.len();
            }
            neighbors.swap(left, right);
            (left, right)
        };
        let candidate_faces = topology_embedding_faces(&rotation)
            .map_err(|error| format!("topology embedding rotation invalid: {error}"))?;
        if candidate_faces.len() >= faces.len() {
            faces = candidate_faces;
        } else {
            rotation
                .get_mut(region)
                .expect("eligible topology region should exist")
                .swap(left, right);
        }
    }
    if faces.len() != target_faces as usize {
        return Err(format!(
            "topology embedding search exhausted after {search_steps}/{max_steps} rotation decision(s); best witness has {} face(s), Euler target is {target_faces}; spacing tier {spacing_tier}, room embedding attempt {room_order_attempt}",
            faces.len()
        ));
    }
    let embedding_id = format!(
        "rotation.v1.{}",
        hash_json(&rotation)
            .map_err(|error| format!("topology embedding witness hash failed: {error}"))?
    );
    let (raw_positions, terminal_bars) = topology_embedding_positions(
        &rotation,
        &faces,
        seed ^ nonce,
    )
    .map_err(|error| format!("topology embedding {embedding_id} drawing failed: {error}"))?;
    let positions = improve_topology_drawing_clearance(
        &rotation,
        if terminal_bars {
            orient_topology_terminals(raw_positions)
        } else {
            raw_positions
        },
        seed ^ nonce ^ 0x84B9_3F2D_571A_CE60,
    );
    validate_topology_drawing(&rotation, &positions)?;
    Ok(TopologyEmbedding {
        positions,
        embedding_id,
        faces: target_faces,
        target_faces,
        search_steps,
        terminal_bars,
    })
}

fn orient_topology_terminals(
    positions: BTreeMap<String, TopologyPoint>,
) -> BTreeMap<String, TopologyPoint> {
    let Some(start) = positions.get("region.start").copied() else {
        return positions;
    };
    let Some(goal) = positions.get("region.goal").copied() else {
        return positions;
    };
    let dx = f64::from(goal.x - start.x);
    let dy = f64::from(goal.y - start.y);
    let length = dx.hypot(dy);
    if length <= f64::EPSILON {
        return positions;
    }
    let cosine = dx / length;
    let sine = dy / length;
    let mut oriented = positions
        .into_iter()
        .map(|(region, point)| {
            let relative_x = f64::from(point.x - start.x);
            let relative_y = f64::from(point.y - start.y);
            (
                region,
                TopologyPoint {
                    x: (relative_x * cosine + relative_y * sine).round() as i32,
                    y: (-relative_x * sine + relative_y * cosine).round() as i32,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let nonterminal_x = oriented
        .iter()
        .filter(|(region, _)| {
            region.as_str() != "region.start" && region.as_str() != "region.goal"
        })
        .map(|(_, point)| point.x)
        .collect::<Vec<_>>();
    let minimum = nonterminal_x.iter().min().copied();
    let maximum = nonterminal_x.iter().max().copied();
    if let (Some(minimum), Some(maximum)) = (minimum, maximum) {
        let current_start = oriented["region.start"].x;
        let current_goal = oriented["region.goal"].x;
        if minimum <= current_start || maximum >= current_goal {
            let padding = (maximum - minimum).abs().saturating_div(8).max(1);
            if let Some(start) = oriented.get_mut("region.start") {
                start.x = minimum.saturating_sub(padding);
                start.y = 0;
            }
            if let Some(goal) = oriented.get_mut("region.goal") {
                goal.x = maximum.saturating_add(padding);
                goal.y = 0;
            }
        }
    }
    oriented
}

fn topology_reachable_regions(
    start: &str,
    rotation: &BTreeMap<String, Vec<String>>,
) -> BTreeSet<String> {
    let mut reachable = BTreeSet::from([start.to_owned()]);
    let mut queue = VecDeque::from([start.to_owned()]);
    while let Some(region) = queue.pop_front() {
        for neighbor in rotation.get(region.as_str()).into_iter().flatten() {
            if reachable.insert(neighbor.clone()) {
                queue.push_back(neighbor.clone());
            }
        }
    }
    reachable
}

fn topology_embedding_faces(
    rotation: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<Vec<String>>, String> {
    let half_edge_count = rotation.values().map(Vec::len).sum::<usize>();
    let mut visited = BTreeSet::<(String, String)>::new();
    let mut faces = Vec::new();
    for (from, neighbors) in rotation {
        for to in neighbors {
            if visited.contains(&(from.clone(), to.clone())) {
                continue;
            }
            let start = (from.clone(), to.clone());
            let mut edge = start.clone();
            let mut face = Vec::new();
            for _ in 0..=half_edge_count {
                if !visited.insert(edge.clone()) {
                    if edge == start {
                        break;
                    }
                    return Err(format!(
                        "half-edge {} -> {} entered an existing face",
                        edge.0, edge.1
                    ));
                }
                face.push(edge.0.clone());
                let at = edge.1.clone();
                let around = rotation
                    .get(at.as_str())
                    .ok_or_else(|| format!("rotation lacks region {at}"))?;
                let incoming = around
                    .iter()
                    .position(|neighbor| neighbor == &edge.0)
                    .ok_or_else(|| format!("rotation lacks reciprocal edge {} -> {at}", edge.0))?;
                let next = around[(incoming + around.len() - 1) % around.len()].clone();
                edge = (at, next);
                if edge == start {
                    break;
                }
            }
            if edge != start {
                return Err(format!(
                    "face traversal from {} -> {} exceeded {half_edge_count} half-edges",
                    start.0, start.1
                ));
            }
            faces.push(face);
        }
    }
    if visited.len() != half_edge_count {
        return Err(format!(
            "face traversal visited {} of {half_edge_count} half-edges",
            visited.len()
        ));
    }
    Ok(faces)
}

fn topology_embedding_positions(
    rotation: &BTreeMap<String, Vec<String>>,
    faces: &[Vec<String>],
    seed: u64,
) -> Result<(BTreeMap<String, TopologyPoint>, bool), String> {
    let mut candidates = Vec::<(BTreeMap<String, TopologyPoint>, bool)>::new();
    if let Some(separator_positions) = topology_separator_band_positions(rotation) {
        if validate_topology_drawing(rotation, &separator_positions).is_ok() {
            candidates.push((separator_positions, true));
        }
    }
    let mut active = rotation.keys().cloned().collect::<BTreeSet<_>>();
    let mut removed = Vec::<(String, String)>::new();
    loop {
        let leaves = active
            .iter()
            .filter_map(|region| {
                let neighbors = rotation
                    .get(region.as_str())
                    .into_iter()
                    .flatten()
                    .filter(|neighbor| active.contains(*neighbor))
                    .cloned()
                    .collect::<Vec<_>>();
                (neighbors.len() <= 1).then(|| {
                    (
                        region.clone(),
                        neighbors.first().cloned().unwrap_or_default(),
                    )
                })
            })
            .collect::<Vec<_>>();
        if leaves.is_empty() || leaves.len() == active.len() {
            break;
        }
        for (region, parent) in leaves {
            active.remove(region.as_str());
            removed.push((region, parent));
        }
    }
    if active.len() < 3 {
        return Err(format!(
            "topology embedding position search requires a cyclic core; found {} core region(s)",
            active.len()
        ));
    }
    let core_rotation = active
        .iter()
        .map(|region| {
            (
                region.clone(),
                rotation
                    .get(region.as_str())
                    .expect("active region should have rotation")
                    .iter()
                    .filter(|neighbor| active.contains(*neighbor))
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let drawing_rotation = core_rotation.clone();
    let mut core_faces = topology_embedding_faces(&drawing_rotation)?;
    core_faces.sort_by(|left, right| {
        let left_unique = left.iter().collect::<BTreeSet<_>>().len();
        let right_unique = right.iter().collect::<BTreeSet<_>>().len();
        right_unique
            .cmp(&left_unique)
            .then_with(|| left.join("\u{0}").cmp(&right.join("\u{0}")))
    });
    if !core_faces.is_empty() {
        let face_offset = seed as usize % core_faces.len();
        core_faces.rotate_left(face_offset);
    }
    let mut last_error = "no core face was available".to_owned();
    for (face_index, outer_face) in core_faces.into_iter().enumerate() {
        let mut outer = Vec::new();
        for region in outer_face {
            if !outer.contains(&region) {
                outer.push(region);
            }
        }
        if outer.len() < 3 {
            continue;
        }
        match harmonic_topology_positions(&drawing_rotation, &outer) {
            Ok(mut positions) => {
                attach_topology_leaves(&mut positions, &removed, seed);
                let quantized = improve_topology_drawing_clearance(
                    rotation,
                    quantize_topology_positions(&positions),
                    seed ^ face_index as u64,
                );
                match validate_topology_drawing(rotation, &quantized) {
                    Ok(()) => candidates.push((quantized, false)),
                    Err(error) => last_error = error,
                }
            }
            Err(error) => last_error = error,
        }
    }
    candidates.sort_by(|(left, left_terminal_bars), (right, right_terminal_bars)| {
        let scored_left = if *left_terminal_bars {
            orient_topology_terminals(left.clone())
        } else {
            left.clone()
        };
        let scored_right = if *right_terminal_bars {
            orient_topology_terminals(right.clone())
        } else {
            right.clone()
        };
        let left_ratio = topology_drawing_clearance(rotation, &scored_left).unwrap_or(0.0)
            / topology_drawing_span(&scored_left);
        let right_ratio = topology_drawing_clearance(rotation, &scored_right).unwrap_or(0.0)
            / topology_drawing_span(&scored_right);
        topology_portal_capacity_penalty(rotation, &scored_left, *left_terminal_bars)
            .cmp(&topology_portal_capacity_penalty(
                rotation,
                &scored_right,
                *right_terminal_bars,
            ))
            .then_with(|| {
                right_ratio
            .total_cmp(&left_ratio)
            })
            .then_with(|| {
                topology_drawing_span(&scored_left)
                    .total_cmp(&topology_drawing_span(&scored_right))
            })
            .then_with(|| left.cmp(right))
            .then_with(|| left_terminal_bars.cmp(right_terminal_bars))
    });
    candidates.into_iter().next().ok_or_else(|| {
        format!(
            "topology embedding drawing search exhausted across {} certified face(s): {last_error}",
            faces.len()
        )
    })
}

fn topology_separator_band_positions(
    rotation: &BTreeMap<String, Vec<String>>,
) -> Option<BTreeMap<String, TopologyPoint>> {
    const BAND_STEP: i32 = 1_000;
    const BAND_HALF_STEP: i32 = BAND_STEP / 2;
    const IN_BAND_STEP: i32 = 75;
    const JITTER_RADIUS: i32 = 400;
    let left_terminal = "region.start";
    let right_terminal = "region.goal";
    if !rotation.contains_key(left_terminal) || !rotation.contains_key(right_terminal) {
        return None;
    }
    let terminals = BTreeSet::from([left_terminal.to_owned(), right_terminal.to_owned()]);
    let mut remaining = rotation
        .keys()
        .filter(|region| !terminals.contains(*region))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut components = Vec::<Vec<String>>::new();
    while let Some(root) = remaining.iter().next().cloned() {
        remaining.remove(root.as_str());
        let mut component = BTreeSet::from([root.clone()]);
        let mut queue = VecDeque::from([root]);
        while let Some(region) = queue.pop_front() {
            for neighbor in rotation.get(region.as_str()).into_iter().flatten() {
                if terminals.contains(neighbor) || !remaining.remove(neighbor.as_str()) {
                    continue;
                }
                component.insert(neighbor.clone());
                queue.push_back(neighbor.clone());
            }
        }
        components.push(component.into_iter().collect());
    }
    if components.len() < 2 {
        return None;
    }
    components.sort();
    let left_distances = topology_graph_distances(left_terminal, rotation);
    let right_distances = topology_graph_distances(right_terminal, rotation);
    let terminal_y = i32::try_from(components.len().saturating_sub(1))
        .ok()?
        .saturating_mul(BAND_HALF_STEP);
    let mut positions = BTreeMap::from([
        (
            left_terminal.to_owned(),
            TopologyPoint {
                x: 0,
                y: terminal_y,
            },
        ),
        (
            right_terminal.to_owned(),
            TopologyPoint {
                x: 1_000,
                y: terminal_y,
            },
        ),
    ]);
    for (component_index, component) in components.iter_mut().enumerate() {
        component.sort_by(|left, right| {
            let left_from = *left_distances.get(left.as_str()).unwrap_or(&usize::MAX);
            let left_to = *right_distances.get(left.as_str()).unwrap_or(&usize::MAX);
            let right_from = *left_distances.get(right.as_str()).unwrap_or(&usize::MAX);
            let right_to = *right_distances.get(right.as_str()).unwrap_or(&usize::MAX);
            left_from
                .saturating_mul(right_from.saturating_add(right_to))
                .cmp(&right_from.saturating_mul(left_from.saturating_add(left_to)))
                .then_with(|| left.cmp(right))
        });
        for (order, region) in component.iter().enumerate() {
            let from = *left_distances.get(region.as_str())?;
            let to = *right_distances.get(region.as_str())?;
            let total = from.saturating_add(to).max(1);
            let x = i32::try_from(from.saturating_mul(1_000) / total).ok()?;
            let centered_order = i32::try_from(order.saturating_mul(2)).ok()?
                - i32::try_from(component.len().saturating_sub(1)).ok()?;
            let y = i32::try_from(component_index)
                .ok()?
                .saturating_mul(BAND_STEP)
                .saturating_add(centered_order.saturating_mul(IN_BAND_STEP));
            positions.insert(region.clone(), TopologyPoint { x, y });
        }
    }
    if validate_topology_drawing(rotation, &positions).is_ok() {
        return Some(positions);
    }
    let mut rng = TopologyRng::new(geometry_layout_order_key(
        rotation.keys().map(String::as_str).collect::<Vec<_>>().join("|").as_str(),
        rotation.values().map(Vec::len).sum::<usize>() as u64,
        0,
    ));
    for _ in 0..4_096 {
        let mut candidate = positions.clone();
        for (component_index, component) in components.iter().enumerate() {
            let band_y = i32::try_from(component_index)
                .ok()?
                .saturating_mul(BAND_STEP);
            for region in component {
                let point = candidate.get_mut(region.as_str())?;
                let jitter = i32::try_from(rng.index(
                    usize::try_from(JITTER_RADIUS.saturating_mul(2).saturating_add(1)).ok()?,
                ))
                .ok()?
                .saturating_sub(JITTER_RADIUS);
                point.y = band_y.saturating_add(jitter);
            }
        }
        if validate_topology_drawing(rotation, &candidate).is_ok() {
            return Some(candidate);
        }
    }
    None
}

fn topology_graph_distances(
    start: &str,
    rotation: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, usize> {
    let mut distances = BTreeMap::from([(start.to_owned(), 0_usize)]);
    let mut queue = VecDeque::from([start.to_owned()]);
    while let Some(region) = queue.pop_front() {
        let next_distance = distances[region.as_str()].saturating_add(1);
        for neighbor in rotation.get(region.as_str()).into_iter().flatten() {
            if distances.contains_key(neighbor.as_str()) {
                continue;
            }
            distances.insert(neighbor.clone(), next_distance);
            queue.push_back(neighbor.clone());
        }
    }
    distances
}

fn harmonic_topology_positions(
    rotation: &BTreeMap<String, Vec<String>>,
    outer: &[String],
) -> Result<BTreeMap<String, (f64, f64)>, String> {
    let mut positions = BTreeMap::new();
    for (index, region) in outer.iter().enumerate() {
        let angle = std::f64::consts::TAU * index as f64 / outer.len() as f64;
        positions.insert(region.clone(), (angle.cos(), angle.sin()));
    }
    let inner = rotation
        .keys()
        .filter(|region| !positions.contains_key(*region))
        .cloned()
        .collect::<Vec<_>>();
    if inner.is_empty() {
        return Ok(positions);
    }
    let indices = inner
        .iter()
        .enumerate()
        .map(|(index, region)| (region.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut matrix = vec![vec![0.0; inner.len()]; inner.len()];
    let mut x_values = vec![0.0; inner.len()];
    let mut y_values = vec![0.0; inner.len()];
    for (row, region) in inner.iter().enumerate() {
        let neighbors = rotation
            .get(region.as_str())
            .ok_or_else(|| format!("harmonic embedding lacks region {region}"))?;
        matrix[row][row] = neighbors.len() as f64;
        for neighbor in neighbors {
            if let Some(column) = indices.get(neighbor.as_str()) {
                matrix[row][*column] -= 1.0;
            } else if let Some((x, y)) = positions.get(neighbor.as_str()) {
                x_values[row] += x;
                y_values[row] += y;
            } else {
                return Err(format!(
                    "harmonic embedding lacks neighbor position for {neighbor}"
                ));
            }
        }
    }
    let solved_x = solve_topology_linear_system(matrix.clone(), x_values)?;
    let solved_y = solve_topology_linear_system(matrix, y_values)?;
    for (index, region) in inner.into_iter().enumerate() {
        positions.insert(region, (solved_x[index], solved_y[index]));
    }
    Ok(positions)
}

fn solve_topology_linear_system(
    mut matrix: Vec<Vec<f64>>,
    mut values: Vec<f64>,
) -> Result<Vec<f64>, String> {
    for pivot in 0..matrix.len() {
        let best = (pivot..matrix.len())
            .max_by(|left, right| {
                matrix[*left][pivot]
                    .abs()
                    .partial_cmp(&matrix[*right][pivot].abs())
                    .unwrap_or(Ordering::Equal)
            })
            .expect("pivot range should not be empty");
        if matrix[best][pivot].abs() < 1.0e-9 {
            return Err(format!(
                "harmonic embedding matrix is singular at pivot {pivot}"
            ));
        }
        matrix.swap(pivot, best);
        values.swap(pivot, best);
        let divisor = matrix[pivot][pivot];
        for column in pivot..matrix.len() {
            matrix[pivot][column] /= divisor;
        }
        values[pivot] /= divisor;
        for row in 0..matrix.len() {
            if row == pivot {
                continue;
            }
            let factor = matrix[row][pivot];
            if factor.abs() < 1.0e-12 {
                continue;
            }
            for column in pivot..matrix.len() {
                matrix[row][column] -= factor * matrix[pivot][column];
            }
            values[row] -= factor * values[pivot];
        }
    }
    Ok(values)
}

fn attach_topology_leaves(
    positions: &mut BTreeMap<String, (f64, f64)>,
    removed: &[(String, String)],
    seed: u64,
) {
    let mut attached_by_parent = BTreeMap::<String, usize>::new();
    for (region, parent) in removed.iter().rev() {
        let Some((parent_x, parent_y)) = positions.get(parent.as_str()).copied() else {
            continue;
        };
        let (sum_x, sum_y) = positions
            .values()
            .fold((0.0, 0.0), |(sum_x, sum_y), (x, y)| {
                (sum_x + x, sum_y + y)
            });
        let count = positions.len().max(1) as f64;
        let mut angle = (parent_y - sum_y / count).atan2(parent_x - sum_x / count);
        if !angle.is_finite()
            || ((parent_x - sum_x / count).abs() + (parent_y - sum_y / count).abs()) < 1.0e-6
        {
            angle = (geometry_layout_order_key(region, seed, 0) % 6_283) as f64 / 1_000.0;
        }
        let attached = attached_by_parent.entry(parent.clone()).or_insert(0);
        let offset_index = *attached as f64;
        *attached += 1;
        angle += (offset_index - 0.5) * 0.55;
        let distance = 0.7 + offset_index * 0.18;
        positions.insert(
            region.clone(),
            (
                parent_x + angle.cos() * distance,
                parent_y + angle.sin() * distance,
            ),
        );
    }
}

fn quantize_topology_positions(
    positions: &BTreeMap<String, (f64, f64)>,
) -> BTreeMap<String, TopologyPoint> {
    positions
        .iter()
        .map(|(region, (x, y))| {
            (
                region.clone(),
                TopologyPoint {
                    x: (x * 1_000_000.0).round() as i32,
                    y: (y * 1_000_000.0).round() as i32,
                },
            )
        })
        .collect()
}

fn validate_topology_drawing(
    rotation: &BTreeMap<String, Vec<String>>,
    positions: &BTreeMap<String, TopologyPoint>,
) -> Result<(), String> {
    validate_topology_drawing_combinatorial(rotation, positions)?;
    let minimum_clearance = topology_drawing_clearance(rotation, positions)?;
    let required_clearance = topology_drawing_span(positions) / 32.0;
    if minimum_clearance < required_clearance {
        return Err(format!(
            "topology drawing feature clearance {minimum_clearance:.3} is below required {required_clearance:.3}"
        ));
    }
    Ok(())
}

fn validate_topology_drawing_combinatorial(
    rotation: &BTreeMap<String, Vec<String>>,
    positions: &BTreeMap<String, TopologyPoint>,
) -> Result<(), String> {
    let mut occupied = BTreeMap::<(i32, i32), &str>::new();
    for (region, point) in positions {
        if let Some(other) = occupied.insert((point.x, point.y), region.as_str()) {
            return Err(format!(
                "topology drawing overlaps regions {other} and {region}"
            ));
        }
    }
    let edges = rotation
        .iter()
        .flat_map(|(left, neighbors)| {
            neighbors
                .iter()
                .filter(move |right| left < *right)
                .map(move |right| (left.as_str(), right.as_str()))
        })
        .collect::<Vec<_>>();
    for (index, (left, right)) in edges.iter().enumerate() {
        let left_point = positions
            .get(*left)
            .ok_or_else(|| format!("topology drawing lacks region {left}"))?;
        let right_point = positions
            .get(*right)
            .ok_or_else(|| format!("topology drawing lacks region {right}"))?;
        for (other_left, other_right) in edges.iter().skip(index + 1) {
            if left == other_left
                || left == other_right
                || right == other_left
                || right == other_right
            {
                continue;
            }
            let other_left_point = positions
                .get(*other_left)
                .expect("topology edge endpoint should have a position");
            let other_right_point = positions
                .get(*other_right)
                .expect("topology edge endpoint should have a position");
            if topology_segments_cross(
                left_point,
                right_point,
                other_left_point,
                other_right_point,
            ) {
                return Err(format!(
                    "topology drawing crosses {left}--{right} with {other_left}--{other_right}"
                ));
            }
        }
        for (region, point) in positions {
            if region == left || region == right {
                continue;
            }
            if topology_point_on_segment(point, left_point, right_point) {
                return Err(format!(
                    "topology drawing places region {region} on edge {left}--{right}"
                ));
            }
        }
    }
    Ok(())
}

fn topology_drawing_span(positions: &BTreeMap<String, TopologyPoint>) -> f64 {
    let minimum_x = positions.values().map(|point| point.x).min().unwrap_or(0);
    let maximum_x = positions.values().map(|point| point.x).max().unwrap_or(0);
    let minimum_y = positions.values().map(|point| point.y).min().unwrap_or(0);
    let maximum_y = positions.values().map(|point| point.y).max().unwrap_or(0);
    f64::from((maximum_x - minimum_x).max(maximum_y - minimum_y).max(1))
}

fn topology_portal_capacity_penalty(
    rotation: &BTreeMap<String, Vec<String>>,
    positions: &BTreeMap<String, TopologyPoint>,
    terminal_bars: bool,
) -> u32 {
    rotation
        .iter()
        .map(|(region, neighbors)| {
            if terminal_bars && matches!(region.as_str(), "region.start" | "region.goal") {
                return 0;
            }
            let Some(point) = positions.get(region) else {
                return u32::MAX;
            };
            let mut counts = BTreeMap::<&str, usize>::new();
            for neighbor in neighbors {
                let Some(other) = positions.get(neighbor) else {
                    return u32::MAX;
                };
                let dx = other.x - point.x;
                let dy = other.y - point.y;
                let side = if dx.abs() >= dy.abs() {
                    if dx >= 0 { "east" } else { "west" }
                } else if dy >= 0 {
                    "south"
                } else {
                    "north"
                };
                *counts.entry(side).or_default() += 1;
            }
            portal_capacity_penalty_for_counts(&counts)
        })
        .fold(0_u32, u32::saturating_add)
}

fn improve_topology_drawing_clearance(
    rotation: &BTreeMap<String, Vec<String>>,
    positions: BTreeMap<String, TopologyPoint>,
    seed: u64,
) -> BTreeMap<String, TopologyPoint> {
    if validate_topology_drawing_combinatorial(rotation, &positions).is_err() {
        return positions;
    }
    const TARGET_CLEARANCE_RATIO: f64 = 1.0 / 16.0;
    let initial_ratio = topology_drawing_clearance(rotation, &positions).unwrap_or(0.0)
        / topology_drawing_span(&positions);
    if initial_ratio >= TARGET_CLEARANCE_RATIO {
        return positions;
    }
    let regions = positions
        .keys()
        .filter(|region| {
            region.as_str() != "region.start" && region.as_str() != "region.goal"
        })
        .cloned()
        .collect::<Vec<_>>();
    if regions.is_empty() {
        return positions;
    }
    let mut rng = TopologyRng::new(seed ^ 0xD1B5_4A32_D192_ED03);
    let mut best = positions;
    let mut best_ratio = topology_drawing_clearance(rotation, &best).unwrap_or(0.0)
        / topology_drawing_span(&best);
    for attempt in 0..32_768_u32 {
        let mut candidate = best.clone();
        let region = &regions[rng.index(regions.len())];
        let span = topology_drawing_span(&best).round().max(1.0) as i32;
        let divisor = 6_i32.saturating_add(i32::try_from(attempt / 2_048).unwrap_or(i32::MAX));
        let radius = (span / divisor).max(span / 128).max(1);
        let offset_span = usize::try_from(radius.saturating_mul(2).saturating_add(1))
            .unwrap_or(usize::MAX);
        let dx = i32::try_from(rng.index(offset_span)).unwrap_or(i32::MAX) - radius;
        let dy = i32::try_from(rng.index(offset_span)).unwrap_or(i32::MAX) - radius;
        let point = candidate
            .get_mut(region.as_str())
            .expect("clearance search region should exist");
        point.x = point.x.saturating_add(dx);
        point.y = point.y.saturating_add(dy);
        if validate_topology_drawing_combinatorial(rotation, &candidate).is_err() {
            continue;
        }
        let ratio = topology_drawing_clearance(rotation, &candidate).unwrap_or(0.0)
            / topology_drawing_span(&candidate);
        if ratio <= best_ratio {
            continue;
        }
        best = candidate;
        best_ratio = ratio;
        if best_ratio >= TARGET_CLEARANCE_RATIO {
            break;
        }
    }
    best
}

fn topology_drawing_clearance(
    rotation: &BTreeMap<String, Vec<String>>,
    positions: &BTreeMap<String, TopologyPoint>,
) -> Result<f64, String> {
    let edges = rotation
        .iter()
        .flat_map(|(left, neighbors)| {
            neighbors
                .iter()
                .filter(move |right| left < *right)
                .map(move |right| (left.as_str(), right.as_str()))
        })
        .collect::<Vec<_>>();
    let mut minimum = f64::INFINITY;
    for (index, (region, point)) in positions.iter().enumerate() {
        for other in positions.values().skip(index + 1) {
            let dx = f64::from(point.x - other.x);
            let dy = f64::from(point.y - other.y);
            minimum = minimum.min(dx.hypot(dy));
        }
        for (left, right) in &edges {
            if region == left || region == right {
                continue;
            }
            let left_point = positions
                .get(*left)
                .ok_or_else(|| format!("topology drawing lacks region {left}"))?;
            let right_point = positions
                .get(*right)
                .ok_or_else(|| format!("topology drawing lacks region {right}"))?;
            minimum = minimum.min(topology_point_segment_distance(
                point,
                left_point,
                right_point,
            ));
        }
    }
    if minimum.is_finite() {
        Ok(minimum)
    } else {
        Ok(1.0)
    }
}

fn topology_point_segment_distance(
    point: &TopologyPoint,
    left: &TopologyPoint,
    right: &TopologyPoint,
) -> f64 {
    let segment_x = f64::from(right.x - left.x);
    let segment_y = f64::from(right.y - left.y);
    let length_squared = segment_x * segment_x + segment_y * segment_y;
    if length_squared <= f64::EPSILON {
        return f64::from(point.x - left.x).hypot(f64::from(point.y - left.y));
    }
    let relative_x = f64::from(point.x - left.x);
    let relative_y = f64::from(point.y - left.y);
    let projection =
        ((relative_x * segment_x + relative_y * segment_y) / length_squared).clamp(0.0, 1.0);
    let closest_x = f64::from(left.x) + projection * segment_x;
    let closest_y = f64::from(left.y) + projection * segment_y;
    (f64::from(point.x) - closest_x).hypot(f64::from(point.y) - closest_y)
}

fn topology_orientation(
    left: &TopologyPoint,
    middle: &TopologyPoint,
    right: &TopologyPoint,
) -> i64 {
    i64::from(middle.x - left.x) * i64::from(right.y - left.y)
        - i64::from(middle.y - left.y) * i64::from(right.x - left.x)
}

fn topology_segments_cross(
    left: &TopologyPoint,
    right: &TopologyPoint,
    other_left: &TopologyPoint,
    other_right: &TopologyPoint,
) -> bool {
    let first_left = topology_orientation(left, right, other_left);
    let first_right = topology_orientation(left, right, other_right);
    let second_left = topology_orientation(other_left, other_right, left);
    let second_right = topology_orientation(other_left, other_right, right);
    (first_left > 0 && first_right < 0 || first_left < 0 && first_right > 0)
        && (second_left > 0 && second_right < 0
            || second_left < 0 && second_right > 0)
}

fn topology_point_on_segment(
    point: &TopologyPoint,
    left: &TopologyPoint,
    right: &TopologyPoint,
) -> bool {
    topology_orientation(left, right, point) == 0
        && point.x >= left.x.min(right.x)
        && point.x <= left.x.max(right.x)
        && point.y >= left.y.min(right.y)
        && point.y <= left.y.max(right.y)
}

fn place_and_route_physical_geometry_attempt(
    region_specs: &[(usize, String, String, &IntermediateRegion)],
    connection_plan: &PhysicalConnectionPlan,
    spacing: &GeometrySpacing,
    seed: u64,
    room_order_attempt: u32,
    max_route_attempts: u32,
    topology_embedding: Option<&TopologyEmbedding>,
) -> Result<
    (
        Vec<GeometryRoom>,
        Vec<GeometryCorridor>,
        GeometryBounds,
        u32,
        u32,
        u32,
        PhysicalRouteSearchEvidence,
    ),
    GeometryPlacementAttemptError,
> {
    let region_depths = region_specs
        .iter()
        .map(|(depth, _, _, region)| (region.id.as_str(), *depth))
        .collect::<BTreeMap<_, _>>();
    let mut next_order_by_depth = BTreeMap::new();
    let mut region_orders = BTreeMap::new();
    for (depth, _, _, region) in region_specs {
        let order = next_order_by_depth.entry(*depth).or_insert(0_usize);
        region_orders.insert(region.id.as_str(), *order);
        *order += 1;
    }
    let canonical_attempts = max_route_attempts.min(2);
    let port_attempts = [
        (0_u32, canonical_attempts),
        (1_u32, max_route_attempts.saturating_sub(canonical_attempts)),
    ];
    let mut attempted_orders = 0_u32;
    let mut last_error = "no physical route order was attempted".to_owned();
    for (port_order_attempt, route_attempt_limit) in port_attempts {
        if route_attempt_limit == 0 {
            continue;
        }
        let port_demands = physical_port_demands(
            connection_plan,
            &region_depths,
            &region_orders,
            seed,
            port_order_attempt,
            topology_embedding.map(|embedding| &embedding.positions),
            topology_embedding.is_some_and(|embedding| embedding.terminal_bars),
        )
        .map_err(GeometryPlacementAttemptError::Invalid)?;
        let (rooms, bounds) = place_geometry_rooms(
            region_specs,
            spacing,
            &port_demands,
            topology_embedding.map(|embedding| &embedding.positions),
            topology_embedding.is_some_and(|embedding| embedding.terminal_bars),
        )?;
        match route_physical_sections(
            connection_plan,
            &rooms,
            &bounds,
            seed,
            room_order_attempt
                .saturating_mul(GEOMETRY_PORT_ORDER_COUNT)
                .saturating_add(port_order_attempt),
            route_attempt_limit,
        ) {
            Ok((corridors, route_order_attempt, routes_tried, route_evidence)) => {
                attempted_orders += routes_tried;
                return Ok((
                    rooms,
                    corridors,
                    bounds,
                    port_order_attempt,
                    route_order_attempt,
                    attempted_orders,
                    route_evidence,
                ));
            }
            Err(GeometryPlacementAttemptError::Invalid(error)) => {
                return Err(GeometryPlacementAttemptError::Invalid(error));
            }
            Err(GeometryPlacementAttemptError::RoutesUnavailable {
                attempted_orders: routes_tried,
                last_error: error,
            }) => {
                attempted_orders += routes_tried;
                last_error = error;
            }
        }
    }
    Err(GeometryPlacementAttemptError::RoutesUnavailable {
        attempted_orders,
        last_error,
    })
}

fn place_geometry_rooms(
    region_specs: &[(usize, String, String, &IntermediateRegion)],
    spacing: &GeometrySpacing,
    port_demands: &BTreeMap<String, Vec<PhysicalPortDemand>>,
    topology_positions: Option<&BTreeMap<String, TopologyPoint>>,
    topology_terminal_bars: bool,
) -> Result<(Vec<GeometryRoom>, GeometryBounds), GeometryPlacementAttemptError> {
    if let Some(positions) = topology_positions {
        return place_geometry_rooms_from_topology(
            region_specs,
            spacing,
            port_demands,
            positions,
            topology_terminal_bars,
        );
    }
    let mut column_widths = BTreeMap::<usize, i32>::new();
    for (depth, _, _, region) in region_specs {
        let (width, _) = connection_aware_room_size(
            region,
            port_demands
                .get(region.id.as_str())
                .map(Vec::as_slice)
                .unwrap_or_default(),
        );
        column_widths
            .entry(*depth)
            .and_modify(|existing| *existing = (*existing).max(width))
            .or_insert(width);
    }
    let mut column_origins = BTreeMap::new();
    let mut next_x = spacing.room_margin;
    for (depth, width) in &column_widths {
        column_origins.insert(*depth, next_x);
        next_x += *width + spacing.column_gap;
    }
    let mut next_y_by_depth = BTreeMap::<usize, i32>::new();
    let mut rooms = Vec::new();
    for (depth, _role, _id, region) in region_specs.iter().cloned() {
        let demands = port_demands
            .get(region.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default();
        let (width, height) = connection_aware_room_size(region, demands);
        let x = *column_origins
            .get(&depth)
            .ok_or_else(|| {
                GeometryPlacementAttemptError::Invalid(format!(
                    "missing room column for graph depth {depth}"
                ))
            })?;
        let y = *next_y_by_depth.entry(depth).or_insert(spacing.room_margin);
        next_y_by_depth.insert(depth, y + height + spacing.row_gap);
        rooms.push(GeometryRoom {
            id: room_id(region.id.as_str()),
            source_region: region.id.clone(),
            source_nodes: region.node_ids.clone(),
            role: region.role.clone(),
            geometry_role: region.geometry_role.clone(),
            footprint_class: region.footprint_class.clone(),
            rect: GeometryRect {
                x,
                y,
                width,
                height,
            },
            ports: Vec::new(),
            style_tags: geometry_room_style_tags(region),
        });
    }
    assign_physical_room_ports(&mut rooms, port_demands, None)
        .map_err(GeometryPlacementAttemptError::Invalid)?;
    let bounds = geometry_bounds(&rooms, GEOMETRY_ROUTE_GRID, spacing.room_margin);
    Ok((rooms, bounds))
}

fn place_geometry_rooms_from_topology(
    region_specs: &[(usize, String, String, &IntermediateRegion)],
    spacing: &GeometrySpacing,
    port_demands: &BTreeMap<String, Vec<PhysicalPortDemand>>,
    positions: &BTreeMap<String, TopologyPoint>,
    topology_terminal_bars: bool,
) -> Result<(Vec<GeometryRoom>, GeometryBounds), GeometryPlacementAttemptError> {
    let mut sizes = BTreeMap::new();
    let mut maximum_dimension = 0_i32;
    for (_, _, _, region) in region_specs {
        let size = connection_aware_room_size(
            region,
            port_demands
                .get(region.id.as_str())
                .map(Vec::as_slice)
                .unwrap_or_default(),
        );
        maximum_dimension = maximum_dimension.max(size.0).max(size.1);
        sizes.insert(region.id.as_str(), size);
    }
    let points = positions.values().collect::<Vec<_>>();
    let minimum_axis_distance = points
        .iter()
        .enumerate()
        .flat_map(|(index, left)| {
            points.iter().skip(index + 1).map(move |right| {
                i64::from((left.x - right.x).abs().max((left.y - right.y).abs()))
            })
        })
        .filter(|distance| *distance > 0)
        .min()
        .ok_or_else(|| {
            GeometryPlacementAttemptError::Invalid(
                "topology embedding has no distinct room positions".to_owned(),
            )
        })? as f64;
    // Room placement only needs center separation sufficient for the room
    // envelopes. The exclusive router separately proves corridor-to-room and
    // corridor-to-corridor clearance; using the drawing's smallest node-to-edge
    // distance as a global room scale turns one near abstract edge into
    // multi-thousand-cell corridors everywhere.
    let target_separation = f64::from(maximum_dimension);
    let scale = target_separation / minimum_axis_distance.max(1.0);
    let minimum_x = points
        .iter()
        .map(|point| point.x)
        .min()
        .expect("topology positions should not be empty");
    let minimum_y = points
        .iter()
        .map(|point| point.y)
        .min()
        .expect("topology positions should not be empty");
    let mut rooms = Vec::new();
    for (_, _, _, region) in region_specs.iter().cloned() {
        let point = positions.get(region.id.as_str()).ok_or_else(|| {
            GeometryPlacementAttemptError::Invalid(format!(
                "topology embedding lacks room position for {}",
                region.id
            ))
        })?;
        let (width, mut height) = sizes[region.id.as_str()];
        let demands = port_demands
            .get(region.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default();
        let vertical_orders = demands
            .iter()
            .filter(|demand| matches!(demand.side.as_str(), "east" | "west"))
            .map(|demand| demand.opposite_order)
            .collect::<Vec<_>>();
        let terminal_vertical_span = topology_terminal_bars.then_some(()).and_then(|()| matches!(
            region.id.as_str(),
            "region.start" | "region.goal"
        )
        .then(|| {
            let minimum = vertical_orders.iter().min().copied()?;
            let maximum = vertical_orders.iter().max().copied()?;
            (minimum != maximum).then_some((minimum, maximum))
        })
        .flatten());
        let topology_y = if let Some((minimum, maximum)) = terminal_vertical_span {
            let mapped_span = (f64::from(maximum - minimum) * scale).round() as i32;
            height = align_geometry(
                height.max(mapped_span + GEOMETRY_PORT_MARGIN * 2),
                GEOMETRY_ROUTE_GRID * 2,
            );
            f64::from(minimum + maximum) / 2.0
        } else {
            f64::from(point.y)
        };
        let center_x = f64::from(spacing.room_margin + maximum_dimension / 2)
            + f64::from(point.x - minimum_x) * scale;
        let center_y = f64::from(spacing.room_margin + maximum_dimension / 2)
            + (topology_y - f64::from(minimum_y)) * scale;
        let x = align_geometry((center_x - f64::from(width) / 2.0).round() as i32, GEOMETRY_ROUTE_GRID);
        let y = align_geometry((center_y - f64::from(height) / 2.0).round() as i32, GEOMETRY_ROUTE_GRID);
        rooms.push(GeometryRoom {
            id: room_id(region.id.as_str()),
            source_region: region.id.clone(),
            source_nodes: region.node_ids.clone(),
            role: region.role.clone(),
            geometry_role: region.geometry_role.clone(),
            footprint_class: region.footprint_class.clone(),
            rect: GeometryRect {
                x,
                y,
                width,
                height,
            },
            ports: Vec::new(),
            style_tags: geometry_room_style_tags(region),
        });
    }
    assign_physical_room_ports(&mut rooms, port_demands, Some(scale))
        .map_err(GeometryPlacementAttemptError::Invalid)?;
    let bounds = geometry_bounds(&rooms, GEOMETRY_ROUTE_GRID, spacing.room_margin);
    Ok((rooms, bounds))
}

fn physical_port_demands(
    plan: &PhysicalConnectionPlan,
    depths: &BTreeMap<&str, usize>,
    orders: &BTreeMap<&str, usize>,
    seed: u64,
    port_order_attempt: u32,
    topology_positions: Option<&BTreeMap<String, TopologyPoint>>,
    topology_terminal_bars: bool,
) -> Result<BTreeMap<String, Vec<PhysicalPortDemand>>, String> {
    let mut demands = BTreeMap::<String, Vec<PhysicalPortDemand>>::new();
    for section in &plan.sections {
        if section.topology != "corridor_2" || section.terminal_regions.len() != 2 {
            return Err(format!("unsupported physical section topology on {}", section.id));
        }
        let left = section.terminal_regions[0].as_str();
        let right = section.terminal_regions[1].as_str();
        let left_depth = depths.get(left).copied().unwrap_or(0);
        let right_depth = depths.get(right).copied().unwrap_or(0);
        let left_order = orders.get(left).copied().unwrap_or(0);
        let right_order = orders.get(right).copied().unwrap_or(0);
        let (left_side, right_side, left_opposite_order, right_opposite_order) =
            if let Some(positions) = topology_positions {
                let left_position = positions
                    .get(left)
                    .ok_or_else(|| format!("topology embedding lacks region {left}"))?;
                let right_position = positions
                    .get(right)
                    .ok_or_else(|| format!("topology embedding lacks region {right}"))?;
                let (left_side, right_side) = if topology_terminal_bars {
                    topology_terminal_port_sides(left, right, positions).unwrap_or_else(|| {
                        topology_port_sides(
                            left_position,
                            right_position,
                            port_order_attempt,
                            seed,
                            section.id.as_str(),
                        )
                    })
                } else {
                    topology_port_sides(
                        left_position,
                        right_position,
                        port_order_attempt,
                        seed,
                        section.id.as_str(),
                    )
                };
                let left_order = if matches!(left_side, "north" | "south") {
                    right_position.x
                } else {
                    right_position.y
                };
                let right_order = if matches!(right_side, "north" | "south") {
                    left_position.x
                } else {
                    left_position.y
                };
                (left_side, right_side, left_order, right_order)
            } else {
                let (left_side, right_side) = physical_port_sides(
                    left_depth,
                    right_depth,
                    left_order,
                    right_order,
                    seed,
                    port_order_attempt,
                    section.id.as_str(),
                );
                (
                    left_side,
                    right_side,
                    i32::try_from(right_order).unwrap_or(i32::MAX),
                    i32::try_from(left_order).unwrap_or(i32::MAX),
                )
            };
        demands.entry(left.to_owned()).or_default().push(PhysicalPortDemand {
            section_id: section.id.clone(),
            side: left_side.to_owned(),
            width: section.width,
            opposite_order: left_opposite_order,
        });
        demands.entry(right.to_owned()).or_default().push(PhysicalPortDemand {
            section_id: section.id.clone(),
            side: right_side.to_owned(),
            width: section.width,
            opposite_order: right_opposite_order,
        });
    }
    for room_demands in demands.values_mut() {
        room_demands.sort_by(|left, right| {
            left.side.cmp(&right.side).then_with(|| {
                compare_physical_port_demands(left, right, seed, port_order_attempt)
            })
        });
    }
    Ok(demands)
}

fn topology_port_sides(
    left: &TopologyPoint,
    right: &TopologyPoint,
    attempt: u32,
    seed: u64,
    section_id: &str,
) -> (&'static str, &'static str) {
    let dx = right.x - left.x;
    let dy = right.y - left.y;
    let horizontal = if dx >= 0 {
        ("east", "west")
    } else {
        ("west", "east")
    };
    let vertical = if dy >= 0 {
        ("south", "north")
    } else {
        ("north", "south")
    };
    match attempt {
        0 => {
            if dx.abs() >= dy.abs() {
                horizontal
            } else {
                vertical
            }
        }
        _ => {
            let variant =
                geometry_layout_order_key(section_id, seed, u64::from(attempt)) % 3;
            match variant {
                0 => {
                    if dx.abs() < dy.abs() {
                        horizontal
                    } else {
                        vertical
                    }
                }
                1 => (horizontal.0, vertical.1),
                _ => (vertical.0, horizontal.1),
            }
        }
    }
}

fn topology_terminal_port_sides(
    left: &str,
    right: &str,
    positions: &BTreeMap<String, TopologyPoint>,
) -> Option<(&'static str, &'static str)> {
    let start = positions.get("region.start")?;
    let goal = positions.get("region.goal")?;
    let dx = goal.x - start.x;
    let dy = goal.y - start.y;
    if dx.abs() < dy.abs() {
        return None;
    }
    let start_side = if dx >= 0 { "east" } else { "west" };
    let goal_side = if dx >= 0 { "west" } else { "east" };
    let side_toward = |from: &str, to: &str| {
        let from_x = positions.get(from)?.x;
        let to_x = positions.get(to)?.x;
        Some(if to_x >= from_x { "east" } else { "west" })
    };
    match (left, right) {
        ("region.start", "region.goal") => Some((start_side, goal_side)),
        ("region.goal", "region.start") => Some((goal_side, start_side)),
        ("region.start", _) => Some((start_side, side_toward(right, left)?)),
        (_, "region.start") => Some((side_toward(left, right)?, start_side)),
        ("region.goal", _) => Some((goal_side, side_toward(right, left)?)),
        (_, "region.goal") => Some((side_toward(left, right)?, goal_side)),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn physical_port_sides(
    left_depth: usize,
    right_depth: usize,
    left_order: usize,
    right_order: usize,
    seed: u64,
    attempt: u32,
    section_id: &str,
) -> (&'static str, &'static str) {
    let vertical = if left_order <= right_order {
        ("south", "north")
    } else {
        ("north", "south")
    };
    let horizontal = if left_depth <= right_depth {
        ("east", "west")
    } else {
        ("west", "east")
    };
    if attempt == 0 {
        return if left_depth == right_depth {
            vertical
        } else {
            horizontal
        };
    }

    let variant = geometry_layout_order_key(section_id, seed, u64::from(attempt)) % 3;
    if left_depth == right_depth {
        match variant {
            0 => horizontal,
            1 => (vertical.0, horizontal.1),
            _ => (horizontal.0, vertical.1),
        }
    } else {
        match variant {
            0 => vertical,
            1 => (horizontal.0, vertical.1),
            _ => (vertical.0, horizontal.1),
        }
    }
}

fn compare_physical_port_demands(
    left: &PhysicalPortDemand,
    right: &PhysicalPortDemand,
    seed: u64,
    attempt: u32,
) -> Ordering {
    match attempt % GEOMETRY_PORT_ORDER_COUNT {
        0 => left
            .opposite_order
            .cmp(&right.opposite_order)
            .then_with(|| left.section_id.cmp(&right.section_id)),
        1 => right
            .opposite_order
            .cmp(&left.opposite_order)
            .then_with(|| left.section_id.cmp(&right.section_id)),
        2 => geometry_layout_order_key(left.section_id.as_str(), seed, u64::from(attempt))
            .cmp(&geometry_layout_order_key(
                right.section_id.as_str(),
                seed,
                u64::from(attempt),
            ))
            .then_with(|| left.section_id.cmp(&right.section_id)),
        _ => geometry_layout_order_key(right.section_id.as_str(), seed, u64::from(attempt))
            .cmp(&geometry_layout_order_key(
                left.section_id.as_str(),
                seed,
                u64::from(attempt),
            ))
            .then_with(|| left.section_id.cmp(&right.section_id)),
    }
}

fn connection_aware_room_size(
    region: &IntermediateRegion,
    demands: &[PhysicalPortDemand],
) -> (i32, i32) {
    let (base_width, base_height) = room_size_for_region(region);
    let count = |side: &str| demands.iter().filter(|demand| demand.side == side).count() as i32;
    let horizontal = count("north").max(count("south"));
    let vertical = count("east").max(count("west"));
    let span = |ports: i32| {
        if ports == 0 { 0 } else { GEOMETRY_PORT_MARGIN * 2 + (ports - 1) * GEOMETRY_PORT_SPACING }
    };
    (
        align_geometry(base_width.max(span(horizontal)), GEOMETRY_ROUTE_GRID * 2),
        align_geometry(base_height.max(span(vertical)), GEOMETRY_ROUTE_GRID * 2),
    )
}

fn align_geometry(value: i32, grid: i32) -> i32 {
    ((value + grid - 1) / grid) * grid
}

fn assign_physical_room_ports(
    rooms: &mut [GeometryRoom],
    demands: &BTreeMap<String, Vec<PhysicalPortDemand>>,
    topology_scale: Option<f64>,
) -> Result<(), String> {
    for room in rooms {
        let room_demands = demands
            .get(room.source_region.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default();
        for side in ["north", "east", "south", "west"] {
            let side_demands = room_demands
                .iter()
                .filter(|demand| demand.side == side)
                .collect::<Vec<_>>();
            let count = side_demands.len() as i32;
            let order_bounds = side_demands
                .iter()
                .map(|demand| demand.opposite_order)
                .fold(None::<(i32, i32)>, |bounds, order| {
                    Some(match bounds {
                        Some((minimum, maximum)) => {
                            (minimum.min(order), maximum.max(order))
                        }
                        None => (order, order),
                    })
                });
            let mapped_offsets = topology_scale
                .zip(order_bounds)
                .filter(|(scale, (minimum, maximum))| {
                    (f64::from(maximum - minimum) * *scale).round() as i32
                        >= (count - 1) * GEOMETRY_PORT_SPACING
                })
                .and_then(|(scale, (minimum, maximum))| {
                    let midpoint = f64::from(minimum + maximum) / 2.0;
                    let half_span = match side {
                        "north" | "south" => room.rect.width / 2,
                        "east" | "west" => room.rect.height / 2,
                        _ => 0,
                    };
                    let offsets = side_demands
                        .iter()
                        .map(|demand| {
                            let mapped =
                                ((f64::from(demand.opposite_order) - midpoint) * scale).round()
                                    as i32;
                            align_geometry_nearest(
                                mapped.clamp(
                                    -half_span + GEOMETRY_PORT_MARGIN,
                                    half_span - GEOMETRY_PORT_MARGIN,
                                ),
                                GEOMETRY_ROUTE_GRID,
                            )
                        })
                        .collect::<Vec<_>>();
                    (offsets.iter().copied().collect::<BTreeSet<_>>().len()
                        == offsets.len())
                    .then_some(offsets)
                });
            for (index, demand) in side_demands.into_iter().enumerate() {
                let fixed_offset =
                    (index as i32 * 2 - (count - 1)) * GEOMETRY_PORT_SPACING / 2;
                let offset = mapped_offsets
                    .as_ref()
                    .and_then(|offsets| offsets.get(index).copied())
                    .unwrap_or(fixed_offset);
                let point = match side {
                    "north" => GeometryPoint { x: room.rect.x + room.rect.width / 2 + offset, y: room.rect.y },
                    "east" => GeometryPoint { x: room.rect.x + room.rect.width, y: room.rect.y + room.rect.height / 2 + offset },
                    "south" => GeometryPoint { x: room.rect.x + room.rect.width / 2 + offset, y: room.rect.y + room.rect.height },
                    "west" => GeometryPoint { x: room.rect.x, y: room.rect.y + room.rect.height / 2 + offset },
                    _ => return Err(format!("unsupported room port side {side}")),
                };
                room.ports.push(GeometryRoomPort {
                    id: format!("port.{}.{}", slugify_label(room.id.as_str()), slugify_label(demand.section_id.as_str())),
                    section_id: demand.section_id.clone(),
                    side: side.to_owned(),
                    point,
                    width: demand.width,
                });
            }
        }
    }
    Ok(())
}

fn align_geometry_nearest(value: i32, grid: i32) -> i32 {
    (f64::from(value) / f64::from(grid)).round() as i32 * grid
}

fn geometry_contents(
    candidate: &Candidate,
    intermediate: &IntermediateBreakdown,
    rooms: &[GeometryRoom],
) -> Vec<GeometryContent> {
    let nodes_by_id = candidate
        .graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let regions_by_id = intermediate
        .regions
        .iter()
        .map(|region| (region.id.as_str(), region))
        .collect::<BTreeMap<_, _>>();
    let mut contents = Vec::new();
    for room in rooms {
        let region = regions_by_id.get(room.source_region.as_str()).copied();
        for node_id in &room.source_nodes {
            let Some(node) = nodes_by_id.get(node_id.as_str()).copied() else {
                continue;
            };
            let Some((kind, label, tags)) = content_annotation_for_node(node, room, region) else {
                continue;
            };
            contents.push(GeometryContent {
                id: format!(
                    "content.{}.{}",
                    slugify_label(room.id.as_str()),
                    slugify_label(kind.as_str())
                ),
                room_id: room.id.clone(),
                source_ref: format!("node:{};region:{}", node.id, room.source_region),
                kind,
                label,
                tags,
            });
        }
    }
    contents
}

fn content_annotation_for_node(
    node: &Node,
    room: &GeometryRoom,
    region: Option<&IntermediateRegion>,
) -> Option<(String, String, Vec<String>)> {
    let (kind, label) = if node.kind == NodeKind::Start {
        ("start_marker", "Start")
    } else if node.kind == NodeKind::Goal {
        ("goal_marker", "Goal")
    } else if node_has_tag(node, "boss") {
        ("boss_threshold", "Boss Threshold")
    } else if node_has_tag(node, "hazard") || node.kind == NodeKind::Hazard {
        ("hazard", "Hazard")
    } else if node_has_tag(node, "reward") || node.kind == NodeKind::Treasure {
        ("reward_cache", "Reward Cache")
    } else if node.kind == NodeKind::Key {
        ("key_pickup", "Key Pickup")
    } else if node.kind == NodeKind::Gate || node_has_tag(node, "lock") {
        ("locked_gate", "Locked Gate")
    } else if node.kind == NodeKind::Shortcut {
        ("shortcut_marker", "Shortcut Marker")
    } else if node.kind == NodeKind::Secret {
        ("secret_route_marker", "Secret Route")
    } else if node.kind == NodeKind::Resource {
        ("resource_clue", "Resource Clue")
    } else {
        return None;
    };
    let mut tags = vec![
        kind.to_owned(),
        node.kind.as_str().to_owned(),
        room.role.clone(),
        room.geometry_role.clone(),
    ];
    tags.extend(node.tags.clone());
    tags.extend(room.style_tags.clone());
    if let Some(region) = region {
        tags.extend(region.entrance_expectations.clone());
    }
    Some((kind.to_owned(), label.to_owned(), dedupe_strings(tags)))
}

fn route_physical_sections(
    plan: &PhysicalConnectionPlan,
    rooms: &[GeometryRoom],
    bounds: &GeometryBounds,
    seed: u64,
    order_nonce: u32,
    max_attempts: u32,
) -> Result<
    (
        Vec<GeometryCorridor>,
        u32,
        u32,
        PhysicalRouteSearchEvidence,
    ),
    GeometryPlacementAttemptError,
> {
    let rooms_by_region = rooms
        .iter()
        .map(|room| (room.source_region.as_str(), room))
        .collect::<BTreeMap<_, _>>();
    let mut sections = plan.sections.iter().collect::<Vec<_>>();
    sections.sort_by(|left, right| {
        let left_rooms = section_rooms(left, &rooms_by_region);
        let right_rooms = section_rooms(right, &rooms_by_region);
        let left_distance = left_rooms
            .map(|(from, to)| geometry_room_distance(from, to))
            .unwrap_or(0);
        let right_distance = right_rooms
            .map(|(from, to)| geometry_room_distance(from, to))
            .unwrap_or(0);
        right_distance.cmp(&left_distance).then_with(|| left.id.cmp(&right.id))
    });
    let mut distance_orders = vec![sections.clone()];
    let mut reversed = sections.clone();
    reversed.reverse();
    distance_orders.push(reversed);
    let mut seeded = sections.clone();
    seeded.sort_by(|left, right| {
        geometry_layout_order_key(left.id.as_str(), seed, u64::from(order_nonce))
            .cmp(&geometry_layout_order_key(
                right.id.as_str(),
                seed,
                u64::from(order_nonce),
            ))
            .then_with(|| left.id.cmp(&right.id))
    });
    let mut separator_order = sections.clone();
    separator_order.sort_by(|left, right| {
        let terminal_rank = |section: &PhysicalConnectionSection| {
            u8::from(section.terminal_regions.iter().any(|region| {
                region == "region.start" || region == "region.goal"
            }))
        };
        let constrained_rank = |section: &PhysicalConnectionSection| {
            u8::from(
                section
                    .traversal_refs
                    .iter()
                    .all(|reference| reference.traversal == "open"),
            )
        };
        constrained_rank(left)
            .cmp(&constrained_rank(right))
            .then_with(|| terminal_rank(left).cmp(&terminal_rank(right)))
            .then_with(|| left.id.cmp(&right.id))
    });
    let (orders, order_offset) = if max_attempts >= GEOMETRY_ROUTE_ORDER_COUNT {
        (
            vec![
                separator_order.clone(),
                distance_orders[0].clone(),
                distance_orders[1].clone(),
                seeded.clone(),
            ],
            0_u32,
        )
    } else if order_nonce % GEOMETRY_PORT_ORDER_COUNT == 0 {
        (distance_orders, 0_u32)
    } else {
        (vec![seeded, separator_order], 2_u32)
    };
    let mut attempted_orders = 0_u32;
    let mut last_error = "no physical route order was attempted".to_owned();
    for (index, order) in orders.into_iter().take(max_attempts as usize).enumerate() {
        attempted_orders += 1;
        match try_route_physical_sections(&order, &rooms_by_region, rooms, bounds) {
            Ok((corridors, evidence)) => {
                return Ok((
                    corridors,
                    order_offset + index as u32,
                    attempted_orders,
                    evidence,
                ));
            }
            Err(PhysicalRouteAttemptError::Invalid(error)) => {
                return Err(GeometryPlacementAttemptError::Invalid(error));
            }
            Err(PhysicalRouteAttemptError::Unavailable(error)) => last_error = error,
        }
    }
    Err(GeometryPlacementAttemptError::RoutesUnavailable {
        attempted_orders,
        last_error,
    })
}

fn section_rooms<'a>(
    section: &PhysicalConnectionSection,
    rooms: &BTreeMap<&str, &'a GeometryRoom>,
) -> Option<(&'a GeometryRoom, &'a GeometryRoom)> {
    if section.terminal_regions.len() != 2 {
        return None;
    }
    Some((
        *rooms.get(section.terminal_regions[0].as_str())?,
        *rooms.get(section.terminal_regions[1].as_str())?,
    ))
}

fn geometry_room_distance(left: &GeometryRoom, right: &GeometryRoom) -> u32 {
    let left = rect_center(&left.rect);
    let right = rect_center(&right.rect);
    left.x.abs_diff(right.x) + left.y.abs_diff(right.y)
}

fn try_route_physical_sections(
    sections: &[&PhysicalConnectionSection],
    rooms_by_region: &BTreeMap<&str, &GeometryRoom>,
    rooms: &[GeometryRoom],
    bounds: &GeometryBounds,
) -> Result<(Vec<GeometryCorridor>, PhysicalRouteSearchEvidence), PhysicalRouteAttemptError> {
    for section in sections {
        let (from_room, to_room) = section_rooms(section, rooms_by_region)
            .ok_or_else(|| {
                PhysicalRouteAttemptError::Invalid(format!(
                    "section {} references missing terminal room",
                    section.id
                ))
            })?;
        let from_port = from_room
            .ports
            .iter()
            .find(|port| port.section_id == section.id)
            .ok_or_else(|| {
                PhysicalRouteAttemptError::Invalid(format!(
                    "room {} lacks port for {}",
                    from_room.id, section.id
                ))
            })?;
        let to_port = to_room
            .ports
            .iter()
            .find(|port| port.section_id == section.id)
            .ok_or_else(|| {
                PhysicalRouteAttemptError::Invalid(format!(
                    "room {} lacks port for {}",
                    to_room.id, section.id
                ))
            })?;
        let _ = (from_port, to_port);
    }
    let mut evidence = PhysicalRouteSearchEvidence::default();
    let mut route_order = sections.to_vec();
    let mut routed_result = None;
    for repair_attempt in 0..=GEOMETRY_CONFLICT_REPAIR_BUDGET {
        let mut attempt_evidence = PhysicalRouteSearchEvidence::default();
        let mut routed = Vec::<(&PhysicalConnectionSection, Vec<GeometryPoint>)>::new();
        if search_physical_section_routes(
            &route_order,
            rooms_by_region,
            rooms,
            bounds,
            &mut routed,
            &mut attempt_evidence,
        ) {
            merge_physical_route_evidence(&mut evidence, &attempt_evidence);
            routed_result = Some(routed);
            break;
        }
        let failed_section = attempt_evidence.last_failed_section.clone();
        let blocking_owners = attempt_evidence.blocking_owners.clone();
        merge_physical_route_evidence(&mut evidence, &attempt_evidence);
        if repair_attempt == GEOMETRY_CONFLICT_REPAIR_BUDGET {
            break;
        }
        let Some(failed_index) = route_order
            .iter()
            .position(|section| section.id == failed_section)
        else {
            break;
        };
        let blocking_index = route_order
            .iter()
            .enumerate()
            .filter(|(index, section)| {
                *index < failed_index && blocking_owners.contains(section.id.as_str())
            })
            .map(|(index, _)| index)
            .min();
        let Some(blocking_index) = blocking_index else {
            break;
        };
        let failed = route_order.remove(failed_index);
        route_order.insert(blocking_index, failed);
        evidence.repairs = evidence.repairs.saturating_add(1);
    }
    let Some(routed) = routed_result else {
        let budget = evidence
            .budget_exhausted
            .map(|budget| format!("; {budget} budget exhausted"))
            .unwrap_or_default();
        let blockers = if evidence.blocking_owners.is_empty() {
            "none observed".to_owned()
        } else {
            evidence
                .blocking_owners
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        };
        return Err(PhysicalRouteAttemptError::Unavailable(format!(
            "single-floor route search exhausted after routing at most {}/{} physical section(s), {} decision(s), {} backtrack(s), {} path alternative(s), {} repair(s), and {} grid expansion(s), with {} path expansion exhaustion(s){budget}; last failed section: {} {}; blocking owners: {blockers}",
            evidence.deepest_routed,
            sections.len(),
            evidence.decisions,
            evidence.backtracks,
            evidence.path_alternatives,
            evidence.repairs,
            evidence.grid_expansions,
            evidence.path_expansion_exhaustions,
            evidence.last_failed_section,
            evidence.last_failed_ports,
        )));
    };
    let mut corridors = Vec::new();
    for (section, path) in routed {
        let (from_room, to_room) = section_rooms(section, rooms_by_region)
            .expect("validated section rooms should remain available");
        let from_port = from_room
            .ports
            .iter()
            .find(|port| port.section_id == section.id)
            .expect("validated source port should remain available");
        let to_port = to_room
            .ports
            .iter()
            .find(|port| port.section_id == section.id)
            .expect("validated target port should remain available");
        let source_connector = section.source_connectors.first().cloned().unwrap_or_default();
        let source_edge = section.source_edges.first().cloned().unwrap_or_default();
        let traversal_hint = if section
            .traversal_refs
            .iter()
            .all(|reference| reference.traversal == "open")
        {
            "open".to_owned()
        } else {
            section
                .traversal_refs
                .first()
                .map(|reference| reference.traversal.clone())
                .unwrap_or_else(|| "open".to_owned())
        };
        corridors.push(GeometryCorridor {
            id: format!("corridor.{}", slugify_label(section.id.as_str())),
            physical_section: section.id.clone(),
            source_connector,
            source_edge,
            source_connectors: section.source_connectors.clone(),
            source_edges: section.source_edges.clone(),
            traversal_refs: section.traversal_refs.clone(),
            from_room: from_room.id.clone(),
            to_room: to_room.id.clone(),
            traversal_hint,
            semantic_tags: section.semantic_tags.clone(),
            width: section.width,
            from_port: from_port.id.clone(),
            to_port: to_port.id.clone(),
            points: compress_geometry_route(path),
        });
    }
    corridors.sort_by(|left, right| left.physical_section.cmp(&right.physical_section));
    Ok((corridors, evidence))
}

fn merge_physical_route_evidence(
    target: &mut PhysicalRouteSearchEvidence,
    source: &PhysicalRouteSearchEvidence,
) {
    target.decisions = target.decisions.saturating_add(source.decisions);
    target.backtracks = target.backtracks.saturating_add(source.backtracks);
    target.path_alternatives = target
        .path_alternatives
        .saturating_add(source.path_alternatives);
    target.repairs = target.repairs.saturating_add(source.repairs);
    target.deepest_routed = target.deepest_routed.max(source.deepest_routed);
    target
        .blocking_owners
        .extend(source.blocking_owners.iter().cloned());
    target.budget_exhausted = source.budget_exhausted;
    target.grid_expansions = target
        .grid_expansions
        .saturating_add(source.grid_expansions);
    target.path_expansion_exhaustions = target
        .path_expansion_exhaustions
        .saturating_add(source.path_expansion_exhaustions);
    target.last_failed_section = source.last_failed_section.clone();
    target.last_failed_ports = source.last_failed_ports.clone();
}

#[allow(clippy::too_many_arguments)]
fn search_physical_section_routes<'a>(
    sections: &[&'a PhysicalConnectionSection],
    rooms_by_region: &BTreeMap<&str, &GeometryRoom>,
    rooms: &[GeometryRoom],
    bounds: &GeometryBounds,
    routed: &mut Vec<(&'a PhysicalConnectionSection, Vec<GeometryPoint>)>,
    evidence: &mut PhysicalRouteSearchEvidence,
) -> bool {
    let mut prepared = Vec::new();
    for section in sections {
        let Some((from_room, to_room)) = section_rooms(section, rooms_by_region) else {
            return false;
        };
        let Some(from_port) = from_room
            .ports
            .iter()
            .find(|port| port.section_id == section.id)
        else {
            return false;
        };
        let Some(to_port) = to_room
            .ports
            .iter()
            .find(|port| port.section_id == section.id)
        else {
            return false;
        };
        let alternatives = route_physical_section_alternatives(
            from_room,
            from_port,
            to_room,
            to_port,
            section.width,
            rooms,
            &BTreeMap::new(),
            bounds,
        );
        evidence
            .blocking_owners
            .extend(alternatives.blocking_owners.iter().cloned());
        evidence.grid_expansions = evidence
            .grid_expansions
            .saturating_add(alternatives.grid_expansions);
        evidence.path_expansion_exhaustions = evidence
            .path_expansion_exhaustions
            .saturating_add(alternatives.expansion_exhaustions);
        evidence.path_alternatives = evidence
            .path_alternatives
            .saturating_add(alternatives.paths.len() as u32);
        if alternatives.paths.is_empty() {
            evidence.last_failed_section = section.id.clone();
            evidence.last_failed_ports = format!(
                "from {}@{},{}:{} to {}@{},{}:{}",
                from_room.source_region,
                from_port.point.x,
                from_port.point.y,
                from_port.side,
                to_room.source_region,
                to_port.point.x,
                to_port.point.y,
                to_port.side,
            );
            return false;
        }
        prepared.push(PreparedPhysicalSectionRoutes {
            section,
            paths: alternatives.paths,
        });
    }
    let remaining = (0..prepared.len()).collect::<Vec<_>>();
    search_prepared_physical_section_routes(
        &prepared,
        remaining,
        rooms_by_region,
        rooms,
        bounds,
        BTreeMap::new(),
        routed,
        evidence,
    )
}

fn search_prepared_physical_section_routes<'a>(
    sections: &[PreparedPhysicalSectionRoutes<'a>],
    remaining: Vec<usize>,
    rooms_by_region: &BTreeMap<&str, &GeometryRoom>,
    rooms: &[GeometryRoom],
    bounds: &GeometryBounds,
    reserved: BTreeMap<(i32, i32), String>,
    routed: &mut Vec<(&'a PhysicalConnectionSection, Vec<GeometryPoint>)>,
    evidence: &mut PhysicalRouteSearchEvidence,
) -> bool {
    let depth = sections.len().saturating_sub(remaining.len());
    evidence.deepest_routed = evidence.deepest_routed.max(depth);
    if remaining.is_empty() {
        return true;
    }
    if evidence.decisions >= GEOMETRY_ROUTE_DECISION_BUDGET {
        evidence.budget_exhausted = Some("decision");
        return false;
    }
    let mut selected = None::<(usize, Vec<usize>, BTreeSet<String>)>;
    for section_index in remaining.iter().copied() {
        let mut viable = Vec::new();
        let mut blockers = BTreeSet::new();
        for (path_index, path) in sections[section_index].paths.iter().enumerate() {
            let mut blocked = false;
            for point in path.iter().skip(1) {
                if let Some(owner) = reserved.get(&(point.x, point.y)) {
                    blockers.insert(owner.clone());
                    blocked = true;
                }
            }
            if !blocked {
                viable.push(path_index);
            }
        }
        if selected
            .as_ref()
            .is_none_or(|(_, current, _)| viable.len() < current.len())
        {
            selected = Some((section_index, viable, blockers));
        }
    }
    let Some((section_index, viable, blockers)) = selected else {
        return false;
    };
    let prepared = &sections[section_index];
    let mut candidate_paths = viable
        .into_iter()
        .map(|path_index| prepared.paths[path_index].clone())
        .collect::<Vec<_>>();
    if candidate_paths.is_empty() {
        let Some((from_room, to_room)) = section_rooms(prepared.section, rooms_by_region) else {
            return false;
        };
        let Some(from_port) = from_room
            .ports
            .iter()
            .find(|port| port.section_id == prepared.section.id)
        else {
            return false;
        };
        let Some(to_port) = to_room
            .ports
            .iter()
            .find(|port| port.section_id == prepared.section.id)
        else {
            return false;
        };
        let alternatives = route_physical_section_alternatives(
            from_room,
            from_port,
            to_room,
            to_port,
            prepared.section.width,
            rooms,
            &reserved,
            bounds,
        );
        evidence
            .blocking_owners
            .extend(alternatives.blocking_owners.iter().cloned());
        evidence.grid_expansions = evidence
            .grid_expansions
            .saturating_add(alternatives.grid_expansions);
        evidence.path_expansion_exhaustions = evidence
            .path_expansion_exhaustions
            .saturating_add(alternatives.expansion_exhaustions);
        evidence.path_alternatives = evidence
            .path_alternatives
            .saturating_add(alternatives.paths.len() as u32);
        candidate_paths = alternatives.paths;
        if candidate_paths.is_empty() {
            evidence.blocking_owners.extend(blockers);
            evidence.last_failed_section = prepared.section.id.clone();
            evidence.last_failed_ports =
                "blocked by bounded dynamic exclusive-route repair".to_owned();
            return false;
        }
    }
    let next_remaining = remaining
        .into_iter()
        .filter(|candidate| *candidate != section_index)
        .collect::<Vec<_>>();
    for (alternative_index, path) in candidate_paths.into_iter().enumerate() {
        if evidence.decisions >= GEOMETRY_ROUTE_DECISION_BUDGET {
            evidence.budget_exhausted = Some("decision");
            return false;
        }
        evidence.decisions += 1;
        let mut next_reserved = reserved.clone();
        reserve_geometry_route(
            &path,
            prepared.section.width,
            prepared.section.id.as_str(),
            &mut next_reserved,
        );
        routed.push((prepared.section, path));
        if search_prepared_physical_section_routes(
            sections,
            next_remaining.clone(),
            rooms_by_region,
            rooms,
            bounds,
            next_reserved,
            routed,
            evidence,
        ) {
            return true;
        }
        routed.pop();
        if evidence.backtracks >= GEOMETRY_ROUTE_BACKTRACK_BUDGET {
            evidence.budget_exhausted = Some("backtrack");
            return false;
        }
        evidence.backtracks += 1;
        if alternative_index > 0 || evidence.deepest_routed > depth + 1 {
            evidence.repairs += 1;
        }
    }
    if evidence.last_failed_section.is_empty() {
        evidence.last_failed_section = prepared.section.id.clone();
        evidence.last_failed_ports =
            "blocked by precomputed exclusive route reservations".to_owned();
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn route_physical_section_alternatives(
    from_room: &GeometryRoom,
    from_port: &GeometryRoomPort,
    to_room: &GeometryRoom,
    to_port: &GeometryRoomPort,
    width: i32,
    rooms: &[GeometryRoom],
    reserved: &BTreeMap<(i32, i32), String>,
    bounds: &GeometryBounds,
) -> PhysicalPathAlternatives {
    let mut result = PhysicalPathAlternatives::default();
    let start = GeometryPoint {
        x: from_port.point.x,
        y: from_port.point.y,
    };
    let end = GeometryPoint {
        x: to_port.point.x,
        y: to_port.point.y,
    };
    let middle_x = align_geometry((start.x + end.x) / 2, GEOMETRY_ROUTE_GRID);
    let middle_y = align_geometry((start.y + end.y) / 2, GEOMETRY_ROUTE_GRID);
    let clearance = width / 2 + GEOMETRY_CORRIDOR_SEPARATION;
    let start_escape = geometry_port_escape_point(from_port, clearance);
    let end_escape = geometry_port_escape_point(to_port, clearance);
    let waypoint_candidates = vec![
        vec![
            start.clone(),
            GeometryPoint {
                x: end.x,
                y: start.y,
            },
            end.clone(),
        ],
        vec![
            start.clone(),
            GeometryPoint {
                x: start.x,
                y: end.y,
            },
            end.clone(),
        ],
        vec![
            start.clone(),
            GeometryPoint {
                x: middle_x,
                y: start.y,
            },
            GeometryPoint {
                x: middle_x,
                y: end.y,
            },
            end.clone(),
        ],
        vec![
            start.clone(),
            GeometryPoint {
                x: start.x,
                y: middle_y,
            },
            GeometryPoint {
                x: end.x,
                y: middle_y,
            },
            end.clone(),
        ],
    ];
    for waypoints in waypoint_candidates {
        let path = rasterize_topology_waypoints(&waypoints);
        let mut blocking_owners = BTreeSet::new();
        if path.iter().skip(1).all(|point| {
            geometry_route_available(
                (point.x, point.y),
                from_room,
                from_port,
                to_room,
                to_port,
                width,
                rooms,
                reserved,
                &BTreeSet::new(),
                bounds,
                &mut blocking_owners,
            )
        }) && !result.paths.iter().any(|existing| existing == &path)
        {
            result.paths.push(path);
            if result.paths.len() >= GEOMETRY_PATH_ALTERNATIVES as usize {
                return result;
            }
        }
        result.blocking_owners.extend(blocking_owners);
    }
    let detour_margin = clearance + GEOMETRY_ROUTE_GRID;
    let mut detour_x = rooms
        .iter()
        .flat_map(|room| {
            [
                align_geometry_nearest(room.rect.x - detour_margin, GEOMETRY_ROUTE_GRID),
                align_geometry_nearest(
                    room.rect.x + room.rect.width + detour_margin,
                    GEOMETRY_ROUTE_GRID,
                ),
            ]
        })
        .filter(|x| *x >= 0 && *x <= bounds.width)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    detour_x.sort_by_key(|x| (x.abs_diff(middle_x), *x));
    let mut detour_y = rooms
        .iter()
        .flat_map(|room| {
            [
                align_geometry_nearest(room.rect.y - detour_margin, GEOMETRY_ROUTE_GRID),
                align_geometry_nearest(
                    room.rect.y + room.rect.height + detour_margin,
                    GEOMETRY_ROUTE_GRID,
                ),
            ]
        })
        .filter(|y| *y >= 0 && *y <= bounds.height)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    detour_y.sort_by_key(|y| (y.abs_diff(middle_y), *y));
    let detour_waypoints = detour_x
        .into_iter()
        .map(|x| {
            vec![
                start.clone(),
                start_escape.clone(),
                GeometryPoint {
                    x,
                    y: start_escape.y,
                },
                GeometryPoint { x, y: end_escape.y },
                end_escape.clone(),
                end.clone(),
            ]
        })
        .chain(detour_y.into_iter().map(|y| {
            vec![
                start.clone(),
                start_escape.clone(),
                GeometryPoint {
                    x: start_escape.x,
                    y,
                },
                GeometryPoint { x: end_escape.x, y },
                end_escape.clone(),
                end.clone(),
            ]
        }));
    for waypoints in detour_waypoints {
        let path = rasterize_topology_waypoints(&waypoints);
        let mut blocking_owners = BTreeSet::new();
        if path.iter().skip(1).all(|point| {
            geometry_route_available(
                (point.x, point.y),
                from_room,
                from_port,
                to_room,
                to_port,
                width,
                rooms,
                reserved,
                &BTreeSet::new(),
                bounds,
                &mut blocking_owners,
            )
        }) && !result.paths.iter().any(|existing| existing == &path)
        {
            result.paths.push(path);
            if result.paths.len() >= GEOMETRY_PATH_ALTERNATIVES as usize {
                return result;
            }
        }
        result.blocking_owners.extend(blocking_owners);
    }
    let mut excluded = BTreeSet::new();
    for nonce in 0..GEOMETRY_PATH_ALTERNATIVES {
        let mut blocking_owners = BTreeSet::new();
        let mut grid_expansions = 0_u32;
        let mut expansion_exhausted = false;
        let Some(path) = route_physical_section(
            from_room,
            from_port,
            to_room,
            to_port,
            width,
            rooms,
            reserved,
            &excluded,
            bounds,
            nonce,
            &mut blocking_owners,
            &mut grid_expansions,
            &mut expansion_exhausted,
        ) else {
            result.blocking_owners.extend(blocking_owners);
            result.grid_expansions = result.grid_expansions.saturating_add(grid_expansions);
            result.expansion_exhaustions = result
                .expansion_exhaustions
                .saturating_add(u32::from(expansion_exhausted));
            break;
        };
        result.blocking_owners.extend(blocking_owners);
        result.grid_expansions = result.grid_expansions.saturating_add(grid_expansions);
        let interior_start = (path.len() / 4).max(1);
        let interior_end = path.len().saturating_sub(interior_start + 1);
        if interior_end > interior_start {
            let span = interior_end - interior_start;
            let selected =
                interior_start + (nonce as usize * span / GEOMETRY_PATH_ALTERNATIVES as usize);
            exclude_geometry_route_band(&path, selected, &mut excluded);
        }
        if result.paths.iter().any(|existing| existing == &path) {
            continue;
        }
        result.paths.push(path);
        if result.paths.len() >= GEOMETRY_PATH_ALTERNATIVES as usize {
            break;
        }
    }
    // Staircases are a final completeness fallback. Prefer the sparse
    // orthogonal witnesses and turn-aware grid paths above so downstream
    // catalog realization does not receive one bend per diagonal grid step.
    for prefer_horizontal in [true, false] {
        let path = rasterize_geometry_staircase_route(
            &start,
            &start_escape,
            &end_escape,
            &end,
            prefer_horizontal,
        );
        let mut blocking_owners = BTreeSet::new();
        if path.iter().skip(1).all(|point| {
            geometry_route_available(
                (point.x, point.y),
                from_room,
                from_port,
                to_room,
                to_port,
                width,
                rooms,
                reserved,
                &BTreeSet::new(),
                bounds,
                &mut blocking_owners,
            )
        }) && !result.paths.iter().any(|existing| existing == &path)
        {
            result.paths.push(path);
            if result.paths.len() >= GEOMETRY_PATH_ALTERNATIVES as usize {
                return result;
            }
        }
        result.blocking_owners.extend(blocking_owners);
    }
    result
}

fn geometry_port_escape_point(port: &GeometryRoomPort, clearance: i32) -> GeometryPoint {
    let (dx, dy) = direction_vector(port.side.as_str());
    let steps = align_geometry(clearance, GEOMETRY_ROUTE_GRID) / GEOMETRY_ROUTE_GRID + 1;
    GeometryPoint {
        x: port.point.x + dx * steps * GEOMETRY_ROUTE_GRID,
        y: port.point.y + dy * steps * GEOMETRY_ROUTE_GRID,
    }
}

fn rasterize_geometry_staircase_route(
    start: &GeometryPoint,
    start_escape: &GeometryPoint,
    end_escape: &GeometryPoint,
    end: &GeometryPoint,
    prefer_horizontal: bool,
) -> Vec<GeometryPoint> {
    let mut path = rasterize_topology_waypoints(&[start.clone(), start_escape.clone()]);
    let mut cursor = (start_escape.x, start_escape.y);
    let target = (end_escape.x, end_escape.y);
    let span_x = i64::from(target.0 - cursor.0);
    let span_y = i64::from(target.1 - cursor.1);
    let origin = cursor;
    while cursor != target {
        let mut candidates = Vec::new();
        if cursor.0 != target.0 {
            candidates.push((
                cursor.0 + (target.0 - cursor.0).signum() * GEOMETRY_ROUTE_GRID,
                cursor.1,
                true,
            ));
        }
        if cursor.1 != target.1 {
            candidates.push((
                cursor.0,
                cursor.1 + (target.1 - cursor.1).signum() * GEOMETRY_ROUTE_GRID,
                false,
            ));
        }
        candidates.sort_by_key(|candidate| {
            let relative_x = i64::from(candidate.0 - origin.0);
            let relative_y = i64::from(candidate.1 - origin.1);
            (
                (span_x * relative_y - span_y * relative_x).unsigned_abs(),
                u8::from(candidate.2 != prefer_horizontal),
            )
        });
        let next = candidates[0];
        cursor = (next.0, next.1);
        path.push(GeometryPoint {
            x: cursor.0,
            y: cursor.1,
        });
    }
    path.extend(rasterize_topology_waypoints(&[
        end_escape.clone(),
        end.clone(),
    ]));
    dedupe_points(path)
}

fn exclude_geometry_route_band(
    path: &[GeometryPoint],
    center: usize,
    excluded: &mut BTreeSet<(i32, i32)>,
) {
    let start = center.saturating_sub(4).max(1);
    let end = center
        .saturating_add(5)
        .min(path.len().saturating_sub(1));
    for point in path.iter().take(end).skip(start) {
        for dy in -2_i32..=2 {
            for dx in -2_i32..=2 {
                if dx.abs() + dy.abs() <= 2 {
                    excluded.insert((
                        point.x + dx * GEOMETRY_ROUTE_GRID,
                        point.y + dy * GEOMETRY_ROUTE_GRID,
                    ));
                }
            }
        }
    }
}

fn rasterize_topology_waypoints(waypoints: &[GeometryPoint]) -> Vec<GeometryPoint> {
    let mut path = Vec::new();
    for segment in waypoints.windows(2) {
        let from = &segment[0];
        let to = &segment[1];
        if from.x != to.x && from.y != to.y {
            return Vec::new();
        }
        let dx = (to.x - from.x).signum() * GEOMETRY_ROUTE_GRID;
        let dy = (to.y - from.y).signum() * GEOMETRY_ROUTE_GRID;
        let mut cursor = (from.x, from.y);
        if path.is_empty() {
            path.push(GeometryPoint {
                x: cursor.0,
                y: cursor.1,
            });
        }
        while cursor != (to.x, to.y) {
            cursor = (cursor.0 + dx, cursor.1 + dy);
            path.push(GeometryPoint {
                x: cursor.0,
                y: cursor.1,
            });
        }
    }
    dedupe_points(path)
}

#[allow(clippy::too_many_arguments)]
fn route_physical_section(
    from_room: &GeometryRoom,
    from_port: &GeometryRoomPort,
    to_room: &GeometryRoom,
    to_port: &GeometryRoomPort,
    width: i32,
    rooms: &[GeometryRoom],
    reserved: &BTreeMap<(i32, i32), String>,
    excluded: &BTreeSet<(i32, i32)>,
    bounds: &GeometryBounds,
    nonce: u32,
    blocking_owners: &mut BTreeSet<String>,
    grid_expansions: &mut u32,
    expansion_exhausted: &mut bool,
) -> Option<Vec<GeometryPoint>> {
    let start = (from_port.point.x, from_port.point.y);
    let end = (to_port.point.x, to_port.point.y);
    const START_DIRECTION: u8 = 4;
    const TURN_COST: u32 = 16;
    let start_state = (start.0, start.1, START_DIRECTION);
    let mut queue = BinaryHeap::new();
    queue.push(Reverse((
        geometry_route_heuristic(start, end).saturating_mul(2),
        u32::MAX,
        geometry_route_tie_key(start, nonce),
        start.0,
        start.1,
        START_DIRECTION,
    )));
    let mut costs = HashMap::from([(start_state, 0_u32)]);
    let mut previous = HashMap::new();
    let mut final_state = None;
    while let Some(Reverse((_estimate, reverse_cost, _tie, x, y, direction))) = queue.pop() {
        let cost = u32::MAX - reverse_cost;
        if *grid_expansions >= GEOMETRY_PATH_EXPANSION_BUDGET {
            *expansion_exhausted = true;
            blocking_owners.insert(format!(
                "path_expansion_budget:{GEOMETRY_PATH_EXPANSION_BUDGET}"
            ));
            return None;
        }
        *grid_expansions += 1;
        let state = (x, y, direction);
        if costs.get(&state).copied() != Some(cost) {
            continue;
        }
        let position = (x, y);
        if position == end {
            final_state = Some(state);
            break;
        }
        let mut neighbors = vec![
            (position.0 + GEOMETRY_ROUTE_GRID, position.1, 0_u8),
            (position.0, position.1 + GEOMETRY_ROUTE_GRID, 1_u8),
            (position.0 - GEOMETRY_ROUTE_GRID, position.1, 2_u8),
            (position.0, position.1 - GEOMETRY_ROUTE_GRID, 3_u8),
        ];
        neighbors.sort_by_key(|neighbor| {
            (
                neighbor.0.abs_diff(end.0) + neighbor.1.abs_diff(end.1),
                geometry_route_tie_key((neighbor.0, neighbor.1), nonce),
            )
        });
        for neighbor in neighbors {
            let neighbor_position = (neighbor.0, neighbor.1);
            if !geometry_route_available(
                    neighbor_position,
                    from_room,
                    from_port,
                    to_room,
                    to_port,
                    width,
                    rooms,
                    reserved,
                    excluded,
                    bounds,
                    blocking_owners,
                ) {
                continue;
            }
            let turn_cost =
                u32::from(direction != START_DIRECTION && direction != neighbor.2) * TURN_COST;
            let next_cost = cost.saturating_add(1).saturating_add(turn_cost);
            let neighbor_state = (neighbor.0, neighbor.1, neighbor.2);
            if costs
                .get(&neighbor_state)
                .is_some_and(|existing| *existing <= next_cost)
            {
                continue;
            }
            costs.insert(neighbor_state, next_cost);
            previous.insert(neighbor_state, state);
            queue.push(Reverse((
                next_cost.saturating_add(
                    geometry_route_heuristic(neighbor_position, end).saturating_mul(2),
                ),
                u32::MAX - next_cost,
                geometry_route_tie_key(neighbor_position, nonce),
                neighbor.0,
                neighbor.1,
                neighbor.2,
            )));
        }
    }
    let mut cursor = final_state?;
    let mut path = vec![GeometryPoint { x: end.0, y: end.1 }];
    while cursor != start_state {
        cursor = *previous.get(&cursor)?;
        path.push(GeometryPoint {
            x: cursor.0,
            y: cursor.1,
        });
    }
    path.reverse();
    Some(path)
}

fn geometry_route_heuristic(position: (i32, i32), end: (i32, i32)) -> u32 {
    (position.0.abs_diff(end.0) + position.1.abs_diff(end.1))
        / u32::try_from(GEOMETRY_ROUTE_GRID).expect("positive route grid")
}

fn geometry_route_tie_key(position: (i32, i32), nonce: u32) -> u64 {
    let x = u64::from(position.0 as u32);
    let y = u64::from(position.1 as u32);
    x.wrapping_mul(0x9E37_79B1)
        .rotate_left(nonce.saturating_mul(7) % 63 + 1)
        ^ y.wrapping_mul(0x85EB_CA77)
        ^ u64::from(nonce).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
}

#[allow(clippy::too_many_arguments)]
fn geometry_route_available(
    position: (i32, i32),
    from_room: &GeometryRoom,
    from_port: &GeometryRoomPort,
    to_room: &GeometryRoom,
    to_port: &GeometryRoomPort,
    width: i32,
    rooms: &[GeometryRoom],
    reserved: &BTreeMap<(i32, i32), String>,
    excluded: &BTreeSet<(i32, i32)>,
    bounds: &GeometryBounds,
    blocking_owners: &mut BTreeSet<String>,
) -> bool {
    if position.0 < 0
        || position.1 < 0
        || position.0 > bounds.width
        || position.1 > bounds.height
        || excluded.contains(&position)
    {
        return false;
    }
    if let Some(owner) = reserved.get(&position) {
        blocking_owners.insert(owner.clone());
        return false;
    }
    let clearance = width / 2 + GEOMETRY_CORRIDOR_SEPARATION;
    rooms.iter().all(|room| {
        let blocked = position.0 >= room.rect.x - clearance
            && position.0 <= room.rect.x + room.rect.width + clearance
            && position.1 >= room.rect.y - clearance
            && position.1 <= room.rect.y + room.rect.height + clearance;
        if !blocked {
            return true;
        }
        let approach =
            (room.id == from_room.id
                && geometry_port_approach_contains(position, from_port, clearance))
                || (room.id == to_room.id
                    && geometry_port_approach_contains(position, to_port, clearance));
        if !approach {
            blocking_owners.insert(format!("room:{}", room.source_region));
        }
        approach
    })
}

fn geometry_port_approach_contains(
    position: (i32, i32),
    port: &GeometryRoomPort,
    clearance: i32,
) -> bool {
    let (dx, dy) = direction_vector(port.side.as_str());
    let steps = align_geometry(clearance, GEOMETRY_ROUTE_GRID) / GEOMETRY_ROUTE_GRID + 1;
    (0..=steps).any(|step| {
        position
            == (
                port.point.x + dx * step * GEOMETRY_ROUTE_GRID,
                port.point.y + dy * step * GEOMETRY_ROUTE_GRID,
            )
    })
}

fn reserve_geometry_route(
    path: &[GeometryPoint],
    width: i32,
    section_id: &str,
    reserved: &mut BTreeMap<(i32, i32), String>,
) {
    let required = width / 2
        + GEOMETRY_CORRIDOR_SEPARATION
        + GEOMETRY_MAX_CORRIDOR_HALF_WIDTH;
    let radius = required.saturating_sub(1) / GEOMETRY_ROUTE_GRID;
    for point in path {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() + dy.abs() <= radius {
                    reserved
                        .entry((
                            point.x + dx * GEOMETRY_ROUTE_GRID,
                            point.y + dy * GEOMETRY_ROUTE_GRID,
                        ))
                        .or_insert_with(|| section_id.to_owned());
                }
            }
        }
    }
}

fn compress_geometry_route(path: Vec<GeometryPoint>) -> Vec<GeometryPoint> {
    if path.len() <= 2 {
        return path;
    }
    let mut compressed = vec![path[0].clone()];
    for index in 1..path.len() - 1 {
        let previous = &path[index - 1];
        let current = &path[index];
        let next = &path[index + 1];
        if (current.x - previous.x, current.y - previous.y)
            != (next.x - current.x, next.y - current.y)
        {
            compressed.push(current.clone());
        }
    }
    compressed.push(path[path.len() - 1].clone());
    dedupe_points(compressed)
}

fn rect_center(rect: &GeometryRect) -> GeometryPoint {
    GeometryPoint {
        x: rect.x + rect.width / 2,
        y: rect.y + rect.height / 2,
    }
}

fn dedupe_points(points: Vec<GeometryPoint>) -> Vec<GeometryPoint> {
    let mut deduped = Vec::new();
    for point in points {
        if !deduped
            .last()
            .is_some_and(|last: &GeometryPoint| last.x == point.x && last.y == point.y)
        {
            deduped.push(point);
        }
    }
    deduped
}

fn corridor_width(connector: &IntermediateConnector) -> i32 {
    if connector
        .affordances
        .iter()
        .any(|affordance| affordance == "locked_threshold")
    {
        18
    } else if connector
        .affordances
        .iter()
        .any(|affordance| affordance == "pressure_route")
    {
        20
    } else if connector
        .affordances
        .iter()
        .any(|affordance| affordance == "hidden_passage")
    {
        10
    } else if connector
        .affordances
        .iter()
        .any(|affordance| affordance == "shortcut_link")
    {
        14
    } else {
        12
    }
}

fn corridor_semantic_tags(connector: &IntermediateConnector) -> Vec<String> {
    let mut tags = vec![connector.traversal_hint.clone()];
    tags.extend(connector.intents.clone());
    tags.extend(connector.affordances.clone());
    dedupe_strings(tags)
}

fn room_size_for_region(region: &IntermediateRegion) -> (i32, i32) {
    match (region.scale_band.as_str(), region.footprint_class.as_str()) {
        ("large", "hub") => (152, 112),
        ("large", _) => (144, 104),
        ("medium", "pressure_lane") => (136, 80),
        ("medium", "threshold") | ("medium", "threshold_large") => (112, 80),
        ("medium", _) => (120, 88),
        ("small", "small_pocket") | ("small", "pocket") => (88, 72),
        ("small", "small_marker") => (80, 64),
        ("small", _) => (96, 72),
        _ => match region.role.as_str() {
            "landmark_hub" => (152, 112),
            "reward" => (88, 72),
            "pressure" => (136, 80),
            "gate" | "boss_gate" => (112, 80),
            _ => (120, 88),
        },
    }
}

fn geometry_room_style_tags(region: &IntermediateRegion) -> Vec<String> {
    let mut tags = vec![
        region.role.clone(),
        region.geometry_role.clone(),
        region.scale_band.clone(),
    ];
    tags.extend(region.entrance_expectations.clone());
    dedupe_strings(tags)
}

fn geometry_bounds(rooms: &[GeometryRoom], grid: i32, room_margin: i32) -> GeometryBounds {
    let width = rooms
        .iter()
        .map(|room| room.rect.x + room.rect.width)
        .max()
        .unwrap_or(0)
        + room_margin;
    let height = rooms
        .iter()
        .map(|room| room.rect.y + room.rect.height)
        .max()
        .unwrap_or(0)
        + room_margin;
    GeometryBounds {
        width: width.max(640),
        height: height.max(480),
        grid,
    }
}

fn room_id(region_id: &str) -> String {
    format!("room.{}", slugify_label(region_id))
}
