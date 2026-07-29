#[allow(unused_imports)]
use crate::*;

pub(crate) fn init_candidate(args: InitArgs) -> Result<(), String> {
    let intent: SeedIntent = read_json(&args.intent)?;
    let mut candidate = create_initial_candidate(&intent, args.seed);
    candidate.provenance.push(ProvenanceStep {
        step: 1,
        command: "init".to_owned(),
        seed: Some(args.seed),
        summary: format!("Initialized {} from {}", candidate.candidate_id, intent.id),
    });
    write_json(&args.out, &candidate)?;
    let receipt = receipt(
        "init",
        Some(args.seed),
        Some(&hash_file(&args.intent)?),
        Some(&hash_json(&candidate)?),
        Some(&args.out),
        Vec::new(),
    );
    write_json(&args.receipt, &receipt)?;
    append_transcript(
        args.transcript.as_deref(),
        "init",
        Some(&args.out),
        Some(&args.receipt),
        Some(args.seed),
        json!({ "intent": display_path(&args.intent) }),
    )?;
    Ok(())
}

pub(crate) fn create_initial_candidate(intent: &SeedIntent, seed: u64) -> Candidate {
    Candidate {
        kind: "rusty_procgen.candidate.v1".to_owned(),
        schema_version: 1,
        candidate_id: format!("candidate.{}.{}", intent.id, seed),
        seed,
        dimension_model: "topology_graph".to_owned(),
        source_intent: Some(intent.id.clone()),
        provenance: Vec::new(),
        graph: IntentGraph {
            nodes: vec![
                Node {
                    id: "start".to_owned(),
                    kind: NodeKind::Start,
                    label: "Start".to_owned(),
                    tags: vec!["critical".to_owned()],
                    grants_item: None,
                },
                Node {
                    id: "goal".to_owned(),
                    kind: NodeKind::Goal,
                    label: "Goal".to_owned(),
                    tags: vec!["critical".to_owned()],
                    grants_item: None,
                },
            ],
            edges: vec![Edge {
                id: "edge.start.goal".to_owned(),
                from: "start".to_owned(),
                to: "goal".to_owned(),
                kind: EdgeKind::CriticalPath,
                traversal: TraversalKind::Open,
                required_item: None,
                tags: vec!["initial".to_owned()],
            }],
        },
    }
}

pub(crate) fn apply_rule(args: ApplyRuleArgs) -> Result<(), String> {
    let mut candidate: Candidate = read_json(&args.state)?;
    let input_hash = hash_file(&args.state)?;
    let diagnostics = apply_graph_rule(&mut candidate, args.rule, args.seed);
    let status = if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Fatal)
    {
        "rejected"
    } else {
        candidate.provenance.push(ProvenanceStep {
            step: candidate.provenance.len() as u32 + 1,
            command: format!("graph apply-rule {}", args.rule.as_str()),
            seed: Some(args.seed),
            summary: format!("Applied {}", args.rule.as_str()),
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
        command: format!("graph apply-rule {}", args.rule.as_str()),
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
        "graph apply-rule",
        if status == "ok" {
            Some(&args.out)
        } else {
            None
        },
        Some(&args.receipt),
        Some(args.seed),
        json!({ "rule": args.rule.as_str(), "state": display_path(&args.state) }),
    )?;
    if status == "ok" {
        Ok(())
    } else {
        Err("graph rule was rejected; see receipt diagnostics".to_owned())
    }
}

pub(crate) fn fork_command(args: ForkArgs) -> Result<(), String> {
    let candidate: Candidate = read_json(&args.state)?;
    let input_hash = hash_file(&args.state)?;
    let forked = fork_candidate(candidate, &args.label, args.seed);
    write_json(&args.out, &forked)?;
    let receipt = receipt(
        "graph fork",
        Some(args.seed),
        Some(&input_hash),
        Some(&hash_json(&forked)?),
        Some(&args.out),
        Vec::new(),
    );
    write_json(&args.receipt, &receipt)?;
    append_transcript(
        args.transcript.as_deref(),
        "graph fork",
        Some(&args.out),
        Some(&args.receipt),
        Some(args.seed),
        json!({
            "state": display_path(&args.state),
            "label": args.label
        }),
    )?;
    Ok(())
}

pub(crate) fn fork_candidate(mut candidate: Candidate, label: &str, seed: u64) -> Candidate {
    let source_id = candidate.candidate_id.clone();
    let label_slug = slugify_label(label);
    candidate.candidate_id = format!("{}.fork.{}.{}", source_id, label_slug, seed);
    candidate.seed = seed;
    candidate.provenance.push(ProvenanceStep {
        step: candidate.provenance.len() as u32 + 1,
        command: "graph fork".to_owned(),
        seed: Some(seed),
        summary: format!(
            "Forked {source_id} as {} with label {label_slug}",
            candidate.candidate_id
        ),
    });
    candidate
}

pub(crate) fn apply_graph_rule(
    candidate: &mut Candidate,
    rule: GraphRule,
    seed: u64,
) -> Vec<Diagnostic> {
    let mut proposed = candidate.clone();
    let mut diagnostics = mutate_graph_rule(&mut proposed, rule, seed);
    if diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != Severity::Fatal)
    {
        diagnostics.extend(
            validate_graph(&proposed)
                .diagnostics
                .into_iter()
                .filter(|diagnostic| diagnostic.severity == Severity::Fatal),
        );
    }
    if diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != Severity::Fatal)
    {
        *candidate = proposed;
    }
    diagnostics
}

fn mutate_graph_rule(candidate: &mut Candidate, rule: GraphRule, seed: u64) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    match rule {
        GraphRule::LockKeyLoop => {
            if candidate
                .graph
                .nodes
                .iter()
                .any(|node| node.id == "gate.locked_1")
            {
                diagnostics.push(fatal(
                    "rule_already_applied",
                    Some("gate.locked_1"),
                    None,
                    "lock_key_loop is already present.",
                ));
                return diagnostics;
            }
            candidate
                .graph
                .edges
                .retain(|edge| edge.id != "edge.start.goal");
            candidate.graph.nodes.extend([
                Node {
                    id: "gate.locked_1".to_owned(),
                    kind: NodeKind::Gate,
                    label: "Locked Gate".to_owned(),
                    tags: vec!["lock".to_owned(), "critical".to_owned()],
                    grants_item: None,
                },
                Node {
                    id: "key.gate_1".to_owned(),
                    kind: NodeKind::Key,
                    label: "Gate Key".to_owned(),
                    tags: vec!["key".to_owned(), "branch".to_owned()],
                    grants_item: Some("item.gate_key_1".to_owned()),
                },
            ]);
            candidate.graph.edges.extend([
                Edge {
                    id: "edge.start.gate_1".to_owned(),
                    from: "start".to_owned(),
                    to: "gate.locked_1".to_owned(),
                    kind: EdgeKind::CriticalPath,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["approach".to_owned()],
                },
                Edge {
                    id: "edge.gate_1.goal".to_owned(),
                    from: "gate.locked_1".to_owned(),
                    to: "goal".to_owned(),
                    kind: EdgeKind::CriticalPath,
                    traversal: TraversalKind::Locked,
                    required_item: Some("item.gate_key_1".to_owned()),
                    tags: vec!["lock".to_owned()],
                },
                Edge {
                    id: "edge.start.key_1".to_owned(),
                    from: "start".to_owned(),
                    to: "key.gate_1".to_owned(),
                    kind: EdgeKind::KeyBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["branch".to_owned()],
                },
                Edge {
                    id: "edge.key_1.gate_1".to_owned(),
                    from: "key.gate_1".to_owned(),
                    to: "gate.locked_1".to_owned(),
                    kind: EdgeKind::KeyBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["return".to_owned()],
                },
            ]);
        }
        GraphRule::OptionalTreasureDetour => {
            let suffix = stable_suffix(seed);
            let treasure_id = format!("treasure.{suffix}");
            candidate.graph.nodes.push(Node {
                id: treasure_id.clone(),
                kind: NodeKind::Treasure,
                label: "Optional Treasure".to_owned(),
                tags: vec!["optional".to_owned(), "reward".to_owned()],
                grants_item: None,
            });
            candidate.graph.edges.extend([
                Edge {
                    id: format!("edge.start.{treasure_id}"),
                    from: "start".to_owned(),
                    to: treasure_id.clone(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["detour".to_owned()],
                },
                Edge {
                    id: format!("edge.{treasure_id}.goal"),
                    from: treasure_id,
                    to: "goal".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["rejoin".to_owned()],
                },
            ]);
        }
        GraphRule::OneWayShortcut => {
            if candidate
                .graph
                .edges
                .iter()
                .any(|edge| edge.id == "edge.goal.start.shortcut")
            {
                diagnostics.push(fatal(
                    "rule_already_applied",
                    None,
                    Some("edge.goal.start.shortcut"),
                    "one_way_shortcut is already present.",
                ));
                return diagnostics;
            }
            candidate.graph.nodes.push(Node {
                id: "shortcut.return_1".to_owned(),
                kind: NodeKind::Shortcut,
                label: "Return Shortcut".to_owned(),
                tags: vec!["shortcut".to_owned()],
                grants_item: None,
            });
            candidate.graph.edges.extend([
                Edge {
                    id: "edge.goal.shortcut_1".to_owned(),
                    from: "goal".to_owned(),
                    to: "shortcut.return_1".to_owned(),
                    kind: EdgeKind::Shortcut,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["shortcut".to_owned()],
                },
                Edge {
                    id: "edge.shortcut_1.start".to_owned(),
                    from: "shortcut.return_1".to_owned(),
                    to: "start".to_owned(),
                    kind: EdgeKind::Shortcut,
                    traversal: TraversalKind::OneWayReturn,
                    required_item: None,
                    tags: vec!["return".to_owned()],
                },
            ]);
        }
        GraphRule::SecretBypass => {
            if candidate
                .graph
                .edges
                .iter()
                .any(|edge| edge.id == "edge.start.goal.secret")
            {
                diagnostics.push(fatal_with_hint(
                    "rule_already_applied",
                    None,
                    Some("edge.start.goal.secret"),
                    "secret_bypass is already present.",
                    "Choose a different bypass rule or start from an earlier candidate.",
                ));
                return diagnostics;
            }
            candidate.graph.nodes.push(Node {
                id: "secret.bypass_1".to_owned(),
                kind: NodeKind::Secret,
                label: "Secret Bypass".to_owned(),
                tags: vec!["secret".to_owned(), "bypass".to_owned()],
                grants_item: None,
            });
            candidate.graph.edges.extend([
                Edge {
                    id: "edge.start.secret_1".to_owned(),
                    from: "start".to_owned(),
                    to: "secret.bypass_1".to_owned(),
                    kind: EdgeKind::SecretBypass,
                    traversal: TraversalKind::Hidden,
                    required_item: None,
                    tags: vec!["hidden".to_owned()],
                },
                Edge {
                    id: "edge.secret_1.goal".to_owned(),
                    from: "secret.bypass_1".to_owned(),
                    to: "goal".to_owned(),
                    kind: EdgeKind::SecretBypass,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["bypass".to_owned()],
                },
            ]);
        }
        GraphRule::HubSpokeCluster => {
            if has_node(candidate, "hub.central_1") {
                diagnostics.push(fatal_with_hint(
                    "rule_already_applied",
                    Some("hub.central_1"),
                    None,
                    "hub_spoke_cluster is already present.",
                    "Use an alternate hub id or apply a different branch pattern.",
                ));
                return diagnostics;
            }
            candidate.graph.nodes.extend([
                Node {
                    id: "hub.central_1".to_owned(),
                    kind: NodeKind::Junction,
                    label: "Wayfinding Hub".to_owned(),
                    tags: vec![
                        "hub".to_owned(),
                        "wayfinding_anchor".to_owned(),
                        "merge".to_owned(),
                    ],
                    grants_item: None,
                },
                Node {
                    id: "resource.clue_1".to_owned(),
                    kind: NodeKind::Resource,
                    label: "Route Clue".to_owned(),
                    tags: vec!["optional".to_owned(), "preparation".to_owned()],
                    grants_item: None,
                },
                Node {
                    id: "hazard.watch_1".to_owned(),
                    kind: NodeKind::Hazard,
                    label: "Watched Passage".to_owned(),
                    tags: vec!["optional".to_owned(), "hazard".to_owned()],
                    grants_item: None,
                },
                Node {
                    id: "treasure.cache_1".to_owned(),
                    kind: NodeKind::Treasure,
                    label: "Hub Cache".to_owned(),
                    tags: vec!["optional".to_owned(), "reward".to_owned()],
                    grants_item: None,
                },
            ]);
            candidate.graph.edges.extend([
                Edge {
                    id: "edge.start.hub_1".to_owned(),
                    from: "start".to_owned(),
                    to: "hub.central_1".to_owned(),
                    kind: EdgeKind::CriticalPath,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["approach".to_owned()],
                },
                Edge {
                    id: "edge.hub_1.goal".to_owned(),
                    from: "hub.central_1".to_owned(),
                    to: "goal".to_owned(),
                    kind: EdgeKind::CriticalPath,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["rejoin".to_owned()],
                },
                Edge {
                    id: "edge.hub_1.clue_1".to_owned(),
                    from: "hub.central_1".to_owned(),
                    to: "resource.clue_1".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["branch".to_owned()],
                },
                Edge {
                    id: "edge.clue_1.hub_1".to_owned(),
                    from: "resource.clue_1".to_owned(),
                    to: "hub.central_1".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["return".to_owned()],
                },
                Edge {
                    id: "edge.hub_1.watch_1".to_owned(),
                    from: "hub.central_1".to_owned(),
                    to: "hazard.watch_1".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["branch".to_owned(), "pressure".to_owned()],
                },
                Edge {
                    id: "edge.watch_1.goal".to_owned(),
                    from: "hazard.watch_1".to_owned(),
                    to: "goal".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["rejoin".to_owned()],
                },
                Edge {
                    id: "edge.hub_1.cache_1".to_owned(),
                    from: "hub.central_1".to_owned(),
                    to: "treasure.cache_1".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["branch".to_owned()],
                },
                Edge {
                    id: "edge.cache_1.hub_1".to_owned(),
                    from: "treasure.cache_1".to_owned(),
                    to: "hub.central_1".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["return".to_owned()],
                },
            ]);
        }
        GraphRule::NestedLockKeyChain => {
            if has_node(candidate, "gate.locked_2") {
                diagnostics.push(fatal_with_hint(
                    "rule_already_applied",
                    Some("gate.locked_2"),
                    None,
                    "nested_lock_key_chain is already present.",
                    "Nested locks use fixed gate/key ids; start from a candidate before this rule.",
                ));
                return diagnostics;
            }
            if !has_node(candidate, "gate.locked_1") {
                diagnostics.push(fatal_with_hint(
                    "missing_required_pattern",
                    Some("gate.locked_1"),
                    None,
                    "nested_lock_key_chain requires an existing first lock/key loop.",
                    "Apply lock_key_loop before nested_lock_key_chain.",
                ));
                return diagnostics;
            }
            candidate
                .graph
                .edges
                .retain(|edge| edge.id != "edge.gate_1.goal");
            candidate.graph.nodes.extend([
                Node {
                    id: "gate.locked_2".to_owned(),
                    kind: NodeKind::Gate,
                    label: "Inner Locked Gate".to_owned(),
                    tags: vec!["lock".to_owned(), "critical".to_owned()],
                    grants_item: None,
                },
                Node {
                    id: "key.deep_2".to_owned(),
                    kind: NodeKind::Key,
                    label: "Inner Gate Key".to_owned(),
                    tags: vec![
                        "key".to_owned(),
                        "branch".to_owned(),
                        "preparation".to_owned(),
                    ],
                    grants_item: Some("item.deep_key_2".to_owned()),
                },
            ]);
            candidate.graph.edges.extend([
                Edge {
                    id: "edge.gate_1.gate_2".to_owned(),
                    from: "gate.locked_1".to_owned(),
                    to: "gate.locked_2".to_owned(),
                    kind: EdgeKind::CriticalPath,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["approach".to_owned()],
                },
                Edge {
                    id: "edge.gate_2.goal".to_owned(),
                    from: "gate.locked_2".to_owned(),
                    to: "goal".to_owned(),
                    kind: EdgeKind::CriticalPath,
                    traversal: TraversalKind::Locked,
                    required_item: Some("item.deep_key_2".to_owned()),
                    tags: vec!["locked".to_owned()],
                },
                Edge {
                    id: "edge.gate_1.key_2".to_owned(),
                    from: "gate.locked_1".to_owned(),
                    to: "key.deep_2".to_owned(),
                    kind: EdgeKind::KeyBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["branch".to_owned()],
                },
                Edge {
                    id: "edge.key_2.gate_2".to_owned(),
                    from: "key.deep_2".to_owned(),
                    to: "gate.locked_2".to_owned(),
                    kind: EdgeKind::KeyBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["return".to_owned()],
                },
            ]);
        }
        GraphRule::HazardResourceTradeoff => {
            if has_node(candidate, "hazard.sluice_1") {
                diagnostics.push(fatal_with_hint(
                    "rule_already_applied",
                    Some("hazard.sluice_1"),
                    None,
                    "hazard_resource_tradeoff is already present.",
                    "Apply a different pressure pattern or use a fresh candidate.",
                ));
                return diagnostics;
            }
            candidate.graph.nodes.extend([
                Node {
                    id: "hazard.sluice_1".to_owned(),
                    kind: NodeKind::Hazard,
                    label: "Flooded Sluice".to_owned(),
                    tags: vec!["optional".to_owned(), "hazard".to_owned()],
                    grants_item: None,
                },
                Node {
                    id: "resource.safety_1".to_owned(),
                    kind: NodeKind::Resource,
                    label: "Safety Cache".to_owned(),
                    tags: vec!["optional".to_owned(), "preparation".to_owned()],
                    grants_item: Some("item.safety_cache_1".to_owned()),
                },
            ]);
            candidate.graph.edges.extend([
                Edge {
                    id: "edge.start.safety_1".to_owned(),
                    from: "start".to_owned(),
                    to: "resource.safety_1".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["branch".to_owned()],
                },
                Edge {
                    id: "edge.safety_1.sluice_1".to_owned(),
                    from: "resource.safety_1".to_owned(),
                    to: "hazard.sluice_1".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["preparation".to_owned()],
                },
                Edge {
                    id: "edge.start.sluice_1".to_owned(),
                    from: "start".to_owned(),
                    to: "hazard.sluice_1".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["branch".to_owned(), "pressure".to_owned()],
                },
                Edge {
                    id: "edge.sluice_1.goal".to_owned(),
                    from: "hazard.sluice_1".to_owned(),
                    to: "goal".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["rejoin".to_owned()],
                },
            ]);
        }
        GraphRule::BossPreparationLoop => {
            if has_node(candidate, "gate.boss_1") {
                diagnostics.push(fatal_with_hint(
                    "rule_already_applied",
                    Some("gate.boss_1"),
                    None,
                    "boss_preparation_loop is already present.",
                    "Boss preparation currently uses one fixed boss gate per candidate.",
                ));
                return diagnostics;
            }
            let approach_from = if has_node(candidate, "gate.locked_2") {
                candidate
                    .graph
                    .edges
                    .retain(|edge| edge.id != "edge.gate_2.goal");
                "gate.locked_2"
            } else if has_node(candidate, "gate.locked_1") {
                candidate
                    .graph
                    .edges
                    .retain(|edge| edge.id != "edge.gate_1.goal");
                "gate.locked_1"
            } else {
                candidate
                    .graph
                    .edges
                    .retain(|edge| edge.id != "edge.start.goal");
                "start"
            };
            candidate.graph.nodes.extend([
                Node {
                    id: "gate.boss_1".to_owned(),
                    kind: NodeKind::Gate,
                    label: "Boss Threshold".to_owned(),
                    tags: vec!["boss".to_owned(), "critical".to_owned()],
                    grants_item: None,
                },
                Node {
                    id: "resource.boss_prep_1".to_owned(),
                    kind: NodeKind::Resource,
                    label: "Boss Preparation".to_owned(),
                    tags: vec!["preparation".to_owned(), "optional".to_owned()],
                    grants_item: Some("item.boss_preparation_1".to_owned()),
                },
            ]);
            candidate.graph.edges.extend([
                Edge {
                    id: "edge.approach.boss_1".to_owned(),
                    from: approach_from.to_owned(),
                    to: "gate.boss_1".to_owned(),
                    kind: EdgeKind::CriticalPath,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["approach".to_owned()],
                },
                Edge {
                    id: "edge.boss_1.goal".to_owned(),
                    from: "gate.boss_1".to_owned(),
                    to: "goal".to_owned(),
                    kind: EdgeKind::CriticalPath,
                    traversal: TraversalKind::Locked,
                    required_item: Some("item.boss_preparation_1".to_owned()),
                    tags: vec!["locked".to_owned(), "boss".to_owned()],
                },
                Edge {
                    id: "edge.approach.boss_prep_1".to_owned(),
                    from: approach_from.to_owned(),
                    to: "resource.boss_prep_1".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["branch".to_owned(), "preparation".to_owned()],
                },
                Edge {
                    id: "edge.boss_prep_1.boss_1".to_owned(),
                    from: "resource.boss_prep_1".to_owned(),
                    to: "gate.boss_1".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["return".to_owned()],
                },
            ]);
        }
        GraphRule::GatedTreasureBranch => {
            if has_node(candidate, "treasure.gated_1") {
                diagnostics.push(fatal_with_hint(
                    "rule_already_applied",
                    Some("treasure.gated_1"),
                    None,
                    "gated_treasure_branch is already present.",
                    "Use optional_treasure_detour for repeatable reward branches.",
                ));
                return diagnostics;
            }
            candidate.graph.nodes.extend([
                Node {
                    id: "key.treasure_1".to_owned(),
                    kind: NodeKind::Key,
                    label: "Treasure Key".to_owned(),
                    tags: vec!["key".to_owned(), "optional".to_owned()],
                    grants_item: Some("item.treasure_key_1".to_owned()),
                },
                Node {
                    id: "treasure.gated_1".to_owned(),
                    kind: NodeKind::Treasure,
                    label: "Gated Treasure".to_owned(),
                    tags: vec!["optional".to_owned(), "reward".to_owned()],
                    grants_item: None,
                },
            ]);
            candidate.graph.edges.extend([
                Edge {
                    id: "edge.start.treasure_key_1".to_owned(),
                    from: "start".to_owned(),
                    to: "key.treasure_1".to_owned(),
                    kind: EdgeKind::KeyBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["branch".to_owned()],
                },
                Edge {
                    id: "edge.treasure_key_1.treasure_1".to_owned(),
                    from: "key.treasure_1".to_owned(),
                    to: "treasure.gated_1".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Locked,
                    required_item: Some("item.treasure_key_1".to_owned()),
                    tags: vec!["locked".to_owned(), "branch".to_owned()],
                },
                Edge {
                    id: "edge.treasure_1.goal".to_owned(),
                    from: "treasure.gated_1".to_owned(),
                    to: "goal".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Locked,
                    required_item: Some("item.treasure_key_1".to_owned()),
                    tags: vec!["locked".to_owned(), "rejoin".to_owned()],
                },
            ]);
        }
        GraphRule::BranchMergeShortcut => {
            if has_node(candidate, "junction.merge_1") {
                diagnostics.push(fatal_with_hint(
                    "rule_already_applied",
                    Some("junction.merge_1"),
                    None,
                    "branch_merge_shortcut is already present.",
                    "Merge shortcuts use a fixed merge node until batch generation adds variant ids.",
                ));
                return diagnostics;
            }
            let secondary_source = if has_node(candidate, "hub.central_1") {
                "hub.central_1"
            } else if has_node(candidate, "treasure.gated_1") {
                "treasure.gated_1"
            } else if has_node(candidate, "key.gate_1") {
                "key.gate_1"
            } else {
                diagnostics.push(fatal_with_hint(
                    "missing_required_pattern",
                    None,
                    None,
                    "branch_merge_shortcut needs an existing branch or hub to merge.",
                    "Apply hub_spoke_cluster, gated_treasure_branch, or lock_key_loop first.",
                ));
                return diagnostics;
            };
            let secondary_guard_item = guarded_node_item(candidate, secondary_source);
            candidate.graph.nodes.push(Node {
                id: "junction.merge_1".to_owned(),
                kind: NodeKind::Junction,
                label: "Branch Merge".to_owned(),
                tags: vec!["merge".to_owned(), "wayfinding_anchor".to_owned()],
                grants_item: None,
            });
            candidate.graph.edges.extend([
                Edge {
                    id: "edge.start.merge_1".to_owned(),
                    from: "start".to_owned(),
                    to: "junction.merge_1".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["branch".to_owned()],
                },
                Edge {
                    id: "edge.secondary.merge_1".to_owned(),
                    from: secondary_source.to_owned(),
                    to: "junction.merge_1".to_owned(),
                    kind: EdgeKind::OptionalBranch,
                    traversal: if secondary_guard_item.is_some() {
                        TraversalKind::Locked
                    } else {
                        TraversalKind::Open
                    },
                    required_item: secondary_guard_item.clone(),
                    tags: if secondary_guard_item.is_some() {
                        vec!["locked".to_owned(), "rejoin".to_owned()]
                    } else {
                        vec!["rejoin".to_owned()]
                    },
                },
                Edge {
                    id: "edge.merge_1.goal.shortcut".to_owned(),
                    from: "junction.merge_1".to_owned(),
                    to: "goal".to_owned(),
                    kind: EdgeKind::Shortcut,
                    traversal: TraversalKind::Open,
                    required_item: None,
                    tags: vec!["shortcut".to_owned(), "rejoin".to_owned()],
                },
            ]);
        }
    }
    diagnostics
}

pub(crate) fn has_node(candidate: &Candidate, node_id: &str) -> bool {
    candidate.graph.nodes.iter().any(|node| node.id == node_id)
}

pub(crate) fn guarded_node_item(candidate: &Candidate, node_id: &str) -> Option<String> {
    let mut guard_items = candidate
        .graph
        .edges
        .iter()
        .filter(|edge| edge.to == node_id && edge.traversal == TraversalKind::Locked)
        .filter_map(|edge| edge.required_item.clone())
        .collect::<Vec<_>>();
    guard_items.sort();
    guard_items.dedup();
    (guard_items.len() == 1).then(|| guard_items.remove(0))
}

pub(crate) fn has_edge(candidate: &Candidate, edge_id: &str) -> bool {
    candidate.graph.edges.iter().any(|edge| edge.id == edge_id)
}

pub(crate) fn graph_rules_command(args: RuleMetadataArgs) -> Result<(), String> {
    let report = rule_metadata_report();
    if let Some(out) = args.out {
        write_json(&out, &report)
    } else {
        let text = serde_json::to_string_pretty(&report)
            .map_err(|error| format!("failed to encode rule metadata: {error}"))?;
        println!("{text}");
        Ok(())
    }
}

pub(crate) fn rule_metadata_report() -> RuleMetadataReport {
    RuleMetadataReport {
        kind: "rusty_procgen.rule_metadata.v1".to_owned(),
        schema_version: 1,
        rules: vec![
            RuleMetadata {
                id: GraphRule::LockKeyLoop.as_str().to_owned(),
                intent: "Replace direct start-goal route with a locked gate and reachable key branch."
                    .to_owned(),
                required_patterns: Vec::new(),
                duplicate_markers: vec!["gate.locked_1".to_owned()],
                emitted_node_tags: vec!["critical".to_owned(), "key".to_owned(), "lock".to_owned()],
                emitted_edge_tags: vec!["approach".to_owned(), "branch".to_owned(), "return".to_owned()],
                compatibility_hints: vec![
                    "Useful first structural rule for nested locks and boss approaches.".to_owned(),
                ],
                repair_hints: vec!["Apply before nested_lock_key_chain.".to_owned()],
            },
            RuleMetadata {
                id: GraphRule::OptionalTreasureDetour.as_str().to_owned(),
                intent: "Add a repeatable optional reward detour that rejoins the goal route.".to_owned(),
                required_patterns: Vec::new(),
                duplicate_markers: Vec::new(),
                emitted_node_tags: vec!["optional".to_owned(), "reward".to_owned()],
                emitted_edge_tags: vec!["detour".to_owned(), "rejoin".to_owned()],
                compatibility_hints: vec![
                    "Seed-derived ids allow multiple treasure detours when seeds differ.".to_owned(),
                ],
                repair_hints: vec!["Use when score needs branch value without changing critical path.".to_owned()],
            },
            RuleMetadata {
                id: GraphRule::OneWayShortcut.as_str().to_owned(),
                intent: "Add a one-way return route from goal back to start.".to_owned(),
                required_patterns: Vec::new(),
                duplicate_markers: vec!["edge.goal.start.shortcut".to_owned()],
                emitted_node_tags: vec!["shortcut".to_owned()],
                emitted_edge_tags: vec!["return".to_owned(), "shortcut".to_owned()],
                compatibility_hints: vec![
                    "Best after the critical route is already meaningful.".to_owned(),
                ],
                repair_hints: vec!["Start from a pre-shortcut candidate if duplicate rejected.".to_owned()],
            },
            RuleMetadata {
                id: GraphRule::SecretBypass.as_str().to_owned(),
                intent: "Add a hidden optional bypass from start to goal.".to_owned(),
                required_patterns: Vec::new(),
                duplicate_markers: vec!["edge.start.goal.secret".to_owned()],
                emitted_node_tags: vec!["bypass".to_owned(), "secret".to_owned()],
                emitted_edge_tags: vec!["bypass".to_owned(), "hidden".to_owned()],
                compatibility_hints: vec![
                    "Can reduce perceived lock importance; use when bypasses are desired.".to_owned(),
                ],
                repair_hints: vec!["Avoid if selection wants strict lock/key progression.".to_owned()],
            },
            RuleMetadata {
                id: GraphRule::HubSpokeCluster.as_str().to_owned(),
                intent: "Create a wayfinding hub with optional spokes, returns, and rejoin routes."
                    .to_owned(),
                required_patterns: Vec::new(),
                duplicate_markers: vec!["hub.central_1".to_owned()],
                emitted_node_tags: vec![
                    "hazard".to_owned(),
                    "hub".to_owned(),
                    "merge".to_owned(),
                    "optional".to_owned(),
                    "preparation".to_owned(),
                    "reward".to_owned(),
                    "wayfinding_anchor".to_owned(),
                ],
                emitted_edge_tags: vec![
                    "approach".to_owned(),
                    "branch".to_owned(),
                    "pressure".to_owned(),
                    "rejoin".to_owned(),
                    "return".to_owned(),
                ],
                compatibility_hints: vec![
                    "Good early rule when an agent needs local choice and orientation.".to_owned(),
                ],
                repair_hints: vec![
                    "If hub diagnostics fire, add return/rejoin routes or a wayfinding anchor.".to_owned(),
                ],
            },
            RuleMetadata {
                id: GraphRule::NestedLockKeyChain.as_str().to_owned(),
                intent: "Add a second gate/key layer behind the first lock.".to_owned(),
                required_patterns: vec!["lock_key_loop".to_owned()],
                duplicate_markers: vec!["gate.locked_2".to_owned()],
                emitted_node_tags: vec![
                    "branch".to_owned(),
                    "critical".to_owned(),
                    "key".to_owned(),
                    "lock".to_owned(),
                    "preparation".to_owned(),
                ],
                emitted_edge_tags: vec!["approach".to_owned(), "branch".to_owned(), "locked".to_owned(), "return".to_owned()],
                compatibility_hints: vec![
                    "Requires gate.locked_1; apply lock_key_loop first.".to_owned(),
                ],
                repair_hints: vec!["Move key branches before locked edges if validation fails.".to_owned()],
            },
            RuleMetadata {
                id: GraphRule::HazardResourceTradeoff.as_str().to_owned(),
                intent: "Add a pressure branch paired with a preparation resource.".to_owned(),
                required_patterns: Vec::new(),
                duplicate_markers: vec!["hazard.sluice_1".to_owned()],
                emitted_node_tags: vec!["hazard".to_owned(), "optional".to_owned(), "preparation".to_owned()],
                emitted_edge_tags: vec!["branch".to_owned(), "preparation".to_owned(), "pressure".to_owned(), "rejoin".to_owned()],
                compatibility_hints: vec![
                    "Pairs well with hubs and boss preparation loops.".to_owned(),
                ],
                repair_hints: vec!["Add rejoin edges after hazards if diagnostics report terminal pressure.".to_owned()],
            },
            RuleMetadata {
                id: GraphRule::BossPreparationLoop.as_str().to_owned(),
                intent: "Insert a boss gate with a preparation branch returning to the approach.".to_owned(),
                required_patterns: Vec::new(),
                duplicate_markers: vec!["gate.boss_1".to_owned()],
                emitted_node_tags: vec!["boss".to_owned(), "critical".to_owned(), "optional".to_owned(), "preparation".to_owned()],
                emitted_edge_tags: vec!["approach".to_owned(), "boss".to_owned(), "branch".to_owned(), "locked".to_owned(), "preparation".to_owned(), "return".to_owned()],
                compatibility_hints: vec![
                    "Uses deepest known lock gate as approach if one exists.".to_owned(),
                ],
                repair_hints: vec!["Keep preparation reachable before the boss locked edge.".to_owned()],
            },
            RuleMetadata {
                id: GraphRule::GatedTreasureBranch.as_str().to_owned(),
                intent: "Add an optional key-gated treasure branch that rejoins progression.".to_owned(),
                required_patterns: Vec::new(),
                duplicate_markers: vec!["treasure.gated_1".to_owned()],
                emitted_node_tags: vec!["key".to_owned(), "optional".to_owned(), "reward".to_owned()],
                emitted_edge_tags: vec!["branch".to_owned(), "locked".to_owned(), "rejoin".to_owned()],
                compatibility_hints: vec![
                    "Useful when a candidate needs reward tension without blocking the goal.".to_owned(),
                ],
                repair_hints: vec!["Ensure the treasure key remains reachable before the gated reward.".to_owned()],
            },
            RuleMetadata {
                id: GraphRule::BranchMergeShortcut.as_str().to_owned(),
                intent: "Add a merge node and shortcut from an existing branch or hub back to goal.".to_owned(),
                required_patterns: vec![
                    "hub_spoke_cluster or gated_treasure_branch or lock_key_loop".to_owned(),
                ],
                duplicate_markers: vec!["junction.merge_1".to_owned()],
                emitted_node_tags: vec!["merge".to_owned(), "wayfinding_anchor".to_owned()],
                emitted_edge_tags: vec!["branch".to_owned(), "rejoin".to_owned(), "shortcut".to_owned()],
                compatibility_hints: vec![
                    "Requires an upstream branch source; hub_spoke_cluster is the clearest pairing.".to_owned(),
                ],
                repair_hints: vec!["Apply a branch or hub rule first if missing_required_pattern is reported.".to_owned()],
            },
        ],
    }
}
