use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use rusty_procgen_preflight::core::{
    replay_catalog_generation_trace, CatalogAwareGenerationPolicy,
    CatalogAwareGenerationProvenance, CatalogGenerationTrace, CatalogGenerationTraceEventBody,
    CatalogGenerationTraceLimits, CatalogGenerationTraceRequest, CorridorRealization, ProcgenCore,
};
use rusty_procgen_preflight::{
    Candidate, Geometry2dArtifact, IntermediateBreakdown, PieceBuildPlan, PieceShapeMatchReport,
    ShapeCatalog,
};
use serde::Serialize;
use serde_json::Value;

struct TraceFixture {
    candidate: Candidate,
    geometry: Geometry2dArtifact,
    plan: PieceBuildPlan,
    catalog: ShapeCatalog,
    policy: CatalogAwareGenerationPolicy,
    provenance: CatalogAwareGenerationProvenance,
    seed: u64,
}

impl TraceFixture {
    fn checked() -> Self {
        let accepted: Value = serde_json::from_str(include_str!(
            "../../../../artifacts/samples/batch-v2/candidate-000/accepted.json"
        ))
        .expect("checked accepted artifact");
        let candidate =
            serde_json::from_value(accepted["candidate"].clone()).expect("checked candidate");
        let intermediate: IntermediateBreakdown = serde_json::from_str(include_str!(
            "../../../../artifacts/samples/batch-v2/candidate-000/intermediate-breakdown.json"
        ))
        .expect("checked intermediate");
        let geometry = serde_json::from_str(include_str!(
            "../../../../artifacts/samples/batch-v2/candidate-000/geometry-2d.json"
        ))
        .expect("checked geometry");
        let plan = ProcgenCore::emit_piece_plan(
            &candidate,
            &intermediate,
            &geometry,
            CorridorRealization::Catalog,
        )
        .expect("catalog piece plan");
        let catalog = serde_json::from_str(include_str!(
            "../../../../fixtures/shape-catalogs/2d-basic.json"
        ))
        .expect("checked catalog");
        let policy = serde_json::from_str(include_str!(
            "../../../../fixtures/policies/catalog-aware-generation-default.json"
        ))
        .expect("checked catalog-aware policy");
        let shape_match: PieceShapeMatchReport = serde_json::from_str(include_str!(
            "../../../../artifacts/samples/batch-v2/candidate-000/piece-shape-match.json"
        ))
        .expect("checked shape match");
        Self {
            candidate,
            geometry,
            plan,
            catalog,
            policy,
            provenance: CatalogAwareGenerationProvenance {
                candidate_ref: "memory/candidate.json".to_owned(),
                geometry_ref: "memory/geometry.json".to_owned(),
                piece_plan_ref: "memory/piece-plan.json".to_owned(),
                catalog_ref: "memory/catalog.json".to_owned(),
                result_ref: "memory/catalog-aware-result.json".to_owned(),
            },
            seed: shape_match.seed,
        }
    }

    fn request(
        &self,
        trace_limits: CatalogGenerationTraceLimits,
    ) -> CatalogGenerationTraceRequest<'_> {
        CatalogGenerationTraceRequest {
            candidate: &self.candidate,
            source_geometry: &self.geometry,
            source_plan: &self.plan,
            catalog: &self.catalog,
            generation_policy: &self.policy,
            provenance: &self.provenance,
            seed: self.seed,
            trace_limits,
        }
    }
}

fn assert_trace_rejected(
    trace: &CatalogGenerationTrace,
    result: &rusty_procgen_preflight::core::CatalogAwareGenerationResult,
    fixture: &TraceFixture,
    expected_code: &str,
) {
    let error =
        replay_catalog_generation_trace(trace, result, fixture.request(trace.limits.clone()))
            .expect_err("tampered trace must reject");
    assert_eq!(error.code, expected_code);
}

fn write_json(path: &Path, value: &impl Serialize) {
    let encoded = serde_json::to_string_pretty(value).expect("encode fixture");
    fs::write(path, format!("{encoded}\n")).expect("write fixture");
}

fn unique_temp_dir() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "rusty-procgen-catalog-generation-trace-{}-{unique}",
        std::process::id()
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EventHashInput<'a> {
    index: u32,
    attempt: Option<u32>,
    previous_hash: &'a str,
    body: &'a CatalogGenerationTraceEventBody,
}

fn rechain_trace(trace: &mut CatalogGenerationTrace) {
    let mut previous_hash = trace.root_hash.clone();
    let mut body_bytes = 0_u64;
    for event in &mut trace.events {
        event.previous_hash = previous_hash;
        event.event_hash = ProcgenCore::canonical_hash(&EventHashInput {
            index: event.index,
            attempt: event.attempt,
            previous_hash: event.previous_hash.as_str(),
            body: &event.body,
        })
        .expect("rechain event");
        body_bytes = body_bytes
            .checked_add(
                u64::try_from(
                    serde_json::to_vec(&event.body)
                        .expect("encode event body")
                        .len(),
                )
                .expect("event body length"),
            )
            .expect("event body byte sum");
        previous_hash = event.event_hash.clone();
    }
    trace.event_body_bytes = body_bytes;
    trace.final_event_hash = previous_hash;
}

#[test]
fn semantic_trace_is_bounded_hash_closed_replayable_and_cli_equivalent() {
    let fixture = TraceFixture::checked();
    let first = ProcgenCore::realize_catalog_aware_traced(
        fixture.request(CatalogGenerationTraceLimits::default()),
    )
    .expect("first traced run");
    let repeated = ProcgenCore::realize_catalog_aware_traced(
        fixture.request(CatalogGenerationTraceLimits::default()),
    )
    .expect("repeated traced run");
    assert_eq!(
        serde_json::to_value(&first.result).expect("first result JSON"),
        serde_json::to_value(&repeated.result).expect("repeated result JSON")
    );
    assert_eq!(first.trace, repeated.trace);
    let untraced = ProcgenCore::realize_catalog_aware(
        &fixture.candidate,
        &fixture.geometry,
        &fixture.plan,
        &fixture.catalog,
        &fixture.policy,
        &fixture.provenance,
        fixture.seed,
    )
    .expect("untraced result");
    assert_eq!(
        serde_json::to_value(&first.result).expect("traced result JSON"),
        serde_json::to_value(untraced).expect("untraced result JSON"),
        "tracing changed the generation result"
    );

    let replay = replay_catalog_generation_trace(
        &first.trace,
        &first.result,
        fixture.request(first.trace.limits.clone()),
    )
    .expect("replay traced run");
    assert_eq!(replay.frames.len(), first.trace.events.len());
    assert_eq!(replay.final_output_hash, first.trace.final_output_hash);
    assert_eq!(replay.final_event_hash, first.trace.final_event_hash);
    assert_eq!(replay.attempts.len(), first.result.attempts.len());
    assert!(first.trace.events.iter().any(|event| matches!(
        event.body,
        CatalogGenerationTraceEventBody::RoomDomainEvaluated { .. }
    )));
    assert!(first.trace.events.iter().any(|event| matches!(
        event.body,
        CatalogGenerationTraceEventBody::RoomPlaced { .. }
    )));
    assert!(first.trace.events.iter().any(|event| matches!(
        event.body,
        CatalogGenerationTraceEventBody::SectionRoutingFinished {
            ref status,
            ..
        } if status == "found"
    )));
    assert!(first.trace.events.iter().any(|event| matches!(
        event.body,
        CatalogGenerationTraceEventBody::ValidationCompleted { .. }
    )));

    let exact_limits = CatalogGenerationTraceLimits {
        max_events: u32::try_from(first.trace.events.len()).expect("event count"),
        max_event_body_bytes: first.trace.event_body_bytes,
        max_visual_cells: first.trace.visual_cell_count,
    };
    let exact = ProcgenCore::realize_catalog_aware_traced(fixture.request(exact_limits.clone()))
        .expect("exact trace limits");
    assert_eq!(exact.trace.limits, exact_limits);
    assert_eq!(exact.trace.event_body_bytes, first.trace.event_body_bytes);
    assert_eq!(exact.trace.visual_cell_count, first.trace.visual_cell_count);

    let event_error =
        ProcgenCore::realize_catalog_aware_traced(fixture.request(CatalogGenerationTraceLimits {
            max_events: exact_limits.max_events - 1,
            ..exact_limits.clone()
        }))
        .expect_err("one-over event quota must reject");
    assert_eq!(event_error.code, "trace_event_quota_exceeded");
    let byte_error =
        ProcgenCore::realize_catalog_aware_traced(fixture.request(CatalogGenerationTraceLimits {
            max_event_body_bytes: exact_limits.max_event_body_bytes - 1,
            ..exact_limits.clone()
        }))
        .expect_err("one-over byte quota must reject");
    assert_eq!(byte_error.code, "trace_event_body_byte_quota_exceeded");
    let visual_error =
        ProcgenCore::realize_catalog_aware_traced(fixture.request(CatalogGenerationTraceLimits {
            max_visual_cells: exact_limits.max_visual_cells - 1,
            ..exact_limits.clone()
        }))
        .expect_err("one-over visual-cell quota must reject");
    assert_eq!(visual_error.code, "trace_visual_cell_quota_exceeded");

    let mut input_tamper = first.trace.clone();
    input_tamper.input_hashes.candidate_hash = "fnv1a64:0000000000000000".to_owned();
    assert_trace_rejected(
        &input_tamper,
        &first.result,
        &fixture,
        "trace_input_hash_mismatch",
    );
    let mut order_tamper = first.trace.clone();
    order_tamper.events.swap(1, 2);
    assert_trace_rejected(
        &order_tamper,
        &first.result,
        &fixture,
        "trace_event_order_invalid",
    );
    let mut body_tamper = first.trace.clone();
    match &mut body_tamper.events[1].body {
        CatalogGenerationTraceEventBody::AttemptStarted { room_slack_cells } => {
            *room_slack_cells += 1;
        }
        other => panic!("expected attempt start, got {other:?}"),
    }
    assert_trace_rejected(
        &body_tamper,
        &first.result,
        &fixture,
        "trace_event_hash_mismatch",
    );
    let mut link_tamper = first.trace.clone();
    link_tamper.events[2].previous_hash = "fnv1a64:1111111111111111".to_owned();
    assert_trace_rejected(
        &link_tamper,
        &first.result,
        &fixture,
        "trace_previous_hash_mismatch",
    );
    let mut output_tamper = first.trace.clone();
    output_tamper.final_output_hash = "fnv1a64:2222222222222222".to_owned();
    assert_trace_rejected(
        &output_tamper,
        &first.result,
        &fixture,
        "trace_final_output_hash_mismatch",
    );
    let mut selection_tamper = first.trace.clone();
    selection_tamper.selection.reason = "caller_selected".to_owned();
    assert_trace_rejected(
        &selection_tamper,
        &first.result,
        &fixture,
        "trace_selection_mismatch",
    );
    let mut rechained_room_tamper = first.trace.clone();
    let placement = rechained_room_tamper
        .events
        .iter_mut()
        .find_map(|event| match &mut event.body {
            CatalogGenerationTraceEventBody::RoomPlaced { placement } => Some(placement),
            _ => None,
        })
        .expect("room placement event");
    placement.origin.x += 1;
    for cell in &mut placement.occupied_cells {
        cell.x += 1;
    }
    for cell in &mut placement.reserved_cells {
        cell.x += 1;
    }
    rechain_trace(&mut rechained_room_tamper);
    assert_trace_rejected(
        &rechained_room_tamper,
        &first.result,
        &fixture,
        "trace_selected_rooms_mismatch",
    );

    let root = unique_temp_dir();
    fs::create_dir_all(&root).expect("create CLI fixture");
    let candidate_path = root.join("candidate.json");
    let geometry_path = root.join("geometry.json");
    let plan_path = root.join("piece-plan.json");
    let catalog_path = root.join("catalog.json");
    let policy_path = root.join("policy.json");
    let result_path = root.join("result.json");
    let trace_path = root.join("trace.json");
    write_json(&candidate_path, &fixture.candidate);
    write_json(&geometry_path, &fixture.geometry);
    write_json(&plan_path, &fixture.plan);
    write_json(&catalog_path, &fixture.catalog);
    write_json(&policy_path, &fixture.policy);
    let cli_provenance = CatalogAwareGenerationProvenance {
        candidate_ref: candidate_path.to_string_lossy().into_owned(),
        geometry_ref: geometry_path.to_string_lossy().into_owned(),
        piece_plan_ref: plan_path.to_string_lossy().into_owned(),
        catalog_ref: catalog_path.to_string_lossy().into_owned(),
        result_ref: result_path.to_string_lossy().into_owned(),
    };
    let cli_public = ProcgenCore::realize_catalog_aware_traced(CatalogGenerationTraceRequest {
        candidate: &fixture.candidate,
        source_geometry: &fixture.geometry,
        source_plan: &fixture.plan,
        catalog: &fixture.catalog,
        generation_policy: &fixture.policy,
        provenance: &cli_provenance,
        seed: fixture.seed,
        trace_limits: exact_limits.clone(),
    })
    .expect("public CLI-equivalent trace");
    let output = Command::new(env!("CARGO_BIN_EXE_rusty-procgen"))
        .args(["build", "realize-catalog-aware", "--candidate"])
        .arg(&candidate_path)
        .arg("--geometry")
        .arg(&geometry_path)
        .arg("--piece-plan")
        .arg(&plan_path)
        .arg("--catalog")
        .arg(&catalog_path)
        .arg("--policy")
        .arg(&policy_path)
        .arg("--seed")
        .arg(fixture.seed.to_string())
        .arg("--out")
        .arg(&result_path)
        .arg("--trace-out")
        .arg(&trace_path)
        .arg("--trace-max-events")
        .arg(exact_limits.max_events.to_string())
        .arg("--trace-max-event-body-bytes")
        .arg(exact_limits.max_event_body_bytes.to_string())
        .arg("--trace-max-visual-cells")
        .arg(exact_limits.max_visual_cells.to_string())
        .output()
        .expect("run traced CLI");
    assert!(
        output.status.success(),
        "traced CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let cli_result: Value =
        serde_json::from_slice(&fs::read(&result_path).expect("read CLI result"))
            .expect("decode CLI result");
    let cli_trace: CatalogGenerationTrace =
        serde_json::from_slice(&fs::read(&trace_path).expect("read CLI trace"))
            .expect("decode CLI trace");
    assert_eq!(
        cli_result,
        serde_json::to_value(cli_public.result).expect("public result JSON")
    );
    assert_eq!(cli_trace, cli_public.trace);

    let result_sentinel = b"result-sentinel\n";
    let trace_sentinel = b"trace-sentinel\n";
    fs::write(&result_path, result_sentinel).expect("write result sentinel");
    fs::write(&trace_path, trace_sentinel).expect("write trace sentinel");
    let rejected = Command::new(env!("CARGO_BIN_EXE_rusty-procgen"))
        .args(["build", "realize-catalog-aware", "--candidate"])
        .arg(&candidate_path)
        .arg("--geometry")
        .arg(&geometry_path)
        .arg("--piece-plan")
        .arg(&plan_path)
        .arg("--catalog")
        .arg(&catalog_path)
        .arg("--policy")
        .arg(&policy_path)
        .arg("--seed")
        .arg(fixture.seed.to_string())
        .arg("--out")
        .arg(&result_path)
        .arg("--trace-out")
        .arg(&trace_path)
        .arg("--trace-max-events")
        .arg((exact_limits.max_events - 1).to_string())
        .arg("--trace-max-event-body-bytes")
        .arg(exact_limits.max_event_body_bytes.to_string())
        .arg("--trace-max-visual-cells")
        .arg(exact_limits.max_visual_cells.to_string())
        .output()
        .expect("run rejected traced CLI");
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("trace_event_quota_exceeded"));
    assert_eq!(
        fs::read(&result_path).expect("read result sentinel"),
        result_sentinel
    );
    assert_eq!(
        fs::read(&trace_path).expect("read trace sentinel"),
        trace_sentinel
    );
    fs::remove_dir_all(root).expect("remove CLI fixture");
}

#[test]
fn exhausted_trace_closes_every_attempt_and_preserves_typed_selection() {
    let fixture = TraceFixture::checked();
    let policy = CatalogAwareGenerationPolicy {
        max_routing_states_per_section: 100,
        ..fixture.policy.clone()
    };
    let request = || CatalogGenerationTraceRequest {
        candidate: &fixture.candidate,
        source_geometry: &fixture.geometry,
        source_plan: &fixture.plan,
        catalog: &fixture.catalog,
        generation_policy: &policy,
        provenance: &fixture.provenance,
        seed: fixture.seed,
        trace_limits: CatalogGenerationTraceLimits::default(),
    };
    let run =
        ProcgenCore::realize_catalog_aware_traced(request()).expect("budget-exhausted traced run");
    assert!(!run.result.ok);
    assert_eq!(
        run.result.exhausted_classification.as_deref(),
        Some("search_budget_exhaustion")
    );
    assert_eq!(run.trace.selection.selected_attempt, None);
    assert_eq!(
        run.trace.selection.classification,
        "search_budget_exhaustion"
    );
    assert_eq!(
        run.trace.selection.reason,
        "generation_attempt_budget_exhausted"
    );
    assert_eq!(
        run.result.attempts.len(),
        usize::try_from(policy.max_generation_attempts).expect("attempt count")
    );
    let replay =
        replay_catalog_generation_trace(&run.trace, &run.result, request()).expect("replay");
    assert_eq!(replay.attempts.len(), run.result.attempts.len());
    assert!(replay
        .attempts
        .iter()
        .all(|attempt| attempt.classification == "search_budget_exhaustion"));
}
