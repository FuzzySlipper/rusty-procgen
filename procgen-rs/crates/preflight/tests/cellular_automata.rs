use rusty_procgen_preflight::cellular_automata::{
    run_ca_scenario, CaAutomaton, CaBoundaryPolicy, CaBounds, CaCellState, CaCoord, CaError,
    CaNeighborhood, CaRule, CaScenario, CaScenarioSuite, CaSeedCell, CaWorkloadClass,
    CA_DELTA_FIXTURE_SET_KIND, CA_MAX_CELL_STEPS_PER_SCENARIO, CA_MAX_CELL_STEPS_PER_SUITE,
    CA_MAX_SEED_CELLS, CA_MAX_STEPS,
};

fn checked_suite() -> CaScenarioSuite {
    serde_json::from_str(include_str!("../../../../fixtures/ca/scenarios.v1.json"))
        .expect("checked CA scenario suite")
}

fn sparse_scenario() -> CaScenario {
    checked_suite()
        .scenarios
        .into_iter()
        .find(|scenario| scenario.id == "sparse-propagation")
        .expect("sparse scenario")
}

fn dense_scenario(id: &str, volume: i32, steps: u32) -> CaScenario {
    CaScenario {
        id: id.to_owned(),
        workload: CaWorkloadClass::DenseChurn,
        seed: 11,
        bounds: CaBounds {
            min: CaCoord { x: 0, y: 0, z: 0 },
            max_exclusive: CaCoord {
                x: volume,
                y: 1,
                z: 1,
            },
        },
        neighborhood: CaNeighborhood::VonNeumann6,
        boundary: CaBoundaryPolicy::FixedEmpty,
        rule: CaRule::ParityChurnV1,
        steps,
        initial_cells: Vec::new(),
    }
}

#[test]
fn checked_scenarios_generate_exact_checked_delta_fixtures() {
    let suite = checked_suite();
    suite.validate().expect("valid checked scenario suite");
    let generated = suite
        .generate_delta_fixtures()
        .expect("generated delta fixtures");
    let checked: serde_json::Value =
        serde_json::from_str(include_str!("../../../../fixtures/ca/delta-traces.v1.json"))
            .expect("checked delta fixture JSON");
    assert_eq!(
        serde_json::to_value(&generated).expect("generated JSON"),
        checked
    );
    assert_eq!(generated.kind, CA_DELTA_FIXTURE_SET_KIND);
    assert_eq!(generated.traces.len(), 5);

    let classes = generated
        .traces
        .iter()
        .map(|trace| trace.workload)
        .collect::<Vec<_>>();
    assert!(classes.contains(&CaWorkloadClass::SparsePropagation));
    assert!(classes.contains(&CaWorkloadClass::DenseChurn));
    assert!(classes.contains(&CaWorkloadClass::CrossBoundary));
    assert!(classes.contains(&CaWorkloadClass::LargeResidentSmallHotRegion));
    assert!(classes.contains(&CaWorkloadClass::HighSurfaceArea));
}

#[test]
fn runs_repeat_and_seed_or_rule_changes_are_explicit() {
    let scenario = sparse_scenario();
    let first = run_ca_scenario(scenario.clone()).expect("first trace");
    let repeated = run_ca_scenario(scenario.clone()).expect("repeated trace");
    assert_eq!(first, repeated);

    let mut changed_seed = scenario.clone();
    changed_seed.seed += 1;
    let changed_seed = run_ca_scenario(changed_seed).expect("changed-seed trace");
    assert_ne!(first.final_scenario_hash, changed_seed.final_scenario_hash);

    let mut changed_rule = scenario;
    changed_rule.rule = CaRule::ParityChurnV1;
    let changed_rule = run_ca_scenario(changed_rule).expect("changed-rule trace");
    assert_ne!(first.steps[0].delta_hash, changed_rule.steps[0].delta_hash);
    assert_ne!(first.final_scenario_hash, changed_rule.final_scenario_hash);
}

#[test]
fn deltas_are_ordered_bounded_and_structurally_accounted() {
    for scenario in checked_suite().scenarios {
        let volume = scenario.bounds.volume().expect("bounded volume");
        let trace = run_ca_scenario(scenario).expect("trace");
        for step in trace.steps {
            assert!(step
                .deltas
                .windows(2)
                .all(|pair| pair[0].coord < pair[1].coord));
            assert_eq!(step.changed_cell_count as usize, step.deltas.len());
            assert_eq!(
                step.state_counts.empty
                    + step.state_counts.source
                    + step.state_counts.frontier
                    + step.state_counts.trail,
                volume
            );
            assert!(step.active_cell_count <= volume);
            assert!(step.evaluated_cell_count <= volume);
            for delta in step.deltas {
                assert_ne!(delta.previous, delta.current);
            }
        }
    }
}

#[test]
fn wrapped_boundary_propagates_across_negative_and_positive_edges() {
    let scenario = checked_suite()
        .scenarios
        .into_iter()
        .find(|scenario| scenario.id == "cross-boundary")
        .expect("cross-boundary scenario");
    let mut automaton = CaAutomaton::new(scenario).expect("automaton");
    let first = automaton.step().expect("first step");
    assert!(first.deltas.iter().any(|delta| {
        delta.coord == CaCoord { x: -4, y: 0, z: 4 } && delta.current == CaCellState::Frontier
    }));
    assert!(first.deltas.iter().any(|delta| {
        delta.coord == CaCoord { x: 4, y: 0, z: -4 } && delta.current == CaCellState::Frontier
    }));
}

#[test]
fn frontier_can_stabilize_and_empty_state_remains_extinct() {
    let mut stable = sparse_scenario();
    stable.bounds = CaBounds {
        min: CaCoord { x: 0, y: 0, z: 0 },
        max_exclusive: CaCoord { x: 3, y: 1, z: 3 },
    };
    stable.initial_cells = vec![CaSeedCell {
        coord: CaCoord { x: 1, y: 0, z: 1 },
        state: CaCellState::Source,
    }];
    stable.steps = 6;
    let stable = run_ca_scenario(stable).expect("stable trace");
    assert_eq!(
        stable.steps.last().expect("last step").changed_cell_count,
        0
    );

    let mut extinct = sparse_scenario();
    extinct.initial_cells.clear();
    extinct.steps = 2;
    let extinct = run_ca_scenario(extinct).expect("extinct trace");
    assert!(extinct
        .steps
        .iter()
        .all(|step| step.changed_cell_count == 0 && step.active_cell_count == 0));
}

#[test]
fn malformed_bounds_overflow_and_quotas_reject_typed() {
    let mut invalid = sparse_scenario();
    invalid.bounds.max_exclusive.x = invalid.bounds.min.x;
    assert_eq!(
        CaAutomaton::new(invalid)
            .expect_err("invalid bounds")
            .code(),
        "invalid_bounds"
    );

    let mut overflow = sparse_scenario();
    overflow.bounds = CaBounds {
        min: CaCoord {
            x: i32::MIN,
            y: i32::MIN,
            z: i32::MIN,
        },
        max_exclusive: CaCoord {
            x: i32::MAX,
            y: i32::MAX,
            z: i32::MAX,
        },
    };
    overflow.initial_cells.clear();
    assert_eq!(
        CaAutomaton::new(overflow)
            .expect_err("overflowing volume")
            .code(),
        "volume_overflow"
    );

    let mut oversized = sparse_scenario();
    oversized.bounds = CaBounds {
        min: CaCoord { x: 0, y: 0, z: 0 },
        max_exclusive: CaCoord {
            x: 1_025,
            y: 1_025,
            z: 1,
        },
    };
    oversized.initial_cells.clear();
    assert_eq!(
        CaAutomaton::new(oversized)
            .expect_err("volume quota")
            .code(),
        "volume_quota_exceeded"
    );

    let mut too_many_steps = sparse_scenario();
    too_many_steps.steps = CA_MAX_STEPS + 1;
    assert_eq!(
        CaAutomaton::new(too_many_steps)
            .expect_err("step quota")
            .code(),
        "step_quota_exceeded"
    );

    let mut too_many_seeds = sparse_scenario();
    too_many_seeds.initial_cells = vec![
        CaSeedCell {
            coord: CaCoord { x: 0, y: 0, z: 0 },
            state: CaCellState::Source,
        };
        CA_MAX_SEED_CELLS + 1
    ];
    assert_eq!(
        CaAutomaton::new(too_many_seeds)
            .expect_err("seed quota")
            .code(),
        "seed_quota_exceeded"
    );
}

#[test]
fn aggregate_cell_step_quotas_bound_retained_work_before_generation() {
    let exact_scenario = dense_scenario(
        "exact-scenario-cell-step-limit",
        i32::try_from(CA_MAX_CELL_STEPS_PER_SCENARIO / 4).expect("bounded dimension"),
        4,
    );
    exact_scenario
        .validate()
        .expect("exact scenario cell-step limit");

    let mut one_over_scenario = exact_scenario.clone();
    one_over_scenario.id = "over-scenario-cell-step-limit".to_owned();
    one_over_scenario.bounds.max_exclusive.x += 1;
    assert!(matches!(
        one_over_scenario.validate(),
        Err(CaError::CellStepQuotaExceeded {
            cell_steps,
            limit: CA_MAX_CELL_STEPS_PER_SCENARIO,
            ..
        }) if cell_steps == CA_MAX_CELL_STEPS_PER_SCENARIO + 4
    ));

    let half_suite_limit =
        i32::try_from(CA_MAX_CELL_STEPS_PER_SUITE / 2 / 4).expect("bounded dimension");
    let exact_suite = CaScenarioSuite {
        kind: "rusty_procgen.ca_scenario_suite.v1".to_owned(),
        schema_version: 1,
        scenarios: vec![
            dense_scenario("exact-suite-a", half_suite_limit, 4),
            dense_scenario("exact-suite-b", half_suite_limit, 4),
        ],
    };
    exact_suite.validate().expect("exact suite cell-step limit");

    let mut one_over_suite = exact_suite;
    one_over_suite
        .scenarios
        .push(dense_scenario("over-suite", 1, 1));
    assert!(matches!(
        one_over_suite.validate(),
        Err(CaError::SuiteCellStepQuotaExceeded {
            cell_steps,
            limit: CA_MAX_CELL_STEPS_PER_SUITE,
        }) if cell_steps == CA_MAX_CELL_STEPS_PER_SUITE + 1
    ));

    assert!(matches!(
        one_over_suite.generate_delta_fixtures(),
        Err(CaError::SuiteCellStepQuotaExceeded { .. })
    ));
}

#[test]
fn malformed_seed_and_suite_identities_reject_typed() {
    let mut duplicate = sparse_scenario();
    duplicate
        .initial_cells
        .push(duplicate.initial_cells[0].clone());
    assert_eq!(
        CaAutomaton::new(duplicate)
            .expect_err("duplicate seed")
            .code(),
        "duplicate_seed_cell"
    );

    let mut outside = sparse_scenario();
    outside.initial_cells[0].coord.x = 100;
    assert_eq!(
        CaAutomaton::new(outside).expect_err("outside seed").code(),
        "seed_outside_bounds"
    );

    let mut empty = sparse_scenario();
    empty.initial_cells[0].state = CaCellState::Empty;
    assert_eq!(
        CaAutomaton::new(empty).expect_err("empty seed").code(),
        "empty_seed_cell"
    );

    let mut suite = checked_suite();
    suite.scenarios.push(suite.scenarios[0].clone());
    assert_eq!(
        suite.validate().expect_err("duplicate scenario").code(),
        "duplicate_scenario_id"
    );

    let mut wrong_kind = checked_suite();
    wrong_kind.kind = "invalid.ca_scenario_suite.v2".to_owned();
    assert!(matches!(
        wrong_kind.validate(),
        Err(CaError::InvalidArtifactIdentity { .. })
    ));
}

#[test]
fn public_readback_rejects_coordinates_outside_the_domain() {
    let automaton = CaAutomaton::new(CaScenario {
        id: "readback".to_owned(),
        workload: CaWorkloadClass::SparsePropagation,
        seed: 1,
        bounds: CaBounds {
            min: CaCoord { x: 0, y: 0, z: 0 },
            max_exclusive: CaCoord { x: 2, y: 2, z: 2 },
        },
        neighborhood: CaNeighborhood::Moore26,
        boundary: CaBoundaryPolicy::FixedEmpty,
        rule: CaRule::FrontierTrailV1,
        steps: 1,
        initial_cells: Vec::new(),
    })
    .expect("automaton");
    assert_eq!(
        automaton
            .state_at(CaCoord { x: -1, y: 0, z: 0 })
            .expect_err("outside readback")
            .code(),
        "coordinate_outside_bounds"
    );
}
