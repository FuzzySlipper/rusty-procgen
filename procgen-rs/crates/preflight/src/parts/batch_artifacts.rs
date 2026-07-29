#[allow(unused_imports)]
use crate::*;

pub(crate) fn baseline_command(args: BaselineArgs) -> Result<(), String> {
    fs::create_dir_all(&args.out_dir)
        .map_err(|error| format!("failed to create {}: {error}", args.out_dir.display()))?;
    let intent_path = PathBuf::from("fixtures/intents/first-slice.intent.json");
    let transcript = args.out_dir.join("transcript.jsonl");
    if transcript.exists() {
        fs::remove_file(&transcript)
            .map_err(|error| format!("failed to reset {}: {error}", transcript.display()))?;
    }
    let base = args.out_dir.join("candidate-000-base.json");
    let base_receipt = args.out_dir.join("receipt-000-init.json");
    init_candidate(InitArgs {
        intent: intent_path,
        seed: args.seed,
        out: base.clone(),
        receipt: base_receipt,
        transcript: Some(transcript.clone()),
    })?;

    let mut current = base;
    for (index, rule) in [
        GraphRule::LockKeyLoop,
        GraphRule::OptionalTreasureDetour,
        GraphRule::OneWayShortcut,
        GraphRule::SecretBypass,
    ]
    .into_iter()
    .enumerate()
    {
        let next = args
            .out_dir
            .join(format!("candidate-{:03}-{}.json", index + 1, rule.as_str()));
        let receipt_path =
            args.out_dir
                .join(format!("receipt-{:03}-{}.json", index + 1, rule.as_str()));
        apply_rule(ApplyRuleArgs {
            state: current,
            rule,
            seed: args.seed + index as u64 + 1,
            out: next.clone(),
            receipt: receipt_path,
            transcript: Some(transcript.clone()),
        })?;
        current = next;
    }

    let validation = args.out_dir.join("validation.graph.json");
    validate_graph_command(ReportOutArgs {
        state: current.clone(),
        out: validation.clone(),
    })?;
    append_transcript(
        Some(&transcript),
        "validate graph",
        Some(&validation),
        None,
        None,
        json!({ "state": display_path(&current) }),
    )?;
    let score = args.out_dir.join("score.graph.json");
    score_graph_command(ReportOutArgs {
        state: current.clone(),
        out: score.clone(),
    })?;
    append_transcript(
        Some(&transcript),
        "score graph",
        Some(&score),
        None,
        None,
        json!({ "state": display_path(&current) }),
    )?;
    let layout = args.out_dir.join("layout-2d.json");
    let layout_receipt = args.out_dir.join("receipt-005-embed-2d.json");
    embed_2d_command(Embed2dArgs {
        state: current.clone(),
        seed: args.seed + 10,
        out: layout.clone(),
        receipt: layout_receipt,
        transcript: Some(transcript.clone()),
    })?;
    accept_command(AcceptArgs {
        candidate: current,
        layout,
        validation,
        score,
        out: args.out_dir.join("accepted.json"),
        receipt: args.out_dir.join("receipt-006-accept.json"),
        transcript: Some(transcript),
    })?;
    println!("baseline run written to {}", args.out_dir.display());
    Ok(())
}

pub(crate) fn batch_generate_command(args: BatchGenerateArgs) -> Result<(), String> {
    fs::create_dir_all(&args.out_dir)
        .map_err(|error| format!("failed to create {}: {error}", args.out_dir.display()))?;
    let profile_path = args
        .profile
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_BATCH_PROFILE));
    let profile = load_batch_profile(&profile_path)?;
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let mut seen_fingerprints: BTreeMap<String, String> = BTreeMap::new();
    for index in 0..args.count {
        let sequence = batch_profile_sequence(&profile, index)?;
        let candidate_seed = args.seed + index as u64 * 100;
        let run_dir = args.out_dir.join(format!("candidate-{index:03}"));
        if run_dir.exists() {
            fs::remove_dir_all(&run_dir)
                .map_err(|error| format!("failed to reset {}: {error}", run_dir.display()))?;
        }
        fs::create_dir_all(&run_dir)
            .map_err(|error| format!("failed to create {}: {error}", run_dir.display()))?;
        let transcript = run_dir.join("transcript.jsonl");
        if transcript.exists() {
            fs::remove_file(&transcript)
                .map_err(|error| format!("failed to reset {}: {error}", transcript.display()))?;
        }

        let mut current = run_dir.join("candidate-000-base.json");
        init_candidate(InitArgs {
            intent: PathBuf::from("fixtures/intents/first-slice.intent.json"),
            seed: candidate_seed,
            out: current.clone(),
            receipt: run_dir.join("receipt-000-init.json"),
            transcript: Some(transcript.clone()),
        })?;

        for (rule_index, rule) in sequence.rules.iter().copied().enumerate() {
            let next = run_dir.join(format!(
                "candidate-{:03}-{}.json",
                rule_index + 1,
                rule.as_str()
            ));
            apply_rule(ApplyRuleArgs {
                state: current,
                rule,
                seed: candidate_seed + rule_index as u64 + 1,
                out: next.clone(),
                receipt: run_dir.join(format!(
                    "receipt-{:03}-{}.json",
                    rule_index + 1,
                    rule.as_str()
                )),
                transcript: Some(transcript.clone()),
            })?;
            current = next;
        }

        let candidate: Candidate = read_json(&current)?;
        let validation = validate_graph(&candidate);
        let validation_path = run_dir.join("validation.graph.json");
        write_json(&validation_path, &validation)?;
        append_transcript(
            Some(&transcript),
            "validate graph",
            Some(&validation_path),
            None,
            None,
            json!({ "state": display_path(&current) }),
        )?;

        if !validation.ok {
            rejected.push(SelectionRejection {
                candidate_id: candidate.candidate_id,
                profile_sequence: sequence.label.clone(),
                candidate_ref: display_path(&current),
                physical_connection_plan_ref: None,
                diagnostics: validation.diagnostics,
            });
            continue;
        }

        let (intermediate_refs, intermediate_validation) =
            write_intermediate_artifacts(&candidate, &run_dir)?;
        append_transcript(
            Some(&transcript),
            "intermediate artifacts",
            Some(Path::new(&intermediate_refs.intermediate_breakdown_ref)),
            Some(Path::new(&intermediate_refs.intermediate_validation_ref)),
            None,
            json!({
                "analysis": intermediate_refs.analysis_ref,
                "compatibleRules": intermediate_refs.compatible_rules_ref,
                "spatialIntent": intermediate_refs.spatial_intent_ref
            }),
        )?;
        if !intermediate_validation.ok {
            rejected.push(SelectionRejection {
                candidate_id: candidate.candidate_id,
                profile_sequence: sequence.label.clone(),
                candidate_ref: display_path(&current),
                physical_connection_plan_ref: None,
                diagnostics: intermediate_validation.diagnostics,
            });
            continue;
        }

        let score = score_graph(&candidate);
        let topology_fingerprint = topology_fingerprint(&candidate);
        let duplicate_of = seen_fingerprints
            .get(topology_fingerprint.as_str())
            .cloned();
        if duplicate_of.is_none() {
            seen_fingerprints.insert(topology_fingerprint.clone(), candidate.candidate_id.clone());
        }
        let budget_checks = budget_checks(profile.budgets.as_ref(), &score, &candidate);
        let budget_penalty = budget_checks.iter().filter(|check| !check.ok).count() as f64 * 0.05;
        let selection_score = (score.overall - budget_penalty).max(0.0);
        let score_path = run_dir.join("score.graph.json");
        write_json(&score_path, &score)?;
        append_transcript(
            Some(&transcript),
            "score graph",
            Some(&score_path),
            None,
            None,
            json!({ "state": display_path(&current) }),
        )?;

        let layout_path = run_dir.join("layout-2d.json");
        embed_2d_command(Embed2dArgs {
            state: current.clone(),
            seed: candidate_seed + 90,
            out: layout_path.clone(),
            receipt: run_dir.join("receipt-090-embed-2d.json"),
            transcript: Some(transcript.clone()),
        })?;
        let artifact_path = run_dir.join("accepted.json");
        accept_command(AcceptArgs {
            candidate: current.clone(),
            layout: layout_path.clone(),
            validation: validation_path.clone(),
            score: score_path.clone(),
            out: artifact_path.clone(),
            receipt: run_dir.join("receipt-091-accept.json"),
            transcript: Some(transcript),
        })?;

        accepted.push(SelectionEntry {
            candidate_id: candidate.candidate_id.clone(),
            profile_sequence: sequence.label.clone(),
            topology_fingerprint,
            duplicate_of,
            budget_checks,
            budget_penalty,
            selection_score,
            artifact_ref: display_path(&artifact_path),
            validation_ref: display_path(&validation_path),
            score_ref: display_path(&score_path),
            layout_ref: display_path(&layout_path),
            analysis_ref: intermediate_refs.analysis_ref,
            compatible_rules_ref: intermediate_refs.compatible_rules_ref,
            spatial_intent_ref: intermediate_refs.spatial_intent_ref,
            intermediate_breakdown_ref: intermediate_refs.intermediate_breakdown_ref,
            intermediate_validation_ref: intermediate_refs.intermediate_validation_ref,
            physical_connection_plan_ref: None,
            geometry_ref: None,
            geometry_validation_ref: None,
            html_preview_ref: None,
            html_ref: None,
            shape_catalog_ref: None,
            catalog_inspection_ref: None,
            piece_plan_ref: None,
            shape_match_ref: None,
            piece_placement_ref: None,
            piece_placement_validation_ref: None,
            built_flow_validation_ref: None,
            overall: score.overall,
            metrics: score.metrics,
            tags: collect_tags(&candidate),
        });
    }

    accepted.sort_by(|left, right| {
        right
            .selection_score
            .partial_cmp(&left.selection_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    let mut enriched = Vec::new();
    for (index, mut entry) in accepted.into_iter().enumerate() {
        match write_selection_preview_artifacts(&mut entry, args.seed + 9_100 + index as u64) {
            Ok(()) => enriched.push(entry),
            Err(error) => {
                let code = selection_rejection_code(error.as_str());
                rejected.push(SelectionRejection {
                    candidate_id: entry.candidate_id,
                    profile_sequence: entry.profile_sequence,
                    candidate_ref: entry.artifact_ref,
                    physical_connection_plan_ref: entry.physical_connection_plan_ref,
                    diagnostics: vec![fatal(
                        code,
                        None,
                        None,
                        format!(
                            "Physical connection planning or exclusive placement failed: {error}"
                        ),
                    )],
                });
            }
        }
    }
    let accepted = enriched;
    let report = SelectionReport {
        kind: "rusty_procgen.selection_report.v1".to_owned(),
        schema_version: 1,
        batch_id: format!("batch.v2.{}", args.seed),
        profile_id: profile.profile_id,
        profile_ref: display_path(&profile_path),
        seed: args.seed,
        requested_count: args.count,
        generated_count: accepted.len() + rejected.len(),
        accepted,
        rejected,
    };
    write_json(&args.out_dir.join("selection-report.json"), &report)?;
    println!(
        "batch run wrote {} accepted and {} rejected candidate(s) to {}",
        report.accepted.len(),
        report.rejected.len(),
        args.out_dir.display()
    );
    Ok(())
}

pub(crate) fn selection_rejection_code(error: &str) -> &'static str {
    if error.starts_with("geometry search exhausted") {
        "selection_geometry_route_search_exhausted"
    } else if error.starts_with("invalid physical geometry plan")
        || error.contains("unsupported physical section topology")
    {
        "selection_physical_section_plan_incompatible"
    } else if error.contains("port") && error.contains("exhausted") {
        "selection_geometry_port_allocation_exhausted"
    } else if error.starts_with("catalog shape/transform search exhausted")
        || error.starts_with("pure catalog coverage rejected")
    {
        "selection_catalog_shape_transform_exhausted"
    } else if error.starts_with("pure catalog search exhausted") {
        "selection_catalog_piece_search_exhausted"
    } else if error.starts_with("global shape/piece search budget exhausted") {
        "selection_global_search_budget_exhausted"
    } else if error.starts_with("piece realization search exhausted")
        || error.starts_with("piece route search exhausted")
    {
        "selection_piece_route_clearance_exhausted"
    } else {
        "selection_physical_embedding_validation_failed"
    }
}

pub(crate) fn search_shape_realizations<T>(
    catalog: &ShapeCatalog,
    piece_plan: &PieceBuildPlan,
    match_args: &BuildMatchShapesArgs,
    max_alternatives: u32,
    mut realize: impl FnMut(&PieceShapeMatchReport) -> Result<T, String>,
) -> Result<(PieceShapeMatchReport, T), String> {
    let mut last_realization_error = "no shape alternative was attempted".to_owned();
    for alternative_attempt in 0..max_alternatives {
        let shape_match =
            match_shapes_with_attempt(catalog, piece_plan, match_args, alternative_attempt);
        if !shape_match.ok {
            return Err(format!(
                "catalog shape/transform search exhausted with {} unmatched requirement(s) at alternative {alternative_attempt}",
                shape_match.unmatched_count
            ));
        }
        match realize(&shape_match) {
            Ok(realization) => return Ok((shape_match, realization)),
            Err(error) if error.starts_with("piece realization search exhausted") => {
                last_realization_error = error;
            }
            Err(error) => return Err(error),
        }
    }
    Err(format!(
        "global shape/piece search budget exhausted after {max_alternatives} shape/transform alternative(s); last realization failure: {last_realization_error}"
    ))
}

pub(crate) fn write_selection_preview_artifacts(
    entry: &mut SelectionEntry,
    seed: u64,
) -> Result<(), String> {
    let artifact_path = PathBuf::from(&entry.artifact_ref);
    let run_dir = artifact_path
        .parent()
        .ok_or_else(|| {
            format!(
                "accepted artifact {} has no parent directory",
                entry.artifact_ref
            )
        })?
        .to_path_buf();
    let accepted_artifact: AcceptedArtifact = read_json(&artifact_path)?;
    let intermediate_path = PathBuf::from(&entry.intermediate_breakdown_ref);
    let intermediate: IntermediateBreakdown = read_json(&intermediate_path)?;

    let connection_plan_path = run_dir.join("physical-connection-plan.json");
    let geometry_path = run_dir.join("geometry-2d.json");
    let geometry_validation_path = run_dir.join("geometry-2d.validation.json");
    let html_path = run_dir.join("geometry-2d.preview.html");
    let html_preview_path = run_dir.join("html-preview.json");
    let shape_catalog_path = PathBuf::from(DEFAULT_SHAPE_CATALOG);
    let catalog_inspection_path = run_dir.join("shape-catalog.report.json");
    let piece_plan_path = run_dir.join("piece-plan.json");
    let shape_match_path = run_dir.join("piece-shape-match.json");
    let placement_path = run_dir.join("piece-placement.json");
    let placement_validation_path = run_dir.join("piece-placement.validation.json");
    let built_flow_validation_path = run_dir.join("built-flow.validation.json");

    let connection_plan_args = PhysicalConnectionPlanArgs {
        candidate: artifact_path.clone(),
        intermediate: intermediate_path.clone(),
        out: connection_plan_path.clone(),
    };
    let connection_plan = plan_physical_connections(
        &accepted_artifact.candidate,
        &intermediate,
        &connection_plan_args,
    )?;
    write_json(&connection_plan_path, &connection_plan)?;
    entry.physical_connection_plan_ref = Some(display_path(&connection_plan_path));

    let geometry_args = GeometryEmit2dArgs {
        candidate: artifact_path.clone(),
        intermediate: intermediate_path.clone(),
        connection_plan: connection_plan_path.clone(),
        layout_policy: Some(PathBuf::from(
            "fixtures/geometry-layout-policies/compact-first-v1.json",
        )),
        seed,
        out: geometry_path.clone(),
    };
    let geometry = emit_geometry_2d(
        &accepted_artifact.candidate,
        &intermediate,
        &connection_plan,
        &geometry_args,
        seed,
    )?;
    write_json(&geometry_path, &geometry)?;

    let geometry_validation = validate_geometry_2d(&geometry);
    write_json(&geometry_validation_path, &geometry_validation)?;
    if !geometry_validation.ok {
        return Err(format!(
            "selection {} geometry validation failed with {} fatal diagnostic(s)",
            entry.candidate_id, geometry_validation.fatal_count
        ));
    }

    let html = render_geometry_preview_html(
        &geometry,
        &geometry_validation,
        &display_path(&geometry_path),
        &display_path(&geometry_validation_path),
    );
    write_text(&html_path, &html)?;

    let preview = HtmlPreviewArtifact {
        kind: "rusty_procgen.html_preview.v1".to_owned(),
        schema_version: 1,
        preview_id: format!("preview.{}", geometry.geometry_id),
        geometry_ref: display_path(&geometry_path),
        validation_ref: display_path(&geometry_validation_path),
        html_ref: display_path(&html_path),
        screenshot_hint: None,
    };
    write_json(&html_preview_path, &preview)?;

    entry.geometry_ref = Some(preview.geometry_ref.clone());
    entry.geometry_validation_ref = Some(preview.validation_ref.clone());
    entry.html_preview_ref = Some(display_path(&html_preview_path));
    entry.html_ref = Some(preview.html_ref.clone());

    let catalog: ShapeCatalog = read_json(&shape_catalog_path)?;
    let catalog_report = inspect_shape_catalog(&catalog, &shape_catalog_path);
    if catalog_report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Fatal)
    {
        return Err(format!(
            "shape catalog inspection failed with {} diagnostic(s)",
            catalog_report.diagnostics.len()
        ));
    }
    write_json(&catalog_inspection_path, &catalog_report)?;

    let piece_plan_args = BuildEmitPiecePlanArgs {
        candidate: artifact_path.clone(),
        intermediate: intermediate_path,
        geometry: geometry_path.clone(),
        corridor_realization: CorridorRealization::Hybrid,
        out: piece_plan_path.clone(),
    };
    let piece_plan = emit_piece_build_plan(
        &accepted_artifact.candidate,
        &intermediate,
        &geometry,
        &piece_plan_args,
    )?;
    write_json(&piece_plan_path, &piece_plan)?;

    let match_args = BuildMatchShapesArgs {
        catalog: shape_catalog_path.clone(),
        piece_plan: piece_plan_path.clone(),
        seed: seed + 31,
        out: shape_match_path.clone(),
    };
    let assemble_args = BuildAssembleArgs {
        catalog: shape_catalog_path.clone(),
        piece_plan: piece_plan_path.clone(),
        shape_match: shape_match_path.clone(),
        connectivity: GridConnectivity::FourWay,
        out: placement_path.clone(),
    };
    const SHAPE_ALTERNATIVE_ATTEMPTS: u32 = 4;
    let (shape_match, placement) = search_shape_realizations(
        &catalog,
        &piece_plan,
        &match_args,
        SHAPE_ALTERNATIVE_ATTEMPTS,
        |shape_match| assemble_piece_placement(&catalog, &piece_plan, shape_match, &assemble_args),
    )?;
    write_json(&shape_match_path, &shape_match)?;
    write_json(&placement_path, &placement)?;

    let placement_validation = validate_piece_placement(&placement);
    write_json(&placement_validation_path, &placement_validation)?;
    if !placement_validation.ok {
        return Err(format!(
            "selection {} piece placement validation failed with {} fatal diagnostic(s)",
            entry.candidate_id, placement_validation.fatal_count
        ));
    }

    let flow_args = BuildValidateFlowArgs {
        candidate: artifact_path.clone(),
        geometry: geometry_path.clone(),
        piece_plan: piece_plan_path.clone(),
        piece_placement: placement_path.clone(),
        out: built_flow_validation_path.clone(),
    };
    let built_flow_validation = validate_built_flow(
        &accepted_artifact.candidate,
        &geometry,
        &piece_plan,
        &placement,
        &flow_args,
    );
    write_json(&built_flow_validation_path, &built_flow_validation)?;
    if !built_flow_validation.ok {
        return Err(format!(
            "selection {} built flow validation failed with {} fatal diagnostic(s)",
            entry.candidate_id, built_flow_validation.fatal_count
        ));
    }

    entry.shape_catalog_ref = Some(display_path(&shape_catalog_path));
    entry.catalog_inspection_ref = Some(display_path(&catalog_inspection_path));
    entry.piece_plan_ref = Some(display_path(&piece_plan_path));
    entry.shape_match_ref = Some(display_path(&shape_match_path));
    entry.piece_placement_ref = Some(display_path(&placement_path));
    entry.piece_placement_validation_ref = Some(display_path(&placement_validation_path));
    entry.built_flow_validation_ref = Some(display_path(&built_flow_validation_path));

    let transcript = run_dir.join("transcript.jsonl");
    append_transcript(
        Some(&transcript),
        "geometry and piece build preview",
        Some(&piece_plan_path),
        Some(&placement_validation_path),
        Some(seed),
        json!({
            "geometry": preview.geometry_ref,
            "validation": preview.validation_ref,
            "html": preview.html_ref,
            "catalog": display_path(&shape_catalog_path),
            "catalogInspection": display_path(&catalog_inspection_path),
            "shapeMatch": display_path(&shape_match_path),
            "placement": display_path(&placement_path)
            ,"builtFlowValidation": display_path(&built_flow_validation_path)
        }),
    )?;
    Ok(())
}

pub(crate) fn load_batch_profile(path: &Path) -> Result<BatchProfile, String> {
    let profile: BatchProfile = read_json(path)?;
    if profile.kind != "rusty_procgen.batch_profile.v1" {
        return Err(format!(
            "batch profile {} has unsupported kind {}",
            path.display(),
            profile.kind
        ));
    }
    if profile.sequences.is_empty() {
        return Err(format!(
            "batch profile {} must contain at least one sequence",
            path.display()
        ));
    }
    for sequence in &profile.sequences {
        if sequence.rules.is_empty() {
            return Err(format!(
                "batch profile {} sequence {} has no rules",
                path.display(),
                sequence.label
            ));
        }
    }
    Ok(profile)
}

pub(crate) fn batch_profile_sequence(
    profile: &BatchProfile,
    index: usize,
) -> Result<&BatchProfileSequence, String> {
    profile
        .sequences
        .get(index % profile.sequences.len())
        .ok_or_else(|| "batch profile has no sequences".to_owned())
}

pub(crate) fn topology_fingerprint(candidate: &Candidate) -> String {
    let mut lines = Vec::new();
    for node in &candidate.graph.nodes {
        let mut tags = node.tags.clone();
        tags.sort();
        let incoming = candidate
            .graph
            .edges
            .iter()
            .filter(|edge| edge.to == node.id)
            .count();
        let outgoing = candidate
            .graph
            .edges
            .iter()
            .filter(|edge| edge.from == node.id)
            .count();
        lines.push(format!(
            "node:{}:{incoming}:{outgoing}:{}",
            node.kind.as_str(),
            tags.join(",")
        ));
    }
    for edge in &candidate.graph.edges {
        let mut tags = edge.tags.clone();
        tags.sort();
        lines.push(format!(
            "edge:{:?}:{:?}:required={}:{}",
            edge.kind,
            edge.traversal,
            edge.required_item.is_some(),
            tags.join(",")
        ));
    }
    lines.sort();
    format!("topology:{:016x}", fnv1a64(lines.join("\n").as_bytes()))
}

pub(crate) fn budget_checks(
    budgets: Option<&IntentBudget>,
    score: &ScoreReport,
    candidate: &Candidate,
) -> Vec<BudgetCheck> {
    let Some(budgets) = budgets else {
        return Vec::new();
    };
    let mut checks = Vec::new();
    if let Some(max_locked_edges) = budgets.max_locked_edges {
        let actual = metric_usize(score, "lockedEdgeCount");
        checks.push(BudgetCheck {
            code: "max_locked_edges".to_owned(),
            ok: actual <= max_locked_edges,
            detail: format!("locked edges {actual} <= budget {max_locked_edges}"),
        });
    }
    if let Some(min_optional_branches) = budgets.min_optional_branches {
        let actual = metric_usize(score, "optionalBranchCount");
        checks.push(BudgetCheck {
            code: "min_optional_branches".to_owned(),
            ok: actual >= min_optional_branches,
            detail: format!("optional branches {actual} >= budget {min_optional_branches}"),
        });
    }
    if budgets.require_hub.unwrap_or(false) {
        let actual = metric_usize(score, "hubCount");
        checks.push(BudgetCheck {
            code: "require_hub".to_owned(),
            ok: actual > 0,
            detail: format!("hub count {actual} > 0"),
        });
    }
    if budgets.require_boss.unwrap_or(false) {
        let actual = metric_usize(score, "bossCount");
        checks.push(BudgetCheck {
            code: "require_boss".to_owned(),
            ok: actual > 0,
            detail: format!("boss count {actual} > 0"),
        });
    }
    if let Some(max_dead_ends) = budgets.max_dead_ends {
        let actual = dead_end_count(candidate);
        checks.push(BudgetCheck {
            code: "max_dead_ends".to_owned(),
            ok: actual <= max_dead_ends,
            detail: format!("dead ends {actual} <= budget {max_dead_ends}"),
        });
    }
    checks
}

pub(crate) fn metric_usize(score: &ScoreReport, metric: &str) -> usize {
    score.metrics.get(metric).copied().unwrap_or(0.0) as usize
}

pub(crate) fn collect_tags(candidate: &Candidate) -> Vec<String> {
    let mut tags = BTreeSet::new();
    for node in &candidate.graph.nodes {
        for tag in &node.tags {
            tags.insert(tag.clone());
        }
    }
    for edge in &candidate.graph.edges {
        for tag in &edge.tags {
            tags.insert(tag.clone());
        }
    }
    tags.into_iter().collect()
}
