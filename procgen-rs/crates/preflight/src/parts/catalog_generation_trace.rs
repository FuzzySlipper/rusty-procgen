use std::fmt;

use crate::*;

pub const DEFAULT_CATALOG_TRACE_MAX_EVENTS: u32 = 1_024;
pub const DEFAULT_CATALOG_TRACE_MAX_EVENT_BODY_BYTES: u64 = 1_048_576;
pub const DEFAULT_CATALOG_TRACE_MAX_VISUAL_CELLS: u64 = 131_072;

const HARD_CATALOG_TRACE_MAX_EVENTS: u32 = 4_096;
const HARD_CATALOG_TRACE_MAX_EVENT_BODY_BYTES: u64 = 4_194_304;
const HARD_CATALOG_TRACE_MAX_VISUAL_CELLS: u64 = 1_048_576;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGenerationTraceLimits {
    pub max_events: u32,
    pub max_event_body_bytes: u64,
    pub max_visual_cells: u64,
}

impl Default for CatalogGenerationTraceLimits {
    fn default() -> Self {
        Self {
            max_events: DEFAULT_CATALOG_TRACE_MAX_EVENTS,
            max_event_body_bytes: DEFAULT_CATALOG_TRACE_MAX_EVENT_BODY_BYTES,
            max_visual_cells: DEFAULT_CATALOG_TRACE_MAX_VISUAL_CELLS,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CatalogGenerationTraceRequest<'a> {
    pub candidate: &'a Candidate,
    pub source_geometry: &'a Geometry2dArtifact,
    pub source_plan: &'a PieceBuildPlan,
    pub catalog: &'a ShapeCatalog,
    pub generation_policy: &'a CatalogAwareGenerationPolicy,
    pub provenance: &'a CatalogAwareGenerationProvenance,
    pub seed: u64,
    pub trace_limits: CatalogGenerationTraceLimits,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGenerationTraceInputHashes {
    pub candidate_hash: String,
    pub source_geometry_hash: String,
    pub source_plan_hash: String,
    pub catalog_hash: String,
    pub generation_policy_hash: String,
    pub provenance_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGenerationTraceRoomCandidate {
    pub shape_id: String,
    pub transform: String,
    pub score: i32,
    pub rank: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGenerationTraceRoomPlacement {
    pub piece_id: String,
    pub requirement_kind: String,
    pub shape_id: String,
    pub transform: String,
    pub origin: GridCell,
    pub occupied_cells: Vec<GridCell>,
    pub reserved_cells: Vec<GridCell>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGenerationTraceRoute {
    pub section_id: String,
    pub cells: Vec<GridCell>,
    pub states_visited: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum CatalogGenerationTraceEventBody {
    InputBound {
        input_hashes: CatalogGenerationTraceInputHashes,
    },
    AttemptStarted {
        room_slack_cells: i32,
    },
    RoomDomainEvaluated {
        piece_id: String,
        requirement_kind: String,
        candidates: Vec<CatalogGenerationTraceRoomCandidate>,
    },
    RoomPlaced {
        placement: CatalogGenerationTraceRoomPlacement,
    },
    RoomConflict {
        piece_id: String,
        conflicting_cells: Vec<GridCell>,
    },
    SectionRoutingStarted {
        section_id: String,
        start: GridCell,
        goal: GridCell,
        guide: Vec<GridCell>,
        bounds: CatalogGridBounds,
    },
    SectionRoutingFinished {
        section_id: String,
        status: String,
        cells: Vec<GridCell>,
        states_visited: u32,
    },
    ValidationCompleted {
        stage: String,
        ok: bool,
        subject_hash: String,
        diagnostic_codes: Vec<String>,
    },
    AttemptFinished {
        classification: String,
        stage: String,
        detail: String,
        rooms_placed: usize,
        sections_routed: usize,
        routing_states: u32,
    },
    RunFinished {
        selected_attempt: Option<u32>,
        classification: String,
        reason: String,
        output_hash: String,
    },
}

impl CatalogGenerationTraceEventBody {
    fn visual_cell_count(&self) -> Result<u64, CatalogGenerationTraceError> {
        let count = match self {
            Self::RoomPlaced { placement } => placement
                .occupied_cells
                .len()
                .checked_add(placement.reserved_cells.len()),
            Self::RoomConflict {
                conflicting_cells, ..
            } => Some(conflicting_cells.len()),
            Self::SectionRoutingStarted { guide, .. } => Some(guide.len()),
            Self::SectionRoutingFinished { cells, .. } => Some(cells.len()),
            _ => Some(0),
        }
        .ok_or_else(|| {
            CatalogGenerationTraceError::new(
                "trace_visual_cell_overflow",
                "trace visual-cell accounting overflowed",
                None,
                None,
            )
        })?;
        u64::try_from(count).map_err(|_| {
            CatalogGenerationTraceError::new(
                "trace_visual_cell_overflow",
                "trace visual-cell accounting exceeds u64",
                None,
                None,
            )
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGenerationTraceEvent {
    pub index: u32,
    pub attempt: Option<u32>,
    pub previous_hash: String,
    pub event_hash: String,
    pub body: CatalogGenerationTraceEventBody,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGenerationTraceSelection {
    pub selected_attempt: Option<u32>,
    pub classification: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGenerationTrace {
    pub kind: String,
    pub schema_version: u32,
    pub seed: u64,
    pub input_hashes: CatalogGenerationTraceInputHashes,
    pub generation_policy: CatalogAwareGenerationPolicy,
    pub limits: CatalogGenerationTraceLimits,
    pub root_hash: String,
    pub events: Vec<CatalogGenerationTraceEvent>,
    pub event_body_bytes: u64,
    pub visual_cell_count: u64,
    pub final_event_hash: String,
    pub final_output_hash: String,
    pub selection: CatalogGenerationTraceSelection,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAwareGenerationRun {
    pub result: CatalogAwareGenerationResult,
    pub trace: CatalogGenerationTrace,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGenerationReplayFrame {
    pub event_index: u32,
    pub attempt: Option<u32>,
    pub room_count: usize,
    pub route_count: usize,
    pub occupied_cell_count: usize,
    pub routed_cell_count: usize,
    pub state_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGenerationReplayAttempt {
    pub attempt: u32,
    pub rooms: Vec<CatalogGenerationTraceRoomPlacement>,
    pub routes: Vec<CatalogGenerationTraceRoute>,
    pub classification: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGenerationReplay {
    pub frames: Vec<CatalogGenerationReplayFrame>,
    pub attempts: Vec<CatalogGenerationReplayAttempt>,
    pub final_output_hash: String,
    pub final_event_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGenerationTraceError {
    pub code: String,
    pub detail: String,
    pub observed: Option<u64>,
    pub limit: Option<u64>,
}

impl CatalogGenerationTraceError {
    pub(crate) fn new(
        code: &str,
        detail: impl Into<String>,
        observed: Option<u64>,
        limit: Option<u64>,
    ) -> Self {
        Self {
            code: code.to_owned(),
            detail: detail.into(),
            observed,
            limit,
        }
    }
}

impl fmt::Display for CatalogGenerationTraceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for CatalogGenerationTraceError {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogGenerationTraceRootInput<'a> {
    kind: &'static str,
    schema_version: u32,
    seed: u64,
    input_hashes: &'a CatalogGenerationTraceInputHashes,
    generation_policy: &'a CatalogAwareGenerationPolicy,
    limits: &'a CatalogGenerationTraceLimits,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogGenerationTraceEventHashInput<'a> {
    index: u32,
    attempt: Option<u32>,
    previous_hash: &'a str,
    body: &'a CatalogGenerationTraceEventBody,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogGenerationReplayState {
    attempt: Option<u32>,
    domains: BTreeMap<String, BTreeSet<(String, String)>>,
    rooms: BTreeMap<String, CatalogGenerationTraceRoomPlacement>,
    routes: BTreeMap<String, CatalogGenerationTraceRoute>,
    routing_states: u32,
}

pub(crate) struct CatalogGenerationTraceRecorder {
    active: bool,
    seed: u64,
    input_hashes: CatalogGenerationTraceInputHashes,
    generation_policy: CatalogAwareGenerationPolicy,
    limits: CatalogGenerationTraceLimits,
    root_hash: String,
    events: Vec<CatalogGenerationTraceEvent>,
    event_body_bytes: u64,
    visual_cell_count: u64,
    error: Option<CatalogGenerationTraceError>,
}

impl CatalogGenerationTraceRecorder {
    pub(crate) fn disabled() -> Self {
        Self {
            active: false,
            seed: 0,
            input_hashes: CatalogGenerationTraceInputHashes {
                candidate_hash: String::new(),
                source_geometry_hash: String::new(),
                source_plan_hash: String::new(),
                catalog_hash: String::new(),
                generation_policy_hash: String::new(),
                provenance_hash: String::new(),
            },
            generation_policy: CatalogAwareGenerationPolicy {
                kind: String::new(),
                schema_version: 0,
                max_generation_attempts: 0,
                initial_room_slack_cells: 0,
                room_slack_growth_cells: 0,
                max_room_candidates: 0,
                max_routing_states_per_section: 0,
                route_margin_cells: 0,
                guide_distance_weight: 0,
                turn_penalty: 0,
            },
            limits: CatalogGenerationTraceLimits::default(),
            root_hash: String::new(),
            events: Vec::new(),
            event_body_bytes: 0,
            visual_cell_count: 0,
            error: None,
        }
    }

    pub(crate) fn new(
        input: CatalogAwareGenerationInput<'_>,
        limits: CatalogGenerationTraceLimits,
    ) -> Result<Self, CatalogGenerationTraceError> {
        validate_catalog_generation_trace_limits(&limits)?;
        let input_hashes = catalog_generation_trace_input_hashes(input)?;
        let root_hash =
            catalog_generation_trace_root_hash(input.seed, &input_hashes, input.policy, &limits)?;
        let mut recorder = Self {
            active: true,
            seed: input.seed,
            input_hashes: input_hashes.clone(),
            generation_policy: input.policy.clone(),
            limits,
            root_hash,
            events: Vec::new(),
            event_body_bytes: 0,
            visual_cell_count: 0,
            error: None,
        };
        recorder.record(
            None,
            CatalogGenerationTraceEventBody::InputBound { input_hashes },
        );
        if let Some(error) = recorder.error.clone() {
            return Err(error);
        }
        Ok(recorder)
    }

    pub(crate) fn record(
        &mut self,
        attempt: Option<u32>,
        body: CatalogGenerationTraceEventBody,
    ) -> bool {
        if !self.active {
            return true;
        }
        if self.error.is_some() {
            return false;
        }
        let Some(next_count) = u32::try_from(self.events.len())
            .ok()
            .and_then(|count| count.checked_add(1))
        else {
            self.error = Some(CatalogGenerationTraceError::new(
                "trace_event_count_overflow",
                "trace event count overflowed",
                None,
                Some(u64::from(self.limits.max_events)),
            ));
            return false;
        };
        if next_count > self.limits.max_events {
            self.error = Some(CatalogGenerationTraceError::new(
                "trace_event_quota_exceeded",
                format!(
                    "catalog generation trace would contain {next_count} events, limit {}",
                    self.limits.max_events
                ),
                Some(u64::from(next_count)),
                Some(u64::from(self.limits.max_events)),
            ));
            return false;
        }
        let visual_cells = match body.visual_cell_count() {
            Ok(count) => count,
            Err(error) => {
                self.error = Some(error);
                return false;
            }
        };
        let Some(next_visual_cells) = self.visual_cell_count.checked_add(visual_cells) else {
            self.error = Some(CatalogGenerationTraceError::new(
                "trace_visual_cell_overflow",
                "trace visual-cell accounting overflowed",
                None,
                Some(self.limits.max_visual_cells),
            ));
            return false;
        };
        if next_visual_cells > self.limits.max_visual_cells {
            self.error = Some(CatalogGenerationTraceError::new(
                "trace_visual_cell_quota_exceeded",
                format!(
                    "catalog generation trace would retain {next_visual_cells} visual cells, limit {}",
                    self.limits.max_visual_cells
                ),
                Some(next_visual_cells),
                Some(self.limits.max_visual_cells),
            ));
            return false;
        }
        let encoded_body = match serde_json::to_vec(&body) {
            Ok(encoded) => encoded,
            Err(error) => {
                self.error = Some(CatalogGenerationTraceError::new(
                    "trace_encoding_failed",
                    format!("failed to encode trace event: {error}"),
                    None,
                    None,
                ));
                return false;
            }
        };
        let encoded_body_len = match u64::try_from(encoded_body.len()) {
            Ok(length) => length,
            Err(_) => {
                self.error = Some(CatalogGenerationTraceError::new(
                    "trace_event_body_byte_overflow",
                    "trace event body byte count exceeds u64",
                    None,
                    Some(self.limits.max_event_body_bytes),
                ));
                return false;
            }
        };
        let Some(next_event_body_bytes) = self.event_body_bytes.checked_add(encoded_body_len)
        else {
            self.error = Some(CatalogGenerationTraceError::new(
                "trace_event_body_byte_overflow",
                "trace event body byte accounting overflowed",
                None,
                Some(self.limits.max_event_body_bytes),
            ));
            return false;
        };
        if next_event_body_bytes > self.limits.max_event_body_bytes {
            self.error = Some(CatalogGenerationTraceError::new(
                "trace_event_body_byte_quota_exceeded",
                format!(
                    "catalog generation trace would encode {next_event_body_bytes} event-body bytes, limit {}",
                    self.limits.max_event_body_bytes
                ),
                Some(next_event_body_bytes),
                Some(self.limits.max_event_body_bytes),
            ));
            return false;
        }
        let index = next_count - 1;
        let previous_hash = self
            .events
            .last()
            .map(|event| event.event_hash.clone())
            .unwrap_or_else(|| self.root_hash.clone());
        let event_hash = match catalog_generation_trace_event_hash(
            index,
            attempt,
            previous_hash.as_str(),
            &body,
        ) {
            Ok(hash) => hash,
            Err(error) => {
                self.error = Some(error);
                return false;
            }
        };
        self.events.push(CatalogGenerationTraceEvent {
            index,
            attempt,
            previous_hash,
            event_hash,
            body,
        });
        self.event_body_bytes = next_event_body_bytes;
        self.visual_cell_count = next_visual_cells;
        true
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    pub(crate) fn error(&self) -> Option<CatalogGenerationTraceError> {
        self.error.clone()
    }

    pub(crate) fn finish(
        mut self,
        result: &CatalogAwareGenerationResult,
    ) -> Result<CatalogGenerationTrace, CatalogGenerationTraceError> {
        let output_hash = hash_json(result).map_err(|detail| {
            CatalogGenerationTraceError::new("trace_output_hash_failed", detail, None, None)
        })?;
        let selection = catalog_generation_trace_selection(result)?;
        self.record(
            None,
            CatalogGenerationTraceEventBody::RunFinished {
                selected_attempt: selection.selected_attempt,
                classification: selection.classification.clone(),
                reason: selection.reason.clone(),
                output_hash: output_hash.clone(),
            },
        );
        if let Some(error) = self.error {
            return Err(error);
        }
        let final_event_hash = self
            .events
            .last()
            .map(|event| event.event_hash.clone())
            .unwrap_or_else(|| self.root_hash.clone());
        Ok(CatalogGenerationTrace {
            kind: "rusty_procgen.catalog_generation_trace.v1".to_owned(),
            schema_version: 1,
            seed: self.seed,
            input_hashes: self.input_hashes,
            generation_policy: self.generation_policy,
            limits: self.limits,
            root_hash: self.root_hash,
            events: self.events,
            event_body_bytes: self.event_body_bytes,
            visual_cell_count: self.visual_cell_count,
            final_event_hash,
            final_output_hash: output_hash,
            selection,
        })
    }
}

pub(crate) fn trace_record_or_error(
    recorder: &mut CatalogGenerationTraceRecorder,
    attempt: Option<u32>,
    body: CatalogGenerationTraceEventBody,
) -> Result<(), CatalogGenerationTraceError> {
    if recorder.record(attempt, body) {
        Ok(())
    } else {
        Err(recorder.error().unwrap_or_else(|| {
            CatalogGenerationTraceError::new(
                "trace_recording_failed",
                "trace recorder rejected an event without an error",
                None,
                None,
            )
        }))
    }
}

pub fn validate_catalog_generation_trace_limits(
    limits: &CatalogGenerationTraceLimits,
) -> Result<(), CatalogGenerationTraceError> {
    if limits.max_events == 0 || limits.max_events > HARD_CATALOG_TRACE_MAX_EVENTS {
        return Err(CatalogGenerationTraceError::new(
            "trace_event_limit_invalid",
            format!("maxEvents must be from 1 through {HARD_CATALOG_TRACE_MAX_EVENTS}"),
            Some(u64::from(limits.max_events)),
            Some(u64::from(HARD_CATALOG_TRACE_MAX_EVENTS)),
        ));
    }
    if limits.max_event_body_bytes == 0
        || limits.max_event_body_bytes > HARD_CATALOG_TRACE_MAX_EVENT_BODY_BYTES
    {
        return Err(CatalogGenerationTraceError::new(
            "trace_event_body_byte_limit_invalid",
            format!(
                "maxEventBodyBytes must be from 1 through {HARD_CATALOG_TRACE_MAX_EVENT_BODY_BYTES}"
            ),
            Some(limits.max_event_body_bytes),
            Some(HARD_CATALOG_TRACE_MAX_EVENT_BODY_BYTES),
        ));
    }
    if limits.max_visual_cells == 0 || limits.max_visual_cells > HARD_CATALOG_TRACE_MAX_VISUAL_CELLS
    {
        return Err(CatalogGenerationTraceError::new(
            "trace_visual_cell_limit_invalid",
            format!("maxVisualCells must be from 1 through {HARD_CATALOG_TRACE_MAX_VISUAL_CELLS}"),
            Some(limits.max_visual_cells),
            Some(HARD_CATALOG_TRACE_MAX_VISUAL_CELLS),
        ));
    }
    Ok(())
}

pub fn replay_catalog_generation_trace(
    trace: &CatalogGenerationTrace,
    result: &CatalogAwareGenerationResult,
    request: CatalogGenerationTraceRequest<'_>,
) -> Result<CatalogGenerationReplay, CatalogGenerationTraceError> {
    if trace.kind != "rusty_procgen.catalog_generation_trace.v1" || trace.schema_version != 1 {
        return Err(trace_validation_error(
            "trace_schema_unsupported",
            "unsupported catalog generation trace schema",
        ));
    }
    validate_catalog_generation_trace_limits(&trace.limits)?;
    if request.trace_limits != trace.limits || request.seed != trace.seed {
        return Err(trace_validation_error(
            "trace_request_mismatch",
            "trace request seed or limits do not match the trace",
        ));
    }
    let input = CatalogAwareGenerationInput {
        candidate: request.candidate,
        source_geometry: request.source_geometry,
        source_plan: request.source_plan,
        catalog: request.catalog,
        policy: request.generation_policy,
        provenance: request.provenance,
        seed: request.seed,
    };
    let expected_inputs = catalog_generation_trace_input_hashes(input)?;
    if trace.input_hashes != expected_inputs {
        return Err(trace_validation_error(
            "trace_input_hash_mismatch",
            "trace input hashes do not match the supplied generation inputs",
        ));
    }
    if &trace.generation_policy != request.generation_policy {
        return Err(trace_validation_error(
            "trace_policy_mismatch",
            "trace effective generation policy does not match the supplied policy",
        ));
    }
    let expected_root = catalog_generation_trace_root_hash(
        trace.seed,
        &expected_inputs,
        request.generation_policy,
        &trace.limits,
    )?;
    if trace.root_hash != expected_root {
        return Err(trace_validation_error(
            "trace_root_hash_mismatch",
            "trace root hash does not match its inputs and limits",
        ));
    }
    let event_count = u32::try_from(trace.events.len()).map_err(|_| {
        trace_validation_error(
            "trace_event_count_overflow",
            "trace event count exceeds u32",
        )
    })?;
    if event_count > trace.limits.max_events {
        return Err(CatalogGenerationTraceError::new(
            "trace_event_quota_exceeded",
            "trace event count exceeds its declared limit",
            Some(u64::from(event_count)),
            Some(u64::from(trace.limits.max_events)),
        ));
    }

    let mut previous_hash = trace.root_hash.clone();
    let mut body_bytes = 0_u64;
    let mut visual_cells = 0_u64;
    let mut machine = CatalogGenerationReplayMachine::default();
    let mut frames = Vec::with_capacity(trace.events.len());
    let mut attempts = Vec::new();
    for (position, event) in trace.events.iter().enumerate() {
        let index = u32::try_from(position).map_err(|_| {
            trace_validation_error(
                "trace_event_index_overflow",
                "trace event position exceeds u32",
            )
        })?;
        if event.index != index {
            return Err(trace_validation_error(
                "trace_event_order_invalid",
                format!(
                    "trace event at position {position} declares index {}",
                    event.index
                ),
            ));
        }
        if event.previous_hash != previous_hash {
            return Err(trace_validation_error(
                "trace_previous_hash_mismatch",
                format!("trace event {index} does not link to the previous hash"),
            ));
        }
        let expected_event_hash = catalog_generation_trace_event_hash(
            event.index,
            event.attempt,
            event.previous_hash.as_str(),
            &event.body,
        )?;
        if event.event_hash != expected_event_hash {
            return Err(trace_validation_error(
                "trace_event_hash_mismatch",
                format!("trace event {index} body does not match its hash"),
            ));
        }
        let encoded_body = serde_json::to_vec(&event.body).map_err(|error| {
            trace_validation_error(
                "trace_encoding_failed",
                format!("failed to encode trace event {index}: {error}"),
            )
        })?;
        body_bytes = body_bytes
            .checked_add(u64::try_from(encoded_body.len()).map_err(|_| {
                trace_validation_error(
                    "trace_event_body_byte_overflow",
                    "trace event body length exceeds u64",
                )
            })?)
            .ok_or_else(|| {
                trace_validation_error(
                    "trace_event_body_byte_overflow",
                    "trace event body byte accounting overflowed",
                )
            })?;
        visual_cells = visual_cells
            .checked_add(event.body.visual_cell_count()?)
            .ok_or_else(|| {
                trace_validation_error(
                    "trace_visual_cell_overflow",
                    "trace visual-cell accounting overflowed",
                )
            })?;
        machine.apply(event, result, &expected_inputs)?;
        if let Some(completed) = machine.take_completed_attempt() {
            attempts.push(completed);
        }
        frames.push(machine.frame(event.index)?);
        previous_hash = event.event_hash.clone();
    }
    if body_bytes != trace.event_body_bytes || body_bytes > trace.limits.max_event_body_bytes {
        return Err(CatalogGenerationTraceError::new(
            "trace_event_body_byte_mismatch",
            "trace event-body byte evidence is inconsistent or over limit",
            Some(body_bytes),
            Some(trace.limits.max_event_body_bytes),
        ));
    }
    if visual_cells != trace.visual_cell_count || visual_cells > trace.limits.max_visual_cells {
        return Err(CatalogGenerationTraceError::new(
            "trace_visual_cell_mismatch",
            "trace visual-cell evidence is inconsistent or over limit",
            Some(visual_cells),
            Some(trace.limits.max_visual_cells),
        ));
    }
    if previous_hash != trace.final_event_hash {
        return Err(trace_validation_error(
            "trace_final_event_hash_mismatch",
            "trace final event hash does not close the event chain",
        ));
    }
    let output_hash = hash_json(result).map_err(|detail| {
        CatalogGenerationTraceError::new("trace_output_hash_failed", detail, None, None)
    })?;
    if trace.final_output_hash != output_hash {
        return Err(trace_validation_error(
            "trace_final_output_hash_mismatch",
            "trace final output hash does not match the supplied result",
        ));
    }
    let expected_selection = catalog_generation_trace_selection(result)?;
    if trace.selection != expected_selection {
        return Err(trace_validation_error(
            "trace_selection_mismatch",
            "trace selection evidence does not match the generation result",
        ));
    }
    machine.finish(trace, &expected_selection)?;
    let authoritative = record_catalog_aware_generation_trace(input, trace.limits.clone())?;
    let authoritative_output_hash = hash_json(&authoritative.result).map_err(|detail| {
        CatalogGenerationTraceError::new("trace_output_hash_failed", detail, None, None)
    })?;
    if authoritative_output_hash != output_hash {
        return Err(trace_validation_error(
            "trace_authoritative_result_mismatch",
            "supplied result does not match a deterministic rerun of the generation inputs",
        ));
    }
    if authoritative.trace != *trace {
        let mismatch = authoritative
            .trace
            .events
            .iter()
            .zip(trace.events.iter())
            .position(|(expected, observed)| expected != observed)
            .map_or_else(
                || "trace envelope differs from the deterministic rerun".to_owned(),
                |index| {
                    format!(
                        "trace event {index} differs from the deterministic authoritative rerun"
                    )
                },
            );
        return Err(trace_validation_error(
            "trace_authoritative_event_mismatch",
            mismatch,
        ));
    }
    Ok(CatalogGenerationReplay {
        frames,
        attempts,
        final_output_hash: output_hash,
        final_event_hash: previous_hash,
    })
}

#[derive(Default)]
struct CatalogGenerationReplayMachine {
    state: CatalogGenerationReplayState,
    next_attempt: u32,
    pending_sections: BTreeSet<String>,
    completed_attempt: Option<CatalogGenerationReplayAttempt>,
    input_bound: bool,
    run_finished: bool,
}

impl CatalogGenerationReplayMachine {
    fn apply(
        &mut self,
        event: &CatalogGenerationTraceEvent,
        result: &CatalogAwareGenerationResult,
        expected_inputs: &CatalogGenerationTraceInputHashes,
    ) -> Result<(), CatalogGenerationTraceError> {
        if self.run_finished {
            return Err(trace_validation_error(
                "trace_event_after_run_finished",
                format!("event {} appears after run completion", event.index),
            ));
        }
        match &event.body {
            CatalogGenerationTraceEventBody::InputBound { input_hashes } => {
                if event.index != 0
                    || event.attempt.is_some()
                    || self.input_bound
                    || input_hashes != expected_inputs
                {
                    return Err(trace_validation_error(
                        "trace_input_event_invalid",
                        "input binding must be the unique first run-level event",
                    ));
                }
                self.input_bound = true;
            }
            CatalogGenerationTraceEventBody::AttemptStarted { .. } => {
                self.require_input_bound(event.index)?;
                let attempt = event.attempt.ok_or_else(|| {
                    trace_validation_error(
                        "trace_attempt_missing",
                        "attempt-start event has no attempt",
                    )
                })?;
                if self.state.attempt.is_some() || attempt != self.next_attempt {
                    return Err(trace_validation_error(
                        "trace_attempt_order_invalid",
                        format!("attempt {attempt} is not the next expected attempt"),
                    ));
                }
                self.state = CatalogGenerationReplayState {
                    attempt: Some(attempt),
                    ..CatalogGenerationReplayState::default()
                };
                self.pending_sections.clear();
            }
            CatalogGenerationTraceEventBody::RoomDomainEvaluated {
                piece_id,
                candidates,
                ..
            } => {
                self.require_current_attempt(event)?;
                let domain = candidates
                    .iter()
                    .map(|candidate| (candidate.shape_id.clone(), candidate.transform.clone()))
                    .collect::<BTreeSet<_>>();
                if self
                    .state
                    .domains
                    .insert(piece_id.clone(), domain)
                    .is_some()
                {
                    return Err(trace_validation_error(
                        "trace_room_domain_duplicate",
                        format!("room {piece_id} has more than one domain event"),
                    ));
                }
            }
            CatalogGenerationTraceEventBody::RoomPlaced { placement } => {
                self.require_current_attempt(event)?;
                if !self
                    .state
                    .domains
                    .get(placement.piece_id.as_str())
                    .is_some_and(|domain| {
                        domain.contains(&(placement.shape_id.clone(), placement.transform.clone()))
                    })
                {
                    return Err(trace_validation_error(
                        "trace_room_choice_outside_domain",
                        format!(
                            "room {} placement is absent from its recorded domain",
                            placement.piece_id
                        ),
                    ));
                }
                if self
                    .state
                    .rooms
                    .insert(placement.piece_id.clone(), placement.clone())
                    .is_some()
                {
                    return Err(trace_validation_error(
                        "trace_room_duplicate",
                        format!("room {} is placed twice", placement.piece_id),
                    ));
                }
            }
            CatalogGenerationTraceEventBody::RoomConflict {
                piece_id,
                conflicting_cells,
            } => {
                self.require_current_attempt(event)?;
                if conflicting_cells.is_empty() || !self.state.domains.contains_key(piece_id) {
                    return Err(trace_validation_error(
                        "trace_room_conflict_invalid",
                        format!("room {piece_id} conflict has no domain or cells"),
                    ));
                }
            }
            CatalogGenerationTraceEventBody::SectionRoutingStarted { section_id, .. } => {
                self.require_current_attempt(event)?;
                if !self.pending_sections.insert(section_id.clone())
                    || self.state.routes.contains_key(section_id)
                {
                    return Err(trace_validation_error(
                        "trace_section_start_duplicate",
                        format!("section {section_id} starts more than once"),
                    ));
                }
            }
            CatalogGenerationTraceEventBody::SectionRoutingFinished {
                section_id,
                status,
                cells,
                states_visited,
            } => {
                self.require_current_attempt(event)?;
                if !self.pending_sections.remove(section_id) {
                    return Err(trace_validation_error(
                        "trace_section_finish_without_start",
                        format!("section {section_id} finishes without a start"),
                    ));
                }
                self.state.routing_states =
                    self.state.routing_states.saturating_add(*states_visited);
                match status.as_str() {
                    "found" if !cells.is_empty() => {
                        self.state.routes.insert(
                            section_id.clone(),
                            CatalogGenerationTraceRoute {
                                section_id: section_id.clone(),
                                cells: cells.clone(),
                                states_visited: *states_visited,
                            },
                        );
                    }
                    "no_path" | "budget_exhausted" if cells.is_empty() => {}
                    _ => {
                        return Err(trace_validation_error(
                            "trace_section_status_invalid",
                            format!("section {section_id} has invalid status/cells"),
                        ));
                    }
                }
            }
            CatalogGenerationTraceEventBody::ValidationCompleted {
                stage,
                ok,
                subject_hash,
                diagnostic_codes,
            } => {
                let attempt = self.require_current_attempt(event)?;
                if result.selected_attempt == Some(attempt) {
                    validate_selected_result_stage(
                        stage,
                        *ok,
                        subject_hash,
                        diagnostic_codes,
                        result,
                    )?;
                }
            }
            CatalogGenerationTraceEventBody::AttemptFinished {
                classification,
                stage,
                detail,
                rooms_placed,
                sections_routed,
                routing_states,
            } => {
                let attempt = self.require_current_attempt(event)?;
                if !self.pending_sections.is_empty() {
                    return Err(trace_validation_error(
                        "trace_section_unfinished",
                        format!("attempt {attempt} finishes with a pending section"),
                    ));
                }
                let expected = result
                    .attempts
                    .get(usize::try_from(attempt).map_err(|_| {
                        trace_validation_error(
                            "trace_attempt_index_overflow",
                            "attempt index exceeds usize",
                        )
                    })?)
                    .ok_or_else(|| {
                        trace_validation_error(
                            "trace_attempt_missing_from_result",
                            format!("result contains no attempt evidence for {attempt}"),
                        )
                    })?;
                if expected.classification != *classification
                    || expected.stage != *stage
                    || expected.detail != *detail
                    || expected.rooms_placed != *rooms_placed
                    || expected.sections_routed != *sections_routed
                    || expected.routing_states != *routing_states
                {
                    return Err(trace_validation_error(
                        "trace_attempt_evidence_mismatch",
                        format!("attempt {attempt} evidence does not match the result"),
                    ));
                }
                if self.state.rooms.len() != *rooms_placed
                    || self.state.routes.len() != *sections_routed
                    || self.state.routing_states != *routing_states
                {
                    return Err(trace_validation_error(
                        "trace_attempt_state_mismatch",
                        format!("attempt {attempt} visible state does not match its metrics"),
                    ));
                }
                if classification == "success" {
                    validate_selected_visible_state(&self.state, result)?;
                }
                self.completed_attempt = Some(CatalogGenerationReplayAttempt {
                    attempt,
                    rooms: self.state.rooms.values().cloned().collect(),
                    routes: self.state.routes.values().cloned().collect(),
                    classification: classification.clone(),
                });
                self.state.attempt = None;
                self.next_attempt = self.next_attempt.checked_add(1).ok_or_else(|| {
                    trace_validation_error(
                        "trace_attempt_count_overflow",
                        "trace attempt count overflowed",
                    )
                })?;
            }
            CatalogGenerationTraceEventBody::RunFinished {
                selected_attempt,
                classification,
                reason,
                output_hash,
            } => {
                self.require_input_bound(event.index)?;
                if event.attempt.is_some() || self.state.attempt.is_some() {
                    return Err(trace_validation_error(
                        "trace_run_finish_invalid",
                        "run-finished event must be run-level and follow a completed attempt",
                    ));
                }
                let selection = catalog_generation_trace_selection(result)?;
                if *selected_attempt != selection.selected_attempt
                    || *classification != selection.classification
                    || *reason != selection.reason
                    || *output_hash
                        != hash_json(result).map_err(|detail| {
                            CatalogGenerationTraceError::new(
                                "trace_output_hash_failed",
                                detail,
                                None,
                                None,
                            )
                        })?
                {
                    return Err(trace_validation_error(
                        "trace_run_finish_mismatch",
                        "run-finished event does not match the generation result",
                    ));
                }
                self.run_finished = true;
                if usize::try_from(self.next_attempt).ok() != Some(result.attempts.len()) {
                    return Err(trace_validation_error(
                        "trace_attempt_count_mismatch",
                        "trace attempt count does not match the generation result",
                    ));
                }
            }
        }
        Ok(())
    }

    fn require_input_bound(&self, event_index: u32) -> Result<(), CatalogGenerationTraceError> {
        if self.input_bound {
            Ok(())
        } else {
            Err(trace_validation_error(
                "trace_input_event_missing",
                format!("event {event_index} precedes input binding"),
            ))
        }
    }

    fn require_current_attempt(
        &self,
        event: &CatalogGenerationTraceEvent,
    ) -> Result<u32, CatalogGenerationTraceError> {
        let attempt = self.state.attempt.ok_or_else(|| {
            trace_validation_error(
                "trace_attempt_not_active",
                format!("event {} has no active attempt", event.index),
            )
        })?;
        if event.attempt != Some(attempt) {
            return Err(trace_validation_error(
                "trace_attempt_mismatch",
                format!(
                    "event {} does not name active attempt {attempt}",
                    event.index
                ),
            ));
        }
        Ok(attempt)
    }

    fn take_completed_attempt(&mut self) -> Option<CatalogGenerationReplayAttempt> {
        self.completed_attempt.take()
    }

    fn frame(
        &self,
        event_index: u32,
    ) -> Result<CatalogGenerationReplayFrame, CatalogGenerationTraceError> {
        let occupied_cell_count = self.state.rooms.values().try_fold(0_usize, |sum, room| {
            sum.checked_add(room.occupied_cells.len())
        });
        let routed_cell_count = self
            .state
            .routes
            .values()
            .try_fold(0_usize, |sum, route| sum.checked_add(route.cells.len()));
        Ok(CatalogGenerationReplayFrame {
            event_index,
            attempt: self.state.attempt,
            room_count: self.state.rooms.len(),
            route_count: self.state.routes.len(),
            occupied_cell_count: occupied_cell_count.ok_or_else(|| {
                trace_validation_error(
                    "trace_replay_cell_overflow",
                    "replayed room-cell count overflowed",
                )
            })?,
            routed_cell_count: routed_cell_count.ok_or_else(|| {
                trace_validation_error(
                    "trace_replay_cell_overflow",
                    "replayed route-cell count overflowed",
                )
            })?,
            state_hash: hash_json(&self.state).map_err(|detail| {
                CatalogGenerationTraceError::new(
                    "trace_replay_state_hash_failed",
                    detail,
                    None,
                    None,
                )
            })?,
        })
    }

    fn finish(
        &self,
        trace: &CatalogGenerationTrace,
        selection: &CatalogGenerationTraceSelection,
    ) -> Result<(), CatalogGenerationTraceError> {
        if !self.input_bound || !self.run_finished {
            return Err(trace_validation_error(
                "trace_run_incomplete",
                "trace does not contain a complete input-to-result event sequence",
            ));
        }
        if trace.events.last().is_none_or(|event| {
            !matches!(
                event.body,
                CatalogGenerationTraceEventBody::RunFinished { .. }
            )
        }) {
            return Err(trace_validation_error(
                "trace_run_finish_not_last",
                "run-finished must be the final trace event",
            ));
        }
        if selection.selected_attempt.is_some_and(|attempt| {
            attempt
                .checked_add(1)
                .is_none_or(|count| count != self.next_attempt)
        }) || selection.selected_attempt.is_none() && self.next_attempt == 0
        {
            return Err(trace_validation_error(
                "trace_attempt_count_mismatch",
                "trace attempt count does not match selection evidence",
            ));
        }
        Ok(())
    }
}

fn validate_selected_result_stage(
    stage: &str,
    ok: bool,
    subject_hash: &str,
    diagnostic_codes: &[String],
    result: &CatalogAwareGenerationResult,
) -> Result<(), CatalogGenerationTraceError> {
    let (expected_ok, expected_hash, expected_codes) = match stage {
        "geometry_validation" => {
            let report = result.geometry_validation.as_ref().ok_or_else(|| {
                trace_validation_error(
                    "trace_selected_validation_missing",
                    "selected result has no geometry validation",
                )
            })?;
            (
                report.ok,
                report.state_hash.clone(),
                report
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.clone())
                    .collect::<Vec<_>>(),
            )
        }
        "placement_validation" => {
            let report = result.placement_validation.as_ref().ok_or_else(|| {
                trace_validation_error(
                    "trace_selected_validation_missing",
                    "selected result has no placement validation",
                )
            })?;
            (
                report.ok,
                report.state_hash.clone(),
                report
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.clone())
                    .collect::<Vec<_>>(),
            )
        }
        "built_flow_validation" => {
            let report = result.built_flow_validation.as_ref().ok_or_else(|| {
                trace_validation_error(
                    "trace_selected_validation_missing",
                    "selected result has no built-flow validation",
                )
            })?;
            (
                report.ok,
                hash_json(report).map_err(|detail| {
                    CatalogGenerationTraceError::new(
                        "trace_validation_hash_failed",
                        detail,
                        None,
                        None,
                    )
                })?,
                report
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.clone())
                    .collect::<Vec<_>>(),
            )
        }
        _ => {
            return Err(trace_validation_error(
                "trace_validation_stage_invalid",
                format!("selected result trace has unknown validation stage {stage}"),
            ));
        }
    };
    if ok != expected_ok || subject_hash != expected_hash || diagnostic_codes != expected_codes {
        return Err(trace_validation_error(
            "trace_selected_validation_mismatch",
            format!("trace {stage} evidence does not match the selected result"),
        ));
    }
    Ok(())
}

fn validate_selected_visible_state(
    state: &CatalogGenerationReplayState,
    result: &CatalogAwareGenerationResult,
) -> Result<(), CatalogGenerationTraceError> {
    let placement = result.placement.as_ref().ok_or_else(|| {
        trace_validation_error(
            "trace_selected_placement_missing",
            "successful result has no piece placement",
        )
    })?;
    let expected_rooms = placement
        .instances
        .iter()
        .filter(|instance| is_catalog_room_kind(instance.requirement_kind.as_str()))
        .map(|instance| {
            (
                instance.piece_id.clone(),
                CatalogGenerationTraceRoomPlacement {
                    piece_id: instance.piece_id.clone(),
                    requirement_kind: instance.requirement_kind.clone(),
                    shape_id: instance.shape_id.clone(),
                    transform: instance.transform.clone(),
                    origin: instance.origin.clone(),
                    occupied_cells: instance.occupied_cells.clone(),
                    reserved_cells: instance.reserved_cells.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    if state.rooms != expected_rooms {
        return Err(trace_validation_error(
            "trace_selected_rooms_mismatch",
            "replayed room placements do not match the selected result",
        ));
    }

    let plan = result.piece_plan.as_ref().ok_or_else(|| {
        trace_validation_error(
            "trace_selected_plan_missing",
            "successful result has no piece plan",
        )
    })?;
    let expected_sections = plan
        .links
        .iter()
        .map(|link| link.source_section.clone())
        .collect::<BTreeSet<_>>();
    if state.routes.keys().cloned().collect::<BTreeSet<_>>() != expected_sections {
        return Err(trace_validation_error(
            "trace_selected_sections_mismatch",
            "replayed routed sections do not match the selected result",
        ));
    }
    let geometry = result.geometry.as_ref().ok_or_else(|| {
        trace_validation_error(
            "trace_selected_geometry_missing",
            "successful result has no geometry",
        )
    })?;
    for route in state.routes.values() {
        let corridor = geometry
            .corridors
            .iter()
            .find(|corridor| corridor.physical_section == route.section_id)
            .ok_or_else(|| {
                trace_validation_error(
                    "trace_selected_route_missing",
                    format!("selected geometry has no corridor for {}", route.section_id),
                )
            })?;
        if route
            .cells
            .iter()
            .any(|cell| !geometry_corridor_contains_catalog_cell(corridor, cell))
        {
            return Err(trace_validation_error(
                "trace_selected_route_mismatch",
                format!(
                    "replayed route {} leaves the selected geometry corridor",
                    route.section_id
                ),
            ));
        }
    }
    Ok(())
}

fn geometry_corridor_contains_catalog_cell(corridor: &GeometryCorridor, cell: &GridCell) -> bool {
    let x = cell.x.saturating_mul(GEOMETRY_ROUTE_GRID);
    let y = cell.y.saturating_mul(GEOMETRY_ROUTE_GRID);
    corridor.points.windows(2).any(|points| {
        let from = &points[0];
        let to = &points[1];
        if from.x == to.x && x == from.x {
            y >= from.y.min(to.y) && y <= from.y.max(to.y)
        } else if from.y == to.y && y == from.y {
            x >= from.x.min(to.x) && x <= from.x.max(to.x)
        } else {
            false
        }
    })
}

fn catalog_generation_trace_input_hashes(
    input: CatalogAwareGenerationInput<'_>,
) -> Result<CatalogGenerationTraceInputHashes, CatalogGenerationTraceError> {
    Ok(CatalogGenerationTraceInputHashes {
        candidate_hash: trace_hash(input.candidate)?,
        source_geometry_hash: trace_hash(input.source_geometry)?,
        source_plan_hash: trace_hash(input.source_plan)?,
        catalog_hash: trace_hash(input.catalog)?,
        generation_policy_hash: trace_hash(input.policy)?,
        provenance_hash: trace_hash(&(
            input.provenance.candidate_ref.as_str(),
            input.provenance.geometry_ref.as_str(),
            input.provenance.piece_plan_ref.as_str(),
            input.provenance.catalog_ref.as_str(),
            input.provenance.result_ref.as_str(),
        ))?,
    })
}

fn catalog_generation_trace_root_hash(
    seed: u64,
    input_hashes: &CatalogGenerationTraceInputHashes,
    policy: &CatalogAwareGenerationPolicy,
    limits: &CatalogGenerationTraceLimits,
) -> Result<String, CatalogGenerationTraceError> {
    trace_hash(&CatalogGenerationTraceRootInput {
        kind: "rusty_procgen.catalog_generation_trace.v1",
        schema_version: 1,
        seed,
        input_hashes,
        generation_policy: policy,
        limits,
    })
}

fn catalog_generation_trace_event_hash(
    index: u32,
    attempt: Option<u32>,
    previous_hash: &str,
    body: &CatalogGenerationTraceEventBody,
) -> Result<String, CatalogGenerationTraceError> {
    trace_hash(&CatalogGenerationTraceEventHashInput {
        index,
        attempt,
        previous_hash,
        body,
    })
}

fn catalog_generation_trace_selection(
    result: &CatalogAwareGenerationResult,
) -> Result<CatalogGenerationTraceSelection, CatalogGenerationTraceError> {
    if result.ok {
        let selected_attempt = result.selected_attempt.ok_or_else(|| {
            trace_validation_error(
                "trace_selection_missing",
                "successful result has no selected attempt",
            )
        })?;
        Ok(CatalogGenerationTraceSelection {
            selected_attempt: Some(selected_attempt),
            classification: "success".to_owned(),
            reason: "first_successful_attempt".to_owned(),
        })
    } else {
        let classification = result.exhausted_classification.clone().ok_or_else(|| {
            trace_validation_error(
                "trace_exhaustion_missing",
                "exhausted result has no final classification",
            )
        })?;
        Ok(CatalogGenerationTraceSelection {
            selected_attempt: None,
            classification,
            reason: "generation_attempt_budget_exhausted".to_owned(),
        })
    }
}

fn trace_hash<T: Serialize>(value: &T) -> Result<String, CatalogGenerationTraceError> {
    hash_json(value)
        .map_err(|detail| CatalogGenerationTraceError::new("trace_hash_failed", detail, None, None))
}

fn trace_validation_error(code: &str, detail: impl Into<String>) -> CatalogGenerationTraceError {
    CatalogGenerationTraceError::new(code, detail, None, None)
}
