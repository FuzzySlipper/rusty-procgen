#[cfg(test)]
mod tests {
    use super::*;

    fn test_intent(id: &str) -> SeedIntent {
        SeedIntent {
            kind: "asha_procgen.seed_intent.v1".to_owned(),
            id: id.to_owned(),
            title: "Test".to_owned(),
            target_dimension: "topology_graph".to_owned(),
            desired_patterns: Vec::new(),
            notes: Vec::new(),
        }
    }

    fn test_connection_plan(
        candidate: &Candidate,
        intermediate: &IntermediateBreakdown,
    ) -> PhysicalConnectionPlan {
        plan_physical_connections(
            candidate,
            intermediate,
            &PhysicalConnectionPlanArgs {
                candidate: PathBuf::from("artifacts/test/candidate.json"),
                intermediate: PathBuf::from("artifacts/test/intermediate-breakdown.json"),
                out: PathBuf::from("artifacts/test/physical-connection-plan.json"),
            },
        )
        .expect("physical connection plan should emit")
    }

    fn test_breakdown(candidate: &Candidate) -> IntermediateBreakdown {
        let annotations = spatial_intent_report(candidate, None).expect("spatial intent report");
        intermediate_breakdown(
            candidate,
            &annotations,
            Path::new("artifacts/test/spatial-intent.json"),
        )
        .expect("intermediate breakdown should emit")
    }

    #[test]
    fn physical_connection_plan_normalizes_reciprocal_open_edges() {
        let intent = test_intent("reciprocal-open");
        let mut candidate = create_initial_candidate(&intent, 41);
        candidate.graph.edges.push(Edge {
            id: "edge.goal.start.return".to_owned(),
            from: "goal".to_owned(),
            to: "start".to_owned(),
            kind: EdgeKind::Shortcut,
            traversal: TraversalKind::Open,
            required_item: None,
            tags: vec!["return".to_owned()],
        });
        let breakdown = test_breakdown(&candidate);
        let plan = test_connection_plan(&candidate, &breakdown);

        assert_eq!(plan.sections.len(), 1);
        assert_eq!(plan.edge_mappings.len(), 2);
        let section = &plan.sections[0];
        assert_eq!(section.topology, "corridor_2");
        assert_eq!(section.source_edges.len(), 2);
        assert_eq!(section.traversal_refs.len(), 2);
        assert!(section.traversal_refs.iter().any(|reference| {
            reference.from_region == "region.start" && reference.to_region == "region.goal"
        }));
        assert!(section.traversal_refs.iter().any(|reference| {
            reference.from_region == "region.goal" && reference.to_region == "region.start"
        }));
        assert!(plan
            .edge_mappings
            .iter()
            .all(|mapping| mapping.section_id == section.id));
    }

    #[test]
    fn physical_connection_plan_keeps_incompatible_reciprocal_edges_separate() {
        let intent = test_intent("reciprocal-locked");
        let mut candidate = create_initial_candidate(&intent, 42);
        candidate.graph.edges.push(Edge {
            id: "edge.goal.start.locked".to_owned(),
            from: "goal".to_owned(),
            to: "start".to_owned(),
            kind: EdgeKind::Shortcut,
            traversal: TraversalKind::Locked,
            required_item: Some("item.return-key".to_owned()),
            tags: vec!["return".to_owned()],
        });
        let breakdown = test_breakdown(&candidate);
        let plan = test_connection_plan(&candidate, &breakdown);

        assert_eq!(plan.sections.len(), 2);
        assert_eq!(plan.edge_mappings.len(), 2);
        assert_ne!(
            plan.edge_mappings[0].section_id,
            plan.edge_mappings[1].section_id
        );
    }

    #[test]
    fn geometry_routes_an_embeddable_lock_key_plan() {
        let intent = test_intent("embeddable-lock-key");
        let mut candidate = create_initial_candidate(&intent, 43);
        assert!(apply_graph_rule(&mut candidate, GraphRule::LockKeyLoop, 44).is_empty());
        let breakdown = test_breakdown(&candidate);
        let connection_plan = test_connection_plan(&candidate, &breakdown);
        let geometry = emit_geometry_2d(
            &candidate,
            &breakdown,
            &connection_plan,
            &GeometryEmit2dArgs {
                candidate: PathBuf::from("artifacts/test/candidate.json"),
                intermediate: PathBuf::from("artifacts/test/intermediate-breakdown.json"),
                connection_plan: PathBuf::from("artifacts/test/physical-connection-plan.json"),
                layout_policy: None,
                seed: 45,
                out: PathBuf::from("artifacts/test/geometry.json"),
            },
            45,
        )
        .expect("lock-key physical plan should embed");

        assert_eq!(geometry.corridors.len(), connection_plan.sections.len());
        assert!(validate_geometry_2d(&geometry).ok);
    }

    #[test]
    fn geometry_layout_policy_compacts_escalates_and_repeats_deterministically() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let candidate_path =
            repo_root.join("artifacts/samples/batch-v2/candidate-006/candidate-001-lock_key_loop.json");
        let intermediate_path =
            repo_root.join("artifacts/samples/batch-v2/candidate-006/intermediate-breakdown.json");
        let candidate: Candidate = read_json(&candidate_path).expect("sample lock-key candidate");
        let intermediate: IntermediateBreakdown =
            read_json(&intermediate_path).expect("sample lock-key intermediate");
        let connection_plan = test_connection_plan(&candidate, &intermediate);
        let specs = ordered_geometry_region_specs(&candidate, &intermediate);
        let policy = GeometryLayoutPolicy {
            kind: "asha_procgen.geometry_layout_policy.v1".to_owned(),
            schema_version: 1,
            initial_room_margin: 32,
            initial_column_gap: 32,
            initial_row_gap: 32,
            room_margin_growth: 64,
            column_gap_growth: 128,
            row_gap_growth: 64,
            max_spacing_tiers: 3,
            room_order_attempts_per_tier: 1,
            max_search_attempts: 12,
        };

        let first =
            place_and_route_physical_geometry(&specs, &connection_plan, 14_307, &policy)
                .expect("compact-first search should escalate to a routable tier");
        let repeated =
            place_and_route_physical_geometry(&specs, &connection_plan, 14_307, &policy)
                .expect("repeated search should succeed");

        assert_eq!(first.search.spacing_tier, 1);
        assert_eq!(
            first.search.effective_spacing,
            GeometrySpacing {
                room_margin: 96,
                column_gap: 160,
                row_gap: 96,
            }
        );
        assert_eq!(first.search, repeated.search);
        assert_eq!(
            serde_json::to_value(&first.rooms).expect("serialize first rooms"),
            serde_json::to_value(&repeated.rooms).expect("serialize repeated rooms")
        );
        assert_eq!(
            serde_json::to_value(&first.corridors).expect("serialize first corridors"),
            serde_json::to_value(&repeated.corridors).expect("serialize repeated corridors")
        );
    }

    #[test]
    fn geometry_layout_policy_rejects_invalid_values_and_bounds_exhaustion() {
        let mut invalid = default_geometry_layout_policy();
        invalid.column_gap_growth = 7;
        assert!(validate_geometry_layout_policy(&invalid)
            .expect_err("unaligned growth should fail")
            .contains("multiple of 8"));
        let mut oversized = default_geometry_layout_policy();
        oversized.initial_column_gap = 1_024;
        oversized.column_gap_growth = 512;
        oversized.max_spacing_tiers = 8;
        oversized.max_search_attempts = 128;
        assert!(validate_geometry_layout_policy(&oversized)
            .expect_err("oversized final tier should fail")
            .contains("exceeds 2048"));

        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let candidate_path = repo_root
            .join("artifacts/samples/batch-v2/candidate-000/candidate-003-branch_merge_shortcut.json");
        let intermediate_path =
            repo_root.join("artifacts/samples/batch-v2/candidate-000/intermediate-breakdown.json");
        let candidate: Candidate = read_json(&candidate_path).expect("sample hub-merge candidate");
        let intermediate: IntermediateBreakdown =
            read_json(&intermediate_path).expect("sample hub-merge intermediate");
        let connection_plan = test_connection_plan(&candidate, &intermediate);
        let specs = ordered_geometry_region_specs(&candidate, &intermediate);
        let bounded = GeometryLayoutPolicy {
            kind: "asha_procgen.geometry_layout_policy.v1".to_owned(),
            schema_version: 1,
            initial_room_margin: 32,
            initial_column_gap: 32,
            initial_row_gap: 32,
            room_margin_growth: 0,
            column_gap_growth: 0,
            row_gap_growth: 0,
            max_spacing_tiers: 1,
            room_order_attempts_per_tier: 1,
            max_search_attempts: 4,
        };
        let error =
            place_and_route_physical_geometry(&specs, &connection_plan, 14_301, &bounded)
                .expect_err("hub-merge should exhaust this deliberately tiny budget");
        assert!(error.starts_with("geometry search exhausted after 4 route attempt(s)"));
        assert!(error.contains("across 1 spacing tier(s)"));
        assert!(error.contains("last route failure: single-floor route unavailable"));
    }

    #[test]
    fn rejects_private_engine_paths() {
        let private_path = format!("{}/{}/{}", "../asha-engine", "engine-rs", "crates/state");
        let error = reject_private_engine_path("demo", private_path.as_str())
            .expect_err("private engine path should be rejected");
        assert!(error.contains("private ASHA internals"));
    }

    #[test]
    fn geometry_2d_contract_round_trips_minimal_layout() {
        let geometry = Geometry2dArtifact {
            kind: "asha_procgen.geometry_2d.v1".to_owned(),
            schema_version: 1,
            geometry_id: "geometry.test.1".to_owned(),
            candidate_id: "candidate.test.1".to_owned(),
            seed: 99,
            source_candidate_ref: "artifacts/test/candidate.json".to_owned(),
            source_intermediate_ref: "artifacts/test/intermediate-breakdown.json".to_owned(),
            source_connection_plan_ref: "artifacts/test/physical-connection-plan.json".to_owned(),
            connection_plan_id: "physical_connections.candidate.test.1".to_owned(),
            layout_policy: default_geometry_layout_policy(),
            layout_search: GeometryLayoutSearchEvidence {
                spacing_tier: 0,
                room_order_attempt: 0,
                port_order_attempt: 0,
                route_order_attempt: 0,
                search_attempts: 1,
                effective_spacing: geometry_spacing_for_tier(
                    &default_geometry_layout_policy(),
                    0,
                )
                .expect("default spacing"),
            },
            bounds: GeometryBounds {
                width: 480,
                height: 320,
                grid: 8,
            },
            rooms: vec![GeometryRoom {
                id: "room.start".to_owned(),
                source_region: "region.start".to_owned(),
                source_nodes: vec!["start".to_owned()],
                role: "start".to_owned(),
                geometry_role: "entry".to_owned(),
                footprint_class: "marker_room".to_owned(),
                rect: GeometryRect {
                    x: 32,
                    y: 48,
                    width: 96,
                    height: 72,
                },
                ports: Vec::new(),
                style_tags: vec!["entry".to_owned()],
            }],
            corridors: vec![GeometryCorridor {
                id: "corridor.start.goal".to_owned(),
                physical_section: "section.start.goal.open".to_owned(),
                source_connector: "connector.edge_start_goal".to_owned(),
                source_edge: "edge.start.goal".to_owned(),
                source_connectors: vec!["connector.edge_start_goal".to_owned()],
                source_edges: vec!["edge.start.goal".to_owned()],
                traversal_refs: vec![PhysicalTraversalRef {
                    connector_id: "connector.edge_start_goal".to_owned(),
                    edge_id: "edge.start.goal".to_owned(),
                    from_region: "region.start".to_owned(),
                    to_region: "region.goal".to_owned(),
                    traversal: "open".to_owned(),
                    required_item: None,
                }],
                from_room: "room.start".to_owned(),
                to_room: "room.goal".to_owned(),
                traversal_hint: "open".to_owned(),
                semantic_tags: vec!["corridor".to_owned()],
                width: 16,
                from_port: "port.start".to_owned(),
                to_port: "port.goal".to_owned(),
                points: vec![
                    GeometryPoint { x: 128, y: 84 },
                    GeometryPoint { x: 240, y: 84 },
                ],
            }],
            contents: vec![GeometryContent {
                id: "content.start.marker".to_owned(),
                room_id: "room.start".to_owned(),
                source_ref: "start".to_owned(),
                kind: "marker".to_owned(),
                label: "Start".to_owned(),
                tags: vec!["entry".to_owned()],
            }],
            skipped_connectors: Vec::new(),
        };
        let encoded = serde_json::to_string(&geometry).expect("geometry should encode");
        let decoded: Geometry2dArtifact =
            serde_json::from_str(&encoded).expect("geometry should decode");
        assert_eq!(decoded.kind, "asha_procgen.geometry_2d.v1");
        assert_eq!(decoded.rooms[0].rect.width, 96);
        assert_eq!(decoded.corridors[0].points.len(), 2);

        let preview = HtmlPreviewArtifact {
            kind: "asha_procgen.html_preview.v1".to_owned(),
            schema_version: 1,
            preview_id: "preview.test.1".to_owned(),
            geometry_ref: "artifacts/test/geometry.json".to_owned(),
            validation_ref: "artifacts/test/geometry.validation.json".to_owned(),
            html_ref: "artifacts/test/preview.html".to_owned(),
            screenshot_hint: None,
        };
        let encoded = serde_json::to_string(&preview).expect("preview should encode");
        let decoded: HtmlPreviewArtifact =
            serde_json::from_str(&encoded).expect("preview should decode");
        assert_eq!(decoded.kind, "asha_procgen.html_preview.v1");
        assert_eq!(decoded.html_ref, "artifacts/test/preview.html");
    }

    #[test]
    fn validates_lock_key_loop() {
        let intent = SeedIntent {
            kind: "asha_procgen.seed_intent.v1".to_owned(),
            id: "test".to_owned(),
            title: "Test".to_owned(),
            target_dimension: "topology_graph".to_owned(),
            desired_patterns: Vec::new(),
            notes: Vec::new(),
        };
        let mut candidate = create_initial_candidate(&intent, 7);
        assert!(apply_graph_rule(&mut candidate, GraphRule::LockKeyLoop, 8).is_empty());
        let report = validate_graph(&candidate);
        assert!(report.ok, "{report:?}");
    }

    #[test]
    fn validates_v2_graph_grammar_rules() {
        let intent = SeedIntent {
            kind: "asha_procgen.seed_intent.v1".to_owned(),
            id: "v2".to_owned(),
            title: "V2".to_owned(),
            target_dimension: "topology_graph".to_owned(),
            desired_patterns: Vec::new(),
            notes: Vec::new(),
        };
        let mut candidate = create_initial_candidate(&intent, 11);
        for (index, rule) in [
            GraphRule::LockKeyLoop,
            GraphRule::HubSpokeCluster,
            GraphRule::NestedLockKeyChain,
            GraphRule::HazardResourceTradeoff,
            GraphRule::BossPreparationLoop,
            GraphRule::GatedTreasureBranch,
            GraphRule::BranchMergeShortcut,
        ]
        .into_iter()
        .enumerate()
        {
            let diagnostics = apply_graph_rule(&mut candidate, rule, 12 + index as u64);
            assert!(diagnostics.is_empty(), "{rule:?} rejected: {diagnostics:?}");
        }
        let report = validate_graph(&candidate);
        assert!(report.ok, "{report:?}");
        let score = score_graph(&candidate);
        assert_eq!(score.metrics.get("hubCount"), Some(&1.0));
        assert_eq!(score.metrics.get("bossCount"), Some(&1.0));
        assert!(
            score
                .metrics
                .get("pressureEdgeCount")
                .copied()
                .unwrap_or(0.0)
                >= 2.0
        );
    }

    #[test]
    fn rule_metadata_includes_v2_compatibility_hints() {
        let report = rule_metadata_report();
        assert_eq!(report.kind, "asha_procgen.rule_metadata.v1");
        let nested = report
            .rules
            .iter()
            .find(|rule| rule.id == "nested_lock_key_chain")
            .expect("nested lock metadata should exist");
        assert!(nested
            .required_patterns
            .contains(&"lock_key_loop".to_owned()));
        assert!(nested
            .compatibility_hints
            .iter()
            .any(|hint| hint.contains("lock_key_loop first")));
        let merge = report
            .rules
            .iter()
            .find(|rule| rule.id == "branch_merge_shortcut")
            .expect("merge shortcut metadata should exist");
        assert!(merge
            .duplicate_markers
            .contains(&"junction.merge_1".to_owned()));
    }

    #[test]
    fn graph_summary_reports_metrics_and_provenance_tail() {
        let intent = SeedIntent {
            kind: "asha_procgen.seed_intent.v1".to_owned(),
            id: "summary".to_owned(),
            title: "Summary".to_owned(),
            target_dimension: "topology_graph".to_owned(),
            desired_patterns: Vec::new(),
            notes: Vec::new(),
        };
        let mut candidate = create_initial_candidate(&intent, 31);
        candidate.provenance.push(ProvenanceStep {
            step: 1,
            command: "init".to_owned(),
            seed: Some(31),
            summary: "Initialized test candidate".to_owned(),
        });
        assert!(apply_graph_rule(&mut candidate, GraphRule::LockKeyLoop, 32).is_empty());
        candidate.provenance.push(ProvenanceStep {
            step: 2,
            command: "graph apply-rule lock_key_loop".to_owned(),
            seed: Some(32),
            summary: "Applied lock_key_loop".to_owned(),
        });
        let validation = validate_graph(&candidate);
        let score = score_graph(&candidate);
        let summary =
            graph_summary_report(&candidate, &validation, &score).expect("summary should encode");
        assert_eq!(summary.kind, "asha_procgen.graph_summary.v1");
        assert!(summary.validation_ok);
        assert_eq!(summary.node_count, candidate.graph.nodes.len());
        assert!(summary.locked_items.contains(&"item.gate_key_1".to_owned()));
        assert!(summary.tags.contains(&"critical".to_owned()));
        assert_eq!(summary.provenance_tail.len(), 2);
        assert!(summary.metrics.contains_key("lockedEdgeCount"));
    }

    #[test]
    fn fork_candidate_preserves_graph_and_adds_provenance() {
        let intent = SeedIntent {
            kind: "asha_procgen.seed_intent.v1".to_owned(),
            id: "fork".to_owned(),
            title: "Fork".to_owned(),
            target_dimension: "topology_graph".to_owned(),
            desired_patterns: Vec::new(),
            notes: Vec::new(),
        };
        let mut candidate = create_initial_candidate(&intent, 41);
        candidate.provenance.push(ProvenanceStep {
            step: 1,
            command: "init".to_owned(),
            seed: Some(41),
            summary: "Initialized fork source".to_owned(),
        });
        apply_graph_rule(&mut candidate, GraphRule::LockKeyLoop, 42);
        let source_id = candidate.candidate_id.clone();
        let source_graph = candidate.graph.clone();
        let forked = fork_candidate(candidate, "Boss Prep Attempt!", 77);
        assert_eq!(
            forked.candidate_id,
            format!("{source_id}.fork.boss_prep_attempt.77")
        );
        assert_eq!(forked.seed, 77);
        assert_eq!(forked.graph.nodes.len(), source_graph.nodes.len());
        assert_eq!(forked.graph.edges.len(), source_graph.edges.len());
        assert_eq!(forked.provenance.len(), 2);
        let fork_step = forked.provenance.last().expect("fork step should exist");
        assert_eq!(fork_step.command, "graph fork");
        assert_eq!(fork_step.seed, Some(77));
        assert!(fork_step.summary.contains(&source_id));
    }

    #[test]
    fn rejects_duplicate_v2_rule_with_repair_hint() {
        let intent = SeedIntent {
            kind: "asha_procgen.seed_intent.v1".to_owned(),
            id: "duplicate".to_owned(),
            title: "Duplicate".to_owned(),
            target_dimension: "topology_graph".to_owned(),
            desired_patterns: Vec::new(),
            notes: Vec::new(),
        };
        let mut candidate = create_initial_candidate(&intent, 15);
        assert!(apply_graph_rule(&mut candidate, GraphRule::HubSpokeCluster, 16).is_empty());
        let diagnostics = apply_graph_rule(&mut candidate, GraphRule::HubSpokeCluster, 17);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "rule_already_applied" && diagnostic.repair_hint.is_some()
        }));
    }

    #[test]
    fn rejects_missing_required_item() {
        let intent = SeedIntent {
            kind: "asha_procgen.seed_intent.v1".to_owned(),
            id: "broken".to_owned(),
            title: "Broken".to_owned(),
            target_dimension: "topology_graph".to_owned(),
            desired_patterns: Vec::new(),
            notes: Vec::new(),
        };
        let mut candidate = create_initial_candidate(&intent, 9);
        candidate.graph.edges[0].required_item = Some("missing.key".to_owned());
        candidate.graph.edges[0].traversal = TraversalKind::Locked;
        let report = validate_graph(&candidate);
        assert!(!report.ok);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "required_item_unavailable"
                && diagnostic.repair_hint.is_some()));
    }

    #[test]
    fn rejects_incompatible_v2_rule_with_repair_hint() {
        let intent = SeedIntent {
            kind: "asha_procgen.seed_intent.v1".to_owned(),
            id: "incompatible".to_owned(),
            title: "Incompatible".to_owned(),
            target_dimension: "topology_graph".to_owned(),
            desired_patterns: Vec::new(),
            notes: Vec::new(),
        };
        let mut candidate = create_initial_candidate(&intent, 19);
        let diagnostics = apply_graph_rule(&mut candidate, GraphRule::NestedLockKeyChain, 20);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "missing_required_pattern" && diagnostic.repair_hint.is_some()
        }));
    }

    #[test]
    fn validates_v2_structural_repair_hints() {
        let intent = SeedIntent {
            kind: "asha_procgen.seed_intent.v1".to_owned(),
            id: "structural".to_owned(),
            title: "Structural".to_owned(),
            target_dimension: "topology_graph".to_owned(),
            desired_patterns: Vec::new(),
            notes: Vec::new(),
        };
        let mut candidate = create_initial_candidate(&intent, 21);
        candidate.graph.nodes.extend([
            Node {
                id: "hub.broken".to_owned(),
                kind: NodeKind::Junction,
                label: "Broken Hub".to_owned(),
                tags: vec!["hub".to_owned()],
                grants_item: None,
            },
            Node {
                id: "gate.boss_broken".to_owned(),
                kind: NodeKind::Gate,
                label: "Unprepared Boss".to_owned(),
                tags: vec!["boss".to_owned()],
                grants_item: None,
            },
        ]);
        candidate.graph.edges.extend([
            Edge {
                id: "edge.start.broken_hub".to_owned(),
                from: "start".to_owned(),
                to: "hub.broken".to_owned(),
                kind: EdgeKind::OptionalBranch,
                traversal: TraversalKind::Open,
                required_item: None,
                tags: vec!["branch".to_owned()],
            },
            Edge {
                id: "edge.start.boss_broken".to_owned(),
                from: "start".to_owned(),
                to: "gate.boss_broken".to_owned(),
                kind: EdgeKind::CriticalPath,
                traversal: TraversalKind::Open,
                required_item: None,
                tags: vec!["approach".to_owned()],
            },
            Edge {
                id: "edge.boss_broken.goal".to_owned(),
                from: "gate.boss_broken".to_owned(),
                to: "goal".to_owned(),
                kind: EdgeKind::CriticalPath,
                traversal: TraversalKind::Open,
                required_item: None,
                tags: vec!["boss".to_owned()],
            },
        ]);
        let report = validate_graph(&candidate);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "hub_missing_wayfinding_anchor" && diagnostic.repair_hint.is_some()
        }));
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "boss_missing_preparation" && diagnostic.repair_hint.is_some()
        }));
    }

    #[test]
    fn repair_report_prioritizes_missing_provider_actions() {
        let intent = SeedIntent {
            kind: "asha_procgen.seed_intent.v1".to_owned(),
            id: "repair".to_owned(),
            title: "Repair".to_owned(),
            target_dimension: "topology_graph".to_owned(),
            desired_patterns: Vec::new(),
            notes: Vec::new(),
        };
        let mut candidate = create_initial_candidate(&intent, 51);
        candidate.graph.edges[0].required_item = Some("missing.key".to_owned());
        candidate.graph.edges[0].traversal = TraversalKind::Locked;
        let report = repair_report(&candidate).expect("repair report should encode");
        assert_eq!(report.kind, "asha_procgen.repair_report.v1");
        assert!(!report.validation_ok);
        let suggestion = report
            .suggestions
            .iter()
            .find(|suggestion| suggestion.code == "required_item_unavailable")
            .expect("missing provider suggestion should exist");
        assert_eq!(suggestion.severity, Severity::Fatal);
        assert!(suggestion.repair_hint.is_some());
        assert!(suggestion
            .suggested_actions
            .iter()
            .any(|action| action.contains("provider")));
    }

    #[test]
    fn repair_mapping_covers_missing_required_pattern() {
        let diagnostic = fatal_with_hint(
            "missing_required_pattern",
            Some("gate.locked_1"),
            None,
            "nested_lock_key_chain requires an existing first lock/key loop.",
            "Apply lock_key_loop before nested_lock_key_chain.",
        );
        let suggestion = repair_suggestion_for_diagnostic(&diagnostic);
        assert_eq!(suggestion.code, "missing_required_pattern");
        assert!(suggestion
            .suggested_actions
            .iter()
            .any(|action| action.contains("prerequisite pattern")));
    }

    #[test]
    fn repair_report_maps_v2_structural_hints() {
        let intent = SeedIntent {
            kind: "asha_procgen.seed_intent.v1".to_owned(),
            id: "repair-structural".to_owned(),
            title: "Repair Structural".to_owned(),
            target_dimension: "topology_graph".to_owned(),
            desired_patterns: Vec::new(),
            notes: Vec::new(),
        };
        let mut candidate = create_initial_candidate(&intent, 52);
        candidate.graph.nodes.push(Node {
            id: "gate.boss_broken".to_owned(),
            kind: NodeKind::Gate,
            label: "Unprepared Boss".to_owned(),
            tags: vec!["boss".to_owned()],
            grants_item: None,
        });
        candidate.graph.edges.extend([
            Edge {
                id: "edge.start.boss_broken".to_owned(),
                from: "start".to_owned(),
                to: "gate.boss_broken".to_owned(),
                kind: EdgeKind::CriticalPath,
                traversal: TraversalKind::Open,
                required_item: None,
                tags: vec!["approach".to_owned()],
            },
            Edge {
                id: "edge.boss_broken.goal".to_owned(),
                from: "gate.boss_broken".to_owned(),
                to: "goal".to_owned(),
                kind: EdgeKind::CriticalPath,
                traversal: TraversalKind::Open,
                required_item: None,
                tags: vec!["boss".to_owned()],
            },
        ]);
        let report = repair_report(&candidate).expect("repair report should encode");
        let suggestion = report
            .suggestions
            .iter()
            .find(|suggestion| suggestion.code == "boss_missing_preparation")
            .expect("boss preparation suggestion should exist");
        assert!(suggestion
            .suggested_actions
            .iter()
            .any(|action| action.contains("preparation")));
    }

    #[test]
    fn graph_analysis_reports_lock_and_shortcut_signals() {
        let intent = test_intent("analysis");
        let mut candidate = create_initial_candidate(&intent, 61);
        assert!(apply_graph_rule(&mut candidate, GraphRule::LockKeyLoop, 62).is_empty());
        assert!(apply_graph_rule(&mut candidate, GraphRule::OneWayShortcut, 63).is_empty());
        let report = analyze_graph(&candidate).expect("analysis should encode");
        assert_eq!(report.kind, "asha_procgen.graph_analysis.v1");
        assert_eq!(
            report.critical_path.first().map(String::as_str),
            Some("start")
        );
        assert_eq!(
            report.critical_path.last().map(String::as_str),
            Some("goal")
        );
        assert!(report
            .lock_key_order
            .iter()
            .any(|entry| entry.required_item == "item.gate_key_1"
                && entry.provider_reachable_before_lock));
        assert!(report
            .loop_signals
            .iter()
            .any(|signal| signal.signal == "shortcut_loop"));
        assert!(report
            .shortcut_bypass_risks
            .iter()
            .any(|risk| risk.risk == "may_bypass_lock"));
    }

    #[test]
    fn compatible_rules_reports_blocked_duplicate_and_risky() {
        let intent = test_intent("compatibility");
        let mut candidate = create_initial_candidate(&intent, 71);
        let initial = compatible_rules_report(&candidate).expect("compatibility report");
        let nested = initial
            .rules
            .iter()
            .find(|rule| rule.rule == "nested_lock_key_chain")
            .expect("nested rule should be present");
        assert_eq!(nested.status, "blocked");
        assert!(apply_graph_rule(&mut candidate, GraphRule::LockKeyLoop, 72).is_empty());
        assert!(apply_graph_rule(&mut candidate, GraphRule::OneWayShortcut, 73).is_empty());
        let report = compatible_rules_report(&candidate).expect("compatibility report");
        assert_eq!(
            report
                .rules
                .iter()
                .find(|rule| rule.rule == "lock_key_loop")
                .map(|rule| rule.status.as_str()),
            Some("duplicate")
        );
        assert_eq!(
            report
                .rules
                .iter()
                .find(|rule| rule.rule == "one_way_shortcut")
                .map(|rule| rule.status.as_str()),
            Some("duplicate")
        );
        assert_eq!(
            report
                .rules
                .iter()
                .find(|rule| rule.rule == "secret_bypass")
                .map(|rule| rule.status.as_str()),
            Some("risky")
        );
    }

    #[test]
    fn repair_apply_adds_rejoin_and_refuses_ambiguous_target() {
        let intent = test_intent("repair-apply");
        let mut candidate = create_initial_candidate(&intent, 81);
        candidate.graph.nodes.push(Node {
            id: "treasure.loose".to_owned(),
            kind: NodeKind::Treasure,
            label: "Loose Treasure".to_owned(),
            tags: vec!["optional".to_owned(), "reward".to_owned()],
            grants_item: None,
        });
        candidate.graph.edges.push(Edge {
            id: "edge.start.loose".to_owned(),
            from: "start".to_owned(),
            to: "treasure.loose".to_owned(),
            kind: EdgeKind::OptionalBranch,
            traversal: TraversalKind::Open,
            required_item: None,
            tags: vec!["branch".to_owned()],
        });
        let diagnostics = apply_repair_action(
            &mut candidate,
            RepairAction::AddRejoinEdge,
            Some("treasure.loose"),
            82,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(candidate
            .graph
            .edges
            .iter()
            .any(|edge| edge.from == "treasure.loose"
                && edge.to == "goal"
                && edge_has_tag(edge, "repair")));
        let rejected = apply_repair_action(
            &mut candidate,
            RepairAction::AddRejoinEdge,
            Some("start"),
            83,
        );
        assert!(rejected
            .iter()
            .any(|diagnostic| diagnostic.code == "repair_target_ambiguous"));
    }

    #[test]
    fn repair_apply_removes_orphan_node() {
        let intent = test_intent("repair-orphan");
        let mut candidate = create_initial_candidate(&intent, 84);
        candidate.graph.nodes.push(Node {
            id: "secret.orphan".to_owned(),
            kind: NodeKind::Secret,
            label: "Orphan Secret".to_owned(),
            tags: vec!["secret".to_owned()],
            grants_item: None,
        });
        let diagnostics = apply_repair_action(
            &mut candidate,
            RepairAction::RemoveOrphanNode,
            Some("secret.orphan"),
            85,
        );
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(!has_node(&candidate, "secret.orphan"));
    }

    #[test]
    fn topology_fingerprint_is_stable_and_budget_checks_fail_cleanly() {
        let intent = test_intent("fingerprint");
        let mut left = create_initial_candidate(&intent, 91);
        let mut right = create_initial_candidate(&intent, 92);
        assert!(apply_graph_rule(&mut left, GraphRule::LockKeyLoop, 93).is_empty());
        assert!(apply_graph_rule(&mut left, GraphRule::OptionalTreasureDetour, 94).is_empty());
        assert!(apply_graph_rule(&mut right, GraphRule::LockKeyLoop, 95).is_empty());
        assert!(apply_graph_rule(&mut right, GraphRule::OptionalTreasureDetour, 96).is_empty());
        assert_eq!(topology_fingerprint(&left), topology_fingerprint(&right));
        let budgets = IntentBudget {
            require_hub: Some(true),
            min_optional_branches: Some(3),
            max_dead_ends: Some(0),
            ..IntentBudget::default()
        };
        let checks = budget_checks(Some(&budgets), &score_graph(&left), &left);
        assert!(checks
            .iter()
            .any(|check| check.code == "require_hub" && !check.ok));
        assert!(checks
            .iter()
            .any(|check| check.code == "max_dead_ends" && check.ok));
    }

    #[test]
    fn spatial_intent_annotation_marks_core_intents() {
        let intent = test_intent("spatial");
        let mut candidate = create_initial_candidate(&intent, 101);
        for (index, rule) in [
            GraphRule::LockKeyLoop,
            GraphRule::HubSpokeCluster,
            GraphRule::HazardResourceTradeoff,
            GraphRule::OneWayShortcut,
        ]
        .into_iter()
        .enumerate()
        {
            assert!(apply_graph_rule(&mut candidate, rule, 102 + index as u64).is_empty());
        }
        let report = spatial_intent_report(&candidate, None).expect("spatial intent report");
        assert!(report.annotations.iter().any(|annotation| {
            annotation.target_id == "hub.central_1"
                && annotation.intents.contains(&"landmark_hub".to_owned())
        }));
        assert!(report.annotations.iter().any(|annotation| {
            annotation.target_id == "edge.gate_1.goal"
                && annotation
                    .intents
                    .contains(&"visible_before_reachable".to_owned())
        }));
        assert!(report.annotations.iter().any(|annotation| {
            annotation
                .intents
                .contains(&"shortcut_connector".to_owned())
        }));
        assert!(report
            .annotations
            .iter()
            .any(|annotation| { annotation.intents.contains(&"pressure_path".to_owned()) }));
    }

    #[test]
    fn intermediate_breakdown_validates_and_catches_invalid_cases() {
        let intent = test_intent("breakdown");
        let mut candidate = create_initial_candidate(&intent, 111);
        assert!(apply_graph_rule(&mut candidate, GraphRule::LockKeyLoop, 112).is_empty());
        assert!(apply_graph_rule(&mut candidate, GraphRule::HubSpokeCluster, 113).is_empty());
        let annotations = spatial_intent_report(&candidate, None).expect("spatial intent report");
        let mut breakdown = intermediate_breakdown(
            &candidate,
            &annotations,
            Path::new("artifacts/test/spatial-intent.json"),
        )
        .expect("breakdown should encode");
        let report = validate_intermediate_breakdown(&breakdown);
        assert!(report.ok, "{report:?}");
        breakdown.regions.retain(|region| region.role != "goal");
        let missing_goal = validate_intermediate_breakdown(&breakdown);
        assert!(missing_goal.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "intermediate_goal_missing" && diagnostic.severity == Severity::Fatal
        }));
        let connector = breakdown
            .connectors
            .first_mut()
            .expect("connector should exist");
        connector.to_region = "region.missing".to_owned();
        connector.intents.push("vertical_candidate".to_owned());
        let invalid_connector = validate_intermediate_breakdown(&breakdown);
        assert!(invalid_connector
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "intermediate_connector_endpoint_missing" }));
        assert!(invalid_connector.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "intermediate_vertical_candidate_unsupported"
        }));
    }

    #[test]
    fn intermediate_breakdown_emits_geometry_prep_hints() {
        let intent = test_intent("geometry-prep");
        let mut candidate = create_initial_candidate(&intent, 121);
        for (index, rule) in [
            GraphRule::LockKeyLoop,
            GraphRule::HubSpokeCluster,
            GraphRule::HazardResourceTradeoff,
            GraphRule::GatedTreasureBranch,
            GraphRule::BranchMergeShortcut,
        ]
        .into_iter()
        .enumerate()
        {
            assert!(apply_graph_rule(&mut candidate, rule, 122 + index as u64).is_empty());
        }
        let annotations = spatial_intent_report(&candidate, None).expect("spatial intent report");
        let breakdown = intermediate_breakdown(
            &candidate,
            &annotations,
            Path::new("artifacts/test/spatial-intent.json"),
        )
        .expect("breakdown should encode");
        assert_eq!(breakdown.schema_version, 2);
        let hub = breakdown
            .regions
            .iter()
            .find(|region| region.node_ids == vec!["hub.central_1".to_owned()])
            .expect("hub region should exist");
        assert_eq!(hub.geometry_role, "landmark_junction");
        assert_eq!(hub.footprint_class, "hub");
        assert_eq!(hub.scale_band, "large");
        assert_eq!(hub.anchor_quality, "explicit");
        assert!(hub
            .entrance_expectations
            .contains(&"multi_spoke_orientation".to_owned()));

        let gate = breakdown
            .regions
            .iter()
            .find(|region| region.node_ids == vec!["gate.locked_1".to_owned()])
            .expect("gate region should exist");
        assert_eq!(gate.geometry_role, "threshold");
        assert!(gate
            .entrance_expectations
            .contains(&"locked_threshold_preview".to_owned()));

        let hazard = breakdown
            .regions
            .iter()
            .find(|region| region.node_ids == vec!["hazard.sluice_1".to_owned()])
            .expect("hazard region should exist");
        assert_eq!(hazard.footprint_class, "pressure_lane");
        assert!(hazard
            .entrance_expectations
            .contains(&"readable_hazard_approach".to_owned()));

        let reward = breakdown
            .regions
            .iter()
            .find(|region| region.node_ids == vec!["treasure.gated_1".to_owned()])
            .expect("reward region should exist");
        assert_eq!(reward.geometry_role, "reward_pocket");
        assert_eq!(reward.scale_band, "small");

        let locked_connector = breakdown
            .connectors
            .iter()
            .find(|connector| connector.edge_id == "edge.gate_1.goal")
            .expect("locked connector should exist");
        assert_eq!(locked_connector.traversal_hint, "locked");
        assert!(locked_connector
            .affordances
            .contains(&"locked_threshold".to_owned()));
        assert!(locked_connector
            .constraint_refs
            .iter()
            .any(|reference| reference.contains("preserve_lock_preview")));

        let shortcut_connector = breakdown
            .connectors
            .iter()
            .find(|connector| connector.edge_id == "edge.merge_1.goal.shortcut")
            .expect("shortcut connector should exist");
        assert!(shortcut_connector
            .affordances
            .contains(&"shortcut_link".to_owned()));
        assert!(shortcut_connector
            .constraint_refs
            .iter()
            .any(|reference| reference.contains("preserve_shortcut_connector")));

        assert!(breakdown.constraints.iter().any(|constraint| {
            constraint.target_type == "edge"
                && constraint
                    .graph_refs
                    .contains(&"edge.gate_1.goal".to_owned())
                && constraint
                    .source_intents
                    .contains(&"visible_before_reachable".to_owned())
        }));
    }

    #[test]
    fn intermediate_validation_catches_geometry_prep_gaps() {
        let intent = test_intent("geometry-prep-validation");
        let mut candidate = create_initial_candidate(&intent, 131);
        for (index, rule) in [
            GraphRule::LockKeyLoop,
            GraphRule::HubSpokeCluster,
            GraphRule::GatedTreasureBranch,
            GraphRule::BranchMergeShortcut,
            GraphRule::SecretBypass,
        ]
        .into_iter()
        .enumerate()
        {
            assert!(apply_graph_rule(&mut candidate, rule, 132 + index as u64).is_empty());
        }
        let annotations = spatial_intent_report(&candidate, None).expect("spatial intent report");
        let breakdown = intermediate_breakdown(
            &candidate,
            &annotations,
            Path::new("artifacts/test/spatial-intent.json"),
        )
        .expect("breakdown should encode");
        let valid = validate_intermediate_breakdown(&breakdown);
        assert!(valid.ok, "{valid:?}");

        let mut missing_affordance = breakdown.clone();
        missing_affordance
            .connectors
            .first_mut()
            .expect("connector should exist")
            .affordances
            .clear();
        let report = validate_intermediate_breakdown(&missing_affordance);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "intermediate_connector_affordance_missing" }));

        let mut missing_gated_constraint = breakdown.clone();
        let locked = missing_gated_constraint
            .connectors
            .iter_mut()
            .find(|connector| connector.edge_id == "edge.gate_1.goal")
            .expect("locked connector should exist");
        locked.constraint_refs.clear();
        let report = validate_intermediate_breakdown(&missing_gated_constraint);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "intermediate_gated_constraint_missing"));

        let mut missing_shortcut_affordance = breakdown.clone();
        let shortcut = missing_shortcut_affordance
            .connectors
            .iter_mut()
            .find(|connector| connector.edge_id == "edge.merge_1.goal.shortcut")
            .expect("shortcut connector should exist");
        shortcut
            .affordances
            .retain(|affordance| affordance != "shortcut_link");
        let report = validate_intermediate_breakdown(&missing_shortcut_affordance);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "intermediate_shortcut_affordance_missing" }));

        let mut missing_region_prep = breakdown.clone();
        missing_region_prep
            .regions
            .first_mut()
            .expect("region should exist")
            .geometry_role
            .clear();
        let report = validate_intermediate_breakdown(&missing_region_prep);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "intermediate_region_geometry_prep_missing" }));

        let mut unsupported_3d = breakdown;
        unsupported_3d
            .connectors
            .first_mut()
            .expect("connector should exist")
            .affordances
            .push("vertical_shaft".to_owned());
        let report = validate_intermediate_breakdown(&unsupported_3d);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "intermediate_3d_claim_unsupported"));
    }

    #[test]
    fn geometry_emit_2d_places_variable_non_overlapping_rooms() {
        let intent = test_intent("geometry-emit");
        let mut candidate = create_initial_candidate(&intent, 141);
        for (index, rule) in [GraphRule::LockKeyLoop]
        .into_iter()
        .enumerate()
        {
            assert!(apply_graph_rule(&mut candidate, rule, 142 + index as u64).is_empty());
        }
        let annotations = spatial_intent_report(&candidate, None).expect("spatial intent report");
        let intermediate = intermediate_breakdown(
            &candidate,
            &annotations,
            Path::new("artifacts/test/spatial-intent.json"),
        )
        .expect("breakdown should encode");
        let args = GeometryEmit2dArgs {
            candidate: PathBuf::from("artifacts/test/candidate.json"),
            intermediate: PathBuf::from("artifacts/test/intermediate-breakdown.json"),
            connection_plan: PathBuf::from("artifacts/test/physical-connection-plan.json"),
            layout_policy: None,
            seed: 150,
            out: PathBuf::from("artifacts/test/geometry.json"),
        };
        let connection_plan = test_connection_plan(&candidate, &intermediate);
        let geometry = emit_geometry_2d(&candidate, &intermediate, &connection_plan, &args, 150)
            .expect("geometry should emit");
        assert_eq!(geometry.kind, "asha_procgen.geometry_2d.v1");
        assert_eq!(geometry.rooms.len(), intermediate.regions.len());
        assert_eq!(geometry.corridors.len(), connection_plan.sections.len());
        assert_eq!(geometry.skipped_connectors.len(), 0);
        assert!(geometry.bounds.width > 640);
        let gate = geometry
            .rooms
            .iter()
            .find(|room| room.source_region == "region.gate_locked_1")
            .expect("gate room should exist");
        let start = geometry
            .rooms
            .iter()
            .find(|room| room.source_region == "region.start")
            .expect("start room should exist");
        let goal = geometry
            .rooms
            .iter()
            .find(|room| room.source_region == "region.goal")
            .expect("goal room should exist");
        assert!(gate.style_tags.contains(&"threshold".to_owned()));
        assert!(start.style_tags.contains(&"entry".to_owned()));
        assert!(goal.style_tags.contains(&"destination".to_owned()));
        for (index, left) in geometry.rooms.iter().enumerate() {
            for right in geometry.rooms.iter().skip(index + 1) {
                assert!(
                    !rectangles_overlap(&left.rect, &right.rect),
                    "{} overlaps {}",
                    left.id,
                    right.id
                );
            }
        }
    }

    #[test]
    fn geometry_emit_2d_routes_semantic_corridors() {
        let intent = test_intent("geometry-corridors");
        let mut candidate = create_initial_candidate(&intent, 251);
        for (index, rule) in [GraphRule::LockKeyLoop]
        .into_iter()
        .enumerate()
        {
            assert!(apply_graph_rule(&mut candidate, rule, 252 + index as u64).is_empty());
        }
        let annotations = spatial_intent_report(&candidate, None).expect("spatial intent report");
        let intermediate = intermediate_breakdown(
            &candidate,
            &annotations,
            Path::new("artifacts/test/spatial-intent.json"),
        )
        .expect("breakdown should encode");
        let args = GeometryEmit2dArgs {
            candidate: PathBuf::from("artifacts/test/candidate.json"),
            intermediate: PathBuf::from("artifacts/test/intermediate-breakdown.json"),
            connection_plan: PathBuf::from("artifacts/test/physical-connection-plan.json"),
            layout_policy: None,
            seed: 253,
            out: PathBuf::from("artifacts/test/geometry.json"),
        };
        let connection_plan = test_connection_plan(&candidate, &intermediate);
        let geometry = emit_geometry_2d(&candidate, &intermediate, &connection_plan, &args, 253)
            .expect("geometry should emit");
        assert_eq!(geometry.corridors.len(), connection_plan.sections.len());
        assert!(geometry.skipped_connectors.is_empty());

        let locked = geometry
            .corridors
            .iter()
            .find(|corridor| corridor.source_edge == "edge.gate_1.goal")
            .expect("locked threshold corridor should exist");
        assert_eq!(locked.width, 18);
        assert!(locked
            .semantic_tags
            .contains(&"locked_threshold".to_owned()));

        let rooms_by_id = geometry
            .rooms
            .iter()
            .map(|room| (room.id.as_str(), room))
            .collect::<BTreeMap<_, _>>();
        for corridor in &geometry.corridors {
            let first = corridor
                .points
                .first()
                .expect("corridor should have a start point");
            let last = corridor
                .points
                .last()
                .expect("corridor should have an end point");
            let from_room = rooms_by_id
                .get(corridor.from_room.as_str())
                .expect("corridor from room should resolve");
            let to_room = rooms_by_id
                .get(corridor.to_room.as_str())
                .expect("corridor to room should resolve");
            assert!(
                point_on_rect_boundary(first, &from_room.rect),
                "{} does not start on {}",
                corridor.id,
                from_room.id
            );
            assert!(
                point_on_rect_boundary(last, &to_room.rect),
                "{} does not end on {}",
                corridor.id,
                to_room.id
            );
        }
    }

    #[test]
    fn geometry_emit_2d_annotates_room_contents() {
        let intent = test_intent("geometry-contents");
        let mut candidate = create_initial_candidate(&intent, 351);
        for (index, rule) in [GraphRule::LockKeyLoop]
        .into_iter()
        .enumerate()
        {
            assert!(apply_graph_rule(&mut candidate, rule, 352 + index as u64).is_empty());
        }
        let annotations = spatial_intent_report(&candidate, None).expect("spatial intent report");
        let intermediate = intermediate_breakdown(
            &candidate,
            &annotations,
            Path::new("artifacts/test/spatial-intent.json"),
        )
        .expect("breakdown should encode");
        let args = GeometryEmit2dArgs {
            candidate: PathBuf::from("artifacts/test/candidate.json"),
            intermediate: PathBuf::from("artifacts/test/intermediate-breakdown.json"),
            connection_plan: PathBuf::from("artifacts/test/physical-connection-plan.json"),
            layout_policy: None,
            seed: 353,
            out: PathBuf::from("artifacts/test/geometry.json"),
        };
        let connection_plan = test_connection_plan(&candidate, &intermediate);
        let geometry = emit_geometry_2d(&candidate, &intermediate, &connection_plan, &args, 353)
            .expect("geometry should emit");
        let content_kinds = geometry
            .contents
            .iter()
            .map(|content| content.kind.as_str())
            .collect::<BTreeSet<_>>();
        for expected in ["key_pickup", "locked_gate"] {
            assert!(
                content_kinds.contains(expected),
                "{expected} content missing"
            );
        }
        for content in &geometry.contents {
            assert!(!content.label.is_empty());
            assert!(content.source_ref.contains("node:"));
            assert!(content.source_ref.contains("region:"));
            assert!(geometry.rooms.iter().any(|room| room.id == content.room_id));
            assert!(
                content.tags.contains(&content.kind),
                "{} tags should include kind",
                content.id
            );
        }
    }

    #[test]
    fn geometry_validate_2d_accepts_valid_full_stack_geometry() {
        let geometry = full_stack_geometry_fixture(451);
        let report = validate_geometry_2d(&geometry);
        assert!(report.ok, "{:?}", report.diagnostics);
        assert_eq!(report.kind, "asha_procgen.validation.geometry_2d.v1");
    }

    #[test]
    fn geometry_validate_2d_catches_invalid_cases() {
        let geometry = full_stack_geometry_fixture(551);

        let mut overlapping = geometry.clone();
        overlapping.rooms[1].rect = overlapping.rooms[0].rect.clone();
        let report = validate_geometry_2d(&overlapping);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "geometry_room_overlap"));

        let mut missing_corridors = geometry.clone();
        missing_corridors.corridors.clear();
        let report = validate_geometry_2d(&missing_corridors);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "geometry_connector_coverage_missing"));

        let mut broken_mapping = geometry.clone();
        broken_mapping.corridors[0]
            .source_edges
            .push("edge.not_in_traversal_refs".to_owned());
        let report = validate_geometry_2d(&broken_mapping);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "geometry_corridor_traversal_mapping_mismatch"
        }));

        let mut bad_search_evidence = geometry.clone();
        bad_search_evidence.layout_search.effective_spacing.column_gap +=
            GEOMETRY_ROUTE_GRID;
        let report = validate_geometry_2d(&bad_search_evidence);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "geometry_layout_search_spacing_mismatch"
        }));

        let mut bad_content_anchor = geometry.clone();
        bad_content_anchor
            .contents
            .first_mut()
            .expect("content should exist")
            .room_id = "room.missing".to_owned();
        let report = validate_geometry_2d(&bad_content_anchor);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "geometry_content_room_missing"));

        let mut unreachable_goal = geometry;
        let goal_room_id = unreachable_goal
            .rooms
            .iter()
            .find(|room| room.role == "goal")
            .expect("goal room should exist")
            .id
            .clone();
        unreachable_goal
            .corridors
            .retain(|corridor| corridor.to_room != goal_room_id);
        let report = validate_geometry_2d(&unreachable_goal);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "geometry_goal_unreachable"));
    }

    #[test]
    fn preview_html_renders_required_sections() {
        let geometry = full_stack_geometry_fixture(651);
        let validation = validate_geometry_2d(&geometry);
        assert!(validation.ok, "{:?}", validation.diagnostics);
        let html = render_geometry_preview_html(
            &geometry,
            &validation,
            "artifacts/test/geometry.json",
            "artifacts/test/geometry.validation.json",
        );
        for expected in [
            "<!doctype html>",
            "data-preview-kind=\"asha_procgen.html_preview.v1\"",
            "<svg",
            "<polyline",
            "<rect",
            "Dungeon Preview",
            "Validation: ok",
            "Key Pickup",
        ] {
            assert!(html.contains(expected), "{expected} missing");
        }
    }

    #[test]
    fn preview_html_refuses_invalid_geometry_without_override() {
        let mut geometry = full_stack_geometry_fixture(751);
        geometry.rooms[1].rect = geometry.rooms[0].rect.clone();
        let validation = validate_geometry_2d(&geometry);
        assert!(!validation.ok);
        let error = validate_preview_inputs(&geometry, &validation, false)
            .expect_err("invalid geometry should need explicit preview override");
        assert!(error.contains("--allow-invalid"));
        validate_preview_inputs(&geometry, &validation, true)
            .expect("allow-invalid should render diagnostics");
        let html = render_geometry_preview_html(
            &geometry,
            &validation,
            "artifacts/test/geometry.json",
            "artifacts/test/geometry.validation.json",
        );
        assert!(html.contains("Validation: invalid"));
        assert!(html.contains("geometry_room_overlap"));
    }

    #[test]
    fn piece_plan_emits_explicit_room_corridor_and_semantic_requirements() {
        let (candidate, intermediate, geometry) = full_stack_geometry_inputs(851);
        let args = BuildEmitPiecePlanArgs {
            candidate: PathBuf::from("artifacts/test/candidate.json"),
            intermediate: PathBuf::from("artifacts/test/intermediate-breakdown.json"),
            geometry: PathBuf::from("artifacts/test/geometry.json"),
            corridor_realization: CorridorRealization::Hybrid,
            out: PathBuf::from("artifacts/test/piece-plan.json"),
        };
        let plan = emit_piece_build_plan(&candidate, &intermediate, &geometry, &args)
            .expect("piece plan should emit");

        assert_eq!(plan.kind, "asha_procgen.piece_build_plan.v1");
        assert_eq!(plan.candidate_id, candidate.candidate_id);
        assert_eq!(plan.geometry_id, geometry.geometry_id);
        assert!(plan.requirements.len() > geometry.rooms.len() + geometry.corridors.len() * 2);
        assert_eq!(plan.links.len(), geometry.corridors.len());
        assert!(plan
            .links
            .iter()
            .all(|link| link.route_points.len() >= 2));

        let requirement_kinds = plan
            .requirements
            .iter()
            .map(|requirement| requirement.kind.as_str())
            .collect::<BTreeSet<_>>();
        for required in ["room", "threshold", "key", "corridor", "bend"] {
            assert!(requirement_kinds.contains(required), "{required} requirement missing");
        }

        for corridor in &geometry.corridors {
            let corridor_ref = format!("geometryCorridor:{}", corridor.id);
            let corridor_requirements = plan
                .requirements
                .iter()
                .filter(|requirement| requirement.source_refs.contains(&corridor_ref))
                .collect::<Vec<_>>();
            assert!(
                corridor_requirements
                    .iter()
                    .any(|requirement| {
                        requirement.kind == "corridor" || requirement.kind == "bend"
                    }),
                "{} missing route-covering catalog piece requirements",
                corridor.id
            );
            assert!(corridor_requirements.iter().all(|requirement| {
                requirement.required_shape_tags.contains(&"corridor".to_owned())
                    && requirement
                        .required_shape_tags
                        .iter()
                        .any(|tag| tag.starts_with("span_") || tag.starts_with("bend_"))
            }));
        }
        assert!(!requirement_kinds.contains("junction"));

        let sockets = plan
            .content_requirements
            .iter()
            .map(|requirement| requirement.required_socket.as_str())
            .collect::<BTreeSet<_>>();
        for required in ["gate_line", "key_pickup"] {
            assert!(sockets.contains(required), "{required} content socket missing");
        }

        let link_tags = plan
            .links
            .iter()
            .flat_map(|link| link.tags.iter().map(String::as_str))
            .collect::<BTreeSet<_>>();
        for required in ["locked_threshold"] {
            assert!(link_tags.contains(required), "{required} link tag missing");
        }
        assert!(
            plan.links
                .iter()
                .any(|link| link.traversal == "open"),
            "normal open corridor link missing"
        );

        let locked_requirement = plan
            .requirements
            .iter()
            .find(|requirement| requirement.tags.contains(&"locked_threshold".to_owned()))
            .expect("locked corridor requirement should exist");
        assert!(
            locked_requirement
                .source_refs
                .iter()
                .any(|source_ref| source_ref.starts_with("edge:"))
        );

        let pure_args = BuildEmitPiecePlanArgs {
            corridor_realization: CorridorRealization::Catalog,
            out: PathBuf::from("artifacts/test/pure-catalog-piece-plan.json"),
            ..args
        };
        let pure_plan = emit_piece_build_plan(
            &candidate,
            &intermediate,
            &geometry,
            &pure_args,
        )
        .expect("pure catalog piece plan should emit");
        assert_ne!(pure_plan.plan_id, plan.plan_id);
        assert!(pure_plan.links.len() > geometry.corridors.len());
        assert!(pure_plan
            .links
            .iter()
            .all(|link| link.route_points.is_empty()));
        for corridor in &geometry.corridors {
            let section_links = pure_plan
                .links
                .iter()
                .filter(|link| link.source_section == corridor.physical_section)
                .collect::<Vec<_>>();
            assert!(
                section_links.len() >= 2,
                "{} must be an explicit room-to-prefab-to-room chain",
                corridor.physical_section
            );
        }
    }

    #[test]
    fn catalog_corridor_span_selection_is_deterministic_and_bounded() {
        assert_eq!(
            catalog_straight_spans(1).expect("short span"),
            vec![CatalogStraightSpan::Short]
        );
        assert_eq!(
            catalog_straight_spans(8).expect("medium span"),
            vec![CatalogStraightSpan::Medium]
        );
        assert_eq!(
            catalog_straight_spans(11).expect("long span"),
            vec![CatalogStraightSpan::Long]
        );
        assert_eq!(
            catalog_straight_spans(20).expect("mixed span"),
            vec![CatalogStraightSpan::Long, CatalogStraightSpan::Short]
        );
        assert!(catalog_straight_spans(10_000).is_err());
    }

    #[test]
    fn pure_catalog_realization_glues_prefabs_without_generated_cells() {
        let room_shape = test_catalog_shape(
            "shape.room.asymmetric",
            &["room"],
            &["identity", "rotate90", "rotate180", "rotate270"],
            vec![test_catalog_exit("door", "east")],
            Vec::new(),
            &[],
        );
        let room_shape = room_shape;
        let mut corridor_shape = test_catalog_shape(
            "shape.corridor.unit",
            &["corridor"],
            &["identity", "rotate90", "rotate180", "rotate270"],
            vec![
                test_catalog_exit("entry", "west"),
                test_catalog_exit("exit", "east"),
            ],
            Vec::new(),
            &[],
        );
        corridor_shape.footprint = vec![
            GridCell { x: 0, y: 0 },
            GridCell { x: 1, y: 0 },
            GridCell { x: 2, y: 0 },
        ];
        corridor_shape.exits[1].x = 3;
        let catalog = test_shape_catalog(vec![room_shape, corridor_shape]);
        let mut start = test_piece_requirement(
            "piece.start",
            "room",
            vec![test_piece_exit("door", "east")],
            Vec::new(),
            &[],
        );
        start.role = "start".to_owned();
        start.tags.push("start".to_owned());
        start
            .placement_hints
            .push("geometryRect:0:0:12:6".to_owned());
        let mut corridor = test_piece_requirement(
            "piece.corridor",
            "corridor",
            vec![
                test_piece_exit("entry", "west"),
                test_piece_exit("exit", "east"),
            ],
            Vec::new(),
            &[],
        );
        corridor
            .source_refs
            .push("physicalSection:section.test".to_owned());
        corridor
            .placement_hints
            .push("point:12:0".to_owned());
        corridor
            .placement_hints
            .push("segment:12:0:30:0".to_owned());
        let mut goal = test_piece_requirement(
            "piece.goal",
            "room",
            vec![test_piece_exit("door", "west")],
            Vec::new(),
            &[],
        );
        goal.role = "goal".to_owned();
        goal.tags.push("goal".to_owned());
        goal
            .placement_hints
            .push("geometryRect:30:0:6:6".to_owned());
        let mut plan = test_piece_plan(vec![start, corridor, goal]);
        plan.links = vec![
            test_piece_link(
                "link.start_corridor",
                "piece.start",
                "door",
                "piece.corridor",
                "entry",
                "section.test",
            ),
            test_piece_link(
                "link.corridor_goal",
                "piece.corridor",
                "exit",
                "piece.goal",
                "door",
                "section.test",
            ),
        ];
        plan.links[0].traversal = "locked".to_owned();
        plan.links[0].required_item = Some("item.test_key".to_owned());
        plan.links[0].tags.push("locked_threshold".to_owned());
        let match_args = test_match_args(6196);
        let shape_match = match_shapes(&catalog, &plan, &match_args);
        assert!(shape_match.ok);
        let assemble_args = BuildAssembleArgs {
            catalog: match_args.catalog.clone(),
            piece_plan: match_args.piece_plan.clone(),
            shape_match: match_args.out.clone(),
            connectivity: GridConnectivity::FourWay,
            out: PathBuf::from("artifacts/test/pure-catalog-placement.json"),
        };
        let placement = assemble_piece_placement(&catalog, &plan, &shape_match, &assemble_args)
            .expect("exact prefab chain should place");
        let repeated = assemble_piece_placement(&catalog, &plan, &shape_match, &assemble_args)
            .expect("exact prefab chain should repeat");
        assert_eq!(
            serde_json::to_value(&placement).expect("placement"),
            serde_json::to_value(&repeated).expect("repeated placement")
        );
        assert_eq!(placement.corridor_realization, CorridorRealization::Catalog);
        assert!(placement.connection_cells.is_empty());
        assert!(placement
            .glued_exits
            .iter()
            .all(|glued| glued.route_points.is_empty()));
        assert!(placement
            .glued_exits
            .iter()
            .all(|glued| pure_catalog_glue_is_direct(glued, &placement.occupied_cells)));
        assert_eq!(
            placement
                .catalog_search
                .as_ref()
                .expect("search evidence")
                .selected
                .len(),
            3
        );
        let search = placement.catalog_search.as_ref().expect("search evidence");
        let start_decision = search
            .selected
            .iter()
            .find(|decision| decision.piece_id == "piece.start")
            .expect("start decision");
        assert_eq!(start_decision.origin, GridCell { x: 1, y: 0 });
        assert_eq!(
            start_decision.origin_bounds,
            Some(CatalogGridBounds {
                min_x: 0,
                max_x: 1,
                min_y: 0,
                max_y: 0,
            })
        );
        assert!(
            search.room_origin_attempts > search.selected.len() as u32,
            "the preferred x=0 origin must fail before the later in-zone x=1 origin succeeds"
        );
        assert!(search
            .selected
            .iter()
            .find(|decision| decision.piece_id == "piece.corridor")
            .and_then(|decision| decision.lane_constraint.as_ref())
            .is_some());
        assert_eq!(placement.gate_portals.len(), 1);
        assert_eq!(
            placement.gate_portals[0].required_item.as_deref(),
            Some("item.test_key")
        );
        assert!(placement.gate_portals[0].cells.iter().all(|portal| {
            placement
                .occupied_cells
                .iter()
                .any(|cell| cell.x == portal.x && cell.y == portal.y)
        }));
        assert!(validate_piece_placement(&placement).ok);

        let mut routed_tamper = placement.clone();
        routed_tamper.connection_cells.push(PlacementCellRef {
            instance_id: "connection.forbidden".to_owned(),
            x: 99,
            y: 99,
        });
        let routed_report = validate_piece_placement(&routed_tamper);
        assert!(routed_report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "piece_catalog_has_procedural_connection_cells"
        }));

        let mut glue_tamper = placement.clone();
        glue_tamper.glued_exits[0].to_cell.x += 1;
        let glue_report = validate_piece_placement(&glue_tamper);
        assert!(glue_report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "piece_catalog_direct_glue_invalid"
        }));

        let mut bounds_tamper = placement.clone();
        let start_decision = bounds_tamper
            .catalog_search
            .as_mut()
            .expect("search")
            .selected
            .iter_mut()
            .find(|decision| decision.piece_id == "piece.start")
            .expect("start decision");
        start_decision.origin_bounds.as_mut().expect("bounds").max_x = 0;
        let bounds_report = validate_piece_placement(&bounds_tamper);
        assert!(bounds_report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "piece_catalog_origin_bounds_invalid"
        }));

        let mut outside_lane_plan = plan.clone();
        outside_lane_plan
            .requirements
            .iter_mut()
            .find(|requirement| requirement.piece_id == "piece.corridor")
            .expect("corridor requirement")
            .placement_hints = vec![
            "point:600:0".to_owned(),
            "segment:600:0:630:0".to_owned(),
        ];
        let outside_lane_match = match_shapes(&catalog, &outside_lane_plan, &match_args);
        let outside_lane_error = assemble_piece_placement(
            &catalog,
            &outside_lane_plan,
            &outside_lane_match,
            &assemble_args,
        )
        .expect_err("a fixed prefab chain outside the serialized lane must reject");
        let outside_lane_evidence = pure_catalog_error_evidence(&outside_lane_error);
        assert_eq!(
            outside_lane_evidence["failure"]["reason"],
            "geometry_constraint_rejected"
        );
        assert!(outside_lane_evidence["failure"]["laneEnvelope"].is_object());

        for max_decisions in [1_u32, 2] {
            let mut bounded_catalog = catalog.clone();
            bounded_catalog.catalog_search_policy.max_decisions = max_decisions;
            let error = assemble_piece_placement(
                &bounded_catalog,
                &plan,
                &shape_match,
                &assemble_args,
            )
            .expect_err("hard decision budget must stop before the next sibling");
            let evidence = pure_catalog_error_evidence(&error);
            assert_eq!(evidence["budgets"]["maxDecisions"], max_decisions);
            assert_eq!(evidence["budgets"]["decisions"], max_decisions);
        }

        let mut impossible_origin_plan = plan.clone();
        impossible_origin_plan
            .requirements
            .iter_mut()
            .find(|requirement| requirement.piece_id == "piece.goal")
            .expect("goal requirement")
            .placement_hints = vec!["geometryRect:36:0:6:6".to_owned()];
        let impossible_origin_match =
            match_shapes(&catalog, &impossible_origin_plan, &match_args);
        for max_backtracks in [1_u32, 2] {
            let mut bounded_catalog = catalog.clone();
            bounded_catalog.catalog_search_policy.max_backtracks = max_backtracks;
            let error = assemble_piece_placement(
                &bounded_catalog,
                &impossible_origin_plan,
                &impossible_origin_match,
                &assemble_args,
            )
            .expect_err("hard backtrack budget must stop before another sibling");
            let evidence = pure_catalog_error_evidence(&error);
            assert_eq!(evidence["budgets"]["maxBacktracks"], max_backtracks);
            assert_eq!(evidence["budgets"]["backtracks"], max_backtracks);
        }
    }

    #[test]
    fn pure_catalog_search_closes_a_loop_by_rotating_room_ports() {
        let bend_room = test_catalog_shape(
            "shape.room.asymmetric_bend",
            &["room"],
            &["identity", "rotate90", "rotate180", "rotate270"],
            vec![
                test_catalog_exit("east_port", "east"),
                test_catalog_exit("south_port", "south"),
            ],
            Vec::new(),
            &[],
        );
        let catalog = test_shape_catalog(vec![bend_room]);
        let mut a = test_piece_requirement(
            "piece.a",
            "room",
            vec![
                test_piece_exit("a_b", "east"),
                test_piece_exit("a_d", "south"),
            ],
            Vec::new(),
            &[],
        );
        a.role = "start".to_owned();
        a.tags.push("start".to_owned());
        let b = test_piece_requirement(
            "piece.b",
            "room",
            vec![
                test_piece_exit("b_c", "east"),
                test_piece_exit("b_a", "south"),
            ],
            Vec::new(),
            &[],
        );
        let c = test_piece_requirement(
            "piece.c",
            "room",
            vec![
                test_piece_exit("c_d", "east"),
                test_piece_exit("c_b", "south"),
            ],
            Vec::new(),
            &[],
        );
        let mut d = test_piece_requirement(
            "piece.d",
            "room",
            vec![
                test_piece_exit("d_a", "east"),
                test_piece_exit("d_c", "south"),
            ],
            Vec::new(),
            &[],
        );
        d.role = "goal".to_owned();
        d.tags.push("goal".to_owned());
        let mut plan = test_piece_plan(vec![a, b, c, d]);
        plan.links = vec![
            test_piece_link("link.a_b", "piece.a", "a_b", "piece.b", "b_a", "section.loop"),
            test_piece_link("link.b_c", "piece.b", "b_c", "piece.c", "c_b", "section.loop"),
            test_piece_link("link.c_d", "piece.c", "c_d", "piece.d", "d_c", "section.loop"),
            test_piece_link("link.d_a", "piece.d", "d_a", "piece.a", "a_d", "section.loop"),
        ];
        let match_args = test_match_args(6186);
        let shape_match = match_shapes(&catalog, &plan, &match_args);
        assert!(shape_match.ok);
        let assemble_args = BuildAssembleArgs {
            catalog: match_args.catalog.clone(),
            piece_plan: match_args.piece_plan.clone(),
            shape_match: match_args.out.clone(),
            connectivity: GridConnectivity::FourWay,
            out: PathBuf::from("artifacts/test/pure-catalog-loop-placement.json"),
        };
        let placement = assemble_piece_placement(&catalog, &plan, &shape_match, &assemble_args)
            .expect("room rotations should close the exact prefab loop");
        assert!(placement.connection_cells.is_empty());
        assert_eq!(placement.glued_exits.len(), 4);
        assert!(placement
            .glued_exits
            .iter()
            .all(|glued| pure_catalog_glue_is_direct(glued, &placement.occupied_cells)));
        let directions = placement
            .instances
            .iter()
            .map(|instance| {
                (
                    instance.piece_id.as_str(),
                    instance
                        .exit_map
                        .iter()
                        .map(|exit| exit.direction.as_str())
                        .collect::<BTreeSet<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(directions["piece.a"], BTreeSet::from(["east", "south"]));
        assert_eq!(directions["piece.b"], BTreeSet::from(["south", "west"]));
        assert_eq!(directions["piece.c"], BTreeSet::from(["north", "west"]));
        assert_eq!(directions["piece.d"], BTreeSet::from(["east", "north"]));
        assert!(validate_piece_placement(&placement).ok);

        let mut impossible_catalog = catalog.clone();
        impossible_catalog.shapes[0].allowed_transforms = vec!["identity".to_owned()];
        let impossible_match = match_shapes(&impossible_catalog, &plan, &match_args);
        assert!(impossible_match.ok);
        let first_error = assemble_piece_placement(
            &impossible_catalog,
            &plan,
            &impossible_match,
            &assemble_args,
        )
        .expect_err("fixed room orientation cannot close the loop");
        let repeated_error = assemble_piece_placement(
            &impossible_catalog,
            &plan,
            &impossible_match,
            &assemble_args,
        )
        .expect_err("the same fixed room orientation must reject repeatably");
        assert_eq!(first_error, repeated_error);
        assert!(first_error.contains("pure catalog search exhausted after"));
        assert!(first_error.contains("decision(s)"));
        assert!(first_error.contains("backtrack(s)"));

    }

    #[test]
    fn procedural_corridor_realization_uses_geometry_lanes_without_corridor_prefabs() {
        let (candidate, intermediate, geometry) = full_stack_geometry_inputs(853);
        let plan_args = BuildEmitPiecePlanArgs {
            candidate: PathBuf::from("artifacts/test/candidate.json"),
            intermediate: PathBuf::from("artifacts/test/intermediate-breakdown.json"),
            geometry: PathBuf::from("artifacts/test/geometry.json"),
            corridor_realization: CorridorRealization::Procedural,
            out: PathBuf::from("artifacts/test/procedural-piece-plan.json"),
        };
        let plan = emit_piece_build_plan(&candidate, &intermediate, &geometry, &plan_args)
            .expect("procedural piece plan should emit");
        let repeated_plan =
            emit_piece_build_plan(&candidate, &intermediate, &geometry, &plan_args)
                .expect("same-mode piece plan should repeat");
        assert_eq!(plan.plan_id, repeated_plan.plan_id);
        let catalog_plan_args = BuildEmitPiecePlanArgs {
            candidate: plan_args.candidate.clone(),
            intermediate: plan_args.intermediate.clone(),
            geometry: plan_args.geometry.clone(),
            corridor_realization: CorridorRealization::Catalog,
            out: PathBuf::from("artifacts/test/catalog-piece-plan.json"),
        };
        let catalog_plan =
            emit_piece_build_plan(&candidate, &intermediate, &geometry, &catalog_plan_args)
                .expect("catalog piece plan should emit");
        assert_ne!(plan.plan_id, catalog_plan.plan_id);
        assert_eq!(
            plan.corridor_realization,
            CorridorRealization::Procedural
        );
        assert!(plan.requirements.iter().all(|requirement| {
            !matches!(
                requirement.kind.as_str(),
                "connector" | "corridor" | "bend" | "junction"
            )
        }));
        assert_eq!(plan.links.len(), geometry.corridors.len());
        assert!(plan
            .links
            .iter()
            .all(|link| link.route_points.len() >= 2));

        let catalog_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join(DEFAULT_SHAPE_CATALOG);
        let catalog: ShapeCatalog = read_json(&catalog_path).expect("shape catalog");
        let match_args = BuildMatchShapesArgs {
            catalog: PathBuf::from(DEFAULT_SHAPE_CATALOG),
            piece_plan: plan_args.out.clone(),
            seed: 884,
            out: PathBuf::from("artifacts/test/procedural-shape-match.json"),
        };
        let shape_match = match_shapes(&catalog, &plan, &match_args);
        assert!(shape_match.ok);
        let catalog_shape_match =
            match_shapes(&catalog, &catalog_plan, &test_match_args(884));
        assert_ne!(shape_match.match_id, catalog_shape_match.match_id);
        let assemble_args = BuildAssembleArgs {
            catalog: PathBuf::from(DEFAULT_SHAPE_CATALOG),
            piece_plan: plan_args.out.clone(),
            shape_match: match_args.out,
            connectivity: GridConnectivity::FourWay,
            out: PathBuf::from("artifacts/test/procedural-placement.json"),
        };
        let stale_match_error =
            assemble_piece_placement(&catalog, &catalog_plan, &shape_match, &assemble_args)
                .expect_err("cross-mode shape match must reject");
        assert!(stale_match_error.contains("does not match"));
        let placement =
            assemble_piece_placement(&catalog, &plan, &shape_match, &assemble_args)
                .expect("procedural corridors should realize inside geometry lanes");
        let repeated =
            assemble_piece_placement(&catalog, &plan, &shape_match, &assemble_args)
                .expect("procedural realization should repeat");
        assert_eq!(
            serde_json::to_value(&placement).expect("placement value"),
            serde_json::to_value(&repeated).expect("repeated placement value")
        );
        assert_eq!(
            placement.corridor_realization,
            CorridorRealization::Procedural
        );
        assert!(placement.instances.iter().all(|instance| {
            !matches!(
                instance.requirement_kind.as_str(),
                "connector" | "corridor" | "bend" | "junction"
            )
        }));
        assert!(validate_piece_placement(&placement).ok);

        let flow_args = BuildValidateFlowArgs {
            candidate: plan_args.candidate,
            geometry: plan_args.geometry,
            piece_plan: plan_args.out,
            piece_placement: assemble_args.out,
            out: PathBuf::from("artifacts/test/procedural-built-flow.json"),
        };
        let flow = validate_built_flow(&candidate, &geometry, &plan, &placement, &flow_args);
        assert!(flow.ok);
        let stale_flow =
            validate_built_flow(&candidate, &geometry, &catalog_plan, &placement, &flow_args);
        assert!(stale_flow.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "built_flow_placement_plan_mismatch"
                || diagnostic.code == "built_flow_corridor_realization_mismatch"
        }));

        let mut diverted_plan = plan.clone();
        diverted_plan.links[0].route_points[1].x += 8;
        let diverted_flow =
            validate_built_flow(&candidate, &geometry, &diverted_plan, &placement, &flow_args);
        assert!(diverted_flow.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "built_flow_link_provenance_mismatch"
        }));

        let mut outside_lane = placement;
        let first_owner = outside_lane.connection_cells[0].instance_id.clone();
        let first_glued = outside_lane
            .glued_exits
            .iter()
            .find(|glued| {
                format!("connection.{}", slugify_label(glued.id.as_str())) == first_owner
            })
            .expect("glued exit for connection owner")
            .clone();
        let moved = outside_lane
            .connection_cells
            .iter_mut()
            .find(|cell| {
                cell.instance_id == first_owner
                    && (cell.x != first_glued.from_cell.x
                        || cell.y != first_glued.from_cell.y)
                    && (cell.x != first_glued.to_cell.x
                        || cell.y != first_glued.to_cell.y)
            })
            .expect("non-endpoint connection cell");
        moved.x += 10_000;
        let report = validate_piece_placement(&outside_lane);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "piece_procedural_corridor_left_geometry_lane"
        }));
    }

    #[test]
    fn corridor_realization_rejects_unknown_serialized_modes() {
        let value = serde_json::json!({
            "kind": "asha_procgen.piece_build_plan.v1",
            "schemaVersion": 1,
            "planId": "piece_plan.invalid",
            "candidateId": "candidate.invalid",
            "geometryId": "geometry.invalid",
            "corridorRealization": "automatic_mixed",
            "sourceCandidateRef": "candidate.json",
            "sourceIntermediateRef": "intermediate.json",
            "sourceGeometryRef": "geometry.json",
            "requirements": [],
            "links": [],
            "contentRequirements": []
        });
        let error = serde_json::from_value::<PieceBuildPlan>(value)
            .expect_err("unknown realization mode must fail closed");
        assert!(error.to_string().contains("unknown variant"));
    }

    #[test]
    fn shape_matcher_rotates_exits_for_piece_requirements() {
        let mut shape = test_catalog_shape(
            "shape.room.one_east",
            &["room"],
            &["identity", "rotate90", "rotate180", "rotate270"],
            vec![test_catalog_exit("exit.east", "east")],
            Vec::new(),
            &["standard_room"],
        );
        shape.footprint = vec![
            GridCell { x: 0, y: 0 },
            GridCell { x: 1, y: 0 },
            GridCell { x: 2, y: 0 },
            GridCell { x: 0, y: 1 },
            GridCell { x: 1, y: 1 },
            GridCell { x: 2, y: 1 },
        ];
        shape.exits[0].x = 3;
        shape.exits[0].y = 1;
        let catalog = test_shape_catalog(vec![shape]);
        let plan = test_piece_plan(vec![test_piece_requirement(
            "piece.room.start",
            "room",
            vec![test_piece_exit("exit.required.north", "north")],
            Vec::new(),
            &["room"],
        )]);
        let args = test_match_args(9001);
        let report = match_shapes(&catalog, &plan, &args);

        assert!(report.ok);
        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].shape_id, "shape.room.one_east");
        assert_eq!(report.matches[0].transform, "rotate270");
        assert_eq!(report.matches[0].exit_map[0].catalog_exit_id, "exit.east");
        assert_eq!(report.matches[0].exit_map[0].x, 1);
        assert_eq!(report.matches[0].exit_map[0].y, -1);
        assert_eq!(report.matches[0].exit_map[0].direction, "north");
    }

    #[test]
    fn shape_matcher_requires_declared_catalog_family_tags() {
        let short = test_catalog_shape(
            "shape.corridor.short",
            &["corridor"],
            &["identity"],
            vec![
                test_catalog_exit("exit.west", "west"),
                test_catalog_exit("exit.east", "east"),
            ],
            Vec::new(),
            &["corridor", "straight", "span_short"],
        );
        let long = test_catalog_shape(
            "shape.corridor.long",
            &["corridor"],
            &["identity"],
            vec![
                test_catalog_exit("exit.west", "west"),
                test_catalog_exit("exit.east", "east"),
            ],
            Vec::new(),
            &["corridor", "straight", "span_long"],
        );
        let mut requirement = test_piece_requirement(
            "piece.corridor.long",
            "corridor",
            vec![
                test_piece_exit("exit.in", "west"),
                test_piece_exit("exit.out", "east"),
            ],
            Vec::new(),
            &["corridor", "straight"],
        );
        requirement.required_shape_tags = vec![
            "corridor".to_owned(),
            "straight".to_owned(),
            "span_long".to_owned(),
        ];
        let report = match_shapes(
            &test_shape_catalog(vec![short, long]),
            &test_piece_plan(vec![requirement]),
            &test_match_args(9051),
        );
        assert!(report.ok);
        assert_eq!(report.matches[0].shape_id, "shape.corridor.long");
        assert!(report.rejections.iter().any(|rejection| {
            rejection.shape_id == "shape.corridor.short"
                && rejection
                    .reasons
                    .iter()
                    .any(|reason| reason == "missing_shape_tags: span_long")
        }));
    }

    #[test]
    fn shape_matcher_reports_missing_sockets_and_exit_count() {
        let catalog = test_shape_catalog(vec![
            test_catalog_shape(
                "shape.threshold.no_gate_line",
                &["threshold"],
                &["identity"],
                vec![test_catalog_exit("exit.west", "west"), test_catalog_exit("exit.east", "east")],
                Vec::new(),
                &["threshold"],
            ),
            test_catalog_shape(
                "shape.corridor.one_exit",
                &["corridor"],
                &["identity"],
                vec![test_catalog_exit("exit.east", "east")],
                Vec::new(),
                &["corridor"],
            ),
        ]);
        let plan = test_piece_plan(vec![
            test_piece_requirement(
                "piece.threshold.locked",
                "threshold",
                vec![test_piece_exit("exit.required.west", "west")],
                vec!["gate_line".to_owned()],
                &["locked_threshold"],
            ),
            test_piece_requirement(
                "piece.corridor.two_exit",
                "corridor",
                vec![
                    test_piece_exit("exit.required.west", "west"),
                    test_piece_exit("exit.required.east", "east"),
                ],
                Vec::new(),
                &["corridor"],
            ),
        ]);
        let report = match_shapes(&catalog, &plan, &test_match_args(9002));

        assert!(!report.ok);
        assert_eq!(report.unmatched_count, 2);
        assert!(report.diagnostics.iter().all(|diagnostic| {
            diagnostic.code == "shape_match_missing" && diagnostic.severity == Severity::Fatal
        }));
        assert!(report.rejections.iter().any(|rejection| {
            rejection.piece_id == "piece.threshold.locked"
                && rejection
                    .reasons
                    .iter()
                    .any(|reason| reason.contains("missing_sockets: gate_line"))
        }));
        assert!(report.rejections.iter().any(|rejection| {
            rejection.piece_id == "piece.corridor.two_exit"
                && rejection
                    .reasons
                    .iter()
                    .any(|reason| reason.contains("exit_count_mismatch"))
        }));
    }

    #[test]
    fn shape_matcher_tie_breaks_deterministically() {
        let left = test_catalog_shape(
            "shape.room.tie_left",
            &["room"],
            &["identity"],
            vec![test_catalog_exit("exit.east", "east")],
            Vec::new(),
            &["standard_room"],
        );
        let right = test_catalog_shape(
            "shape.room.tie_right",
            &["room"],
            &["identity"],
            vec![test_catalog_exit("exit.east", "east")],
            Vec::new(),
            &["standard_room"],
        );
        let plan = test_piece_plan(vec![test_piece_requirement(
            "piece.room.tie",
            "room",
            vec![test_piece_exit("exit.required.east", "east")],
            Vec::new(),
            &["room"],
        )]);

        let report_a = match_shapes(
            &test_shape_catalog(vec![left.clone(), right.clone()]),
            &plan,
            &test_match_args(42),
        );
        let report_b = match_shapes(
            &test_shape_catalog(vec![right.clone(), left.clone()]),
            &plan,
            &test_match_args(42),
        );

        assert!(report_a.ok);
        assert!(report_b.ok);
        assert_eq!(report_a.matches[0].shape_id, report_b.matches[0].shape_id);
        assert_eq!(report_a.matches[0].transform, report_b.matches[0].transform);

        let alternative = match_shapes_with_attempt(
            &test_shape_catalog(vec![left.clone(), right.clone()]),
            &plan,
            &test_match_args(42),
            1,
        );
        let repeated_alternative = match_shapes_with_attempt(
            &test_shape_catalog(vec![right.clone(), left.clone()]),
            &plan,
            &test_match_args(42),
            1,
        );
        assert_eq!(alternative.alternative_attempt, 1);
        assert_eq!(alternative.matches[0].candidate_rank, 1);
        assert_eq!(alternative.matches[0].candidate_count, 2);
        assert_ne!(alternative.matches[0].shape_id, report_a.matches[0].shape_id);
        assert_eq!(
            alternative.matches[0].shape_id,
            repeated_alternative.matches[0].shape_id
        );

        let catalog = test_shape_catalog(vec![left, right]);
        let match_args = test_match_args(42);
        let mut attempted_alternatives = Vec::new();
        let (recovered_match, recovered_shape) = search_shape_realizations(
            &catalog,
            &plan,
            &match_args,
            2,
            |shape_match| {
                attempted_alternatives.push(shape_match.alternative_attempt);
                if shape_match.alternative_attempt == 0 {
                    Err(
                        "piece realization search exhausted after 2 scale tier(s)"
                            .to_owned(),
                    )
                } else {
                    Ok(shape_match.matches[0].shape_id.clone())
                }
            },
        )
        .expect("downstream realization failure should backtrack shape choice");
        assert_eq!(attempted_alternatives, vec![0, 1]);
        assert_eq!(recovered_match.alternative_attempt, 1);
        assert_eq!(recovered_match.matches[0].candidate_rank, 1);
        assert_eq!(recovered_shape, recovered_match.matches[0].shape_id);
    }

    #[test]
    fn piece_placement_assembles_full_stack_without_overlap() {
        let placement = full_stack_piece_placement_fixture(951);
        let report = validate_piece_placement(&placement);

        assert!(report.ok, "{:?}", report.diagnostics);
        assert_eq!(placement.kind, "asha_procgen.piece_placement.v1");
        assert_eq!(placement.grid_connectivity, GridConnectivity::FourWay);
        assert_eq!(placement.placement_policy, PiecePlacementPolicy::default());
        assert!(placement.occupied_cells.len() >= placement.instances.len());
        assert!(!placement.connection_cells.is_empty());
        assert!(placement
            .instances
            .iter()
            .all(|instance| !instance.occupied_cells.is_empty()));
        let (width, height) = placement_bounds(&placement);
        assert!(width < 200, "placement should not collapse into a long atlas: {width}x{height}");
        assert!(height > 10, "placement should preserve source geometry depth: {width}x{height}");
        assert!(placement.instances.iter().all(|instance| {
            !instance.shape_id.is_empty()
                && !instance.source_requirement_ref.is_empty()
                && !instance.source_refs.is_empty()
        }));
        assert!(!placement.glued_exits.is_empty());
        for glued in &placement.glued_exits {
            let owner = format!("connection.{}", slugify_label(glued.id.as_str()));
            let routed = placement
                .connection_cells
                .iter()
                .filter(|cell| cell.instance_id == owner)
                .map(|cell| (cell.x, cell.y))
                .collect::<BTreeSet<_>>();
            assert!(routed.contains(&(glued.from_cell.x, glued.from_cell.y)));
            assert!(routed.contains(&(glued.to_cell.x, glued.to_cell.y)));
        }
        assert!(placement.dangling_exits.is_empty());
    }

    #[test]
    fn piece_realization_backtracks_routes_repeatably_and_fails_closed() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let run_dir = repo_root.join("artifacts/samples/batch-v2/candidate-001");
        let candidate: Candidate =
            read_json(&run_dir.join("candidate-003-boss_preparation_loop.json"))
                .expect("nested-boss candidate");
        let intermediate: IntermediateBreakdown =
            read_json(&run_dir.join("intermediate-breakdown.json"))
                .expect("nested-boss intermediate");
        let connection_plan: PhysicalConnectionPlan =
            read_json(&run_dir.join("physical-connection-plan.json"))
                .expect("nested-boss connection plan");
        let geometry_args = GeometryEmit2dArgs {
            candidate: run_dir.join("candidate-003-boss_preparation_loop.json"),
            intermediate: run_dir.join("intermediate-breakdown.json"),
            connection_plan: run_dir.join("physical-connection-plan.json"),
            layout_policy: Some(
                repo_root.join("fixtures/geometry-layout-policies/compact-first-v1.json"),
            ),
            seed: 14_308,
            out: PathBuf::from("artifacts/test/geometry.json"),
        };
        let geometry = emit_geometry_2d(
            &candidate,
            &intermediate,
            &connection_plan,
            &geometry_args,
            geometry_args.seed,
        )
        .expect("nested-boss geometry should embed");
        let specs = ordered_geometry_region_specs(&candidate, &intermediate);
        let spacing = geometry_spacing_for_tier(&geometry.layout_policy, 0)
            .expect("nested-boss first spacing tier");
        let fixed_order_error = place_and_route_physical_geometry_attempt(
            &specs,
            &connection_plan,
            &spacing,
            geometry_args.seed,
            0,
            2,
        )
        .expect_err("the original fixed room/route decision should reject");
        assert!(matches!(
            fixed_order_error,
            GeometryPlacementAttemptError::RoutesUnavailable { .. }
        ));
        assert_eq!(geometry.layout_search.room_order_attempt, 2);
        assert_eq!(geometry.layout_search.port_order_attempt, 0);

        let mut incompatible_plan = connection_plan.clone();
        incompatible_plan.sections[0].topology = "junction_3".to_owned();
        let incompatible_error = emit_geometry_2d(
            &candidate,
            &intermediate,
            &incompatible_plan,
            &geometry_args,
            geometry_args.seed,
        )
        .expect_err("unsupported physical topology must fail closed");
        assert!(incompatible_error.starts_with("invalid physical geometry plan"));
        assert_eq!(
            selection_rejection_code(incompatible_error.as_str()),
            "selection_physical_section_plan_incompatible"
        );
        let plan_args = BuildEmitPiecePlanArgs {
            candidate: geometry_args.candidate.clone(),
            intermediate: geometry_args.intermediate.clone(),
            geometry: geometry_args.out.clone(),
            corridor_realization: CorridorRealization::Hybrid,
            out: PathBuf::from("artifacts/test/piece-plan.json"),
        };
        let plan =
            emit_piece_build_plan(&candidate, &intermediate, &geometry, &plan_args)
                .expect("nested-boss piece plan");
        let catalog: ShapeCatalog =
            read_json(&repo_root.join(DEFAULT_SHAPE_CATALOG)).expect("shape catalog");
        let shape_match = match_shapes(&catalog, &plan, &test_match_args(14_339));
        assert!(shape_match.ok);
        let assemble_args = BuildAssembleArgs {
            catalog: PathBuf::from(DEFAULT_SHAPE_CATALOG),
            piece_plan: plan_args.out,
            shape_match: PathBuf::from("artifacts/test/piece-shape-match.json"),
            connectivity: GridConnectivity::FourWay,
            out: PathBuf::from("artifacts/test/piece-placement.json"),
        };
        let first = assemble_piece_placement(&catalog, &plan, &shape_match, &assemble_args)
            .expect("route backtracking should recover nested-boss realization");
        let repeated =
            assemble_piece_placement(&catalog, &plan, &shape_match, &assemble_args)
                .expect("repeated realization should recover");
        assert!(first.realization_search.route_attempts >= 1);
        assert_eq!(first.realization_search, repeated.realization_search);
        assert_eq!(
            serde_json::to_value(&first).expect("serialize first placement"),
            serde_json::to_value(&repeated).expect("serialize repeated placement")
        );
        let placement_report = validate_piece_placement(&first);
        assert!(
            placement_report.ok,
            "catalog placement diagnostics: {:?}",
            placement_report.diagnostics
        );
        let flow_args = BuildValidateFlowArgs {
            candidate: geometry_args.candidate.clone(),
            geometry: geometry_args.out.clone(),
            piece_plan: assemble_args.piece_plan.clone(),
            piece_placement: assemble_args.out.clone(),
            out: PathBuf::from("artifacts/test/catalog-built-flow.json"),
        };
        let flow = validate_built_flow(&candidate, &geometry, &plan, &first, &flow_args);
        assert!(flow.ok, "{:?}", flow.diagnostics);
        let mut diverted_plan = plan.clone();
        diverted_plan.links[0].route_points[1].x += 10_000;
        let diverted_flow =
            validate_built_flow(&candidate, &geometry, &diverted_plan, &first, &flow_args);
        assert!(diverted_flow.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "built_flow_link_provenance_mismatch"
        }));

        let mut impossible = first;
        impossible.glued_exits[0].from_cell = GridCell {
            x: -10_000,
            y: -10_000,
        };
        impossible.glued_exits[0].to_cell = GridCell {
            x: 10_000,
            y: 10_000,
        };
        let error = derive_connection_cells(&impossible)
            .expect_err("out-of-bounds glued exits must exhaust bounded route search");
        assert!(error.starts_with(
            "piece route search exhausted after 4 deterministic order attempt(s)"
        ));
    }

    #[test]
    fn piece_placement_validation_catches_overlap_reservation_dangling_and_reachability() {
        let placement = full_stack_piece_placement_fixture(1051);

        let mut overlap = placement.clone();
        let first = overlap.occupied_cells[0].clone();
        overlap.occupied_cells[1].x = first.x;
        overlap.occupied_cells[1].y = first.y;
        let report = validate_piece_placement(&overlap);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "piece_occupied_cell_overlap"));

        let mut reservation = placement.clone();
        let occupied = reservation.occupied_cells[0].clone();
        let reserver = reservation.instances[1].instance_id.clone();
        reservation.reserved_cells.push(PlacementCellRef {
            instance_id: reserver,
            x: occupied.x,
            y: occupied.y,
        });
        let report = validate_piece_placement(&reservation);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "piece_reserved_cell_conflict"));

        let mut touching = placement.clone();
        let first = touching.occupied_cells[0].clone();
        let touching_index = touching
            .occupied_cells
            .iter()
            .position(|cell| cell.instance_id != first.instance_id)
            .expect("fixture should have more than one instance");
        touching.occupied_cells[touching_index].x = first.x + 1;
        touching.occupied_cells[touching_index].y = first.y;
        touching.glued_exits.clear();
        let report = validate_piece_placement(&touching);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "piece_minimum_clearance_violated"));

        let mut dangling = placement.clone();
        dangling.dangling_exits.push(DanglingExit {
            instance_id: dangling.instances[0].instance_id.clone(),
            exit_id: "exit.test".to_owned(),
            reason: "test".to_owned(),
        });
        let report = validate_piece_placement(&dangling);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "piece_required_exit_dangling"));

        let mut unsafe_connection = placement.clone();
        let connection_owner = unsafe_connection.connection_cells[0].instance_id.clone();
        let glued = unsafe_connection
            .glued_exits
            .iter()
            .find(|glued| {
                format!("connection.{}", slugify_label(glued.id.as_str())) == connection_owner
            })
            .expect("connection owner should resolve to a glued exit");
        let occupied_positions = unsafe_connection
            .occupied_cells
            .iter()
            .map(|cell| (cell.x, cell.y))
            .collect::<BTreeSet<_>>();
        let reserved_positions = unsafe_connection
            .reserved_cells
            .iter()
            .map(|cell| (cell.x, cell.y))
            .collect::<BTreeSet<_>>();
        let from_instance = unsafe_connection
            .instances
            .iter()
            .find(|instance| instance.instance_id == glued.from_instance)
            .expect("glued from instance should exist");
        let unsafe_route_cell = from_instance
            .occupied_cells
            .iter()
            .flat_map(|cell| {
                [
                    (cell.x + 1, cell.y),
                    (cell.x - 1, cell.y),
                    (cell.x, cell.y + 1),
                    (cell.x, cell.y - 1),
                ]
            })
            .find(|position| {
                !occupied_positions.contains(position) && !reserved_positions.contains(position)
                    && *position != (glued.from_cell.x, glued.from_cell.y)
                    && *position != (glued.to_cell.x, glued.to_cell.y)
            })
            .expect("fixture should expose a non-exit boundary cell");
        unsafe_connection.connection_cells.push(PlacementCellRef {
            instance_id: connection_owner,
            x: unsafe_route_cell.0,
            y: unsafe_route_cell.1,
        });
        let report = validate_piece_placement(&unsafe_connection);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "piece_connection_wall_clearance_violated"
        }));

        let mut unreachable = placement;
        unreachable.glued_exits.clear();
        unreachable.dangling_exits.clear();
        let report = validate_piece_placement(&unreachable);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "piece_goal_unreachable"));

        let mut grid_unreachable = full_stack_piece_placement_fixture(1052);
        grid_unreachable.connection_cells.clear();
        let report = validate_piece_placement(&grid_unreachable);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "piece_grid_instance_unreachable"));
    }

    #[test]
    fn piece_placement_rejects_shared_room_contact_beyond_endpoint_approaches() {
        let mut placement = full_stack_piece_placement_fixture(1053);
        let baseline = validate_piece_placement(&placement);
        assert!(baseline.ok, "{:?}", baseline.diagnostics);

        let section_room_endpoints = collect_section_room_endpoints(&placement);
        let approach_length = shared_room_approach_length(
            placement.placement_policy.minimum_clearance_cells,
            placement.placement_policy.wall_thickness_cells,
        );
        let mut contact = None;
        for (left_section, left_rooms) in &section_room_endpoints {
            for (right_section, right_rooms) in &section_room_endpoints {
                if left_section >= right_section
                    || !left_rooms
                        .keys()
                        .any(|room| right_rooms.contains_key(room))
                {
                    continue;
                }
                let left_owner = placement
                    .glued_exits
                    .iter()
                    .find(|glued| glued.source_section == *left_section)
                    .map(|glued| {
                        format!("connection.{}", slugify_label(glued.id.as_str()))
                    })
                    .expect("section should have a routed glued exit");
                let right_owners = placement
                    .glued_exits
                    .iter()
                    .filter(|glued| glued.source_section == *right_section)
                    .map(|glued| {
                        format!("connection.{}", slugify_label(glued.id.as_str()))
                    })
                    .collect::<BTreeSet<_>>();
                let left_cells = placement
                    .connection_cells
                    .iter()
                    .filter(|cell| cell.instance_id == left_owner)
                    .map(|cell| (cell.x, cell.y))
                    .collect::<BTreeSet<_>>();
                let Some(position) = placement
                    .connection_cells
                    .iter()
                    .filter(|cell| right_owners.contains(&cell.instance_id))
                    .map(|cell| (cell.x, cell.y))
                    .find(|position| {
                        !left_cells.contains(position)
                            && !connection_contact_at_shared_room(
                                left_section,
                                right_section,
                                *position,
                                *position,
                                &section_room_endpoints,
                                approach_length,
                            )
                    })
                else {
                    continue;
                };
                contact = Some((left_owner, position));
                break;
            }
            if contact.is_some() {
                break;
            }
        }
        let (left_owner, position) =
            contact.expect("fixture should have two sections sharing a room with downstream cells");
        placement.connection_cells.push(PlacementCellRef {
            instance_id: left_owner,
            x: position.0,
            y: position.1,
        });

        let report = validate_piece_placement(&placement);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "piece_connection_section_clearance_violated"
        }));
    }

    #[test]
    fn built_flow_validation_preserves_unique_exits_portals_and_item_progression() {
        let (candidate, geometry, plan, placement) = full_stack_built_flow_fixture(1151);
        let args = BuildValidateFlowArgs {
            candidate: PathBuf::from("artifacts/test/candidate.json"),
            geometry: PathBuf::from("artifacts/test/geometry.json"),
            piece_plan: PathBuf::from("artifacts/test/piece-plan.json"),
            piece_placement: PathBuf::from("artifacts/test/piece-placement.json"),
            out: PathBuf::from("artifacts/test/built-flow.validation.json"),
        };
        let report = validate_built_flow(&candidate, &geometry, &plan, &placement, &args);
        assert!(report.ok, "{:?}", report.diagnostics);
        assert_eq!(placement.gate_portals.len(), candidate.graph.edges.len());
        assert!(report.progression.len() >= 2);

        let endpoint_count = placement.glued_exits.len() * 2;
        let unique_endpoints = placement
            .glued_exits
            .iter()
            .flat_map(|glued| {
                [
                    (glued.from_instance.as_str(), glued.from_exit.as_str()),
                    (glued.to_instance.as_str(), glued.to_exit.as_str()),
                ]
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(unique_endpoints.len(), endpoint_count);
    }

    #[test]
    fn built_flow_validation_fails_closed_on_chain_portal_and_route_tampering() {
        let (candidate, geometry, plan, placement) = full_stack_built_flow_fixture(1251);
        let args = BuildValidateFlowArgs {
            candidate: PathBuf::from("artifacts/test/candidate.json"),
            geometry: PathBuf::from("artifacts/test/geometry.json"),
            piece_plan: PathBuf::from("artifacts/test/piece-plan.json"),
            piece_placement: PathBuf::from("artifacts/test/piece-placement.json"),
            out: PathBuf::from("artifacts/test/built-flow.validation.json"),
        };

        let mut missing_join = placement.clone();
        missing_join.glued_exits.pop();
        let report = validate_built_flow(&candidate, &geometry, &plan, &missing_join, &args);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "built_flow_glued_join_missing"
                || diagnostic.code == "built_flow_glued_join_count"
        }));

        let mut mismatched_portal = placement.clone();
        mismatched_portal.gate_portals[0].source_edge = "edge.not_authored".to_owned();
        mismatched_portal.gate_portals[0].source_edges[0] = "edge.not_authored".to_owned();
        let report = validate_built_flow(&candidate, &geometry, &plan, &mismatched_portal, &args);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "built_flow_extra_gate_portal"));

        let mut broken_route = placement.clone();
        let owner = broken_route.connection_cells[0].instance_id.clone();
        broken_route
            .connection_cells
            .retain(|cell| cell.instance_id != owner);
        let report = validate_built_flow(&candidate, &geometry, &plan, &broken_route, &args);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "built_flow_physical_route_disconnected"));
    }

    #[test]
    fn built_flow_validation_detects_a_post_gate_physical_bypass() {
        let (candidate, geometry, plan, mut placement) = full_stack_built_flow_fixture(1351);
        let args = BuildValidateFlowArgs {
            candidate: PathBuf::from("artifacts/test/candidate.json"),
            geometry: PathBuf::from("artifacts/test/geometry.json"),
            piece_plan: PathBuf::from("artifacts/test/piece-plan.json"),
            piece_placement: PathBuf::from("artifacts/test/piece-placement.json"),
            out: PathBuf::from("artifacts/test/built-flow.validation.json"),
        };
        let no_items = BTreeSet::new();
        let source_nodes = source_reachable_nodes(&candidate, &no_items);
        let locked_edge = candidate
            .graph
            .edges
            .iter()
            .find(|edge| {
                edge.required_item.is_some()
                    && source_nodes.contains(&edge.from)
                    && !source_nodes.contains(&edge.to)
            })
            .expect("fixture should contain a reachable locked threshold");
        let portal = placement
            .gate_portals
            .iter()
            .find(|portal| portal.source_edges.contains(&locked_edge.id))
            .expect("locked source edge should have a controlled portal");
        let portal_glue = placement
            .glued_exits
            .iter()
            .find(|glued| glued.link_id == portal.link_id)
            .expect("controlled portal should identify its glued route");
        let connection_owner = format!("connection.{}", slugify_label(portal_glue.id.as_str()));
        let closed_portal_cells = placement
            .gate_portals
            .iter()
            .filter(|portal| !portal_open_for_items(portal, &no_items))
            .flat_map(|portal| portal.cells.iter().map(|cell| (cell.x, cell.y)))
            .collect::<BTreeSet<_>>();
        let post_gate_cell = placement
            .connection_cells
            .iter()
            .find(|cell| {
                cell.instance_id == connection_owner
                    && !closed_portal_cells.contains(&(cell.x, cell.y))
            })
            .map(|cell| (cell.x, cell.y))
            .expect("locked route should continue beyond its portal");
        let reachable_cell = placement
            .instances
            .iter()
            .find(|instance| {
                instance.source_refs.iter().any(|source_ref| {
                    source_ref
                        .strip_prefix("node:")
                        .is_some_and(|node| source_nodes.contains(node))
                })
            })
            .and_then(|instance| instance.occupied_cells.first())
            .map(|cell| (cell.x, cell.y))
            .expect("fixture should have a physically realized reachable node");
        let bypass_path = test_cardinal_path_avoiding(
            post_gate_cell,
            reachable_cell,
            &closed_portal_cells,
            &placement_walkable_cells(&placement),
        );
        placement
            .connection_cells
            .extend(bypass_path.into_iter().map(|(x, y)| PlacementCellRef {
                instance_id: connection_owner.clone(),
                x,
                y,
            }));

        let report = validate_built_flow(&candidate, &geometry, &plan, &placement, &args);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "built_flow_reachability_mismatch"
                && diagnostic.detail.contains(locked_edge.to.as_str())
        }));
    }

    #[test]
    fn grid_connectivity_distinguishes_cardinal_and_diagonal_neighbors() {
        let cardinal = (1, 0);
        let diagonal = (1, 1);

        assert!(grid_neighbors((0, 0), GridConnectivity::FourWay).contains(&cardinal));
        assert!(!grid_neighbors((0, 0), GridConnectivity::FourWay).contains(&diagonal));
        assert!(grid_neighbors((0, 0), GridConnectivity::EightWay).contains(&diagonal));
    }

    #[test]
    fn connection_routing_uses_declared_exits_instead_of_nearer_walls() {
        let from_owner = "instance.room_a";
        let to_owner = "instance.room_b";
        let occupied_by_cell = [
            ((0, 0), from_owner),
            ((1, 0), from_owner),
            ((0, 1), from_owner),
            ((1, 1), from_owner),
            ((5, 3), to_owner),
            ((6, 3), to_owner),
            ((5, 4), to_owner),
            ((6, 4), to_owner),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        let start = GridCell { x: 2, y: 0 };
        let end = GridCell { x: 4, y: 4 };
        let route = route_bridge_cells(
            &start,
            &end,
            from_owner,
            to_owner,
            "east",
            "west",
            GridConnectivity::FourWay,
            &occupied_by_cell,
            &BTreeSet::new(),
            1,
            1,
            &BTreeMap::new(),
            &BTreeMap::new(),
            "section.test",
            &BTreeSet::new(),
            (-10, 10, -10, 10),
        )
        .expect("declared transformed exits should have a safe route");

        assert_eq!(route.first(), Some(&start));
        assert_eq!(route.last(), Some(&end));
        assert!(!route.contains(&GridCell { x: 2, y: 1 }));
        assert!(!route.contains(&GridCell { x: 4, y: 3 }));
    }

    fn full_stack_geometry_fixture(seed: u64) -> Geometry2dArtifact {
        full_stack_geometry_inputs(seed).2
    }

    fn full_stack_geometry_inputs(
        seed: u64,
    ) -> (Candidate, IntermediateBreakdown, Geometry2dArtifact) {
        let intent = test_intent("geometry-validation");
        let mut candidate = create_initial_candidate(&intent, seed);
        for (index, rule) in [GraphRule::LockKeyLoop]
        .into_iter()
        .enumerate()
        {
            assert!(apply_graph_rule(&mut candidate, rule, seed + 1 + index as u64).is_empty());
        }
        let annotations = spatial_intent_report(&candidate, None).expect("spatial intent report");
        let intermediate = intermediate_breakdown(
            &candidate,
            &annotations,
            Path::new("artifacts/test/spatial-intent.json"),
        )
        .expect("breakdown should encode");
        let args = GeometryEmit2dArgs {
            candidate: PathBuf::from("artifacts/test/candidate.json"),
            intermediate: PathBuf::from("artifacts/test/intermediate-breakdown.json"),
            connection_plan: PathBuf::from("artifacts/test/physical-connection-plan.json"),
            layout_policy: None,
            seed: seed + 20,
            out: PathBuf::from("artifacts/test/geometry.json"),
        };
        let connection_plan = test_connection_plan(&candidate, &intermediate);
        let geometry = emit_geometry_2d(
            &candidate,
            &intermediate,
            &connection_plan,
            &args,
            seed + 20,
        )
        .expect("geometry should emit");
        (candidate, intermediate, geometry)
    }

    fn full_stack_piece_placement_fixture(seed: u64) -> PiecePlacement {
        full_stack_built_flow_fixture(seed).3
    }

    fn full_stack_built_flow_fixture(
        seed: u64,
    ) -> (Candidate, Geometry2dArtifact, PieceBuildPlan, PiecePlacement) {
        let (candidate, intermediate, geometry) = full_stack_geometry_inputs(seed);
        let piece_plan_args = BuildEmitPiecePlanArgs {
            candidate: PathBuf::from("artifacts/test/candidate.json"),
            intermediate: PathBuf::from("artifacts/test/intermediate-breakdown.json"),
            geometry: PathBuf::from("artifacts/test/geometry.json"),
            corridor_realization: CorridorRealization::Hybrid,
            out: PathBuf::from("artifacts/test/piece-plan.json"),
        };
        let piece_plan = emit_piece_build_plan(
            &candidate,
            &intermediate,
            &geometry,
            &piece_plan_args,
        )
        .expect("piece plan should emit");
        let catalog_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join(DEFAULT_SHAPE_CATALOG);
        let catalog: ShapeCatalog = read_json(&catalog_path).expect("shape catalog should load");
        let match_args = test_match_args(seed + 30);
        let shape_match = match_shapes(&catalog, &piece_plan, &match_args);
        assert!(
            shape_match.ok,
            "diagnostics={:?} rejections={:?}",
            shape_match.diagnostics,
            shape_match.rejections
        );
        let assemble_args = BuildAssembleArgs {
            catalog: PathBuf::from("fixtures/shape-catalogs/2d-basic.json"),
            piece_plan: PathBuf::from("artifacts/test/piece-plan.json"),
            shape_match: PathBuf::from("artifacts/test/piece-shape-match.json"),
            connectivity: GridConnectivity::FourWay,
            out: PathBuf::from("artifacts/test/piece-placement.json"),
        };
        let placement = assemble_piece_placement(&catalog, &piece_plan, &shape_match, &assemble_args)
            .expect("piece placement should assemble");
        (candidate, geometry, piece_plan, placement)
    }

    fn test_cardinal_path_avoiding(
        start: (i32, i32),
        goal: (i32, i32),
        blocked: &BTreeSet<(i32, i32)>,
        existing: &BTreeSet<(i32, i32)>,
    ) -> Vec<(i32, i32)> {
        let min_x = existing
            .iter()
            .map(|cell| cell.0)
            .chain([start.0, goal.0])
            .min()
            .unwrap_or(0)
            - 2;
        let max_x = existing
            .iter()
            .map(|cell| cell.0)
            .chain([start.0, goal.0])
            .max()
            .unwrap_or(0)
            + 2;
        let min_y = existing
            .iter()
            .map(|cell| cell.1)
            .chain([start.1, goal.1])
            .min()
            .unwrap_or(0)
            - 2;
        let max_y = existing
            .iter()
            .map(|cell| cell.1)
            .chain([start.1, goal.1])
            .max()
            .unwrap_or(0)
            + 2;
        let mut queue = VecDeque::from([start]);
        let mut previous = BTreeMap::new();
        let mut seen = BTreeSet::from([start]);
        while let Some(cell) = queue.pop_front() {
            if cell == goal {
                let mut path = vec![goal];
                let mut current = goal;
                while current != start {
                    current = previous[&current];
                    path.push(current);
                }
                path.reverse();
                return path;
            }
            for neighbor in grid_neighbors(cell, GridConnectivity::FourWay) {
                if neighbor.0 < min_x
                    || neighbor.0 > max_x
                    || neighbor.1 < min_y
                    || neighbor.1 > max_y
                    || blocked.contains(&neighbor)
                    || !seen.insert(neighbor)
                {
                    continue;
                }
                previous.insert(neighbor, cell);
                queue.push_back(neighbor);
            }
        }
        panic!("expanded fixture bounds should permit a portal-avoiding bypass path");
    }

    fn rectangles_overlap(left: &GeometryRect, right: &GeometryRect) -> bool {
        geometry_rectangles_overlap(left, right)
    }

    fn point_on_rect_boundary(point: &GeometryPoint, rect: &GeometryRect) -> bool {
        geometry_point_on_rect_boundary(point, rect)
    }

    fn test_shape_catalog(shapes: Vec<CatalogShape>) -> ShapeCatalog {
        ShapeCatalog {
            kind: "asha_procgen.shape_catalog.v1".to_owned(),
            schema_version: 1,
            catalog_id: "shape_catalog.test.v1".to_owned(),
            cell_size: 1,
            placement_policy: PiecePlacementPolicy::default(),
            catalog_search_policy: CatalogSearchPolicy::default(),
            shapes,
        }
    }

    fn test_catalog_shape(
        shape_id: &str,
        piece_kinds: &[&str],
        allowed_transforms: &[&str],
        exits: Vec<CatalogExit>,
        feature_sockets: Vec<FeatureSocket>,
        tags: &[&str],
    ) -> CatalogShape {
        CatalogShape {
            shape_id: shape_id.to_owned(),
            label: shape_id.to_owned(),
            piece_kinds: piece_kinds.iter().map(|kind| (*kind).to_owned()).collect(),
            footprint: vec![GridCell { x: 0, y: 0 }],
            reserved_cells: Vec::new(),
            exits,
            allowed_transforms: allowed_transforms
                .iter()
                .map(|transform| (*transform).to_owned())
                .collect(),
            feature_sockets,
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        }
    }

    fn test_catalog_exit(id: &str, direction: &str) -> CatalogExit {
        let (x, y) = direction_vector(direction);
        CatalogExit {
            id: id.to_owned(),
            x,
            y,
            direction: direction.to_owned(),
            width: 1,
            tags: Vec::new(),
        }
    }

    fn test_piece_plan(requirements: Vec<PieceRequirement>) -> PieceBuildPlan {
        PieceBuildPlan {
            kind: "asha_procgen.piece_build_plan.v1".to_owned(),
            schema_version: 1,
            plan_id: "piece_plan.test".to_owned(),
            candidate_id: "candidate.test".to_owned(),
            geometry_id: "geometry.test".to_owned(),
            corridor_realization: CorridorRealization::Catalog,
            source_candidate_ref: "artifacts/test/candidate.json".to_owned(),
            source_intermediate_ref: "artifacts/test/intermediate.json".to_owned(),
            source_geometry_ref: "artifacts/test/geometry.json".to_owned(),
            requirements,
            links: Vec::new(),
            content_requirements: Vec::new(),
        }
    }

    fn test_piece_requirement(
        piece_id: &str,
        kind: &str,
        required_exits: Vec<PieceExitRequirement>,
        required_sockets: Vec<String>,
        tags: &[&str],
    ) -> PieceRequirement {
        PieceRequirement {
            piece_id: piece_id.to_owned(),
            kind: kind.to_owned(),
            role: kind.to_owned(),
            source_refs: Vec::new(),
            required_exits,
            required_sockets,
            required_shape_tags: Vec::new(),
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            placement_hints: Vec::new(),
        }
    }

    fn test_piece_exit(id: &str, direction: &str) -> PieceExitRequirement {
        PieceExitRequirement {
            id: id.to_owned(),
            direction: direction.to_owned(),
            width: 1,
            tags: Vec::new(),
        }
    }

    fn test_piece_link(
        id: &str,
        from_piece: &str,
        from_exit: &str,
        to_piece: &str,
        to_exit: &str,
        source_section: &str,
    ) -> PieceLink {
        PieceLink {
            id: id.to_owned(),
            from_piece: from_piece.to_owned(),
            from_exit: from_exit.to_owned(),
            to_piece: to_piece.to_owned(),
            to_exit: to_exit.to_owned(),
            source_section: source_section.to_owned(),
            source_corridor: "geometry.corridor.test".to_owned(),
            source_edge: "edge.test".to_owned(),
            source_edges: vec!["edge.test".to_owned()],
            traversal_refs: Vec::new(),
            source_ref: "test:piece-link".to_owned(),
            traversal: "open".to_owned(),
            required_item: None,
            tags: Vec::new(),
            route_points: Vec::new(),
        }
    }

    fn test_match_args(seed: u64) -> BuildMatchShapesArgs {
        BuildMatchShapesArgs {
            catalog: PathBuf::from("fixtures/shape-catalogs/2d-basic.json"),
            piece_plan: PathBuf::from("artifacts/test/piece-plan.json"),
            seed,
            out: PathBuf::from("artifacts/test/piece-shape-match.json"),
        }
    }

    fn pure_catalog_error_evidence(error: &str) -> serde_json::Value {
        let serialized = error
            .rsplit_once(PURE_CATALOG_EXHAUSTION_MARKER)
            .map(|(_, evidence)| evidence)
            .expect("pure catalog error must include structured exhaustion evidence");
        serde_json::from_str(serialized).expect("structured exhaustion evidence must be JSON")
    }

    fn placement_bounds(placement: &PiecePlacement) -> (i32, i32) {
        let min_x = placement
            .occupied_cells
            .iter()
            .map(|cell| cell.x)
            .min()
            .unwrap_or(0);
        let max_x = placement
            .occupied_cells
            .iter()
            .map(|cell| cell.x)
            .max()
            .unwrap_or(0);
        let min_y = placement
            .occupied_cells
            .iter()
            .map(|cell| cell.y)
            .min()
            .unwrap_or(0);
        let max_y = placement
            .occupied_cells
            .iter()
            .map(|cell| cell.y)
            .max()
            .unwrap_or(0);
        (max_x - min_x + 1, max_y - min_y + 1)
    }

    #[test]
    fn loads_default_batch_profile_fixture() {
        let profile_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join(DEFAULT_BATCH_PROFILE);
        let profile = load_batch_profile(&profile_path).expect("default profile should load");
        assert_eq!(profile.kind, "asha_procgen.batch_profile.v1");
        assert_eq!(profile.sequences.len(), 7);
        let first = batch_profile_sequence(&profile, 0).expect("first sequence");
        assert_eq!(first.label, "hub-merge");
        assert_eq!(
            first.rules,
            vec![
                GraphRule::LockKeyLoop,
                GraphRule::HubSpokeCluster,
                GraphRule::BranchMergeShortcut
            ]
        );
        let cycled = batch_profile_sequence(&profile, 7).expect("cycled sequence");
        assert_eq!(cycled.label, "hub-merge");
    }

    #[test]
    fn selection_rejection_taxonomy_keeps_exhaustion_distinct_from_incompatibility() {
        for (error, expected) in [
            (
                "geometry search exhausted after 80 route attempt(s)",
                "selection_geometry_route_search_exhausted",
            ),
            (
                "invalid physical geometry plan: unsupported physical section topology",
                "selection_physical_section_plan_incompatible",
            ),
            (
                "room port allocation exhausted after 2 alternatives",
                "selection_geometry_port_allocation_exhausted",
            ),
            (
                "catalog shape/transform search exhausted with 1 unmatched requirement",
                "selection_catalog_shape_transform_exhausted",
            ),
            (
                "pure catalog coverage rejected 1 unmatched requirement",
                "selection_catalog_shape_transform_exhausted",
            ),
            (
                "pure catalog search exhausted after 12 decision(s)",
                "selection_catalog_piece_search_exhausted",
            ),
            (
                "piece realization search exhausted after 2 scale tier(s)",
                "selection_piece_route_clearance_exhausted",
            ),
            (
                "global shape/piece search budget exhausted after 4 alternatives",
                "selection_global_search_budget_exhausted",
            ),
        ] {
            assert_eq!(selection_rejection_code(error), expected);
        }
        assert_eq!(
            selection_rejection_code("piece placement validation failed"),
            "selection_physical_embedding_validation_failed"
        );
    }

    #[test]
    fn loads_default_shape_catalog_fixture() {
        let catalog_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join(DEFAULT_SHAPE_CATALOG);
        let catalog: ShapeCatalog = read_json(&catalog_path).expect("shape catalog should load");
        assert_eq!(catalog.kind, "asha_procgen.shape_catalog.v1");
        assert_eq!(catalog.catalog_id, "shape_catalog.2d_basic.v1");
        assert!(catalog.cell_size > 0);
        assert!(catalog.shapes.len() >= 12);

        let mut shape_ids = BTreeSet::new();
        let mut piece_kinds = BTreeSet::new();
        let mut feature_kinds = BTreeSet::new();
        let allowed_directions = BTreeSet::from(["north", "east", "south", "west"]);
        let allowed_transforms = BTreeSet::from([
            "identity",
            "rotate90",
            "rotate180",
            "rotate270",
            "mirrorX",
            "mirrorY",
        ]);
        for shape in &catalog.shapes {
            assert!(
                shape_ids.insert(shape.shape_id.as_str()),
                "duplicate shape id {}",
                shape.shape_id
            );
            assert!(!shape.piece_kinds.is_empty(), "{} has no piece kinds", shape.shape_id);
            assert!(!shape.footprint.is_empty(), "{} has no footprint", shape.shape_id);
            assert!(!shape.exits.is_empty(), "{} has no exits", shape.shape_id);
            assert!(
                shape
                    .allowed_transforms
                    .iter()
                    .all(|transform| allowed_transforms.contains(transform.as_str())),
                "{} has an unsupported transform",
                shape.shape_id
            );
            for exit in &shape.exits {
                assert!(
                    allowed_directions.contains(exit.direction.as_str()),
                    "{} has unsupported exit direction {}",
                    shape.shape_id,
                    exit.direction
                );
                assert!(exit.width > 0, "{} has non-positive exit width", shape.shape_id);
            }
            piece_kinds.extend(shape.piece_kinds.iter().map(String::as_str));
            feature_kinds.extend(shape.feature_sockets.iter().map(|socket| socket.kind.as_str()));
        }

        for required in [
            "room",
            "corridor",
            "bend",
            "junction",
            "threshold",
            "reward",
            "hazard",
            "boss",
            "secret",
            "shortcut",
            "resource",
        ] {
            assert!(piece_kinds.contains(required), "{required} piece kind missing");
        }
        for (shape_id, family_tag) in [
            ("shape.corridor.straight.1", "span_short"),
            ("shape.corridor.straight.3", "span_medium"),
            ("shape.corridor.straight.7", "span_long"),
            ("shape.corridor.bend.2x2", "bend_small"),
            ("shape.corridor.bend.4x4", "bend_large"),
        ] {
            let shape = catalog
                .shapes
                .iter()
                .find(|shape| shape.shape_id == shape_id)
                .unwrap_or_else(|| panic!("{shape_id} missing"));
            assert!(shape.tags.contains(&family_tag.to_owned()));
        }
        for shape in catalog
            .shapes
            .iter()
            .filter(|shape| shape.piece_kinds.iter().any(|kind| kind == "junction"))
        {
            assert!(shape.tags.contains(&"planned_junction".to_owned()));
            assert!(!shape.piece_kinds.iter().any(|kind| kind == "corridor"));
        }
        for required in [
            "container",
            "boss_space",
            "gate_line",
            "hazard_zone",
            "reward_cache",
            "key_pickup",
            "secret_marker",
            "shortcut_marker",
            "resource_clue",
        ] {
            assert!(feature_kinds.contains(required), "{required} socket missing");
        }

        for exit_count in 1..=4 {
            assert!(
                catalog
                    .shapes
                    .iter()
                    .any(|shape| shape.piece_kinds.iter().any(|kind| kind == "room")
                        && shape.exits.len() == exit_count),
                "{exit_count}-exit room shape missing"
            );
        }
    }

    #[test]
    fn catalog_inspect_reports_default_shape_vocabulary() {
        let catalog_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join(DEFAULT_SHAPE_CATALOG);
        let catalog: ShapeCatalog = read_json(&catalog_path).expect("shape catalog should load");
        let report = inspect_shape_catalog(&catalog, &catalog_path);

        assert_eq!(report.kind, "asha_procgen.catalog_inspection.v1");
        assert_eq!(report.catalog_id, "shape_catalog.2d_basic.v1");
        assert_eq!(report.shape_count, catalog.shapes.len());
        assert_eq!(report.placement_policy, PiecePlacementPolicy::default());
        assert_eq!(
            report.catalog_search_policy,
            CatalogSearchPolicy::default()
        );
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
        for required in ["room", "corridor", "connector", "threshold", "reward", "key"] {
            assert!(report.piece_kinds.contains(&required.to_owned()));
        }
        for required in ["north", "east", "south", "west"] {
            assert!(report.exit_directions.contains(&required.to_owned()));
        }
        assert!(report.transforms.contains(&"rotate90".to_owned()));
    }

    #[test]
    fn catalog_inspect_rejects_unowned_junction_shapes() {
        let junction = test_catalog_shape(
            "shape.junction.unowned",
            &["junction"],
            &["identity"],
            vec![
                test_catalog_exit("exit.west", "west"),
                test_catalog_exit("exit.east", "east"),
                test_catalog_exit("exit.south", "south"),
            ],
            Vec::new(),
            &["junction_t"],
        );
        let catalog = test_shape_catalog(vec![junction]);
        let report = inspect_shape_catalog(&catalog, Path::new("fixtures/test-catalog.json"));
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "catalog_junction_ownership_tag_missing"
        }));
    }

    #[test]
    fn placement_policy_fails_closed_when_walls_or_boundaries_are_unsafe() {
        let mut catalog = test_shape_catalog(Vec::new());
        catalog.placement_policy.minimum_clearance_cells = 2;
        catalog.placement_policy.wall_thickness_cells = 1;
        catalog.placement_policy.doorway_width_cells = 2;
        catalog.placement_policy.preserve_piece_boundaries = false;
        let report = inspect_shape_catalog(&catalog, Path::new("fixtures/test-catalog.json"));
        let codes = report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<BTreeSet<_>>();

        assert!(codes.contains("catalog_minimum_clearance_too_small_for_walls"));
        assert!(codes.contains("catalog_doorway_width_invalid"));
        assert!(codes.contains("catalog_piece_boundary_preservation_required"));

        catalog.placement_policy.minimum_clearance_cells = 3;
        catalog.placement_policy.wall_thickness_cells = 1;
        catalog.placement_policy.doorway_width_cells = 3;
        catalog.placement_policy.preserve_piece_boundaries = true;
        let report = inspect_shape_catalog(&catalog, Path::new("fixtures/test-catalog.json"));
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "catalog_doorway_width_unsupported"));

        catalog.catalog_search_policy.max_decisions = 0;
        let report = inspect_shape_catalog(&catalog, Path::new("fixtures/test-catalog.json"));
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "catalog_search_policy_invalid"));
    }

    #[test]
    fn scoring_rewards_cycles() {
        let intent = SeedIntent {
            kind: "asha_procgen.seed_intent.v1".to_owned(),
            id: "score".to_owned(),
            title: "Score".to_owned(),
            target_dimension: "topology_graph".to_owned(),
            desired_patterns: Vec::new(),
            notes: Vec::new(),
        };
        let mut candidate = create_initial_candidate(&intent, 1);
        let base = score_graph(&candidate).overall;
        apply_graph_rule(&mut candidate, GraphRule::LockKeyLoop, 2);
        apply_graph_rule(&mut candidate, GraphRule::OptionalTreasureDetour, 3);
        let richer = score_graph(&candidate).overall;
        assert!(richer > base);
    }

    #[test]
    fn embeds_valid_graph() {
        let intent = SeedIntent {
            kind: "asha_procgen.seed_intent.v1".to_owned(),
            id: "embed".to_owned(),
            title: "Embed".to_owned(),
            target_dimension: "topology_graph".to_owned(),
            desired_patterns: Vec::new(),
            notes: Vec::new(),
        };
        let mut candidate = create_initial_candidate(&intent, 1);
        apply_graph_rule(&mut candidate, GraphRule::LockKeyLoop, 2);
        let layout = embed_2d(&candidate, 3);
        assert_eq!(layout.candidate_id, candidate.candidate_id);
        assert_eq!(layout.rooms.len(), candidate.graph.nodes.len());
        assert_eq!(layout.links.len(), candidate.graph.edges.len());
    }
}
