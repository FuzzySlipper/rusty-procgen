fn summarize_candidate(args: SummarizeArgs) -> Result<(), String> {
    let candidate: Candidate = read_json(&args.state)?;
    let report = validate_graph(&candidate);
    let score = score_graph(&candidate);
    if args.json || args.out.is_some() {
        let summary = graph_summary_report(&candidate, &report, &score)?;
        if let Some(out) = args.out {
            write_json(&out, &summary)?;
        } else {
            let text = serde_json::to_string_pretty(&summary)
                .map_err(|error| format!("failed to encode graph summary: {error}"))?;
            println!("{text}");
        }
        return Ok(());
    }
    println!("candidate: {}", candidate.candidate_id);
    println!("nodes: {}", candidate.graph.nodes.len());
    println!("edges: {}", candidate.graph.edges.len());
    println!("valid: {}", report.ok);
    println!("overall score: {:.2}", score.overall);
    for node in &candidate.graph.nodes {
        println!("- node {} ({}) {}", node.id, node.kind.as_str(), node.label);
    }
    Ok(())
}

fn graph_summary_report(
    candidate: &Candidate,
    validation: &ValidationReport,
    score: &ScoreReport,
) -> Result<GraphSummaryReport, String> {
    let mut locked_items = BTreeSet::new();
    for edge in &candidate.graph.edges {
        if let Some(item) = edge.required_item.as_deref() {
            locked_items.insert(item.to_owned());
        }
    }
    let dead_ends = candidate
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind != NodeKind::Goal)
        .filter(|node| {
            !candidate
                .graph
                .edges
                .iter()
                .any(|edge| edge.from == node.id)
        })
        .map(|node| node.id.clone())
        .collect();
    let provenance_tail = candidate
        .provenance
        .iter()
        .rev()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Ok(GraphSummaryReport {
        kind: "rusty_procgen.graph_summary.v1".to_owned(),
        schema_version: 1,
        candidate_id: candidate.candidate_id.clone(),
        state_hash: hash_json(candidate)?,
        validation_ok: validation.ok,
        fatal_count: validation.fatal_count,
        score_overall: score.overall,
        metrics: score.metrics.clone(),
        node_count: candidate.graph.nodes.len(),
        edge_count: candidate.graph.edges.len(),
        tags: collect_tags(candidate),
        locked_items: locked_items.into_iter().collect(),
        dead_ends,
        provenance_tail,
        nodes: candidate
            .graph
            .nodes
            .iter()
            .map(|node| NodeSummary {
                id: node.id.clone(),
                kind: node.kind,
                tags: node.tags.clone(),
                grants_item: node.grants_item.clone(),
            })
            .collect(),
        edges: candidate
            .graph
            .edges
            .iter()
            .map(|edge| EdgeSummary {
                id: edge.id.clone(),
                from: edge.from.clone(),
                to: edge.to.clone(),
                kind: edge.kind,
                traversal: edge.traversal,
                required_item: edge.required_item.clone(),
                tags: edge.tags.clone(),
            })
            .collect(),
    })
}

fn analyze_graph_command(args: ReportOutArgs) -> Result<(), String> {
    let candidate: Candidate = read_json(&args.state)?;
    let report = analyze_graph(&candidate)?;
    write_json(&args.out, &report)
}

fn analyze_graph(candidate: &Candidate) -> Result<GraphAnalysisReport, String> {
    let critical_path = shortest_path_nodes(candidate, "start", "goal").unwrap_or_default();
    let dominators = dominator_nodes(candidate);
    let optional_branches = candidate
        .graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::OptionalBranch || edge.kind == EdgeKind::SecretBypass)
        .map(|edge| BranchAnalysis {
            edge_id: edge.id.clone(),
            from: edge.from.clone(),
            to: edge.to.clone(),
            classification: if edge_has_tag(edge, "pressure") {
                "pressure".to_owned()
            } else if edge_has_tag(edge, "rejoin") || edge_has_tag(edge, "return") {
                "rejoin".to_owned()
            } else {
                "optional".to_owned()
            },
            rejoins_goal_route: path_exists(candidate, edge.to.as_str(), "goal"),
        })
        .collect();
    let lock_key_order = candidate
        .graph
        .edges
        .iter()
        .filter_map(|edge| {
            let required_item = edge.required_item.clone()?;
            let provider = candidate
                .graph
                .nodes
                .iter()
                .find(|node| node.grants_item.as_deref() == Some(required_item.as_str()));
            let provider_node = provider.map(|node| node.id.clone());
            let provider_reachable_before_lock = provider.is_some_and(|node| {
                shortest_path_len(candidate, "start", node.id.as_str())
                    .zip(shortest_path_len(candidate, "start", edge.from.as_str()))
                    .is_some_and(|(provider_depth, lock_depth)| provider_depth <= lock_depth + 2)
            });
            Some(LockKeyAnalysis {
                edge_id: edge.id.clone(),
                required_item,
                provider_node,
                provider_reachable_before_lock,
            })
        })
        .collect();
    let loop_signals = candidate
        .graph
        .edges
        .iter()
        .filter(|edge| {
            edge_has_tag(edge, "return")
                || edge_has_tag(edge, "rejoin")
                || edge.kind == EdgeKind::Shortcut
        })
        .map(|edge| LoopSignal {
            edge_id: edge.id.clone(),
            signal: if edge.kind == EdgeKind::Shortcut {
                "shortcut_loop".to_owned()
            } else {
                "rejoin_loop".to_owned()
            },
            detail: format!("{} reconnects {} to {}", edge.id, edge.from, edge.to),
        })
        .collect();
    let locked_count = candidate
        .graph
        .edges
        .iter()
        .filter(|edge| edge.traversal == TraversalKind::Locked)
        .count();
    let shortcut_bypass_risks = candidate
        .graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Shortcut || edge.kind == EdgeKind::SecretBypass)
        .map(|edge| ShortcutRisk {
            edge_id: edge.id.clone(),
            risk: if locked_count > 0 && path_exists(candidate, edge.to.as_str(), "goal") {
                "may_bypass_lock".to_owned()
            } else {
                "low".to_owned()
            },
            detail: format!(
                "{} can compress route from {} to {}",
                edge.id, edge.from, edge.to
            ),
        })
        .collect();
    Ok(GraphAnalysisReport {
        kind: "rusty_procgen.graph_analysis.v1".to_owned(),
        schema_version: 1,
        candidate_id: candidate.candidate_id.clone(),
        state_hash: hash_json(candidate)?,
        critical_path,
        dominators,
        optional_branches,
        lock_key_order,
        loop_signals,
        shortcut_bypass_risks,
    })
}

fn compatible_rules_command(args: ReportOutArgs) -> Result<(), String> {
    let candidate: Candidate = read_json(&args.state)?;
    let report = compatible_rules_report(&candidate)?;
    write_json(&args.out, &report)
}

fn compatible_rules_report(candidate: &Candidate) -> Result<RuleCompatibilityReport, String> {
    Ok(RuleCompatibilityReport {
        kind: "rusty_procgen.rule_compatibility.v1".to_owned(),
        schema_version: 1,
        candidate_id: candidate.candidate_id.clone(),
        state_hash: hash_json(candidate)?,
        rules: all_graph_rules()
            .into_iter()
            .map(|rule| rule_compatibility(candidate, rule))
            .collect(),
    })
}

fn all_graph_rules() -> Vec<GraphRule> {
    vec![
        GraphRule::LockKeyLoop,
        GraphRule::OptionalTreasureDetour,
        GraphRule::OneWayShortcut,
        GraphRule::SecretBypass,
        GraphRule::HubSpokeCluster,
        GraphRule::NestedLockKeyChain,
        GraphRule::HazardResourceTradeoff,
        GraphRule::BossPreparationLoop,
        GraphRule::GatedTreasureBranch,
        GraphRule::BranchMergeShortcut,
    ]
}

fn rule_compatibility(candidate: &Candidate, rule: GraphRule) -> RuleCompatibility {
    let mut status = "applicable".to_owned();
    let mut reasons = Vec::new();
    let mut recommended_actions = Vec::new();
    let duplicate = match rule {
        GraphRule::LockKeyLoop => has_node(candidate, "gate.locked_1"),
        GraphRule::OneWayShortcut => {
            has_edge(candidate, "edge.goal.start.shortcut")
                || has_edge(candidate, "edge.goal.shortcut_1")
                || has_edge(candidate, "edge.shortcut_1.start")
        }
        GraphRule::SecretBypass => has_edge(candidate, "edge.start.goal.secret"),
        GraphRule::HubSpokeCluster => has_node(candidate, "hub.central_1"),
        GraphRule::NestedLockKeyChain => has_node(candidate, "gate.locked_2"),
        GraphRule::HazardResourceTradeoff => has_node(candidate, "hazard.sluice_1"),
        GraphRule::BossPreparationLoop => has_node(candidate, "gate.boss_1"),
        GraphRule::GatedTreasureBranch => has_node(candidate, "treasure.gated_1"),
        GraphRule::BranchMergeShortcut => has_node(candidate, "junction.merge_1"),
        GraphRule::OptionalTreasureDetour => false,
    };
    if duplicate {
        status = "duplicate".to_owned();
        reasons.push("Fixed marker for this rule already exists.".to_owned());
        recommended_actions
            .push("Fork from an earlier candidate or choose another rule.".to_owned());
    }
    if rule == GraphRule::NestedLockKeyChain && !has_node(candidate, "gate.locked_1") {
        status = "blocked".to_owned();
        reasons.push("Requires lock_key_loop / gate.locked_1 first.".to_owned());
        recommended_actions.push("Apply lock_key_loop before nested_lock_key_chain.".to_owned());
    }
    if rule == GraphRule::BranchMergeShortcut
        && !(has_node(candidate, "hub.central_1")
            || has_node(candidate, "treasure.gated_1")
            || has_node(candidate, "key.gate_1"))
    {
        status = "blocked".to_owned();
        reasons.push("Requires an existing branch, hub, or key route to merge.".to_owned());
        recommended_actions.push(
            "Apply hub_spoke_cluster, gated_treasure_branch, or lock_key_loop first.".to_owned(),
        );
    }
    if status == "applicable"
        && rule == GraphRule::SecretBypass
        && candidate
            .graph
            .edges
            .iter()
            .any(|edge| edge.traversal == TraversalKind::Locked)
    {
        status = "risky".to_owned();
        reasons.push("Secret bypass may trivialize existing locked progression.".to_owned());
        recommended_actions.push("Use only when bypass routes are intended.".to_owned());
    }
    RuleCompatibility {
        rule: rule.as_str().to_owned(),
        status,
        reasons,
        recommended_actions,
    }
}
