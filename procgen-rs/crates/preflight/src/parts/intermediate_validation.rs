fn write_intermediate_artifacts(
    candidate: &Candidate,
    run_dir: &Path,
) -> Result<(IntermediateArtifactRefs, ValidationReport), String> {
    let analysis_path = run_dir.join("analysis.graph.json");
    let analysis = analyze_graph(candidate)?;
    write_json(&analysis_path, &analysis)?;

    let compatible_rules_path = run_dir.join("compatible-rules.json");
    let compatible_rules = compatible_rules_report(candidate)?;
    write_json(&compatible_rules_path, &compatible_rules)?;

    let spatial_intent_path = run_dir.join("spatial-intent.json");
    let spatial_intent = spatial_intent_report(candidate, Some(&analysis_path))?;
    write_json(&spatial_intent_path, &spatial_intent)?;

    let intermediate_breakdown_path = run_dir.join("intermediate-breakdown.json");
    let intermediate_breakdown =
        intermediate_breakdown(candidate, &spatial_intent, &spatial_intent_path)?;
    write_json(&intermediate_breakdown_path, &intermediate_breakdown)?;

    let intermediate_validation_path = run_dir.join("intermediate.validation.json");
    let intermediate_validation = validate_intermediate_breakdown(&intermediate_breakdown);
    write_json(&intermediate_validation_path, &intermediate_validation)?;

    Ok((
        IntermediateArtifactRefs {
            analysis_ref: display_path(&analysis_path),
            compatible_rules_ref: display_path(&compatible_rules_path),
            spatial_intent_ref: display_path(&spatial_intent_path),
            intermediate_breakdown_ref: display_path(&intermediate_breakdown_path),
            intermediate_validation_ref: display_path(&intermediate_validation_path),
        },
        intermediate_validation,
    ))
}

fn validate_intermediate_breakdown(breakdown: &IntermediateBreakdown) -> ValidationReport {
    let mut diagnostics = Vec::new();
    let region_ids = breakdown
        .regions
        .iter()
        .map(|region| region.id.as_str())
        .collect::<BTreeSet<_>>();
    if !breakdown
        .regions
        .iter()
        .any(|region| region.role == "start")
    {
        diagnostics.push(fatal(
            "intermediate_start_missing",
            Some("start"),
            None,
            "Intermediate breakdown must contain a start region.",
        ));
    }
    if !breakdown.regions.iter().any(|region| region.role == "goal") {
        diagnostics.push(fatal(
            "intermediate_goal_missing",
            Some("goal"),
            None,
            "Intermediate breakdown must contain a goal region.",
        ));
    }
    for region in &breakdown.regions {
        if region.geometry_role.is_empty()
            || region.footprint_class.is_empty()
            || region.scale_band.is_empty()
            || region.anchor_quality.is_empty()
        {
            diagnostics.push(fatal(
                "intermediate_region_geometry_prep_missing",
                region.node_ids.first().map(String::as_str),
                None,
                "Region is missing geometry-prep role, footprint, scale, or anchor quality.",
            ));
        }
        if !matches!(
            region.scale_band.as_str(),
            "" | "small" | "medium" | "large"
        ) {
            diagnostics.push(fatal(
                "intermediate_region_scale_invalid",
                region.node_ids.first().map(String::as_str),
                None,
                "Region scale band must be small, medium, or large.",
            ));
        }
        if region.role == "landmark_hub" && region.anchor_node.is_none() {
            diagnostics.push(fatal(
                "intermediate_anchor_missing",
                region.node_ids.first().map(String::as_str),
                None,
                "Landmark hub region must declare an anchor node.",
            ));
        }
        if region.role == "landmark_hub" && region.geometry_role != "landmark_junction" {
            diagnostics.push(fatal(
                "intermediate_landmark_geometry_role_missing",
                region.node_ids.first().map(String::as_str),
                None,
                "Landmark hub region must preserve a landmark geometry role.",
            ));
        }
        if region_has_unsupported_3d_claim(region) {
            diagnostics.push(fatal(
                "intermediate_3d_claim_unsupported",
                region.node_ids.first().map(String::as_str),
                None,
                "Region declares vertical or 3D geometry before a geometry-capable schema exists.",
            ));
        }
    }
    for connector in &breakdown.connectors {
        if connector.edge_id.is_empty() {
            diagnostics.push(fatal(
                "intermediate_connector_unbound",
                None,
                Some(connector.id.as_str()),
                "Connector must be bound to a graph edge id.",
            ));
        }
        if !region_ids.contains(connector.from_region.as_str())
            || !region_ids.contains(connector.to_region.as_str())
        {
            diagnostics.push(fatal(
                "intermediate_connector_endpoint_missing",
                None,
                Some(connector.id.as_str()),
                "Connector references a missing source or target region.",
            ));
        }
        if connector.affordances.is_empty() {
            diagnostics.push(fatal(
                "intermediate_connector_affordance_missing",
                None,
                Some(connector.id.as_str()),
                "Connector must declare at least one geometry-prep affordance.",
            ));
        }
        if connector.traversal_hint.is_empty() {
            diagnostics.push(fatal(
                "intermediate_connector_traversal_hint_missing",
                None,
                Some(connector.id.as_str()),
                "Connector must declare a traversal hint.",
            ));
        }
        if connector
            .intents
            .iter()
            .any(|intent| intent == "visible_before_reachable" || intent == "gated_connector")
            && !connector.constraint_refs.iter().any(|reference| {
                reference.contains("preserve_lock_preview")
                    || reference.contains("preserve_gated_connector")
            })
        {
            diagnostics.push(fatal(
                "intermediate_gated_constraint_missing",
                None,
                Some(connector.id.as_str()),
                "Gated connectors must preserve lock preview or gated connector constraints.",
            ));
        }
        if connector
            .intents
            .iter()
            .any(|intent| intent == "hidden_route")
            && !connector
                .affordances
                .iter()
                .any(|affordance| affordance == "hidden_passage")
        {
            diagnostics.push(fatal(
                "intermediate_hidden_affordance_missing",
                None,
                Some(connector.id.as_str()),
                "Hidden routes must declare a hidden_passage affordance.",
            ));
        }
        if connector
            .intents
            .iter()
            .any(|intent| intent == "shortcut_connector")
            && !connector
                .affordances
                .iter()
                .any(|affordance| affordance == "shortcut_link")
        {
            diagnostics.push(fatal(
                "intermediate_shortcut_affordance_missing",
                None,
                Some(connector.id.as_str()),
                "Shortcut connectors must declare a shortcut_link affordance.",
            ));
        }
        if connector
            .intents
            .iter()
            .any(|intent| intent == "one_way_drop")
            && !connector
                .affordances
                .iter()
                .any(|affordance| affordance == "one_way_return")
        {
            diagnostics.push(fatal(
                "intermediate_one_way_affordance_missing",
                None,
                Some(connector.id.as_str()),
                "One-way connectors must declare a one_way_return affordance.",
            ));
        }
        if connector
            .intents
            .iter()
            .any(|intent| intent == "vertical_candidate")
        {
            diagnostics.push(fatal(
                "intermediate_vertical_candidate_unsupported",
                None,
                Some(connector.id.as_str()),
                "Vertical candidates require a later geometry-capable schema.",
            ));
        }
        if connector_has_unsupported_3d_claim(connector) {
            diagnostics.push(fatal(
                "intermediate_3d_claim_unsupported",
                None,
                Some(connector.id.as_str()),
                "Connector declares vertical or 3D geometry before a geometry-capable schema exists.",
            ));
        }
    }
    let fatal_count = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Fatal)
        .count();
    ValidationReport {
        kind: "rusty_procgen.validation.intermediate.v1".to_owned(),
        schema_version: 1,
        state_hash: hash_json(breakdown).unwrap_or_else(|_| "hash_error".to_owned()),
        ok: fatal_count == 0,
        fatal_count,
        diagnostics,
    }
}

fn region_has_unsupported_3d_claim(region: &IntermediateRegion) -> bool {
    [
        region.geometry_role.as_str(),
        region.footprint_class.as_str(),
        region.scale_band.as_str(),
    ]
    .into_iter()
    .chain(region.entrance_expectations.iter().map(String::as_str))
    .any(contains_unsupported_3d_claim)
}

fn connector_has_unsupported_3d_claim(connector: &IntermediateConnector) -> bool {
    connector
        .affordances
        .iter()
        .map(String::as_str)
        .chain([connector.traversal_hint.as_str()])
        .any(contains_unsupported_3d_claim)
}

fn contains_unsupported_3d_claim(value: &str) -> bool {
    value.contains("vertical")
        || value.contains("3d")
        || value.contains("three_d")
        || value.contains("stair")
        || value.contains("shaft")
}

