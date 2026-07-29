//! Deterministic, bounded cellular-automata workloads.
//!
//! This module owns procedural working state and ordered deltas for downstream
//! stress workloads. It has no Rusty Engine, renderer, filesystem, timing, or
//! gameplay dependency. Consumers may translate accepted deltas into the
//! authority they own.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display};

use serde::{Deserialize, Serialize};

pub const CA_SCENARIO_SUITE_KIND: &str = "rusty_procgen.ca_scenario_suite.v1";
pub const CA_DELTA_FIXTURE_SET_KIND: &str = "rusty_procgen.ca_delta_fixture_set.v1";
pub const CA_MAX_VOLUME: u64 = 1_048_576;
pub const CA_MAX_SEED_CELLS: usize = 4_096;
pub const CA_MAX_STEPS: u32 = 4_096;
pub const CA_MAX_SCENARIOS: usize = 32;

const CA_MAX_ID_LENGTH: usize = 96;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaCellState {
    Empty,
    Source,
    Frontier,
    Trail,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaNeighborhood {
    VonNeumann6,
    Moore26,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaBoundaryPolicy {
    FixedEmpty,
    Wrap,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaRule {
    FrontierTrailV1,
    ParityChurnV1,
}

impl CaRule {
    pub const fn id(self) -> &'static str {
        match self {
            Self::FrontierTrailV1 => "frontier_trail_v1",
            Self::ParityChurnV1 => "parity_churn_v1",
        }
    }

    const fn scans_complete_domain(self) -> bool {
        matches!(self, Self::ParityChurnV1)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaWorkloadClass {
    SparsePropagation,
    DenseChurn,
    CrossBoundary,
    LargeResidentSmallHotRegion,
    HighSurfaceArea,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaCoord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaBounds {
    pub min: CaCoord,
    pub max_exclusive: CaCoord,
}

impl CaBounds {
    pub fn contains(self, coord: CaCoord) -> bool {
        coord.x >= self.min.x
            && coord.x < self.max_exclusive.x
            && coord.y >= self.min.y
            && coord.y < self.max_exclusive.y
            && coord.z >= self.min.z
            && coord.z < self.max_exclusive.z
    }

    pub fn dimensions(self) -> Result<[u64; 3], CaError> {
        let x = axis_span("x", self.min.x, self.max_exclusive.x)?;
        let y = axis_span("y", self.min.y, self.max_exclusive.y)?;
        let z = axis_span("z", self.min.z, self.max_exclusive.z)?;
        Ok([x, y, z])
    }

    pub fn volume(self) -> Result<u64, CaError> {
        let [x, y, z] = self.dimensions()?;
        x.checked_mul(y)
            .and_then(|xy| xy.checked_mul(z))
            .ok_or(CaError::VolumeOverflow)
    }

    fn all_coords(self) -> BTreeSet<CaCoord> {
        let mut coords = BTreeSet::new();
        for x in self.min.x..self.max_exclusive.x {
            for y in self.min.y..self.max_exclusive.y {
                for z in self.min.z..self.max_exclusive.z {
                    coords.insert(CaCoord { x, y, z });
                }
            }
        }
        coords
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaSeedCell {
    pub coord: CaCoord,
    pub state: CaCellState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaScenario {
    pub id: String,
    pub workload: CaWorkloadClass,
    pub seed: u64,
    pub bounds: CaBounds,
    pub neighborhood: CaNeighborhood,
    pub boundary: CaBoundaryPolicy,
    pub rule: CaRule,
    pub steps: u32,
    pub initial_cells: Vec<CaSeedCell>,
}

impl CaScenario {
    pub fn validate(&self) -> Result<(), CaError> {
        validate_id("scenario", &self.id)?;
        let volume = self.bounds.volume()?;
        if volume > CA_MAX_VOLUME {
            return Err(CaError::VolumeQuotaExceeded {
                volume,
                limit: CA_MAX_VOLUME,
            });
        }
        if self.steps == 0 || self.steps > CA_MAX_STEPS {
            return Err(CaError::StepQuotaExceeded {
                steps: self.steps,
                limit: CA_MAX_STEPS,
            });
        }
        if self.initial_cells.len() > CA_MAX_SEED_CELLS {
            return Err(CaError::SeedQuotaExceeded {
                count: self.initial_cells.len(),
                limit: CA_MAX_SEED_CELLS,
            });
        }
        let mut seen = BTreeSet::new();
        for cell in &self.initial_cells {
            if cell.state == CaCellState::Empty {
                return Err(CaError::EmptySeedCell { coord: cell.coord });
            }
            if !self.bounds.contains(cell.coord) {
                return Err(CaError::SeedOutsideBounds { coord: cell.coord });
            }
            if !seen.insert(cell.coord) {
                return Err(CaError::DuplicateSeedCell { coord: cell.coord });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaScenarioSuite {
    pub kind: String,
    pub schema_version: u32,
    pub scenarios: Vec<CaScenario>,
}

impl CaScenarioSuite {
    pub fn validate(&self) -> Result<(), CaError> {
        if self.kind != CA_SCENARIO_SUITE_KIND || self.schema_version != 1 {
            return Err(CaError::InvalidArtifactIdentity {
                expected: CA_SCENARIO_SUITE_KIND,
            });
        }
        if self.scenarios.is_empty() || self.scenarios.len() > CA_MAX_SCENARIOS {
            return Err(CaError::ScenarioQuotaExceeded {
                count: self.scenarios.len(),
                limit: CA_MAX_SCENARIOS,
            });
        }
        let mut ids = BTreeSet::new();
        for scenario in &self.scenarios {
            scenario.validate()?;
            if !ids.insert(scenario.id.clone()) {
                return Err(CaError::DuplicateScenarioId {
                    id: scenario.id.clone(),
                });
            }
        }
        Ok(())
    }

    pub fn generate_delta_fixtures(&self) -> Result<CaDeltaFixtureSet, CaError> {
        self.validate()?;
        let traces = self
            .scenarios
            .iter()
            .cloned()
            .map(run_ca_scenario)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CaDeltaFixtureSet {
            kind: CA_DELTA_FIXTURE_SET_KIND.to_owned(),
            schema_version: 1,
            suite_kind: self.kind.clone(),
            traces,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaTouchedBounds {
    pub min: CaCoord,
    pub max_inclusive: CaCoord,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaStateCounts {
    pub empty: u64,
    pub source: u64,
    pub frontier: u64,
    pub trail: u64,
}

impl CaStateCounts {
    pub const fn active(self) -> u64 {
        self.source + self.frontier + self.trail
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaCellDelta {
    pub coord: CaCoord,
    pub previous: CaCellState,
    pub current: CaCellState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaStepEvidence {
    pub step: u32,
    pub active_cell_count: u64,
    pub changed_cell_count: u64,
    pub evaluated_cell_count: u64,
    pub touched_bounds: Option<CaTouchedBounds>,
    pub state_counts: CaStateCounts,
    pub deltas: Vec<CaCellDelta>,
    pub delta_hash: String,
    pub state_hash: String,
    pub cumulative_scenario_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaScenarioTrace {
    pub scenario_id: String,
    pub workload: CaWorkloadClass,
    pub rule_id: String,
    pub seed: u64,
    pub bounds: CaBounds,
    pub neighborhood: CaNeighborhood,
    pub boundary: CaBoundaryPolicy,
    pub initial_state_hash: String,
    pub steps: Vec<CaStepEvidence>,
    pub final_state_hash: String,
    pub final_scenario_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaDeltaFixtureSet {
    pub kind: String,
    pub schema_version: u32,
    pub suite_kind: String,
    pub traces: Vec<CaScenarioTrace>,
}

#[derive(Clone, Debug)]
pub struct CaAutomaton {
    scenario: CaScenario,
    cells: BTreeMap<CaCoord, CaCellState>,
    candidates: BTreeSet<CaCoord>,
    step: u32,
    initial_state_hash: String,
    cumulative_scenario_hash: String,
}

impl CaAutomaton {
    pub fn new(scenario: CaScenario) -> Result<Self, CaError> {
        scenario.validate()?;
        let cells = scenario
            .initial_cells
            .iter()
            .map(|cell| (cell.coord, cell.state))
            .collect::<BTreeMap<_, _>>();
        let initial_state_hash = hash_state(&cells)?;
        let cumulative_scenario_hash = canonical_hash(&InitialHashInput {
            scenario: &scenario,
            initial_state_hash: &initial_state_hash,
        })?;
        let candidates = initial_candidates(&scenario, &cells);
        Ok(Self {
            scenario,
            cells,
            candidates,
            step: 0,
            initial_state_hash,
            cumulative_scenario_hash,
        })
    }

    pub fn scenario(&self) -> &CaScenario {
        &self.scenario
    }

    pub const fn completed_steps(&self) -> u32 {
        self.step
    }

    pub fn initial_state_hash(&self) -> &str {
        &self.initial_state_hash
    }

    pub fn current_state_hash(&self) -> Result<String, CaError> {
        hash_state(&self.cells)
    }

    pub fn state_at(&self, coord: CaCoord) -> Result<CaCellState, CaError> {
        if !self.scenario.bounds.contains(coord) {
            return Err(CaError::CoordinateOutsideBounds { coord });
        }
        Ok(self.cell_state(coord))
    }

    pub fn active_cells(&self) -> Vec<CaSeedCell> {
        self.cells
            .iter()
            .map(|(coord, state)| CaSeedCell {
                coord: *coord,
                state: *state,
            })
            .collect()
    }

    pub fn step(&mut self) -> Result<CaStepEvidence, CaError> {
        self.step_with_candidates(self.candidates.clone())
    }

    fn step_with_candidates(
        &mut self,
        candidates: BTreeSet<CaCoord>,
    ) -> Result<CaStepEvidence, CaError> {
        if self.step >= self.scenario.steps {
            return Err(CaError::DeclaredStepLimitReached {
                completed: self.step,
                declared: self.scenario.steps,
            });
        }
        let next_step = self.step + 1;
        let mut deltas = Vec::new();
        for coord in &candidates {
            let previous = self.cell_state(*coord);
            let current = self.evaluate(*coord, previous, next_step);
            if current != previous {
                deltas.push(CaCellDelta {
                    coord: *coord,
                    previous,
                    current,
                });
            }
        }
        if u64::try_from(deltas.len()).unwrap_or(u64::MAX) > CA_MAX_VOLUME {
            return Err(CaError::DeltaQuotaExceeded {
                count: deltas.len(),
                limit: CA_MAX_VOLUME,
            });
        }

        let mut proposed_cells = self.cells.clone();
        for delta in &deltas {
            if delta.current == CaCellState::Empty {
                proposed_cells.remove(&delta.coord);
            } else {
                proposed_cells.insert(delta.coord, delta.current);
            }
        }
        let state_counts = state_counts(self.scenario.bounds, &proposed_cells)?;
        let touched_bounds = touched_bounds(&deltas);
        let delta_hash = canonical_hash(&DeltaHashInput {
            scenario_id: &self.scenario.id,
            rule_id: self.scenario.rule.id(),
            step: next_step,
            deltas: &deltas,
        })?;
        let state_hash = hash_state(&proposed_cells)?;
        let cumulative_scenario_hash = canonical_hash(&CumulativeHashInput {
            previous_hash: &self.cumulative_scenario_hash,
            delta_hash: &delta_hash,
            state_hash: &state_hash,
            step: next_step,
            active_cell_count: state_counts.active(),
            touched_bounds,
        })?;
        let evaluated_cell_count =
            u64::try_from(candidates.len()).map_err(|_| CaError::VolumeOverflow)?;
        let changed_cell_count =
            u64::try_from(deltas.len()).map_err(|_| CaError::VolumeOverflow)?;

        self.cells = proposed_cells;
        self.candidates = next_candidates(&self.scenario, &deltas);
        self.step = next_step;
        self.cumulative_scenario_hash = cumulative_scenario_hash.clone();

        Ok(CaStepEvidence {
            step: next_step,
            active_cell_count: state_counts.active(),
            changed_cell_count,
            evaluated_cell_count,
            touched_bounds,
            state_counts,
            deltas,
            delta_hash,
            state_hash,
            cumulative_scenario_hash,
        })
    }

    fn cell_state(&self, coord: CaCoord) -> CaCellState {
        self.cells
            .get(&coord)
            .copied()
            .unwrap_or(CaCellState::Empty)
    }

    fn evaluate(&self, coord: CaCoord, previous: CaCellState, next_step: u32) -> CaCellState {
        match self.scenario.rule {
            CaRule::FrontierTrailV1 => match previous {
                CaCellState::Source => CaCellState::Source,
                CaCellState::Frontier => CaCellState::Trail,
                CaCellState::Trail => CaCellState::Trail,
                CaCellState::Empty => {
                    let is_reached = neighbors(&self.scenario, coord)
                        .into_iter()
                        .any(|neighbor| {
                            matches!(
                                self.cell_state(neighbor),
                                CaCellState::Source | CaCellState::Frontier
                            )
                        });
                    if is_reached {
                        CaCellState::Frontier
                    } else {
                        CaCellState::Empty
                    }
                }
            },
            CaRule::ParityChurnV1 => {
                if previous == CaCellState::Source {
                    return CaCellState::Source;
                }
                let parity = i64::from(coord.x.rem_euclid(2))
                    + i64::from(coord.y.rem_euclid(2))
                    + i64::from(coord.z.rem_euclid(2))
                    + i64::from((self.scenario.seed & 1) as u8)
                    + i64::from(next_step & 1);
                if parity.rem_euclid(2) == 0 {
                    CaCellState::Frontier
                } else {
                    CaCellState::Empty
                }
            }
        }
    }
}

pub fn run_ca_scenario(scenario: CaScenario) -> Result<CaScenarioTrace, CaError> {
    let mut automaton = CaAutomaton::new(scenario)?;
    let initial_state_hash = automaton.initial_state_hash.clone();
    let mut steps = Vec::with_capacity(
        usize::try_from(automaton.scenario.steps).map_err(|_| CaError::VolumeOverflow)?,
    );
    while automaton.completed_steps() < automaton.scenario.steps {
        steps.push(automaton.step()?);
    }
    let final_state_hash = automaton.current_state_hash()?;
    Ok(CaScenarioTrace {
        scenario_id: automaton.scenario.id.clone(),
        workload: automaton.scenario.workload,
        rule_id: automaton.scenario.rule.id().to_owned(),
        seed: automaton.scenario.seed,
        bounds: automaton.scenario.bounds,
        neighborhood: automaton.scenario.neighborhood,
        boundary: automaton.scenario.boundary,
        initial_state_hash,
        final_scenario_hash: automaton.cumulative_scenario_hash,
        final_state_hash,
        steps,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaError {
    InvalidArtifactIdentity {
        expected: &'static str,
    },
    InvalidId {
        field: &'static str,
        value: String,
    },
    InvalidBounds {
        axis: &'static str,
        min: i32,
        max_exclusive: i32,
    },
    VolumeOverflow,
    VolumeQuotaExceeded {
        volume: u64,
        limit: u64,
    },
    StepQuotaExceeded {
        steps: u32,
        limit: u32,
    },
    SeedQuotaExceeded {
        count: usize,
        limit: usize,
    },
    ScenarioQuotaExceeded {
        count: usize,
        limit: usize,
    },
    DuplicateScenarioId {
        id: String,
    },
    DuplicateSeedCell {
        coord: CaCoord,
    },
    EmptySeedCell {
        coord: CaCoord,
    },
    SeedOutsideBounds {
        coord: CaCoord,
    },
    CoordinateOutsideBounds {
        coord: CaCoord,
    },
    DeltaQuotaExceeded {
        count: usize,
        limit: u64,
    },
    DeclaredStepLimitReached {
        completed: u32,
        declared: u32,
    },
    Serialization {
        message: String,
    },
}

impl CaError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidArtifactIdentity { .. } => "invalid_artifact_identity",
            Self::InvalidId { .. } => "invalid_id",
            Self::InvalidBounds { .. } => "invalid_bounds",
            Self::VolumeOverflow => "volume_overflow",
            Self::VolumeQuotaExceeded { .. } => "volume_quota_exceeded",
            Self::StepQuotaExceeded { .. } => "step_quota_exceeded",
            Self::SeedQuotaExceeded { .. } => "seed_quota_exceeded",
            Self::ScenarioQuotaExceeded { .. } => "scenario_quota_exceeded",
            Self::DuplicateScenarioId { .. } => "duplicate_scenario_id",
            Self::DuplicateSeedCell { .. } => "duplicate_seed_cell",
            Self::EmptySeedCell { .. } => "empty_seed_cell",
            Self::SeedOutsideBounds { .. } => "seed_outside_bounds",
            Self::CoordinateOutsideBounds { .. } => "coordinate_outside_bounds",
            Self::DeltaQuotaExceeded { .. } => "delta_quota_exceeded",
            Self::DeclaredStepLimitReached { .. } => "declared_step_limit_reached",
            Self::Serialization { .. } => "serialization_failed",
        }
    }
}

impl Display for CaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: ", self.code())?;
        match self {
            Self::InvalidArtifactIdentity { expected } => {
                write!(formatter, "expected {expected} schema version 1")
            }
            Self::InvalidId { field, value } => {
                write!(formatter, "{field} identity is invalid: {value:?}")
            }
            Self::InvalidBounds {
                axis,
                min,
                max_exclusive,
            } => write!(
                formatter,
                "{axis} bounds require maxExclusive > min, got {min}..{max_exclusive}"
            ),
            Self::VolumeOverflow => write!(formatter, "domain volume does not fit u64"),
            Self::VolumeQuotaExceeded { volume, limit } => {
                write!(formatter, "domain volume {volume} exceeds {limit}")
            }
            Self::StepQuotaExceeded { steps, limit } => {
                write!(formatter, "step count {steps} is outside 1..={limit}")
            }
            Self::SeedQuotaExceeded { count, limit } => {
                write!(formatter, "seed count {count} exceeds {limit}")
            }
            Self::ScenarioQuotaExceeded { count, limit } => {
                write!(formatter, "scenario count {count} is outside 1..={limit}")
            }
            Self::DuplicateScenarioId { id } => write!(formatter, "duplicate scenario id {id}"),
            Self::DuplicateSeedCell { coord } => write!(formatter, "duplicate seed at {coord:?}"),
            Self::EmptySeedCell { coord } => write!(formatter, "empty seed at {coord:?}"),
            Self::SeedOutsideBounds { coord } => {
                write!(formatter, "seed {coord:?} lies outside bounds")
            }
            Self::CoordinateOutsideBounds { coord } => {
                write!(formatter, "coordinate {coord:?} lies outside bounds")
            }
            Self::DeltaQuotaExceeded { count, limit } => {
                write!(formatter, "delta count {count} exceeds {limit}")
            }
            Self::DeclaredStepLimitReached {
                completed,
                declared,
            } => write!(
                formatter,
                "already completed declared {declared} steps (completed {completed})"
            ),
            Self::Serialization { message } => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for CaError {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitialHashInput<'a> {
    scenario: &'a CaScenario,
    initial_state_hash: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeltaHashInput<'a> {
    scenario_id: &'a str,
    rule_id: &'a str,
    step: u32,
    deltas: &'a [CaCellDelta],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CumulativeHashInput<'a> {
    previous_hash: &'a str,
    delta_hash: &'a str,
    state_hash: &'a str,
    step: u32,
    active_cell_count: u64,
    touched_bounds: Option<CaTouchedBounds>,
}

fn axis_span(axis: &'static str, min: i32, max_exclusive: i32) -> Result<u64, CaError> {
    if max_exclusive <= min {
        return Err(CaError::InvalidBounds {
            axis,
            min,
            max_exclusive,
        });
    }
    let span = i64::from(max_exclusive) - i64::from(min);
    u64::try_from(span).map_err(|_| CaError::VolumeOverflow)
}

fn validate_id(field: &'static str, value: &str) -> Result<(), CaError> {
    let valid = !value.is_empty()
        && value.len() <= CA_MAX_ID_LENGTH
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        });
    if valid {
        Ok(())
    } else {
        Err(CaError::InvalidId {
            field,
            value: value.to_owned(),
        })
    }
}

fn initial_candidates(
    scenario: &CaScenario,
    cells: &BTreeMap<CaCoord, CaCellState>,
) -> BTreeSet<CaCoord> {
    if scenario.rule.scans_complete_domain() {
        return scenario.bounds.all_coords();
    }
    let mut candidates = BTreeSet::new();
    for coord in cells.keys() {
        candidates.insert(*coord);
        candidates.extend(neighbors(scenario, *coord));
    }
    candidates
}

fn next_candidates(scenario: &CaScenario, deltas: &[CaCellDelta]) -> BTreeSet<CaCoord> {
    if scenario.rule.scans_complete_domain() {
        return scenario.bounds.all_coords();
    }
    let mut candidates = BTreeSet::new();
    for delta in deltas {
        candidates.insert(delta.coord);
        candidates.extend(neighbors(scenario, delta.coord));
    }
    candidates
}

fn neighbors(scenario: &CaScenario, coord: CaCoord) -> BTreeSet<CaCoord> {
    let mut result = BTreeSet::new();
    for dx in -1_i32..=1 {
        for dy in -1_i32..=1 {
            for dz in -1_i32..=1 {
                if dx == 0 && dy == 0 && dz == 0 {
                    continue;
                }
                if scenario.neighborhood == CaNeighborhood::VonNeumann6
                    && dx.abs() + dy.abs() + dz.abs() != 1
                {
                    continue;
                }
                let proposed = [
                    i64::from(coord.x) + i64::from(dx),
                    i64::from(coord.y) + i64::from(dy),
                    i64::from(coord.z) + i64::from(dz),
                ];
                let mapped = match scenario.boundary {
                    CaBoundaryPolicy::FixedEmpty => {
                        let Ok(x) = i32::try_from(proposed[0]) else {
                            continue;
                        };
                        let Ok(y) = i32::try_from(proposed[1]) else {
                            continue;
                        };
                        let Ok(z) = i32::try_from(proposed[2]) else {
                            continue;
                        };
                        let candidate = CaCoord { x, y, z };
                        scenario.bounds.contains(candidate).then_some(candidate)
                    }
                    CaBoundaryPolicy::Wrap => Some(CaCoord {
                        x: wrap_axis(
                            proposed[0],
                            scenario.bounds.min.x,
                            scenario.bounds.max_exclusive.x,
                        ),
                        y: wrap_axis(
                            proposed[1],
                            scenario.bounds.min.y,
                            scenario.bounds.max_exclusive.y,
                        ),
                        z: wrap_axis(
                            proposed[2],
                            scenario.bounds.min.z,
                            scenario.bounds.max_exclusive.z,
                        ),
                    }),
                };
                if let Some(mapped) = mapped {
                    result.insert(mapped);
                }
            }
        }
    }
    result
}

fn wrap_axis(value: i64, min: i32, max_exclusive: i32) -> i32 {
    let min = i64::from(min);
    let span = i64::from(max_exclusive) - min;
    i32::try_from(min + (value - min).rem_euclid(span))
        .expect("validated i32 bounds produce an i32 wrapped coordinate")
}

fn state_counts(
    bounds: CaBounds,
    cells: &BTreeMap<CaCoord, CaCellState>,
) -> Result<CaStateCounts, CaError> {
    let mut counts = CaStateCounts::default();
    for state in cells.values() {
        match state {
            CaCellState::Empty => {}
            CaCellState::Source => counts.source += 1,
            CaCellState::Frontier => counts.frontier += 1,
            CaCellState::Trail => counts.trail += 1,
        }
    }
    counts.empty = bounds
        .volume()?
        .checked_sub(counts.active())
        .ok_or(CaError::VolumeOverflow)?;
    Ok(counts)
}

fn touched_bounds(deltas: &[CaCellDelta]) -> Option<CaTouchedBounds> {
    let first = deltas.first()?.coord;
    let mut min = first;
    let mut max_inclusive = first;
    for delta in &deltas[1..] {
        min.x = min.x.min(delta.coord.x);
        min.y = min.y.min(delta.coord.y);
        min.z = min.z.min(delta.coord.z);
        max_inclusive.x = max_inclusive.x.max(delta.coord.x);
        max_inclusive.y = max_inclusive.y.max(delta.coord.y);
        max_inclusive.z = max_inclusive.z.max(delta.coord.z);
    }
    Some(CaTouchedBounds { min, max_inclusive })
}

fn hash_state(cells: &BTreeMap<CaCoord, CaCellState>) -> Result<String, CaError> {
    let ordered = cells
        .iter()
        .map(|(coord, state)| CaSeedCell {
            coord: *coord,
            state: *state,
        })
        .collect::<Vec<_>>();
    canonical_hash(&ordered)
}

fn canonical_hash<T: Serialize>(value: &T) -> Result<String, CaError> {
    let bytes = serde_json::to_vec(value).map_err(|error| CaError::Serialization {
        message: error.to_string(),
    })?;
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Ok(format!("fnv1a64:{hash:016x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sparse_scenario(boundary: CaBoundaryPolicy) -> CaScenario {
        CaScenario {
            id: "oracle.sparse".to_owned(),
            workload: CaWorkloadClass::SparsePropagation,
            seed: 7,
            bounds: CaBounds {
                min: CaCoord { x: -2, y: 0, z: -2 },
                max_exclusive: CaCoord { x: 3, y: 2, z: 3 },
            },
            neighborhood: CaNeighborhood::VonNeumann6,
            boundary,
            rule: CaRule::FrontierTrailV1,
            steps: 5,
            initial_cells: vec![CaSeedCell {
                coord: CaCoord { x: 2, y: 0, z: 0 },
                state: CaCellState::Source,
            }],
        }
    }

    fn assert_frontier_matches_full_scan(scenario: CaScenario) {
        let mut frontier = CaAutomaton::new(scenario.clone()).expect("frontier automaton");
        let mut oracle = CaAutomaton::new(scenario).expect("oracle automaton");
        while frontier.completed_steps() < frontier.scenario.steps {
            let frontier_step = frontier.step().expect("frontier step");
            let oracle_step = oracle
                .step_with_candidates(oracle.scenario.bounds.all_coords())
                .expect("full scan step");
            assert_eq!(frontier_step.deltas, oracle_step.deltas);
            assert_eq!(frontier_step.state_hash, oracle_step.state_hash);
            assert_eq!(
                frontier_step.cumulative_scenario_hash,
                oracle_step.cumulative_scenario_hash
            );
        }
    }

    #[test]
    fn sparse_frontier_matches_full_scan_for_fixed_and_wrapped_boundaries() {
        assert_frontier_matches_full_scan(sparse_scenario(CaBoundaryPolicy::FixedEmpty));
        assert_frontier_matches_full_scan(sparse_scenario(CaBoundaryPolicy::Wrap));
    }

    #[test]
    fn declared_step_limit_rejects_without_mutation() {
        let mut scenario = sparse_scenario(CaBoundaryPolicy::FixedEmpty);
        scenario.steps = 1;
        let mut automaton = CaAutomaton::new(scenario).expect("automaton");
        automaton.step().expect("declared step");
        let before = automaton.current_state_hash().expect("before hash");
        let error = automaton.step().expect_err("one-over step");
        assert_eq!(error.code(), "declared_step_limit_reached");
        assert_eq!(automaton.current_state_hash().expect("after hash"), before);
        assert_eq!(automaton.completed_steps(), 1);
    }
}
