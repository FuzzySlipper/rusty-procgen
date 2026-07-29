fn score_graph_command(args: ReportOutArgs) -> Result<(), String> {
    let candidate: Candidate = read_json(&args.state)?;
    let report = score_graph(&candidate);
    write_json(&args.out, &report)
}

fn score_graph(candidate: &Candidate) -> ScoreReport {
    let node_count = candidate.graph.nodes.len() as f64;
    let edge_count = candidate.graph.edges.len() as f64;
    let loop_bonus = cycle_count(candidate) as f64;
    let optional_count = candidate
        .graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::OptionalBranch || edge.kind == EdgeKind::SecretBypass)
        .count() as f64;
    let locked_count = candidate
        .graph
        .edges
        .iter()
        .filter(|edge| edge.traversal == TraversalKind::Locked)
        .count() as f64;
    let shortcut_count = candidate
        .graph
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Shortcut)
        .count() as f64;
    let hub_count = count_nodes_with_tag(candidate, "hub") as f64;
    let wayfinding_anchor_count = count_nodes_with_tag(candidate, "wayfinding_anchor") as f64;
    let preparation_count = count_nodes_with_tag(candidate, "preparation") as f64;
    let hazard_count = count_nodes_with_tag(candidate, "hazard") as f64;
    let boss_count = count_nodes_with_tag(candidate, "boss") as f64;
    let merge_count = count_nodes_with_tag(candidate, "merge") as f64;
    let pressure_edge_count = count_edges_with_tag(candidate, "pressure") as f64;
    let rejoin_edge_count = count_edges_with_tag(candidate, "rejoin") as f64;
    let critical_path = shortest_path_len(candidate, "start", "goal").unwrap_or(0) as f64;
    let dead_end_count = dead_end_count(candidate) as f64;
    let mut metrics = BTreeMap::new();
    metrics.insert("nodeCount".to_owned(), node_count);
    metrics.insert("edgeCount".to_owned(), edge_count);
    metrics.insert("criticalPathLength".to_owned(), critical_path);
    metrics.insert("loopCount".to_owned(), loop_bonus);
    metrics.insert("optionalBranchCount".to_owned(), optional_count);
    metrics.insert("lockedEdgeCount".to_owned(), locked_count);
    metrics.insert("shortcutCount".to_owned(), shortcut_count);
    metrics.insert("deadEndCount".to_owned(), dead_end_count);
    metrics.insert("hubCount".to_owned(), hub_count);
    metrics.insert("wayfindingAnchorCount".to_owned(), wayfinding_anchor_count);
    metrics.insert("preparationCount".to_owned(), preparation_count);
    metrics.insert("hazardCount".to_owned(), hazard_count);
    metrics.insert("bossCount".to_owned(), boss_count);
    metrics.insert("mergeCount".to_owned(), merge_count);
    metrics.insert("pressureEdgeCount".to_owned(), pressure_edge_count);
    metrics.insert("rejoinEdgeCount".to_owned(), rejoin_edge_count);

    let raw = 0.10
        + (critical_path.min(8.0) * 0.025)
        + (loop_bonus.min(8.0) * 0.018)
        + (optional_count.min(10.0) * 0.012)
        + (locked_count.min(4.0) * 0.025)
        + (shortcut_count.min(3.0) * 0.018)
        + (hub_count.min(1.0) * 0.035)
        + (wayfinding_anchor_count.min(3.0) * 0.018)
        + (preparation_count.min(4.0) * 0.018)
        + (pressure_edge_count.min(4.0) * 0.015)
        + (rejoin_edge_count.min(6.0) * 0.012)
        + (merge_count.min(3.0) * 0.018)
        + (boss_count.min(1.0) * 0.035)
        - (dead_end_count * 0.04);
    let overall = (raw.clamp(0.0, 1.0) * 100.0).round() / 100.0;
    ScoreReport {
        kind: "rusty_procgen.score.graph.v1".to_owned(),
        schema_version: 1,
        state_hash: hash_json(candidate).unwrap_or_else(|_| "hash_error".to_owned()),
        overall,
        metrics,
        notes: vec![
            "Graph score is a deterministic first-slice heuristic, not a human-quality verdict."
                .to_owned(),
        ],
    }
}

fn count_nodes_with_tag(candidate: &Candidate, tag: &str) -> usize {
    candidate
        .graph
        .nodes
        .iter()
        .filter(|node| node_has_tag(node, tag))
        .count()
}

fn count_edges_with_tag(candidate: &Candidate, tag: &str) -> usize {
    candidate
        .graph
        .edges
        .iter()
        .filter(|edge| edge_has_tag(edge, tag))
        .count()
}

fn shortest_path_len(candidate: &Candidate, start: &str, goal: &str) -> Option<usize> {
    let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in &candidate.graph.edges {
        adjacency
            .entry(edge.from.as_str())
            .or_default()
            .push(edge.to.as_str());
    }
    let mut queue = VecDeque::from([(start, 0usize)]);
    let mut visited = BTreeSet::from([start]);
    while let Some((node, depth)) = queue.pop_front() {
        if node == goal {
            return Some(depth);
        }
        for next in adjacency.get(node).into_iter().flatten() {
            if visited.insert(next) {
                queue.push_back((next, depth + 1));
            }
        }
    }
    None
}

fn shortest_path_nodes(candidate: &Candidate, start: &str, goal: &str) -> Option<Vec<String>> {
    let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in &candidate.graph.edges {
        adjacency
            .entry(edge.from.as_str())
            .or_default()
            .push(edge.to.as_str());
    }
    let mut queue = VecDeque::from([start]);
    let mut visited = BTreeSet::from([start]);
    let mut previous: BTreeMap<&str, &str> = BTreeMap::new();
    while let Some(node) = queue.pop_front() {
        if node == goal {
            let mut path = vec![node.to_owned()];
            let mut cursor = node;
            while let Some(prev) = previous.get(cursor).copied() {
                path.push(prev.to_owned());
                cursor = prev;
            }
            path.reverse();
            return Some(path);
        }
        for next in adjacency.get(node).into_iter().flatten() {
            if visited.insert(next) {
                previous.insert(next, node);
                queue.push_back(next);
            }
        }
    }
    None
}

fn path_exists(candidate: &Candidate, start: &str, goal: &str) -> bool {
    shortest_path_len(candidate, start, goal).is_some()
}

fn dominator_nodes(candidate: &Candidate) -> Vec<String> {
    candidate
        .graph
        .nodes
        .iter()
        .filter(|node| node.id != "start" && node.id != "goal")
        .filter(|node| path_exists(candidate, "start", node.id.as_str()))
        .filter(|node| !path_exists_avoiding_node(candidate, "start", "goal", node.id.as_str()))
        .map(|node| node.id.clone())
        .collect()
}

fn path_exists_avoiding_node(candidate: &Candidate, start: &str, goal: &str, avoid: &str) -> bool {
    if start == avoid || goal == avoid {
        return false;
    }
    let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in &candidate.graph.edges {
        if edge.from == avoid || edge.to == avoid {
            continue;
        }
        adjacency
            .entry(edge.from.as_str())
            .or_default()
            .push(edge.to.as_str());
    }
    let mut queue = VecDeque::from([start]);
    let mut visited = BTreeSet::from([start]);
    while let Some(node) = queue.pop_front() {
        if node == goal {
            return true;
        }
        for next in adjacency.get(node).into_iter().flatten() {
            if visited.insert(next) {
                queue.push_back(next);
            }
        }
    }
    false
}

fn cycle_count(candidate: &Candidate) -> usize {
    let node_count = candidate.graph.nodes.len();
    let edge_count = candidate.graph.edges.len();
    if node_count == 0 {
        return 0;
    }
    let component_count = 1;
    edge_count
        .saturating_sub(node_count)
        .saturating_add(component_count)
}

fn dead_end_count(candidate: &Candidate) -> usize {
    candidate
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
        .count()
}

fn embed_2d_command(args: Embed2dArgs) -> Result<(), String> {
    let candidate: Candidate = read_json(&args.state)?;
    let validation = validate_graph(&candidate);
    if !validation.ok {
        return Err("cannot embed invalid graph candidate".to_owned());
    }
    let input_hash = hash_file(&args.state)?;
    let layout = embed_2d(&candidate, args.seed);
    write_json(&args.out, &layout)?;
    let receipt = receipt(
        "embed 2d",
        Some(args.seed),
        Some(&input_hash),
        Some(&hash_json(&layout)?),
        Some(&args.out),
        Vec::new(),
    );
    write_json(&args.receipt, &receipt)?;
    append_transcript(
        args.transcript.as_deref(),
        "embed 2d",
        Some(&args.out),
        Some(&args.receipt),
        Some(args.seed),
        json!({ "state": display_path(&args.state) }),
    )?;
    Ok(())
}

fn embed_2d(candidate: &Candidate, seed: u64) -> LayoutArtifact {
    let depths = graph_depths(candidate);
    let mut rows_by_depth: BTreeMap<usize, usize> = BTreeMap::new();
    let mut rooms = Vec::new();
    for node in &candidate.graph.nodes {
        let depth = depths.get(node.id.as_str()).copied().unwrap_or(0);
        let row = rows_by_depth.entry(depth).or_insert(0);
        let y_offset = *row as i32;
        *row += 1;
        rooms.push(LayoutRoom {
            node_id: node.id.clone(),
            kind: node.kind,
            label: node.label.clone(),
            x: 80 + depth as i32 * 180,
            y: 80 + y_offset * 110,
            width: 116,
            height: 64,
        });
    }
    LayoutArtifact {
        kind: "rusty_procgen.layout_2d.v1".to_owned(),
        schema_version: 1,
        layout_id: format!("layout.{}.{}", candidate.candidate_id, seed),
        candidate_id: candidate.candidate_id.clone(),
        seed,
        rooms,
        links: candidate
            .graph
            .edges
            .iter()
            .map(|edge| LayoutLink {
                edge_id: edge.id.clone(),
                from_node: edge.from.clone(),
                to_node: edge.to.clone(),
                kind: edge.kind,
                traversal: edge.traversal,
                required_item: edge.required_item.clone(),
            })
            .collect(),
    }
}

fn graph_depths(candidate: &Candidate) -> BTreeMap<&str, usize> {
    let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in &candidate.graph.edges {
        adjacency
            .entry(edge.from.as_str())
            .or_default()
            .push(edge.to.as_str());
    }
    let mut depths = BTreeMap::new();
    let mut queue = VecDeque::from([("start", 0usize)]);
    depths.insert("start", 0);
    while let Some((node, depth)) = queue.pop_front() {
        for next in adjacency.get(node).into_iter().flatten() {
            if !depths.contains_key(next) {
                depths.insert(*next, depth + 1);
                queue.push_back((next, depth + 1));
            }
        }
    }
    depths
}

fn accept_command(args: AcceptArgs) -> Result<(), String> {
    let candidate: Candidate = read_json(&args.candidate)?;
    let layout: LayoutArtifact = read_json(&args.layout)?;
    let validation: ValidationReport = read_json(&args.validation)?;
    let score: ScoreReport = read_json(&args.score)?;
    if !validation.ok {
        return Err("cannot accept artifact with failing validation".to_owned());
    }
    let candidate_hash = hash_json(&candidate)?;
    let layout_hash = hash_json(&layout)?;
    let artifact = AcceptedArtifact {
        kind: "rusty_procgen.accepted_artifact.v1".to_owned(),
        schema_version: 1,
        artifact_id: format!("accepted.{}", candidate.candidate_id),
        candidate_hash: candidate_hash.clone(),
        layout_hash: layout_hash.clone(),
        validation_ref: display_path(&args.validation),
        score_ref: display_path(&args.score),
        candidate,
        layout,
        score_summary: score,
    };
    write_json(&args.out, &artifact)?;
    let receipt = receipt(
        "accept",
        None,
        Some(&candidate_hash),
        Some(&hash_json(&artifact)?),
        Some(&args.out),
        Vec::new(),
    );
    write_json(&args.receipt, &receipt)?;
    append_transcript(
        args.transcript.as_deref(),
        "accept",
        Some(&args.out),
        Some(&args.receipt),
        None,
        json!({
            "candidate": display_path(&args.candidate),
            "layout": display_path(&args.layout),
            "validation": display_path(&args.validation),
            "score": display_path(&args.score)
        }),
    )
}
