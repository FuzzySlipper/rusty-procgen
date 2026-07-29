use rusty_procgen_preflight::cellular_automata::{CaScenario, CaScenarioSuite, CA_MAX_SCENARIOS};

use crate::host::{hash_value, serialization_error};
use crate::model::{
    BenchmarkClock, CaBenchmarkConfig, CaBenchmarkEnvironment, CaRecordedRun, CaScenarioBenchmark,
    CaSpatialBenchmarkEvidence, CaSpatialError, CaSpatialOptions, CaSpatialTrace,
    CA_SPATIAL_BENCHMARK_KIND, CA_SPATIAL_TRACE_KIND, DEFAULT_MAX_RECORDED_RUNS,
};
use crate::CaSpatialHost;

const MAX_WARMUP_RUNS: u32 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BenchmarkRunConfig {
    pub warmup_runs: u32,
    pub recorded_runs: u32,
    pub chunk_size: u32,
    pub max_edits_per_step: usize,
    pub max_mesh_values_per_step: usize,
}

impl Default for BenchmarkRunConfig {
    fn default() -> Self {
        let options = CaSpatialOptions::for_workload(
            rusty_procgen_preflight::cellular_automata::CaWorkloadClass::SparsePropagation,
        );
        Self {
            warmup_runs: 1,
            recorded_runs: 2,
            chunk_size: options.chunk_size,
            max_edits_per_step: options.max_edits_per_step,
            max_mesh_values_per_step: options.max_mesh_values_per_step,
        }
    }
}

pub fn benchmark_suite<C, F>(
    suite: &CaScenarioSuite,
    repository_commit: &str,
    engine_commit: &str,
    environment: CaBenchmarkEnvironment,
    config: BenchmarkRunConfig,
    mut clock_factory: F,
) -> Result<CaSpatialBenchmarkEvidence, CaSpatialError>
where
    C: BenchmarkClock,
    F: FnMut() -> C,
{
    suite.validate()?;
    validate_commit("repository", repository_commit)?;
    validate_commit("Engine", engine_commit)?;
    validate_run_config(config, suite.scenarios.len())?;

    let mut scenarios = Vec::with_capacity(suite.scenarios.len());
    for scenario in &suite.scenarios {
        for _ in 0..config.warmup_runs {
            run_scenario(scenario.clone(), config, &mut clock_factory())?;
        }

        let mut trace = None;
        let mut structural_hash = None;
        let mut recorded_runs = Vec::with_capacity(config.recorded_runs as usize);
        for run in 1..=config.recorded_runs {
            let result = run_scenario(scenario.clone(), config, &mut clock_factory())?;
            if let Some(expected) = &structural_hash {
                if expected != &result.structural_hash {
                    return Err(CaSpatialError::Serialization {
                        detail: format!(
                            "scenario {} changed structural hash across recorded runs: {expected} != {}",
                            scenario.id, result.structural_hash
                        ),
                    });
                }
            } else {
                structural_hash = Some(result.structural_hash.clone());
                trace = Some(result.trace.clone());
            }
            recorded_runs.push(CaRecordedRun {
                run,
                structural_hash: result.structural_hash,
                admission_timing: result.admission_timing,
                step_timings: result.step_timings,
                encoded_trace_bytes: result.encoded_trace_bytes,
            });
        }
        scenarios.push(CaScenarioBenchmark {
            scenario_id: scenario.id.clone(),
            warmup_runs: config.warmup_runs,
            deterministic_structural_evidence: recorded_runs
                .windows(2)
                .all(|runs| runs[0].structural_hash == runs[1].structural_hash),
            recorded_runs,
            trace: trace.expect("positive recorded-run quota"),
        });
    }

    Ok(CaSpatialBenchmarkEvidence {
        kind: CA_SPATIAL_BENCHMARK_KIND.to_owned(),
        schema_version: 1,
        repository_commit: repository_commit.to_owned(),
        engine_commit: engine_commit.to_owned(),
        environment,
        config: CaBenchmarkConfig {
            warmup_runs: config.warmup_runs,
            recorded_runs: config.recorded_runs,
            chunk_size: config.chunk_size,
            max_edits_per_step: config.max_edits_per_step,
            max_mesh_values_per_step: config.max_mesh_values_per_step,
        },
        scenarios,
        non_claims: vec![
            "Timings are same-host observations and are not pass/fail thresholds.".to_owned(),
            "This evidence does not certify renderer playback or browser performance.".to_owned(),
            "This workload is not a gameplay, scheduling, or persistence runtime.".to_owned(),
            "The checked matrix is bounded baseline evidence, not an Engine scale ceiling."
                .to_owned(),
        ],
    })
}

struct ScenarioRun {
    structural_hash: String,
    trace: CaSpatialTrace,
    admission_timing: crate::CaAdmissionTiming,
    step_timings: Vec<crate::CaStepTiming>,
    encoded_trace_bytes: usize,
}

fn run_scenario(
    scenario: CaScenario,
    config: BenchmarkRunConfig,
    clock: &mut impl BenchmarkClock,
) -> Result<ScenarioRun, CaSpatialError> {
    let mut options = CaSpatialOptions::for_workload(scenario.workload);
    options.chunk_size = config.chunk_size;
    options.max_edits_per_step = config.max_edits_per_step;
    options.max_mesh_values_per_step = config.max_mesh_values_per_step;

    let scenario_identity = scenario.clone();
    let (mut host, admission_timing) = CaSpatialHost::admit(scenario, options, clock)?;
    let initial = host.initial().clone();
    let mut steps = Vec::with_capacity(scenario_identity.steps as usize);
    let mut step_timings = Vec::with_capacity(scenario_identity.steps as usize);
    while host.completed_steps() < scenario_identity.steps {
        let prepared = host.prepare_next_step(host.source_revision(), clock)?;
        let (step, timing) = host.commit_prepared(prepared, clock)?;
        steps.push(step);
        step_timings.push(timing);
    }

    let trace = CaSpatialTrace {
        kind: CA_SPATIAL_TRACE_KIND.to_owned(),
        schema_version: 1,
        scenario_id: scenario_identity.id,
        workload: scenario_identity.workload,
        rule_id: scenario_identity.rule.id().to_owned(),
        seed: scenario_identity.seed,
        bounds: scenario_identity.bounds,
        neighborhood: scenario_identity.neighborhood,
        boundary: scenario_identity.boundary,
        materialize_empty: options.materialize_empty,
        initial,
        steps,
    };
    let encoded = serde_json::to_vec(&trace).map_err(serialization_error)?;
    Ok(ScenarioRun {
        structural_hash: hash_value(&trace)?,
        trace,
        admission_timing,
        step_timings,
        encoded_trace_bytes: encoded.len(),
    })
}

fn validate_commit(label: &str, commit: &str) -> Result<(), CaSpatialError> {
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(CaSpatialError::InvalidRunConfig {
            detail: format!("{label} commit must be a full lowercase hexadecimal SHA"),
        });
    }
    Ok(())
}

fn validate_run_config(
    config: BenchmarkRunConfig,
    scenario_count: usize,
) -> Result<(), CaSpatialError> {
    if config.warmup_runs > MAX_WARMUP_RUNS {
        return Err(CaSpatialError::InvalidRunConfig {
            detail: format!(
                "warmup run count {} exceeds {MAX_WARMUP_RUNS}",
                config.warmup_runs
            ),
        });
    }
    if config.recorded_runs == 0 || config.recorded_runs > DEFAULT_MAX_RECORDED_RUNS {
        return Err(CaSpatialError::InvalidRunConfig {
            detail: format!(
                "recorded run count {} is outside 1..={DEFAULT_MAX_RECORDED_RUNS}",
                config.recorded_runs
            ),
        });
    }
    if scenario_count == 0 || scenario_count > CA_MAX_SCENARIOS {
        return Err(CaSpatialError::InvalidRunConfig {
            detail: format!("scenario count {scenario_count} is outside 1..={CA_MAX_SCENARIOS}"),
        });
    }
    Ok(())
}
