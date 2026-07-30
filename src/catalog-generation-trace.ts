import { fnv1a64Json } from './ca-trace-hash.js';

const TRACE_KIND = 'rusty_procgen.catalog_generation_trace.v1';
const RESULT_KIND = 'rusty_procgen.catalog_aware_generation.v1';
const POLICY_KIND = 'rusty_procgen.catalog_aware_generation_policy.v1';
const HARD_MAX_EVENTS = 4_096;
const HARD_MAX_EVENT_BODY_BYTES = 4_194_304;
const HARD_MAX_VISUAL_CELLS = 1_048_576;
const UTF8 = new TextEncoder();

export interface CatalogGenerationTraceLimits {
  readonly maxEvents: number;
  readonly maxEventBodyBytes: number;
  readonly maxVisualCells: number;
}

export interface CatalogAwareGenerationPolicy {
  readonly kind: typeof POLICY_KIND;
  readonly schemaVersion: 1;
  readonly maxGenerationAttempts: number;
  readonly initialRoomSlackCells: number;
  readonly roomSlackGrowthCells: number;
  readonly maxRoomCandidates: number;
  readonly maxRoutingStatesPerSection: number;
  readonly routeMarginCells: number;
  readonly guideDistanceWeight: number;
  readonly turnPenalty: number;
}

export interface CatalogGenerationInputHashes {
  readonly candidateHash: string;
  readonly sourceGeometryHash: string;
  readonly sourcePlanHash: string;
  readonly catalogHash: string;
  readonly generationPolicyHash: string;
  readonly provenanceHash: string;
}

export interface CatalogGridCell {
  readonly x: number;
  readonly y: number;
}

export interface CatalogGridBounds {
  readonly minX: number;
  readonly maxX: number;
  readonly minY: number;
  readonly maxY: number;
}

export interface CatalogGenerationRoomCandidate {
  readonly shapeId: string;
  readonly transform: string;
  readonly score: number;
  readonly rank: number;
}

export interface CatalogGenerationRoomPlacement {
  readonly pieceId: string;
  readonly requirementKind: string;
  readonly shapeId: string;
  readonly transform: string;
  readonly origin: CatalogGridCell;
  readonly occupiedCells: readonly CatalogGridCell[];
  readonly reservedCells: readonly CatalogGridCell[];
}

export interface CatalogGenerationRoute {
  readonly sectionId: string;
  readonly cells: readonly CatalogGridCell[];
  readonly statesVisited: number;
}

export type CatalogGenerationEventBody =
  | {
    readonly type: 'input_bound';
    readonly inputHashes: CatalogGenerationInputHashes;
  }
  | {
    readonly type: 'attempt_started';
    readonly roomSlackCells: number;
  }
  | {
    readonly type: 'room_domain_evaluated';
    readonly pieceId: string;
    readonly requirementKind: string;
    readonly candidates: readonly CatalogGenerationRoomCandidate[];
  }
  | {
    readonly type: 'room_placed';
    readonly placement: CatalogGenerationRoomPlacement;
  }
  | {
    readonly type: 'room_conflict';
    readonly pieceId: string;
    readonly conflictingCells: readonly CatalogGridCell[];
  }
  | {
    readonly type: 'section_routing_started';
    readonly sectionId: string;
    readonly start: CatalogGridCell;
    readonly goal: CatalogGridCell;
    readonly guide: readonly CatalogGridCell[];
    readonly bounds: CatalogGridBounds;
  }
  | {
    readonly type: 'section_routing_finished';
    readonly sectionId: string;
    readonly status: 'found' | 'no_path' | 'budget_exhausted';
    readonly cells: readonly CatalogGridCell[];
    readonly statesVisited: number;
  }
  | {
    readonly type: 'validation_completed';
    readonly stage: string;
    readonly ok: boolean;
    readonly subjectHash: string;
    readonly diagnosticCodes: readonly string[];
  }
  | {
    readonly type: 'attempt_finished';
    readonly classification: string;
    readonly stage: string;
    readonly detail: string;
    readonly roomsPlaced: number;
    readonly sectionsRouted: number;
    readonly routingStates: number;
  }
  | {
    readonly type: 'run_finished';
    readonly selectedAttempt: number | null;
    readonly classification: string;
    readonly reason: string;
    readonly outputHash: string;
  };

export interface CatalogGenerationTraceEvent {
  readonly index: number;
  readonly attempt: number | null;
  readonly previousHash: string;
  readonly eventHash: string;
  readonly body: CatalogGenerationEventBody;
}

export interface CatalogGenerationTraceSelection {
  readonly selectedAttempt: number | null;
  readonly classification: string;
  readonly reason: string;
}

export interface CatalogGenerationTrace {
  readonly kind: typeof TRACE_KIND;
  readonly schemaVersion: 1;
  readonly seed: number;
  readonly inputHashes: CatalogGenerationInputHashes;
  readonly generationPolicy: CatalogAwareGenerationPolicy;
  readonly limits: CatalogGenerationTraceLimits;
  readonly rootHash: string;
  readonly events: readonly CatalogGenerationTraceEvent[];
  readonly eventBodyBytes: number;
  readonly visualCellCount: number;
  readonly finalEventHash: string;
  readonly finalOutputHash: string;
  readonly selection: CatalogGenerationTraceSelection;
}

export interface CatalogAwareAttemptEvidence {
  readonly attempt: number;
  readonly roomSlackCells: number;
  readonly classification: string;
  readonly stage: string;
  readonly detail: string;
  readonly roomsPlaced: number;
  readonly sectionsRouted: number;
  readonly routingStates: number;
}

export interface CatalogGenerationAttempt {
  readonly attempt: number;
  readonly evidence: CatalogAwareAttemptEvidence;
  readonly eventIndices: readonly number[];
  readonly finalRooms: readonly CatalogGenerationRoomPlacement[];
  readonly finalRoutes: readonly CatalogGenerationRoute[];
}

export interface DecodedCatalogGenerationRun {
  readonly trace: CatalogGenerationTrace;
  readonly result: Readonly<Record<string, unknown>>;
  readonly candidateId: string;
  readonly attempts: readonly CatalogGenerationAttempt[];
  readonly outputHash: string;
  readonly selectedAttempt: number | null;
}

export interface CatalogGenerationVisualState {
  readonly attempt: number;
  readonly frame: number;
  readonly frameCount: number;
  readonly event: CatalogGenerationTraceEvent | null;
  readonly rooms: readonly CatalogGenerationRoomPlacement[];
  readonly routes: readonly CatalogGenerationRoute[];
  readonly pendingRoute: {
    readonly sectionId: string;
    readonly start: CatalogGridCell;
    readonly goal: CatalogGridCell;
    readonly guide: readonly CatalogGridCell[];
    readonly bounds: CatalogGridBounds;
  } | null;
  readonly conflict: {
    readonly pieceId: string;
    readonly cells: readonly CatalogGridCell[];
  } | null;
  readonly validation: {
    readonly stage: string;
    readonly ok: boolean;
    readonly diagnosticCodes: readonly string[];
  } | null;
}

interface DecodedResult {
  readonly raw: Readonly<Record<string, unknown>>;
  readonly ok: boolean;
  readonly candidateId: string;
  readonly policy: CatalogAwareGenerationPolicy;
  readonly attempts: readonly CatalogAwareAttemptEvidence[];
  readonly selectedAttempt: number | null;
  readonly exhaustedClassification: string | null;
  readonly geometry: Readonly<Record<string, unknown>> | null;
  readonly geometryValidation: Readonly<Record<string, unknown>> | null;
  readonly piecePlan: Readonly<Record<string, unknown>> | null;
  readonly shapeMatch: Readonly<Record<string, unknown>> | null;
  readonly placement: Readonly<Record<string, unknown>> | null;
  readonly placementValidation: Readonly<Record<string, unknown>> | null;
  readonly builtFlowValidation: Readonly<Record<string, unknown>> | null;
}

interface MutableAttemptState {
  readonly attempt: number;
  readonly roomSlackCells: number;
  readonly domains: Map<string, Set<string>>;
  readonly rooms: Map<string, CatalogGenerationRoomPlacement>;
  readonly routes: Map<string, CatalogGenerationRoute>;
  readonly pending: Map<string, {
    readonly start: CatalogGridCell;
    readonly goal: CatalogGridCell;
    readonly guide: readonly CatalogGridCell[];
    readonly bounds: CatalogGridBounds;
  }>;
  routingStates: number;
}

export function decodeCatalogGenerationRun(
  traceInput: unknown,
  resultInput: unknown,
): DecodedCatalogGenerationRun {
  const result = decodeResult(resultInput);
  const trace = decodeTrace(traceInput);
  const outputHash = fnv1a64Json(result.raw);
  if (trace.finalOutputHash !== outputHash) {
    fail('trace.finalOutputHash', `does not match result hash ${outputHash}`);
  }
  if (!samePolicy(trace.generationPolicy, result.policy)) {
    fail('trace.generationPolicy', 'does not match the result policy');
  }
  const expectedSelection = result.ok
    ? {
      selectedAttempt: result.selectedAttempt,
      classification: 'success',
      reason: 'first_successful_attempt',
    }
    : {
      selectedAttempt: null,
      classification: result.exhaustedClassification
        ?? result.attempts.at(-1)?.classification
        ?? 'generation_infeasibility',
      reason: 'generation_attempt_budget_exhausted',
    };
  if (
    trace.selection.selectedAttempt !== expectedSelection.selectedAttempt
    || trace.selection.classification !== expectedSelection.classification
    || trace.selection.reason !== expectedSelection.reason
  ) {
    fail('trace.selection', 'does not match the supplied result');
  }

  const attempts = validateEventSequence(trace, result, outputHash);
  validateSelectedOutput(attempts, result);
  return {
    trace,
    result: result.raw,
    candidateId: result.candidateId,
    attempts,
    outputHash,
    selectedAttempt: result.selectedAttempt,
  };
}

export function replayCatalogGenerationAttempt(
  run: DecodedCatalogGenerationRun,
  attemptNumber: number,
  frame: number,
): CatalogGenerationVisualState {
  const attempt = run.attempts.find((candidate) => candidate.attempt === attemptNumber);
  if (attempt === undefined) {
    fail('attempt', `trace contains no attempt ${attemptNumber}`);
  }
  const bounded = Math.max(0, Math.min(attempt.eventIndices.length, Math.trunc(frame)));
  const rooms = new Map<string, CatalogGenerationRoomPlacement>();
  const routes = new Map<string, CatalogGenerationRoute>();
  const pending = new Map<string, {
    readonly start: CatalogGridCell;
    readonly goal: CatalogGridCell;
    readonly guide: readonly CatalogGridCell[];
    readonly bounds: CatalogGridBounds;
  }>();
  let conflict: CatalogGenerationVisualState['conflict'] = null;
  let validation: CatalogGenerationVisualState['validation'] = null;
  let event: CatalogGenerationTraceEvent | null = null;
  for (let offset = 0; offset < bounded; offset += 1) {
    event = required(
      run.trace.events,
      required(attempt.eventIndices, offset, 'attempt event index'),
      'event',
    );
    switch (event.body.type) {
      case 'room_placed':
        rooms.set(event.body.placement.pieceId, event.body.placement);
        conflict = null;
        break;
      case 'room_conflict':
        conflict = {
          pieceId: event.body.pieceId,
          cells: event.body.conflictingCells,
        };
        break;
      case 'section_routing_started':
        pending.set(event.body.sectionId, {
          start: event.body.start,
          goal: event.body.goal,
          guide: event.body.guide,
          bounds: event.body.bounds,
        });
        conflict = null;
        break;
      case 'section_routing_finished':
        pending.delete(event.body.sectionId);
        if (event.body.status === 'found') {
          routes.set(event.body.sectionId, {
            sectionId: event.body.sectionId,
            cells: event.body.cells,
            statesVisited: event.body.statesVisited,
          });
        }
        break;
      case 'validation_completed':
        validation = {
          stage: event.body.stage,
          ok: event.body.ok,
          diagnosticCodes: event.body.diagnosticCodes,
        };
        break;
      default:
        break;
    }
  }
  const pendingEntry = [...pending.entries()].at(-1);
  return {
    attempt: attemptNumber,
    frame: bounded,
    frameCount: attempt.eventIndices.length,
    event,
    rooms: [...rooms.values()],
    routes: [...routes.values()],
    pendingRoute: pendingEntry === undefined
      ? null
      : { sectionId: pendingEntry[0], ...pendingEntry[1] },
    conflict,
    validation,
  };
}

export function catalogGenerationStageFrames(
  run: DecodedCatalogGenerationRun,
  attemptNumber: number,
): readonly number[] {
  const attempt = run.attempts.find((candidate) => candidate.attempt === attemptNumber);
  if (attempt === undefined) {
    return [];
  }
  const stages: number[] = [0];
  let previous = '';
  attempt.eventIndices.forEach((eventIndex, offset) => {
    const event = required(run.trace.events, eventIndex, 'event');
    const stage = eventStage(event.body);
    if (stage !== previous) {
      stages.push(offset + 1);
      previous = stage;
    }
  });
  if (stages.at(-1) !== attempt.eventIndices.length) {
    stages.push(attempt.eventIndices.length);
  }
  return stages;
}

function decodeTrace(input: unknown): CatalogGenerationTrace {
  const value = record(input, 'trace');
  exactKeys(value, [
    'kind', 'schemaVersion', 'seed', 'inputHashes', 'generationPolicy', 'limits',
    'rootHash', 'events', 'eventBodyBytes', 'visualCellCount', 'finalEventHash',
    'finalOutputHash', 'selection',
  ], 'trace');
  literal(value.kind, TRACE_KIND, 'trace.kind');
  literal(value.schemaVersion, 1, 'trace.schemaVersion');
  const seed = integer(value.seed, 'trace.seed', 0, Number.MAX_SAFE_INTEGER);
  const inputHashes = decodeInputHashes(value.inputHashes, 'trace.inputHashes');
  const generationPolicy = decodePolicy(value.generationPolicy, 'trace.generationPolicy');
  const limits = decodeLimits(value.limits, 'trace.limits');
  const rootHash = hash(value.rootHash, 'trace.rootHash');
  const events = array(value.events, 'trace.events').map((event, index) =>
    decodeEvent(event, `trace.events[${index}]`));
  const eventBodyBytes = integer(
    value.eventBodyBytes,
    'trace.eventBodyBytes',
    0,
    limits.maxEventBodyBytes,
  );
  const visualCellCount = integer(
    value.visualCellCount,
    'trace.visualCellCount',
    0,
    limits.maxVisualCells,
  );
  const finalEventHash = hash(value.finalEventHash, 'trace.finalEventHash');
  const finalOutputHash = hash(value.finalOutputHash, 'trace.finalOutputHash');
  const selection = decodeSelection(value.selection, 'trace.selection');
  if (events.length > limits.maxEvents) {
    fail('trace.events', `contains ${events.length} events, limit ${limits.maxEvents}`);
  }

  const trace: CatalogGenerationTrace = {
    kind: TRACE_KIND,
    schemaVersion: 1,
    seed,
    inputHashes,
    generationPolicy,
    limits,
    rootHash,
    events,
    eventBodyBytes,
    visualCellCount,
    finalEventHash,
    finalOutputHash,
    selection,
  };
  const expectedGenerationPolicyHash = fnv1a64Json(trace.generationPolicy);
  if (trace.inputHashes.generationPolicyHash !== expectedGenerationPolicyHash) {
    fail(
      'trace.inputHashes.generationPolicyHash',
      `does not match the included generation policy hash ${expectedGenerationPolicyHash}`,
    );
  }
  const expectedRoot = fnv1a64Json({
    kind: trace.kind,
    schemaVersion: trace.schemaVersion,
    seed: trace.seed,
    inputHashes: trace.inputHashes,
    generationPolicy: trace.generationPolicy,
    limits: trace.limits,
  });
  if (trace.rootHash !== expectedRoot) {
    fail('trace.rootHash', `does not match ${expectedRoot}`);
  }
  return trace;
}

function decodeResult(input: unknown): DecodedResult {
  const value = record(input, 'result');
  exactKeys(value, [
    'kind', 'schemaVersion', 'ok', 'candidateId', 'policy', 'attempts',
    'selectedAttempt', 'exhaustedClassification', 'geometry', 'geometryValidation',
    'piecePlan', 'shapeMatch', 'placement', 'placementValidation',
    'builtFlowValidation',
  ], 'result');
  literal(value.kind, RESULT_KIND, 'result.kind');
  literal(value.schemaVersion, 1, 'result.schemaVersion');
  const ok = boolean(value.ok, 'result.ok');
  const candidateId = nonEmptyString(value.candidateId, 'result.candidateId');
  const policy = decodePolicy(value.policy, 'result.policy');
  const attempts = array(value.attempts, 'result.attempts').map((attempt, index) =>
    decodeAttemptEvidence(attempt, `result.attempts[${index}]`));
  const selectedAttempt = nullableInteger(
    value.selectedAttempt,
    'result.selectedAttempt',
    0,
    HARD_MAX_EVENTS,
  );
  const exhaustedClassification = nullableString(
    value.exhaustedClassification,
    'result.exhaustedClassification',
  );
  if (attempts.length === 0 || attempts.length > policy.maxGenerationAttempts) {
    fail('result.attempts', 'must contain one through maxGenerationAttempts entries');
  }
  attempts.forEach((attempt, index) => {
    if (attempt.attempt !== index) {
      fail(`result.attempts[${index}].attempt`, `expected ${index}`);
    }
  });
  if (
    ok
      ? selectedAttempt === null
        || selectedAttempt !== attempts.length - 1
        || exhaustedClassification !== null
      : selectedAttempt !== null || exhaustedClassification === null
  ) {
    fail('result', 'success/exhaustion selection fields are inconsistent');
  }
  const geometry = nullableRecord(value.geometry, 'result.geometry');
  const piecePlan = nullableRecord(value.piecePlan, 'result.piecePlan');
  const placement = nullableRecord(value.placement, 'result.placement');
  const geometryValidation = nullableRecord(
    value.geometryValidation,
    'result.geometryValidation',
  );
  const shapeMatch = nullableRecord(value.shapeMatch, 'result.shapeMatch');
  const placementValidation = nullableRecord(
    value.placementValidation,
    'result.placementValidation',
  );
  const builtFlowValidation = nullableRecord(
    value.builtFlowValidation,
    'result.builtFlowValidation',
  );
  const artifacts = [
    geometry,
    geometryValidation,
    piecePlan,
    shapeMatch,
    placement,
    placementValidation,
    builtFlowValidation,
  ];
  if (ok ? artifacts.some((artifact) => artifact === null) : artifacts.some(
    (artifact) => artifact !== null,
  )) {
    fail(
      'result',
      ok
        ? 'successful result is missing an accepted artifact or validation'
        : 'exhausted result must not publish accepted artifacts or validations',
    );
  }
  return {
    raw: value,
    ok,
    candidateId,
    policy,
    attempts,
    selectedAttempt,
    exhaustedClassification,
    geometry,
    geometryValidation,
    piecePlan,
    shapeMatch,
    placement,
    placementValidation,
    builtFlowValidation,
  };
}

function decodePolicy(input: unknown, path: string): CatalogAwareGenerationPolicy {
  const value = record(input, path);
  exactKeys(value, [
    'kind', 'schemaVersion', 'maxGenerationAttempts', 'initialRoomSlackCells',
    'roomSlackGrowthCells', 'maxRoomCandidates', 'maxRoutingStatesPerSection',
    'routeMarginCells', 'guideDistanceWeight', 'turnPenalty',
  ], path);
  literal(value.kind, POLICY_KIND, `${path}.kind`);
  literal(value.schemaVersion, 1, `${path}.schemaVersion`);
  return {
    kind: POLICY_KIND,
    schemaVersion: 1,
    maxGenerationAttempts: integer(
      value.maxGenerationAttempts,
      `${path}.maxGenerationAttempts`,
      1,
      64,
    ),
    initialRoomSlackCells: integer(
      value.initialRoomSlackCells,
      `${path}.initialRoomSlackCells`,
      -1_000_000,
      1_000_000,
    ),
    roomSlackGrowthCells: integer(
      value.roomSlackGrowthCells,
      `${path}.roomSlackGrowthCells`,
      -1_000_000,
      1_000_000,
    ),
    maxRoomCandidates: integer(value.maxRoomCandidates, `${path}.maxRoomCandidates`, 1, 64),
    maxRoutingStatesPerSection: integer(
      value.maxRoutingStatesPerSection,
      `${path}.maxRoutingStatesPerSection`,
      1,
      1_000_000,
    ),
    routeMarginCells: integer(value.routeMarginCells, `${path}.routeMarginCells`, 0, 1_000_000),
    guideDistanceWeight: integer(
      value.guideDistanceWeight,
      `${path}.guideDistanceWeight`,
      0,
      1_000_000,
    ),
    turnPenalty: integer(value.turnPenalty, `${path}.turnPenalty`, 0, 1_000_000),
  };
}

function decodeLimits(input: unknown, path: string): CatalogGenerationTraceLimits {
  const value = record(input, path);
  exactKeys(value, ['maxEvents', 'maxEventBodyBytes', 'maxVisualCells'], path);
  return {
    maxEvents: integer(value.maxEvents, `${path}.maxEvents`, 1, HARD_MAX_EVENTS),
    maxEventBodyBytes: integer(
      value.maxEventBodyBytes,
      `${path}.maxEventBodyBytes`,
      1,
      HARD_MAX_EVENT_BODY_BYTES,
    ),
    maxVisualCells: integer(
      value.maxVisualCells,
      `${path}.maxVisualCells`,
      1,
      HARD_MAX_VISUAL_CELLS,
    ),
  };
}

function decodeInputHashes(input: unknown, path: string): CatalogGenerationInputHashes {
  const value = record(input, path);
  exactKeys(value, [
    'candidateHash', 'sourceGeometryHash', 'sourcePlanHash', 'catalogHash',
    'generationPolicyHash', 'provenanceHash',
  ], path);
  return {
    candidateHash: hash(value.candidateHash, `${path}.candidateHash`),
    sourceGeometryHash: hash(value.sourceGeometryHash, `${path}.sourceGeometryHash`),
    sourcePlanHash: hash(value.sourcePlanHash, `${path}.sourcePlanHash`),
    catalogHash: hash(value.catalogHash, `${path}.catalogHash`),
    generationPolicyHash: hash(value.generationPolicyHash, `${path}.generationPolicyHash`),
    provenanceHash: hash(value.provenanceHash, `${path}.provenanceHash`),
  };
}

function decodeEvent(input: unknown, path: string): CatalogGenerationTraceEvent {
  const value = record(input, path);
  exactKeys(value, ['index', 'attempt', 'previousHash', 'eventHash', 'body'], path);
  return {
    index: integer(value.index, `${path}.index`, 0, HARD_MAX_EVENTS - 1),
    attempt: nullableInteger(value.attempt, `${path}.attempt`, 0, 63),
    previousHash: hash(value.previousHash, `${path}.previousHash`),
    eventHash: hash(value.eventHash, `${path}.eventHash`),
    body: decodeEventBody(value.body, `${path}.body`),
  };
}

function decodeEventBody(input: unknown, path: string): CatalogGenerationEventBody {
  const value = record(input, path);
  const type = nonEmptyString(value.type, `${path}.type`);
  switch (type) {
    case 'input_bound':
      exactKeys(value, ['type', 'inputHashes'], path);
      return {
        type,
        inputHashes: decodeInputHashes(value.inputHashes, `${path}.inputHashes`),
      };
    case 'attempt_started':
      exactKeys(value, ['type', 'roomSlackCells'], path);
      return {
        type,
        roomSlackCells: integer(
          value.roomSlackCells,
          `${path}.roomSlackCells`,
          -1_000_000,
          1_000_000,
        ),
      };
    case 'room_domain_evaluated':
      exactKeys(value, ['type', 'pieceId', 'requirementKind', 'candidates'], path);
      return {
        type,
        pieceId: nonEmptyString(value.pieceId, `${path}.pieceId`),
        requirementKind: nonEmptyString(value.requirementKind, `${path}.requirementKind`),
        candidates: array(value.candidates, `${path}.candidates`).map((candidate, index) =>
          decodeRoomCandidate(candidate, `${path}.candidates[${index}]`)),
      };
    case 'room_placed':
      exactKeys(value, ['type', 'placement'], path);
      return {
        type,
        placement: decodeRoomPlacement(value.placement, `${path}.placement`),
      };
    case 'room_conflict':
      exactKeys(value, ['type', 'pieceId', 'conflictingCells'], path);
      return {
        type,
        pieceId: nonEmptyString(value.pieceId, `${path}.pieceId`),
        conflictingCells: decodeCells(value.conflictingCells, `${path}.conflictingCells`),
      };
    case 'section_routing_started':
      exactKeys(value, ['type', 'sectionId', 'start', 'goal', 'guide', 'bounds'], path);
      return {
        type,
        sectionId: nonEmptyString(value.sectionId, `${path}.sectionId`),
        start: decodeCell(value.start, `${path}.start`),
        goal: decodeCell(value.goal, `${path}.goal`),
        guide: decodeCells(value.guide, `${path}.guide`),
        bounds: decodeBounds(value.bounds, `${path}.bounds`),
      };
    case 'section_routing_finished': {
      exactKeys(value, ['type', 'sectionId', 'status', 'cells', 'statesVisited'], path);
      const status = nonEmptyString(value.status, `${path}.status`);
      if (status !== 'found' && status !== 'no_path' && status !== 'budget_exhausted') {
        fail(`${path}.status`, `unsupported routing status ${status}`);
      }
      return {
        type,
        sectionId: nonEmptyString(value.sectionId, `${path}.sectionId`),
        status,
        cells: decodeCells(value.cells, `${path}.cells`),
        statesVisited: integer(
          value.statesVisited,
          `${path}.statesVisited`,
          0,
          1_000_000,
        ),
      };
    }
    case 'validation_completed':
      exactKeys(value, ['type', 'stage', 'ok', 'subjectHash', 'diagnosticCodes'], path);
      return {
        type,
        stage: nonEmptyString(value.stage, `${path}.stage`),
        ok: boolean(value.ok, `${path}.ok`),
        subjectHash: hash(value.subjectHash, `${path}.subjectHash`),
        diagnosticCodes: array(value.diagnosticCodes, `${path}.diagnosticCodes`).map(
          (code, index) => nonEmptyString(code, `${path}.diagnosticCodes[${index}]`),
        ),
      };
    case 'attempt_finished':
      exactKeys(value, [
        'type', 'classification', 'stage', 'detail', 'roomsPlaced',
        'sectionsRouted', 'routingStates',
      ], path);
      return {
        type,
        classification: nonEmptyString(value.classification, `${path}.classification`),
        stage: nonEmptyString(value.stage, `${path}.stage`),
        detail: nonEmptyString(value.detail, `${path}.detail`),
        roomsPlaced: integer(value.roomsPlaced, `${path}.roomsPlaced`, 0, HARD_MAX_VISUAL_CELLS),
        sectionsRouted: integer(
          value.sectionsRouted,
          `${path}.sectionsRouted`,
          0,
          HARD_MAX_EVENTS,
        ),
        routingStates: integer(
          value.routingStates,
          `${path}.routingStates`,
          0,
          Number.MAX_SAFE_INTEGER,
        ),
      };
    case 'run_finished':
      exactKeys(value, [
        'type', 'selectedAttempt', 'classification', 'reason', 'outputHash',
      ], path);
      return {
        type,
        selectedAttempt: nullableInteger(value.selectedAttempt, `${path}.selectedAttempt`, 0, 63),
        classification: nonEmptyString(value.classification, `${path}.classification`),
        reason: nonEmptyString(value.reason, `${path}.reason`),
        outputHash: hash(value.outputHash, `${path}.outputHash`),
      };
    default:
      fail(`${path}.type`, `unsupported event type ${type}`);
  }
}

function decodeRoomCandidate(input: unknown, path: string): CatalogGenerationRoomCandidate {
  const value = record(input, path);
  exactKeys(value, ['shapeId', 'transform', 'score', 'rank'], path);
  return {
    shapeId: nonEmptyString(value.shapeId, `${path}.shapeId`),
    transform: nonEmptyString(value.transform, `${path}.transform`),
    score: integer(value.score, `${path}.score`, -2_147_483_648, 2_147_483_647),
    rank: integer(value.rank, `${path}.rank`, 0, Number.MAX_SAFE_INTEGER),
  };
}

function decodeRoomPlacement(input: unknown, path: string): CatalogGenerationRoomPlacement {
  const value = record(input, path);
  exactKeys(value, [
    'pieceId', 'requirementKind', 'shapeId', 'transform', 'origin',
    'occupiedCells', 'reservedCells',
  ], path);
  return {
    pieceId: nonEmptyString(value.pieceId, `${path}.pieceId`),
    requirementKind: nonEmptyString(value.requirementKind, `${path}.requirementKind`),
    shapeId: nonEmptyString(value.shapeId, `${path}.shapeId`),
    transform: nonEmptyString(value.transform, `${path}.transform`),
    origin: decodeCell(value.origin, `${path}.origin`),
    occupiedCells: decodeCells(value.occupiedCells, `${path}.occupiedCells`),
    reservedCells: decodeCells(value.reservedCells, `${path}.reservedCells`),
  };
}

function decodeCell(input: unknown, path: string): CatalogGridCell {
  const value = record(input, path);
  exactKeys(value, ['x', 'y'], path);
  return {
    x: integer(value.x, `${path}.x`, -2_147_483_648, 2_147_483_647),
    y: integer(value.y, `${path}.y`, -2_147_483_648, 2_147_483_647),
  };
}

function decodeCells(input: unknown, path: string): readonly CatalogGridCell[] {
  return array(input, path).map((cell, index) => decodeCell(cell, `${path}[${index}]`));
}

function decodeBounds(input: unknown, path: string): CatalogGridBounds {
  const value = record(input, path);
  exactKeys(value, ['minX', 'maxX', 'minY', 'maxY'], path);
  const bounds = {
    minX: integer(value.minX, `${path}.minX`, -2_147_483_648, 2_147_483_647),
    maxX: integer(value.maxX, `${path}.maxX`, -2_147_483_648, 2_147_483_647),
    minY: integer(value.minY, `${path}.minY`, -2_147_483_648, 2_147_483_647),
    maxY: integer(value.maxY, `${path}.maxY`, -2_147_483_648, 2_147_483_647),
  };
  if (bounds.minX > bounds.maxX || bounds.minY > bounds.maxY) {
    fail(path, 'has inverted limits');
  }
  return bounds;
}

function decodeSelection(input: unknown, path: string): CatalogGenerationTraceSelection {
  const value = record(input, path);
  exactKeys(value, ['selectedAttempt', 'classification', 'reason'], path);
  return {
    selectedAttempt: nullableInteger(value.selectedAttempt, `${path}.selectedAttempt`, 0, 63),
    classification: nonEmptyString(value.classification, `${path}.classification`),
    reason: nonEmptyString(value.reason, `${path}.reason`),
  };
}

function decodeAttemptEvidence(input: unknown, path: string): CatalogAwareAttemptEvidence {
  const value = record(input, path);
  exactKeys(value, [
    'attempt', 'roomSlackCells', 'classification', 'stage', 'detail',
    'roomsPlaced', 'sectionsRouted', 'routingStates',
  ], path);
  return {
    attempt: integer(value.attempt, `${path}.attempt`, 0, 63),
    roomSlackCells: integer(
      value.roomSlackCells,
      `${path}.roomSlackCells`,
      -1_000_000,
      1_000_000,
    ),
    classification: nonEmptyString(value.classification, `${path}.classification`),
    stage: nonEmptyString(value.stage, `${path}.stage`),
    detail: nonEmptyString(value.detail, `${path}.detail`),
    roomsPlaced: integer(value.roomsPlaced, `${path}.roomsPlaced`, 0, HARD_MAX_VISUAL_CELLS),
    sectionsRouted: integer(value.sectionsRouted, `${path}.sectionsRouted`, 0, HARD_MAX_EVENTS),
    routingStates: integer(
      value.routingStates,
      `${path}.routingStates`,
      0,
      Number.MAX_SAFE_INTEGER,
    ),
  };
}

function validateEventSequence(
  trace: CatalogGenerationTrace,
  result: DecodedResult,
  outputHash: string,
): readonly CatalogGenerationAttempt[] {
  let previousHash = trace.rootHash;
  let bodyBytes = 0;
  let visualCells = 0;
  let current: MutableAttemptState | null = null;
  let inputBound = false;
  let runFinished = false;
  const attempts: CatalogGenerationAttempt[] = [];
  const attemptEventIndices: number[][] = [];

  trace.events.forEach((event, position) => {
    if (runFinished) {
      fail(`trace.events[${position}]`, 'appears after run_finished');
    }
    if (event.index !== position) {
      fail(`trace.events[${position}].index`, `expected ${position}`);
    }
    if (event.previousHash !== previousHash) {
      fail(`trace.events[${position}].previousHash`, `expected ${previousHash}`);
    }
    const expectedHash = fnv1a64Json({
      index: event.index,
      attempt: event.attempt,
      previousHash: event.previousHash,
      body: event.body,
    });
    if (event.eventHash !== expectedHash) {
      fail(`trace.events[${position}].eventHash`, `does not match ${expectedHash}`);
    }
    bodyBytes = checkedAdd(
      bodyBytes,
      UTF8.encode(JSON.stringify(event.body)).length,
      'trace.eventBodyBytes',
    );
    visualCells = checkedAdd(
      visualCells,
      eventVisualCells(event.body),
      'trace.visualCellCount',
    );

    if (event.body.type === 'input_bound') {
      if (position !== 0 || event.attempt !== null || inputBound) {
        fail(`trace.events[${position}]`, 'input_bound must be the unique first run event');
      }
      if (JSON.stringify(event.body.inputHashes) !== JSON.stringify(trace.inputHashes)) {
        fail(`trace.events[${position}].body.inputHashes`, 'does not match trace inputs');
      }
      inputBound = true;
    } else if (event.body.type === 'attempt_started') {
      if (!inputBound || current !== null || event.attempt !== attempts.length) {
        fail(`trace.events[${position}]`, 'attempt_started is out of sequence');
      }
      const attempt = requiredNumber(event.attempt, `${position}.attempt`);
      const expectedSlack = trace.generationPolicy.initialRoomSlackCells
        + trace.generationPolicy.roomSlackGrowthCells * attempt;
      if (
        !Number.isSafeInteger(expectedSlack)
        || event.body.roomSlackCells !== expectedSlack
      ) {
        fail(`trace.events[${position}].body.roomSlackCells`, `expected ${expectedSlack}`);
      }
      current = {
        attempt,
        roomSlackCells: event.body.roomSlackCells,
        domains: new Map(),
        rooms: new Map(),
        routes: new Map(),
        pending: new Map(),
        routingStates: 0,
      };
      attemptEventIndices[attempt] = [position];
    } else if (event.body.type === 'run_finished') {
      if (!inputBound || current !== null || event.attempt !== null) {
        fail(`trace.events[${position}]`, 'run_finished has an active attempt');
      }
      if (
        event.body.selectedAttempt !== trace.selection.selectedAttempt
        || event.body.classification !== trace.selection.classification
        || event.body.reason !== trace.selection.reason
        || event.body.outputHash !== outputHash
      ) {
        fail(`trace.events[${position}].body`, 'does not match output/selection binding');
      }
      runFinished = true;
    } else {
      if (current === null || event.attempt !== current.attempt) {
        fail(`trace.events[${position}].attempt`, 'does not name the active attempt');
      }
      required(attemptEventIndices, current.attempt, 'attempt event list').push(position);
      applyAttemptEvent(event, current, result, position);
      if (event.body.type === 'attempt_finished') {
        const evidence = required(result.attempts, current.attempt, 'result attempt');
        attempts.push({
          attempt: current.attempt,
          evidence,
          eventIndices: required(
            attemptEventIndices,
            current.attempt,
            'attempt event list',
          ),
          finalRooms: [...current.rooms.values()],
          finalRoutes: [...current.routes.values()],
        });
        current = null;
      }
    }
    previousHash = event.eventHash;
  });

  if (!inputBound || !runFinished || current !== null) {
    fail('trace.events', 'does not form a complete input-to-result run');
  }
  if (attempts.length !== result.attempts.length) {
    fail('trace.events', 'attempt count does not match the result');
  }
  if (bodyBytes !== trace.eventBodyBytes || bodyBytes > trace.limits.maxEventBodyBytes) {
    fail('trace.eventBodyBytes', `expected ${bodyBytes}, limit ${trace.limits.maxEventBodyBytes}`);
  }
  if (visualCells !== trace.visualCellCount || visualCells > trace.limits.maxVisualCells) {
    fail('trace.visualCellCount', `expected ${visualCells}, limit ${trace.limits.maxVisualCells}`);
  }
  if (previousHash !== trace.finalEventHash) {
    fail('trace.finalEventHash', `expected ${previousHash}`);
  }
  return attempts;
}

function applyAttemptEvent(
  event: CatalogGenerationTraceEvent,
  state: MutableAttemptState,
  result: DecodedResult,
  position: number,
): void {
  const body = event.body;
  switch (body.type) {
    case 'room_domain_evaluated': {
      if (
        body.candidates.length === 0
        || body.candidates.length > result.policy.maxRoomCandidates
        || state.domains.has(body.pieceId)
      ) {
        fail(`trace.events[${position}].body.candidates`, 'invalid or duplicate room domain');
      }
      let priorRank = -1;
      body.candidates.forEach((candidate, index) => {
        if (candidate.rank <= priorRank) {
          fail(
            `trace.events[${position}].body.candidates[${index}].rank`,
            `must be greater than ${priorRank}`,
          );
        }
        priorRank = candidate.rank;
      });
      state.domains.set(
        body.pieceId,
        new Set(body.candidates.map((candidate) =>
          `${candidate.shapeId}\u0000${candidate.transform}`)),
      );
      break;
    }
    case 'room_placed': {
      const placement = body.placement;
      if (
        !state.domains.get(placement.pieceId)?.has(
          `${placement.shapeId}\u0000${placement.transform}`,
        )
        || state.rooms.has(placement.pieceId)
      ) {
        fail(`trace.events[${position}].body.placement`, 'is outside its domain or duplicate');
      }
      uniqueCells(placement.occupiedCells, `${position}.placement.occupiedCells`);
      uniqueCells(placement.reservedCells, `${position}.placement.reservedCells`);
      state.rooms.set(placement.pieceId, placement);
      break;
    }
    case 'room_conflict':
      if (
        !state.domains.has(body.pieceId)
        || body.conflictingCells.length === 0
      ) {
        fail(`trace.events[${position}].body`, 'room conflict has no domain or cells');
      }
      uniqueCells(body.conflictingCells, `${position}.conflictingCells`);
      break;
    case 'section_routing_started':
      if (state.pending.has(body.sectionId) || state.routes.has(body.sectionId)) {
        fail(`trace.events[${position}].body.sectionId`, 'section starts more than once');
      }
      for (const [label, cell] of [
        ['start', body.start],
        ['goal', body.goal],
        ...body.guide.map((cell, index) => [`guide[${index}]`, cell] as const),
      ] as const) {
        if (!cellInBounds(cell, body.bounds)) {
          fail(`trace.events[${position}].body.${label}`, 'lies outside routing bounds');
        }
      }
      state.pending.set(body.sectionId, {
        start: body.start,
        goal: body.goal,
        guide: body.guide,
        bounds: body.bounds,
      });
      break;
    case 'section_routing_finished': {
      const started = state.pending.get(body.sectionId);
      if (started === undefined) {
        fail(`trace.events[${position}].body.sectionId`, 'finishes without a start');
      }
      state.pending.delete(body.sectionId);
      state.routingStates = checkedAdd(
        state.routingStates,
        body.statesVisited,
        `${position}.routingStates`,
      );
      if (body.status === 'found') {
        if (
          body.cells.length === 0
          || !sameCell(body.cells[0], started.start)
          || !sameCell(body.cells.at(-1), started.goal)
        ) {
          fail(`trace.events[${position}].body.cells`, 'does not preserve ordered endpoints');
        }
        uniqueCells(body.cells, `${position}.route.cells`);
        for (let index = 1; index < body.cells.length; index += 1) {
          const prior = required(body.cells, index - 1, 'prior route cell');
          const next = required(body.cells, index, 'route cell');
          if (Math.abs(prior.x - next.x) + Math.abs(prior.y - next.y) !== 1) {
            fail(`trace.events[${position}].body.cells[${index}]`, 'is not cardinally adjacent');
          }
        }
        state.routes.set(body.sectionId, {
          sectionId: body.sectionId,
          cells: body.cells,
          statesVisited: body.statesVisited,
        });
      } else if (body.cells.length !== 0) {
        fail(`trace.events[${position}].body.cells`, `${body.status} must have no route cells`);
      }
      break;
    }
    case 'validation_completed': {
      const expected = resultValidationEvidence(result, body.stage);
      if (
        body.subjectHash !== fnv1a64Json(expected.subject)
        || body.ok !== expected.ok
        || JSON.stringify(body.diagnosticCodes) !== JSON.stringify(expected.diagnosticCodes)
      ) {
        fail(
          `trace.events[${position}].body`,
          `does not match the ${body.stage} result evidence`,
        );
      }
      break;
    }
    case 'attempt_finished': {
      if (state.pending.size !== 0) {
        fail(`trace.events[${position}]`, 'attempt finishes with pending routes');
      }
      const expected = required(result.attempts, state.attempt, 'result attempt');
      const observed: CatalogAwareAttemptEvidence = {
        attempt: state.attempt,
        roomSlackCells: state.roomSlackCells,
        classification: body.classification,
        stage: body.stage,
        detail: body.detail,
        roomsPlaced: body.roomsPlaced,
        sectionsRouted: body.sectionsRouted,
        routingStates: body.routingStates,
      };
      if (
        JSON.stringify(observed) !== JSON.stringify(expected)
        || state.rooms.size !== body.roomsPlaced
        || state.routes.size !== body.sectionsRouted
        || state.routingStates !== body.routingStates
      ) {
        fail(`trace.events[${position}].body`, 'attempt evidence/state does not match result');
      }
      break;
    }
    default:
      fail(`trace.events[${position}].body`, `unexpected ${body.type} in active attempt`);
  }
}

function resultValidationEvidence(
  result: DecodedResult,
  stage: string,
): {
  readonly subject: Readonly<Record<string, unknown>>;
  readonly ok: boolean;
  readonly diagnosticCodes: readonly string[];
} {
  const [subject, report] = (() => {
    switch (stage) {
      case 'geometry_validation':
        return [result.geometry, result.geometryValidation] as const;
      case 'placement_validation':
        return [result.placement, result.placementValidation] as const;
      case 'built_flow_validation':
        return [result.builtFlowValidation, result.builtFlowValidation] as const;
      default:
        fail('validation.stage', `unsupported stage ${stage}`);
    }
  })();
  if (subject === null || report === null) {
    fail('validation', `${stage} has no successful result evidence`);
  }
  return {
    subject,
    ok: boolean(report.ok, `result.${stage}.ok`),
    diagnosticCodes: array(report.diagnostics, `result.${stage}.diagnostics`).map(
      (diagnostic, index) => nonEmptyString(
        record(diagnostic, `result.${stage}.diagnostics[${index}]`).code,
        `result.${stage}.diagnostics[${index}].code`,
      ),
    ),
  };
}

function validateSelectedOutput(
  attempts: readonly CatalogGenerationAttempt[],
  result: DecodedResult,
): void {
  if (!result.ok) {
    return;
  }
  const attempt = attempts.find((candidate) => candidate.attempt === result.selectedAttempt);
  if (attempt === undefined || attempt.evidence.classification !== 'success') {
    fail('trace.selection.selectedAttempt', 'does not identify a successful attempt');
  }
  const placement = requiredRecord(result.placement, 'result.placement');
  const instances = array(placement.instances, 'result.placement.instances');
  const expectedRooms = instances
    .map((instance, index) => {
      const item = record(instance, `result.placement.instances[${index}]`);
      const kind = nonEmptyString(
        item.requirementKind,
        `result.placement.instances[${index}].requirementKind`,
      );
      return ['connector', 'corridor', 'bend', 'junction'].includes(kind)
        ? null
        : decodePlacementInstance(item, `result.placement.instances[${index}]`);
    })
    .filter((room): room is CatalogGenerationRoomPlacement => room !== null)
    .sort((left, right) => left.pieceId.localeCompare(right.pieceId));
  const actualRooms = [...attempt.finalRooms]
    .sort((left, right) => left.pieceId.localeCompare(right.pieceId));
  if (JSON.stringify(actualRooms) !== JSON.stringify(expectedRooms)) {
    fail('trace.selectedAttempt.rooms', 'does not match the selected result placement');
  }
  const plan = requiredRecord(result.piecePlan, 'result.piecePlan');
  const expectedSections = [...new Set(
    array(plan.links, 'result.piecePlan.links')
      .map((link, index) => nonEmptyString(
        record(link, `result.piecePlan.links[${index}]`).sourceSection,
        `result.piecePlan.links[${index}].sourceSection`,
      )),
  )].sort();
  const actualSections = attempt.finalRoutes.map((route) => route.sectionId).sort();
  if (JSON.stringify(actualSections) !== JSON.stringify(expectedSections)) {
    fail('trace.selectedAttempt.routes', 'does not match the selected result sections');
  }
  const expectedRoutes = selectedResultRoutes(placement, expectedSections);
  const actualRoutes = [...attempt.finalRoutes]
    .map((route) => ({ sectionId: route.sectionId, cells: route.cells }))
    .sort((left, right) => left.sectionId.localeCompare(right.sectionId));
  if (JSON.stringify(actualRoutes) !== JSON.stringify(expectedRoutes)) {
    fail('trace.selectedAttempt.routes', 'does not match the selected result route cells');
  }
}

function selectedResultRoutes(
  placement: Readonly<Record<string, unknown>>,
  expectedSections: readonly string[],
): readonly {
  readonly sectionId: string;
  readonly cells: readonly CatalogGridCell[];
}[] {
  const bySection = new Map<string, { readonly tile: number; readonly cell: CatalogGridCell }[]>();
  array(placement.instances, 'result.placement.instances').forEach((instance, index) => {
    const path = `result.placement.instances[${index}]`;
    const item = record(instance, path);
    if (item.role !== 'catalog_route') {
      return;
    }
    const pieceId = nonEmptyString(item.pieceId, `${path}.pieceId`);
    const tileMatch = /\.tile_(\d+)$/.exec(pieceId);
    if (tileMatch === null) {
      fail(`${path}.pieceId`, 'catalog route has no ordered tile suffix');
    }
    const sourceSections = array(item.sourceRefs, `${path}.sourceRefs`)
      .map((source, sourceIndex) =>
        nonEmptyString(source, `${path}.sourceRefs[${sourceIndex}]`))
      .filter((source) => source.startsWith('physicalSection:'));
    if (sourceSections.length !== 1) {
      fail(`${path}.sourceRefs`, 'catalog route must name exactly one physical section');
    }
    const sectionId = required(sourceSections, 0, 'catalog route section')
      .slice('physicalSection:'.length);
    const routes = bySection.get(sectionId) ?? [];
    routes.push({
      tile: Number(required(tileMatch, 1, 'catalog route tile')),
      cell: decodeCell(item.origin, `${path}.origin`),
    });
    bySection.set(sectionId, routes);
  });
  const actualSections = [...bySection.keys()].sort();
  if (JSON.stringify(actualSections) !== JSON.stringify(expectedSections)) {
    fail(
      'result.placement.instances',
      'catalog route instances do not match the selected result sections',
    );
  }
  return actualSections.map((sectionId) => {
    const routeTiles = bySection.get(sectionId);
    if (routeTiles === undefined) {
      fail('result.placement.instances', `missing catalog route ${sectionId}`);
    }
    const tiles = routeTiles
      .sort((left, right) => left.tile - right.tile);
    tiles.forEach((tile, index) => {
      if (tile.tile !== index + 1) {
        fail(
          'result.placement.instances',
          `${sectionId} catalog route tile ${tile.tile} is out of sequence`,
        );
      }
    });
    return {
      sectionId,
      cells: tiles.map((tile) => tile.cell),
    };
  });
}

function decodePlacementInstance(
  value: Readonly<Record<string, unknown>>,
  path: string,
): CatalogGenerationRoomPlacement {
  return {
    pieceId: nonEmptyString(value.pieceId, `${path}.pieceId`),
    requirementKind: nonEmptyString(value.requirementKind, `${path}.requirementKind`),
    shapeId: nonEmptyString(value.shapeId, `${path}.shapeId`),
    transform: nonEmptyString(value.transform, `${path}.transform`),
    origin: decodeCell(value.origin, `${path}.origin`),
    occupiedCells: decodeCells(value.occupiedCells, `${path}.occupiedCells`),
    reservedCells: decodeCells(value.reservedCells, `${path}.reservedCells`),
  };
}

function eventVisualCells(body: CatalogGenerationEventBody): number {
  switch (body.type) {
    case 'room_placed':
      return body.placement.occupiedCells.length + body.placement.reservedCells.length;
    case 'room_conflict':
      return body.conflictingCells.length;
    case 'section_routing_started':
      return body.guide.length;
    case 'section_routing_finished':
      return body.cells.length;
    default:
      return 0;
  }
}

function eventStage(body: CatalogGenerationEventBody): string {
  switch (body.type) {
    case 'attempt_started':
      return 'attempt';
    case 'room_domain_evaluated':
    case 'room_placed':
    case 'room_conflict':
      return 'rooms';
    case 'section_routing_started':
    case 'section_routing_finished':
      return 'routing';
    case 'validation_completed':
      return body.stage;
    case 'attempt_finished':
      return 'result';
    default:
      return body.type;
  }
}

function samePolicy(
  left: CatalogAwareGenerationPolicy,
  right: CatalogAwareGenerationPolicy,
): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function cellInBounds(cell: CatalogGridCell, bounds: CatalogGridBounds): boolean {
  return cell.x >= bounds.minX
    && cell.x <= bounds.maxX
    && cell.y >= bounds.minY
    && cell.y <= bounds.maxY;
}

function sameCell(
  left: CatalogGridCell | undefined,
  right: CatalogGridCell,
): boolean {
  return left?.x === right.x && left.y === right.y;
}

function uniqueCells(cells: readonly CatalogGridCell[], path: string): void {
  const unique = new Set(cells.map((cell) => `${cell.x},${cell.y}`));
  if (unique.size !== cells.length) {
    fail(path, 'contains duplicate cells');
  }
}

function checkedAdd(left: number, right: number, path: string): number {
  const result = left + right;
  if (!Number.isSafeInteger(result) || result < 0) {
    fail(path, 'numeric accounting overflowed');
  }
  return result;
}

function record(value: unknown, path: string): Readonly<Record<string, unknown>> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    fail(path, 'must be an object');
  }
  return value as Readonly<Record<string, unknown>>;
}

function nullableRecord(
  value: unknown,
  path: string,
): Readonly<Record<string, unknown>> | null {
  return value === null ? null : record(value, path);
}

function requiredRecord(
  value: Readonly<Record<string, unknown>> | null,
  path: string,
): Readonly<Record<string, unknown>> {
  if (value === null) {
    fail(path, 'must be present');
  }
  return value;
}

function array(value: unknown, path: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    fail(path, 'must be an array');
  }
  return value;
}

function exactKeys(
  value: Readonly<Record<string, unknown>>,
  expected: readonly string[],
  path: string,
): void {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) {
    fail(path, `must contain exactly ${expected.join(', ')}`);
  }
}

function nonEmptyString(value: unknown, path: string): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > 4_096) {
    fail(path, 'must be a non-empty bounded string');
  }
  return value;
}

function nullableString(value: unknown, path: string): string | null {
  return value === null ? null : nonEmptyString(value, path);
}

function boolean(value: unknown, path: string): boolean {
  if (typeof value !== 'boolean') {
    fail(path, 'must be a boolean');
  }
  return value;
}

function integer(value: unknown, path: string, minimum: number, maximum: number): number {
  if (
    typeof value !== 'number'
    || !Number.isSafeInteger(value)
    || value < minimum
    || value > maximum
  ) {
    fail(path, `must be an integer from ${minimum} through ${maximum}`);
  }
  return value;
}

function nullableInteger(
  value: unknown,
  path: string,
  minimum: number,
  maximum: number,
): number | null {
  return value === null ? null : integer(value, path, minimum, maximum);
}

function requiredNumber(value: number | null, path: string): number {
  if (value === null) {
    fail(path, 'must be present');
  }
  return value;
}

function hash(value: unknown, path: string): string {
  const encoded = nonEmptyString(value, path);
  if (!/^fnv1a64:[0-9a-f]{16}$/.test(encoded)) {
    fail(path, 'must be a canonical FNV-1a 64 hash');
  }
  return encoded;
}

function literal<T extends string | number>(
  value: unknown,
  expected: T,
  path: string,
): asserts value is T {
  if (value !== expected) {
    fail(path, `expected ${JSON.stringify(expected)}`);
  }
}

function required<T>(values: readonly T[], index: number, path: string): T {
  const value = values[index];
  if (value === undefined) {
    fail(path, `missing index ${index}`);
  }
  return value;
}

function fail(path: string, detail: string): never {
  throw new Error(`${path}: ${detail}`);
}
