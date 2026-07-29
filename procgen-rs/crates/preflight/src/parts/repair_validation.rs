fn validate_graph_command(args: ReportOutArgs) -> Result<(), String> {
    let candidate: Candidate = read_json(&args.state)?;
    let report = validate_graph(&candidate);
    write_json(&args.out, &report)?;
    if report.ok {
        Ok(())
    } else {
        Err(format!(
            "graph validation failed with {} fatal diagnostic(s); see {}",
            report.fatal_count,
            args.out.display()
        ))
    }
}

fn repair_suggest_command(args: ReportOutArgs) -> Result<(), String> {
    let candidate: Candidate = read_json(&args.state)?;
    let report = repair_report(&candidate)?;
    write_json(&args.out, &report)
}

fn repair_apply_command(args: RepairApplyArgs) -> Result<(), String> {
    let mut candidate: Candidate = read_json(&args.state)?;
    let input_hash = hash_file(&args.state)?;
    let diagnostics = apply_repair_action(
        &mut candidate,
        args.action,
        args.target.as_deref(),
        args.seed,
    );
    let has_fatal = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Fatal);
    let status = if has_fatal {
        "rejected"
    } else {
        candidate.provenance.push(ProvenanceStep {
            step: candidate.provenance.len() as u32 + 1,
            command: format!("repair apply {}", args.action.as_str()),
            seed: Some(args.seed),
            summary: format!(
                "Applied {}{}",
                args.action.as_str(),
                args.target
                    .as_deref()
                    .map(|target| format!(" to {target}"))
                    .unwrap_or_default()
            ),
        });
        write_json(&args.out, &candidate)?;
        "ok"
    };
    let output_hash = if status == "ok" {
        Some(hash_json(&candidate)?)
    } else {
        None
    };
    let receipt = Receipt {
        kind: "rusty_procgen.receipt.v1".to_owned(),
        schema_version: 1,
        command: format!("repair apply {}", args.action.as_str()),
        status: status.to_owned(),
        seed: Some(args.seed),
        input_hash: Some(input_hash),
        output_hash,
        output_ref: if status == "ok" {
            Some(display_path(&args.out))
        } else {
            None
        },
        diagnostics,
    };
    write_json(&args.receipt, &receipt)?;
    append_transcript(
        args.transcript.as_deref(),
        "repair apply",
        if status == "ok" {
            Some(&args.out)
        } else {
            None
        },
        Some(&args.receipt),
        Some(args.seed),
        json!({
            "state": display_path(&args.state),
            "action": args.action.as_str(),
            "target": args.target
        }),
    )?;
    if status == "ok" {
        Ok(())
    } else {
        Err("repair action was rejected; see receipt diagnostics".to_owned())
    }
}

fn apply_repair_action(
    candidate: &mut Candidate,
    action: RepairAction,
    target: Option<&str>,
    seed: u64,
) -> Vec<Diagnostic> {
    let Some(target) = target else {
        return vec![fatal(
            "repair_target_required",
            None,
            None,
            format!("{} requires --target <node_id>.", action.as_str()),
        )];
    };
    if !has_node(candidate, target) {
        return vec![fatal(
            "repair_target_missing",
            Some(target),
            None,
            "Repair target node does not exist.",
        )];
    }
    match action {
        RepairAction::AddRejoinEdge => apply_add_rejoin_edge(candidate, target, seed),
        RepairAction::RemoveOrphanNode => apply_remove_orphan_node(candidate, target),
    }
}

fn apply_add_rejoin_edge(candidate: &mut Candidate, target: &str, seed: u64) -> Vec<Diagnostic> {
    if target == "goal" {
        return vec![fatal(
            "repair_target_invalid",
            Some(target),
            None,
            "Goal does not need a rejoin edge.",
        )];
    }
    if candidate.graph.edges.iter().any(|edge| edge.from == target) {
        return vec![fatal_with_hint(
            "repair_target_ambiguous",
            Some(target),
            None,
            "Target already has outgoing routes.",
            "Use add_rejoin_edge only on simple terminal branch nodes.",
        )];
    }
    let edge_id = format!(
        "edge.repair.{}.goal.{}",
        slugify_label(target),
        stable_suffix(seed)
    );
    if has_edge(candidate, edge_id.as_str()) {
        return vec![fatal(
            "repair_edge_duplicate",
            None,
            Some(edge_id.as_str()),
            "Repair edge already exists.",
        )];
    }
    candidate.graph.edges.push(Edge {
        id: edge_id,
        from: target.to_owned(),
        to: "goal".to_owned(),
        kind: EdgeKind::OptionalBranch,
        traversal: TraversalKind::Open,
        required_item: None,
        tags: vec!["repair".to_owned(), "rejoin".to_owned()],
    });
    Vec::new()
}

fn apply_remove_orphan_node(candidate: &mut Candidate, target: &str) -> Vec<Diagnostic> {
    if target == "start" || target == "goal" {
        return vec![fatal(
            "repair_target_invalid",
            Some(target),
            None,
            "Start and goal nodes cannot be removed by bounded repair.",
        )];
    }
    if candidate.graph.edges.iter().any(|edge| edge.to == target) {
        return vec![fatal_with_hint(
            "repair_target_not_orphan",
            Some(target),
            None,
            "Target has incoming routes and is not an orphan.",
            "Use remove_orphan_node only for nodes with no incoming route.",
        )];
    }
    candidate.graph.nodes.retain(|node| node.id != target);
    candidate
        .graph
        .edges
        .retain(|edge| edge.from != target && edge.to != target);
    Vec::new()
}

fn repair_report(candidate: &Candidate) -> Result<RepairReport, String> {
    let validation = validate_graph(candidate);
    let mut suggestions: Vec<RepairSuggestion> = validation
        .diagnostics
        .iter()
        .map(repair_suggestion_for_diagnostic)
        .collect();
    suggestions.sort_by(|left, right| {
        severity_rank(left.severity)
            .cmp(&severity_rank(right.severity))
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.node.cmp(&right.node))
            .then_with(|| left.edge.cmp(&right.edge))
    });
    Ok(RepairReport {
        kind: "rusty_procgen.repair_report.v1".to_owned(),
        schema_version: 1,
        candidate_id: candidate.candidate_id.clone(),
        state_hash: hash_json(candidate)?,
        validation_ok: validation.ok,
        fatal_count: validation.fatal_count,
        suggestions,
    })
}

fn repair_suggestion_for_diagnostic(diagnostic: &Diagnostic) -> RepairSuggestion {
    RepairSuggestion {
        code: diagnostic.code.clone(),
        severity: diagnostic.severity,
        node: diagnostic.node.clone(),
        edge: diagnostic.edge.clone(),
        detail: diagnostic.detail.clone(),
        repair_hint: diagnostic.repair_hint.clone(),
        suggested_actions: suggested_actions_for_diagnostic(diagnostic),
    }
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Fatal => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    }
}

fn suggested_actions_for_diagnostic(diagnostic: &Diagnostic) -> Vec<String> {
    match diagnostic.code.as_str() {
        "required_item_unavailable" => vec![
            "Inspect graph rules for a key/resource provider that grants the required item."
                .to_owned(),
            "Fork the candidate before adding or moving a provider branch.".to_owned(),
        ],
        "locked_edge_never_traversed" => vec![
            "Move the provider branch before the locked edge or add an open route to the provider."
                .to_owned(),
            "Run validate graph again before scoring.".to_owned(),
        ],
        "goal_unreachable" => vec![
            "Reconnect the critical path from start to goal under current lock constraints."
                .to_owned(),
            "Use graph summarize --json to inspect dead ends and locked items.".to_owned(),
        ],
        "missing_required_pattern" => vec![
            "Run graph rules and apply the prerequisite pattern before retrying this rule."
                .to_owned(),
            "Fork from an earlier candidate if the prerequisite would conflict with current structure."
                .to_owned(),
        ],
        "hub_incident_edges_low" | "hub_missing_return_or_rejoin" => vec![
            "Add or repair hub spokes so at least one branch returns or rejoins.".to_owned(),
            "Prefer hub_spoke_cluster on a fork when the current hub is too sparse.".to_owned(),
        ],
        "hub_missing_wayfinding_anchor" => {
            vec!["Tag the hub as wayfinding_anchor or replace it with hub_spoke_cluster.".to_owned()]
        }
        "boss_missing_preparation" | "boss_preparation_missing_return" => vec![
            "Add a reachable preparation resource before the boss approach.".to_owned(),
            "Ensure the preparation branch returns or rejoins at the boss gate.".to_owned(),
        ],
        "hazard_missing_rejoin" => {
            vec!["Add a rejoin/return edge after the hazard or remove the terminal pressure branch."
                .to_owned()]
        }
        "merge_upstream_routes_low" => {
            vec!["Add a second upstream branch route before treating this node as a merge.".to_owned()]
        }
        "non_goal_dead_end" => vec![
            "Add a return/rejoin edge unless this is an intentional terminal reward.".to_owned(),
            diagnostic
                .node
                .as_deref()
                .map(|node| format!("Run repair apply --action add_rejoin_edge --target {node}."))
                .unwrap_or_else(|| {
                    "Run repair apply --action add_rejoin_edge with a terminal node target."
                        .to_owned()
                }),
        ],
        "orphan_node" => vec![
            "Add an incoming approach or branch edge from a reachable node.".to_owned(),
            diagnostic
                .node
                .as_deref()
                .map(|node| format!("Run repair apply --action remove_orphan_node --target {node}."))
                .unwrap_or_else(|| {
                    "Run repair apply --action remove_orphan_node with the orphan node target."
                        .to_owned()
                }),
        ],
        "rule_already_applied" => {
            vec!["Fork from an earlier candidate or choose a rule with seed-derived ids.".to_owned()]
        }
        _ => diagnostic
            .repair_hint
            .iter()
            .map(|hint| format!("Use repair hint: {hint}"))
            .collect(),
    }
}

fn validate_graph(candidate: &Candidate) -> ValidationReport {
    let mut diagnostics = Vec::new();
    let node_ids: BTreeSet<&str> = candidate
        .graph
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect();
    let start_count = candidate
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Start)
        .count();
    let goal_count = candidate
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Goal)
        .count();
    if start_count != 1 {
        diagnostics.push(fatal(
            "start_count_invalid",
            None,
            None,
            "Graph must contain exactly one start node.",
        ));
    }
    if goal_count != 1 {
        diagnostics.push(fatal(
            "goal_count_invalid",
            None,
            None,
            "Graph must contain exactly one goal node.",
        ));
    }
    for edge in &candidate.graph.edges {
        if !node_ids.contains(edge.from.as_str()) {
            diagnostics.push(fatal_with_hint(
                "edge_from_missing",
                None,
                Some(edge.id.as_str()),
                "Edge source node is missing.",
                "Create the source node or remove this edge before applying more rules.",
            ));
        }
        if !node_ids.contains(edge.to.as_str()) {
            diagnostics.push(fatal_with_hint(
                "edge_to_missing",
                None,
                Some(edge.id.as_str()),
                "Edge target node is missing.",
                "Create the target node or remove this edge before applying more rules.",
            ));
        }
    }

    let granted_items: BTreeSet<&str> = candidate
        .graph
        .nodes
        .iter()
        .filter_map(|node| node.grants_item.as_deref())
        .collect();
    for edge in &candidate.graph.edges {
        if let Some(required_item) = edge.required_item.as_deref() {
            if !granted_items.contains(required_item) {
                diagnostics.push(fatal_with_hint(
                    "required_item_unavailable",
                    None,
                    Some(edge.id.as_str()),
                    format!("Edge requires {required_item}, but no node grants it."),
                    "Add a reachable key/resource node that grants the required item.",
                ));
            }
        }
    }

    if start_count == 1 && goal_count == 1 {
        let reachable = reachable_with_items(candidate);
        if !reachable.goal_reached {
            diagnostics.push(fatal_with_hint(
                "goal_unreachable",
                Some("goal"),
                None,
                "Goal is not reachable under lock/key constraints.",
                "Add an open route, move item providers before locks, or reconnect the critical path.",
            ));
        }
        for edge in &candidate.graph.edges {
            if edge.traversal == TraversalKind::Locked
                && !reachable.traversed_edges.contains(edge.id.as_str())
            {
                diagnostics.push(fatal_with_hint(
                    "locked_edge_never_traversed",
                    None,
                    Some(edge.id.as_str()),
                    "Locked edge could not be traversed after item collection.",
                    "Move the item provider earlier or add a branch that reaches it before the lock.",
                ));
            }
        }
    }

    let mut incoming: BTreeMap<&str, usize> = BTreeMap::new();
    let mut outgoing: BTreeMap<&str, usize> = BTreeMap::new();
    for edge in &candidate.graph.edges {
        *incoming.entry(edge.to.as_str()).or_insert(0) += 1;
        *outgoing.entry(edge.from.as_str()).or_insert(0) += 1;
    }
    for node in &candidate.graph.nodes {
        if node.kind != NodeKind::Goal && outgoing.get(node.id.as_str()).copied().unwrap_or(0) == 0
        {
            diagnostics.push(warning_with_hint(
                "non_goal_dead_end",
                Some(node.id.as_str()),
                None,
                "Non-goal node has no outgoing route.",
                "Add a return/rejoin edge or tag this as an intentional terminal reward in a later schema.",
            ));
        }
        if node.kind != NodeKind::Start && incoming.get(node.id.as_str()).copied().unwrap_or(0) == 0
        {
            diagnostics.push(warning_with_hint(
                "orphan_node",
                Some(node.id.as_str()),
                None,
                "Node has no incoming route.",
                "Add an incoming approach or branch edge from a reachable node.",
            ));
        }
    }

    validate_v2_patterns(candidate, &incoming, &outgoing, &mut diagnostics);

    let fatal_count = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Fatal)
        .count();
    ValidationReport {
        kind: "rusty_procgen.validation.graph.v1".to_owned(),
        schema_version: 1,
        state_hash: hash_json(candidate).unwrap_or_else(|_| "hash_error".to_owned()),
        ok: fatal_count == 0,
        fatal_count,
        diagnostics,
    }
}

fn validate_v2_patterns(
    candidate: &Candidate,
    incoming: &BTreeMap<&str, usize>,
    outgoing: &BTreeMap<&str, usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for node in candidate
        .graph
        .nodes
        .iter()
        .filter(|node| node_has_tag(node, "hub"))
    {
        let incident = incoming.get(node.id.as_str()).copied().unwrap_or(0)
            + outgoing.get(node.id.as_str()).copied().unwrap_or(0);
        if incident < 3 {
            diagnostics.push(warning_with_hint(
                "hub_incident_edges_low",
                Some(node.id.as_str()),
                None,
                "Hub has fewer than three incident edges.",
                "Add at least two spokes plus a critical approach or continuation.",
            ));
        }
        if !node_has_tag(node, "wayfinding_anchor") {
            diagnostics.push(warning_with_hint(
                "hub_missing_wayfinding_anchor",
                Some(node.id.as_str()),
                None,
                "Hub is missing a wayfinding anchor tag.",
                "Tag the hub as wayfinding_anchor so later embedding can preserve orientation.",
            ));
        }
        let returns_to_hub = candidate.graph.edges.iter().any(|edge| {
            (edge.from == node.id || edge.to == node.id)
                && (edge_has_tag(edge, "return") || edge_has_tag(edge, "rejoin"))
        });
        if !returns_to_hub {
            diagnostics.push(warning_with_hint(
                "hub_missing_return_or_rejoin",
                Some(node.id.as_str()),
                None,
                "Hub has no spoke return or rejoin edge.",
                "Add a return/rejoin edge from at least one spoke back to the hub or main route.",
            ));
        }
    }

    let preparation_nodes: BTreeSet<&str> = candidate
        .graph
        .nodes
        .iter()
        .filter(|node| node_has_tag(node, "preparation"))
        .map(|node| node.id.as_str())
        .collect();
    for boss in candidate
        .graph
        .nodes
        .iter()
        .filter(|node| node_has_tag(node, "boss"))
    {
        if preparation_nodes.is_empty() {
            diagnostics.push(fatal_with_hint(
                "boss_missing_preparation",
                Some(boss.id.as_str()),
                None,
                "Boss node has no preparation branch.",
                "Add a reachable resource or clue tagged preparation before the boss approach.",
            ));
        }
        let preparation_rejoins_boss = candidate.graph.edges.iter().any(|edge| {
            edge.to == boss.id
                && preparation_nodes.contains(edge.from.as_str())
                && (edge_has_tag(edge, "return") || edge_has_tag(edge, "rejoin"))
        });
        if !preparation_nodes.is_empty() && !preparation_rejoins_boss {
            diagnostics.push(warning_with_hint(
                "boss_preparation_missing_return",
                Some(boss.id.as_str()),
                None,
                "Preparation branch does not return to the boss approach.",
                "Add a return/rejoin edge from a preparation node to the boss gate.",
            ));
        }
    }

    for hazard in candidate
        .graph
        .nodes
        .iter()
        .filter(|node| node_has_tag(node, "hazard"))
    {
        let rejoins = candidate.graph.edges.iter().any(|edge| {
            edge.from == hazard.id && (edge_has_tag(edge, "rejoin") || edge_has_tag(edge, "return"))
        });
        if !rejoins {
            diagnostics.push(warning_with_hint(
                "hazard_missing_rejoin",
                Some(hazard.id.as_str()),
                None,
                "Hazard branch does not visibly rejoin progression.",
                "Add a rejoin edge after the hazard or mark the branch as a deliberate terminal.",
            ));
        }
    }

    for merge in candidate
        .graph
        .nodes
        .iter()
        .filter(|node| node_has_tag(node, "merge"))
    {
        let incoming_count = incoming.get(merge.id.as_str()).copied().unwrap_or(0);
        if incoming_count < 2 {
            diagnostics.push(warning_with_hint(
                "merge_upstream_routes_low",
                Some(merge.id.as_str()),
                None,
                "Merge node has fewer than two upstream routes.",
                "Add another branch or shortcut edge into the merge node.",
            ));
        }
    }
}

fn node_has_tag(node: &Node, tag: &str) -> bool {
    node.tags.iter().any(|candidate_tag| candidate_tag == tag)
}

fn edge_has_tag(edge: &Edge, tag: &str) -> bool {
    edge.tags.iter().any(|candidate_tag| candidate_tag == tag)
}

struct Reachability<'a> {
    goal_reached: bool,
    traversed_edges: BTreeSet<&'a str>,
}

fn reachable_with_items(candidate: &Candidate) -> Reachability<'_> {
    let mut adjacency: BTreeMap<&str, Vec<&Edge>> = BTreeMap::new();
    for edge in &candidate.graph.edges {
        adjacency.entry(edge.from.as_str()).or_default().push(edge);
    }
    let mut queue = VecDeque::new();
    let mut visited = BTreeSet::new();
    let mut traversed_edges = BTreeSet::new();
    let mut items = BTreeSet::new();
    queue.push_back("start");
    visited.insert("start");

    while let Some(node_id) = queue.pop_front() {
        if let Some(node) = candidate.graph.nodes.iter().find(|node| node.id == node_id) {
            if let Some(item) = node.grants_item.as_deref() {
                if items.insert(item) {
                    visited.clear();
                    visited.insert("start");
                    queue.clear();
                    queue.push_back("start");
                }
            }
        }
        for edge in adjacency.get(node_id).into_iter().flatten() {
            if edge
                .required_item
                .as_deref()
                .is_some_and(|item| !items.contains(item))
            {
                continue;
            }
            traversed_edges.insert(edge.id.as_str());
            if visited.insert(edge.to.as_str()) {
                queue.push_back(edge.to.as_str());
            }
        }
    }

    Reachability {
        goal_reached: visited.contains("goal"),
        traversed_edges,
    }
}
