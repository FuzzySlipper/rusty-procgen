use std::collections::BTreeMap;

use engine_spatial::VoxelSourceRevision;
use rusty_procgen_engine_ca_benchmark::{
    benchmark_suite, BenchmarkClock, BenchmarkRunConfig, CaBenchmarkEnvironment, CaProjectionOp,
    CaSpatialError, CaSpatialHost, CaSpatialOptions,
};
use rusty_procgen_preflight::cellular_automata::{
    CaBoundaryPolicy, CaBounds, CaCellState, CaCoord, CaNeighborhood, CaRule, CaScenario,
    CaScenarioSuite, CaSeedCell, CaWorkloadClass,
};

const REPOSITORY_COMMIT: &str = "1111111111111111111111111111111111111111";
const ENGINE_COMMIT: &str = "db5641fc4e9d033112bc2b374a35933c3838e39c";

#[derive(Default)]
struct TickClock(u64);

impl BenchmarkClock for TickClock {
    fn now_ns(&mut self) -> u64 {
        let current = self.0;
        self.0 += 10;
        current
    }
}

#[test]
fn checked_suite_is_deterministic_through_real_engine_authority() {
    let suite = checked_suite();
    let evidence = benchmark_suite(
        &suite,
        REPOSITORY_COMMIT,
        ENGINE_COMMIT,
        test_environment(),
        BenchmarkRunConfig {
            warmup_runs: 0,
            recorded_runs: 2,
            ..BenchmarkRunConfig::default()
        },
        TickClock::default,
    )
    .unwrap();

    assert_eq!(evidence.scenarios.len(), 5);
    for scenario in &evidence.scenarios {
        assert!(scenario.deterministic_structural_evidence);
        assert_eq!(scenario.recorded_runs.len(), 2);
        assert_eq!(
            scenario.recorded_runs[0].structural_hash,
            scenario.recorded_runs[1].structural_hash
        );
        assert!(scenario
            .trace
            .steps
            .iter()
            .all(|step| step.readout.projection_revisions_coherent));
        assert!(scenario.recorded_runs.iter().all(|run| run
            .step_timings
            .iter()
            .all(|timing| timing.ca_step_ns == 10)));
        assert_projection_trace_is_self_consistent(&scenario.trace);
    }

    let large = evidence
        .scenarios
        .iter()
        .find(|scenario| scenario.scenario_id == "large-resident-small-hot-region")
        .unwrap();
    assert!(large.trace.materialize_empty);
    assert_eq!(large.trace.initial.readout.solid_voxel_count, 64 * 16 * 64);
    assert!(large.trace.initial.readout.resident_chunk_count > 100);
    assert!(large
        .trace
        .steps
        .iter()
        .all(|step| step.canonical_edit_count < 200));
}

#[test]
fn dropped_and_superseded_previews_do_not_mutate_either_owner() {
    let scenario = checked_suite().scenarios.remove(0);
    let options = CaSpatialOptions::for_workload(scenario.workload);
    let mut clock = TickClock::default();
    let (mut host, _) = CaSpatialHost::admit(scenario, options, &mut clock).unwrap();
    let original_revision = host.source_revision();
    let original_authority = host.authority_hash();
    let original_state = host.current_state_hash().unwrap();
    let original_solids = host.solid_voxel_count();

    let dropped = host
        .prepare_next_step(original_revision, &mut clock)
        .unwrap();
    drop(dropped);
    assert_eq!(host.source_revision(), original_revision);
    assert_eq!(host.authority_hash(), original_authority);
    assert_eq!(host.current_state_hash().unwrap(), original_state);
    assert_eq!(host.solid_voxel_count(), original_solids);

    let first = host
        .prepare_next_step(original_revision, &mut clock)
        .unwrap();
    let superseded = host
        .prepare_next_step(original_revision, &mut clock)
        .unwrap();
    host.commit_prepared(first, &mut clock).unwrap();
    let accepted_revision = host.source_revision();
    let accepted_authority = host.authority_hash();
    let accepted_state = host.current_state_hash().unwrap();

    assert!(matches!(
        host.commit_prepared(superseded, &mut clock),
        Err(CaSpatialError::PreparedCaStateChanged { .. })
    ));
    assert_eq!(host.source_revision(), accepted_revision);
    assert_eq!(host.authority_hash(), accepted_authority);
    assert_eq!(host.current_state_hash().unwrap(), accepted_state);
}

#[test]
fn stale_and_oversized_requests_reject_before_publication() {
    let scenario = checked_suite().scenarios.remove(0);
    let options = CaSpatialOptions::for_workload(scenario.workload);
    let mut clock = TickClock::default();
    let (host, _) = CaSpatialHost::admit(scenario, options, &mut clock).unwrap();
    let original_authority = host.authority_hash();
    let original_state = host.current_state_hash().unwrap();
    assert!(matches!(
        host.prepare_next_step(VoxelSourceRevision::new(99), &mut clock),
        Err(CaSpatialError::RevisionMismatch { .. })
    ));
    assert_eq!(host.authority_hash(), original_authority);
    assert_eq!(host.current_state_hash().unwrap(), original_state);

    let scenario = large_churn_scenario();
    let options = CaSpatialOptions::for_workload(scenario.workload);
    let (host, _) = CaSpatialHost::admit(scenario, options, &mut clock).unwrap();
    let original_revision = host.source_revision();
    let original_authority = host.authority_hash();
    let original_state = host.current_state_hash().unwrap();
    assert!(matches!(
        host.prepare_next_step(original_revision, &mut clock),
        Err(CaSpatialError::StepEditQuotaExceeded { .. })
    ));
    assert_eq!(host.source_revision(), original_revision);
    assert_eq!(host.authority_hash(), original_authority);
    assert_eq!(host.current_state_hash().unwrap(), original_state);
}

#[test]
fn malformed_scenario_and_benchmark_options_fail_closed() {
    let mut scenario = checked_suite().scenarios.remove(0);
    scenario.bounds.max_exclusive.x = scenario.bounds.min.x;
    assert!(matches!(
        CaSpatialHost::admit(
            scenario,
            CaSpatialOptions::for_workload(CaWorkloadClass::SparsePropagation),
            &mut TickClock::default()
        ),
        Err(CaSpatialError::Scenario(_))
    ));

    let scenario = checked_suite().scenarios.remove(0);
    let mut options = CaSpatialOptions::for_workload(scenario.workload);
    options.palette.trail = options.palette.source;
    assert!(matches!(
        CaSpatialHost::admit(scenario, options, &mut TickClock::default()),
        Err(CaSpatialError::InvalidOptions { .. })
    ));

    assert!(matches!(
        benchmark_suite(
            &checked_suite(),
            "short",
            ENGINE_COMMIT,
            test_environment(),
            BenchmarkRunConfig::default(),
            TickClock::default,
        ),
        Err(CaSpatialError::InvalidRunConfig { .. })
    ));
}

fn test_environment() -> CaBenchmarkEnvironment {
    CaBenchmarkEnvironment {
        operating_system: "test-os".to_owned(),
        architecture: "test-arch".to_owned(),
        rustc_version: "rustc test".to_owned(),
        build_profile: "test".to_owned(),
        clock: "scripted 10ns ticks".to_owned(),
    }
}

fn checked_suite() -> CaScenarioSuite {
    serde_json::from_str(include_str!("../../../fixtures/ca/scenarios.v1.json")).unwrap()
}

fn large_churn_scenario() -> CaScenario {
    CaScenario {
        id: "oversized-step".to_owned(),
        workload: CaWorkloadClass::DenseChurn,
        seed: 17,
        bounds: CaBounds {
            min: CaCoord { x: 0, y: 0, z: 0 },
            max_exclusive: CaCoord {
                x: 20,
                y: 1,
                z: 500,
            },
        },
        neighborhood: CaNeighborhood::VonNeumann6,
        boundary: CaBoundaryPolicy::FixedEmpty,
        rule: CaRule::ParityChurnV1,
        steps: 1,
        initial_cells: vec![CaSeedCell {
            coord: CaCoord { x: 0, y: 0, z: 0 },
            state: CaCellState::Source,
        }],
    }
}

fn assert_projection_trace_is_self_consistent(
    trace: &rusty_procgen_engine_ca_benchmark::CaSpatialTrace,
) {
    let mut chunks = trace
        .initial
        .projection_chunks
        .iter()
        .map(|chunk| (chunk.chunk, chunk.buffer_hash.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut revision = trace.initial.readout.source_revision;
    let mut trace_hash = trace.initial.trace_hash.clone();
    for step in &trace.steps {
        assert_eq!(step.revision_before, revision);
        assert_eq!(step.previous_trace_hash, trace_hash);
        for op in &step.projection_ops {
            match op {
                CaProjectionOp::Upsert { chunk } => {
                    assert!(!chunk.positions.is_empty());
                    chunks.insert(chunk.chunk, chunk.buffer_hash.clone());
                }
                CaProjectionOp::Delete { chunk } => {
                    chunks.remove(chunk);
                }
            }
        }
        assert_eq!(chunks.len(), step.readout.mesh_chunk_count);
        revision = step.accepted_revision;
        trace_hash = step.trace_hash.clone();
    }
}
