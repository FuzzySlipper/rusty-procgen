fn annotate_spatial_intent_command(args: AnnotateSpatialIntentArgs) -> Result<(), String> {
    let candidate: Candidate = read_json(&args.state)?;
    if let Some(analysis) = args.analysis.as_deref() {
        let _: GraphAnalysisReport = read_json(analysis)?;
    }
    let report = spatial_intent_report(&candidate, args.analysis.as_deref())?;
    write_json(&args.out, &report)
}

fn spatial_intent_report(
    candidate: &Candidate,
    analysis_ref: Option<&Path>,
) -> Result<SpatialIntentReport, String> {
    let mut annotations = Vec::new();
    for node in &candidate.graph.nodes {
        let mut intents = Vec::new();
        if node.kind == NodeKind::Start {
            intents.push("entry_orientation".to_owned());
        }
        if node.kind == NodeKind::Goal {
            intents.push("destination_readability".to_owned());
        }
        if node_has_tag(node, "hub") || node_has_tag(node, "wayfinding_anchor") {
            intents.push("landmark_hub".to_owned());
        }
        if node_has_tag(node, "boss") {
            intents.push("gated_reveal".to_owned());
        }
        if node_has_tag(node, "hazard") {
            intents.push("pressure_path".to_owned());
        }
        if node_has_tag(node, "lock") || node.kind == NodeKind::Gate {
            intents.push("gated_reveal".to_owned());
        }
        if node_has_tag(node, "reward") {
            intents.push("reward_pocket".to_owned());
        }
        let intents = dedupe_strings(intents);
        if !intents.is_empty() {
            annotations.push(SpatialIntentAnnotation {
                target_type: "node".to_owned(),
                target_id: node.id.clone(),
                rationale: format!("Node {} carries spatial role tags.", node.id),
                intents,
            });
        }
    }
    for edge in &candidate.graph.edges {
        let mut intents = Vec::new();
        if edge.traversal == TraversalKind::Locked || edge.required_item.is_some() {
            intents.push("visible_before_reachable".to_owned());
            intents.push("gated_connector".to_owned());
        }
        if edge_has_tag(edge, "pressure") {
            intents.push("pressure_path".to_owned());
        }
        if edge.kind == EdgeKind::Shortcut || edge_has_tag(edge, "shortcut") {
            intents.push("shortcut_connector".to_owned());
        }
        if edge.traversal == TraversalKind::OneWayReturn {
            intents.push("one_way_drop".to_owned());
        }
        if edge.traversal == TraversalKind::Hidden || edge_has_tag(edge, "hidden") {
            intents.push("hidden_route".to_owned());
        }
        if edge_has_tag(edge, "return") || edge_has_tag(edge, "rejoin") {
            intents.push("merge_rejoin_clarity".to_owned());
        }
        if edge.kind == EdgeKind::SecretBypass {
            intents.push("hidden_route".to_owned());
        }
        let intents = dedupe_strings(intents);
        if !intents.is_empty() {
            annotations.push(SpatialIntentAnnotation {
                target_type: "edge".to_owned(),
                target_id: edge.id.clone(),
                rationale: format!("Edge {} has traversal or topology intent.", edge.id),
                intents,
            });
        }
    }
    Ok(SpatialIntentReport {
        kind: "rusty_procgen.spatial_intent.v1".to_owned(),
        schema_version: 1,
        candidate_id: candidate.candidate_id.clone(),
        state_hash: hash_json(candidate)?,
        analysis_ref: analysis_ref.map(display_path),
        annotations,
    })
}

fn breakdown_emit_command(args: BreakdownEmitArgs) -> Result<(), String> {
    let candidate: Candidate = read_json(&args.state)?;
    let annotations: SpatialIntentReport = read_json(&args.annotations)?;
    let breakdown = intermediate_breakdown(&candidate, &annotations, &args.annotations)?;
    write_json(&args.out, &breakdown)
}

fn intermediate_breakdown(
    candidate: &Candidate,
    annotations: &SpatialIntentReport,
    annotation_path: &Path,
) -> Result<IntermediateBreakdown, String> {
    let mut annotations_by_target: BTreeMap<&str, Vec<&SpatialIntentAnnotation>> = BTreeMap::new();
    for annotation in &annotations.annotations {
        annotations_by_target
            .entry(annotation.target_id.as_str())
            .or_default()
            .push(annotation);
    }
    let regions = candidate
        .graph
        .nodes
        .iter()
        .map(|node| {
            let intents = annotations_by_target
                .get(node.id.as_str())
                .into_iter()
                .flat_map(|items| items.iter())
                .flat_map(|annotation| annotation.intents.iter().map(String::as_str))
                .collect::<BTreeSet<_>>();
            let role = region_role(node, &intents);
            let anchor_node = if matches!(
                role.as_str(),
                "start" | "goal" | "landmark_hub" | "boss_gate"
            ) {
                Some(node.id.clone())
            } else {
                None
            };
            let incoming_count = candidate
                .graph
                .edges
                .iter()
                .filter(|edge| edge.to == node.id)
                .count();
            let outgoing_count = candidate
                .graph
                .edges
                .iter()
                .filter(|edge| edge.from == node.id)
                .count();
            IntermediateRegion {
                id: region_id(node.id.as_str()),
                node_ids: vec![node.id.clone()],
                geometry_role: geometry_role(node, role.as_str(), &intents),
                footprint_class: footprint_class(node, role.as_str(), &intents),
                scale_band: scale_band(node, role.as_str(), &intents),
                anchor_quality: anchor_quality(role.as_str(), anchor_node.as_deref()),
                entrance_expectations: entrance_expectations(
                    node,
                    role.as_str(),
                    &intents,
                    incoming_count,
                    outgoing_count,
                ),
                role,
                anchor_node,
            }
        })
        .collect::<Vec<_>>();
    let constraints = annotations
        .annotations
        .iter()
        .flat_map(|annotation| {
            annotation
                .intents
                .iter()
                .filter_map(|intent| constraint_for_intent(annotation, intent))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let constraint_refs_by_target = constraint_refs_by_target(&constraints);
    let connectors = candidate
        .graph
        .edges
        .iter()
        .map(|edge| {
            let intents = annotations_by_target
                .get(edge.id.as_str())
                .into_iter()
                .flat_map(|items| items.iter())
                .flat_map(|annotation| annotation.intents.clone())
                .collect::<Vec<_>>();
            let intents = dedupe_strings(intents);
            IntermediateConnector {
                id: format!("connector.{}", slugify_label(edge.id.as_str())),
                edge_id: edge.id.clone(),
                from_region: region_id(edge.from.as_str()),
                to_region: region_id(edge.to.as_str()),
                affordances: connector_affordances(edge, &intents),
                traversal_hint: traversal_hint(edge),
                constraint_refs: constraint_refs_by_target
                    .get(edge.id.as_str())
                    .cloned()
                    .unwrap_or_default(),
                intents,
            }
        })
        .collect::<Vec<_>>();
    Ok(IntermediateBreakdown {
        kind: "rusty_procgen.intermediate_breakdown.v1".to_owned(),
        schema_version: 2,
        candidate_id: candidate.candidate_id.clone(),
        state_hash: hash_json(candidate)?,
        annotation_ref: display_path(annotation_path),
        regions,
        connectors,
        constraints,
    })
}

fn region_role(node: &Node, intents: &BTreeSet<&str>) -> String {
    if node.kind == NodeKind::Start {
        "start".to_owned()
    } else if node.kind == NodeKind::Goal {
        "goal".to_owned()
    } else if intents.contains("landmark_hub") {
        "landmark_hub".to_owned()
    } else if node_has_tag(node, "boss") {
        "boss_gate".to_owned()
    } else if node_has_tag(node, "hazard") || intents.contains("pressure_path") {
        "pressure".to_owned()
    } else if node_has_tag(node, "reward") {
        "reward".to_owned()
    } else if node.kind == NodeKind::Gate {
        "gate".to_owned()
    } else {
        "standard".to_owned()
    }
}

fn geometry_role(node: &Node, role: &str, intents: &BTreeSet<&str>) -> String {
    match role {
        "start" => "entry".to_owned(),
        "goal" => "destination".to_owned(),
        "landmark_hub" => "landmark_junction".to_owned(),
        "boss_gate" => "boss_threshold".to_owned(),
        "pressure" => "hazard_route".to_owned(),
        "reward" => "reward_pocket".to_owned(),
        "gate" => "threshold".to_owned(),
        _ if node.kind == NodeKind::Key => "key_pocket".to_owned(),
        _ if node.kind == NodeKind::Shortcut => "shortcut_marker".to_owned(),
        _ if node.kind == NodeKind::Secret => "secret_pocket".to_owned(),
        _ if intents.contains("gated_reveal") => "threshold".to_owned(),
        _ => "chamber".to_owned(),
    }
}

fn footprint_class(node: &Node, role: &str, intents: &BTreeSet<&str>) -> String {
    match role {
        "landmark_hub" => "hub".to_owned(),
        "boss_gate" => "threshold_large".to_owned(),
        "pressure" => "pressure_lane".to_owned(),
        "reward" => "pocket".to_owned(),
        "gate" => "threshold".to_owned(),
        "start" | "goal" => "marker_room".to_owned(),
        _ if node.kind == NodeKind::Key || node.kind == NodeKind::Resource => {
            "small_pocket".to_owned()
        }
        _ if node.kind == NodeKind::Shortcut || node.kind == NodeKind::Secret => {
            "small_marker".to_owned()
        }
        _ if intents.contains("landmark_hub") => "hub".to_owned(),
        _ => "standard_room".to_owned(),
    }
}

fn scale_band(node: &Node, role: &str, intents: &BTreeSet<&str>) -> String {
    match role {
        "landmark_hub" | "boss_gate" => "large".to_owned(),
        "pressure" | "start" | "goal" | "gate" => "medium".to_owned(),
        "reward" => "small".to_owned(),
        _ if node.kind == NodeKind::Key
            || node.kind == NodeKind::Resource
            || node.kind == NodeKind::Shortcut
            || node.kind == NodeKind::Secret =>
        {
            "small".to_owned()
        }
        _ if intents.contains("landmark_hub") => "large".to_owned(),
        _ => "medium".to_owned(),
    }
}

fn anchor_quality(role: &str, anchor_node: Option<&str>) -> String {
    if anchor_node.is_none() {
        "derived".to_owned()
    } else if matches!(role, "start" | "goal" | "landmark_hub" | "boss_gate") {
        "explicit".to_owned()
    } else {
        "derived_anchor".to_owned()
    }
}

fn entrance_expectations(
    node: &Node,
    role: &str,
    intents: &BTreeSet<&str>,
    incoming_count: usize,
    outgoing_count: usize,
) -> Vec<String> {
    let mut expectations = Vec::new();
    match role {
        "start" => expectations.push("entry_spawn".to_owned()),
        "goal" => expectations.push("destination_arrival".to_owned()),
        "landmark_hub" => expectations.push("multi_spoke_orientation".to_owned()),
        "boss_gate" => expectations.push("approach_then_reveal".to_owned()),
        "pressure" => expectations.push("readable_hazard_approach".to_owned()),
        "reward" => expectations.push("optional_reward_access".to_owned()),
        "gate" => expectations.push("locked_threshold_preview".to_owned()),
        _ => {}
    }
    if node.kind == NodeKind::Key || node.kind == NodeKind::Resource {
        expectations.push("pickup_pocket".to_owned());
    }
    if intents.contains("gated_reveal") {
        expectations.push("reveal_line".to_owned());
    }
    if incoming_count > 1 {
        expectations.push("merge_readability".to_owned());
    }
    if outgoing_count > 1 {
        expectations.push("choice_readability".to_owned());
    }
    if expectations.is_empty() {
        expectations.push("standard_passage".to_owned());
    }
    dedupe_strings(expectations)
}

fn constraint_for_intent(
    annotation: &SpatialIntentAnnotation,
    intent: &str,
) -> Option<IntermediateConstraint> {
    let code = constraint_code_for_intent(intent)?;
    Some(IntermediateConstraint {
        code: code.to_owned(),
        target: annotation.target_id.clone(),
        target_type: annotation.target_type.clone(),
        source_intents: vec![intent.to_owned()],
        graph_refs: vec![annotation.target_id.clone()],
        detail: format!("Preserve {intent} for {}.", annotation.target_id),
    })
}

fn constraint_code_for_intent(intent: &str) -> Option<&'static str> {
    Some(match intent {
        "visible_before_reachable" => "preserve_lock_preview",
        "gated_connector" => "preserve_gated_connector",
        "gated_reveal" => "preserve_reveal_sequence",
        "landmark_hub" => "preserve_wayfinding_anchor",
        "pressure_path" => "preserve_pressure_read",
        "shortcut_connector" => "preserve_shortcut_connector",
        "one_way_drop" => "preserve_one_way_return",
        "hidden_route" => "preserve_hidden_route",
        "merge_rejoin_clarity" => "preserve_rejoin_clarity",
        "reward_pocket" => "preserve_reward_pocket",
        "entry_orientation" => "preserve_entry_orientation",
        "destination_readability" => "preserve_destination_readability",
        _ => return None,
    })
}

fn constraint_refs_by_target(
    constraints: &[IntermediateConstraint],
) -> BTreeMap<&str, Vec<String>> {
    let mut refs: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for constraint in constraints {
        refs.entry(constraint.target.as_str())
            .or_default()
            .push(format!("{}:{}", constraint.code, constraint.target));
    }
    for values in refs.values_mut() {
        values.sort();
        values.dedup();
    }
    refs
}

fn connector_affordances(edge: &Edge, intents: &[String]) -> Vec<String> {
    let mut affordances = Vec::new();
    if edge.traversal == TraversalKind::Locked || edge.required_item.is_some() {
        affordances.push("locked_threshold".to_owned());
        affordances.push("preview_line".to_owned());
    }
    if edge.traversal == TraversalKind::Hidden
        || intents.iter().any(|intent| intent == "hidden_route")
    {
        affordances.push("hidden_passage".to_owned());
    }
    if edge.traversal == TraversalKind::OneWayReturn
        || intents.iter().any(|intent| intent == "one_way_drop")
    {
        affordances.push("one_way_return".to_owned());
    }
    if edge.kind == EdgeKind::Shortcut
        || intents.iter().any(|intent| intent == "shortcut_connector")
    {
        affordances.push("shortcut_link".to_owned());
    }
    if intents.iter().any(|intent| intent == "pressure_path") {
        affordances.push("pressure_route".to_owned());
    }
    if intents
        .iter()
        .any(|intent| intent == "merge_rejoin_clarity")
    {
        affordances.push("rejoin_corridor".to_owned());
    }
    if affordances.is_empty() {
        affordances.push("corridor".to_owned());
    }
    dedupe_strings(affordances)
}

fn traversal_hint(edge: &Edge) -> String {
    match edge.traversal {
        TraversalKind::Open => {
            if edge.required_item.is_some() {
                "open_requires_context".to_owned()
            } else {
                "open".to_owned()
            }
        }
        TraversalKind::Locked => "locked".to_owned(),
        TraversalKind::OneWayReturn => "one_way_return".to_owned(),
        TraversalKind::Hidden => "hidden".to_owned(),
    }
}

fn breakdown_validate_command(args: ReportOutArgs) -> Result<(), String> {
    let breakdown: IntermediateBreakdown = read_json(&args.state)?;
    let report = validate_intermediate_breakdown(&breakdown);
    write_json(&args.out, &report)?;
    if report.ok {
        Ok(())
    } else {
        Err(format!(
            "intermediate breakdown validation failed with {} fatal diagnostic(s); see {}",
            report.fatal_count,
            args.out.display()
        ))
    }
}
