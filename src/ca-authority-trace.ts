import {
  decodeRenderFrameDiff,
  renderHandle,
  type EditorGridDescriptor,
  type MeshPayloadDescriptor,
  type RenderDiff,
  type RenderFrameDiff,
} from '@rusty-engine/render-contracts';

import {
  fnv1a64Json,
  sha256Json,
  sha256Serialized,
} from './ca-trace-hash.js';

const CA_BENCHMARK_KIND = 'rusty_procgen.evidence.engine_ca_benchmark.v1';
const CA_TRACE_KIND = 'rusty_procgen.engine_ca_authority_trace.v1';
const MAX_SCENARIOS = 32;
const MAX_STEPS = 4_096;
const MAX_RECORDED_RUNS = 8;
const MAX_PROJECTION_CHUNKS = 4_096;
const MAX_SEED_CELLS = 4_096;
const MAX_CELL_STEPS = 1_048_576;
const MATERIALS = [
  { slot: 1, id: 'rusty-procgen/ca/source', color: [0.94, 0.69, 0.22, 1] },
  { slot: 2, id: 'rusty-procgen/ca/frontier', color: [0.18, 0.76, 0.67, 1] },
  { slot: 3, id: 'rusty-procgen/ca/trail', color: [0.25, 0.47, 0.84, 1] },
  { slot: 4, id: 'rusty-procgen/ca/resident-empty', color: [0.19, 0.23, 0.28, 1] },
] as const;

type Coord = readonly [number, number, number];

export interface CaBounds {
  readonly min: { readonly x: number; readonly y: number; readonly z: number };
  readonly maxExclusive: { readonly x: number; readonly y: number; readonly z: number };
}

export interface CaSeedCell {
  readonly coord: { readonly x: number; readonly y: number; readonly z: number };
  readonly state: 'source' | 'frontier' | 'trail';
}

export interface CaMeshGroup {
  readonly materialSlot: number;
  readonly start: number;
  readonly count: number;
}

export interface CaMeshChunk {
  readonly chunk: Coord;
  readonly contentHash: string;
  readonly bufferHash: string;
  readonly translation: Coord;
  readonly positions: readonly number[];
  readonly normals: readonly number[];
  readonly indices: readonly number[];
  readonly groups: readonly CaMeshGroup[];
  readonly boundsMin: Coord;
  readonly boundsMax: Coord;
  readonly vertices: number;
  readonly quads: number;
  readonly facesCulled: number;
}

export interface CaAuthorityReadout {
  readonly sourceRevision: number;
  readonly authorityHash: string;
  readonly projectionRevisionsCoherent: boolean;
  readonly solidVoxelCount: number;
  readonly residentChunkCount: number;
  readonly colliderChunkCount: number;
  readonly navigationCellCount: number;
  readonly navigationHash: string;
  readonly meshChunkCount: number;
  readonly meshVertexCount: number;
  readonly meshQuadCount: number;
  readonly meshProjectionHash: string;
}

export type CaProjectionOp =
  | { readonly op: 'upsert'; readonly chunk: CaMeshChunk }
  | { readonly op: 'delete'; readonly chunk: Coord };

export interface CaStepEvidence {
  readonly step: number;
  readonly activeCellCount: number;
  readonly changedCellCount: number;
  readonly evaluatedCellCount: number;
  readonly touchedBounds: {
    readonly min: { readonly x: number; readonly y: number; readonly z: number };
    readonly maxInclusive: { readonly x: number; readonly y: number; readonly z: number };
  } | null;
  readonly stateCounts: {
    readonly empty: number;
    readonly source: number;
    readonly frontier: number;
    readonly trail: number;
  };
  readonly deltas: readonly {
    readonly coord: { readonly x: number; readonly y: number; readonly z: number };
    readonly previous: string;
    readonly current: string;
  }[];
  readonly deltaHash: string;
  readonly stateHash: string;
  readonly cumulativeScenarioHash: string;
}

export interface CaSpatialStep {
  readonly ca: CaStepEvidence;
  readonly revisionBefore: number;
  readonly acceptedRevision: number;
  readonly engineChangedVoxels: number;
  readonly canonicalEditCount: number;
  readonly engineDeltaCount: number;
  readonly readout: CaAuthorityReadout;
  readonly projectionOps: readonly CaProjectionOp[];
  readonly projectionDeltaHash: string;
  readonly projectionStateHash: string;
  readonly previousTraceHash: string;
  readonly traceHash: string;
}

export interface CaTiming {
  readonly caStepNs: number;
  readonly requestConstructionNs: number;
  readonly spatialPreviewNs: number;
  readonly authorityCommitNs: number;
  readonly evidenceReadbackNs: number;
  readonly artifactEncodingNs: number;
}

export interface CaRecordedRun {
  readonly run: number;
  readonly structuralHash: string;
  readonly admissionTiming: {
    readonly stateMaterializationNs: number;
    readonly engineBuildNs: number;
    readonly evidenceReadbackNs: number;
    readonly artifactEncodingNs: number;
  };
  readonly stepTimings: readonly CaTiming[];
  readonly encodedTraceBytes: number;
}

export interface CaScenarioEvidence {
  readonly scenarioId: string;
  readonly warmupRuns: number;
  readonly deterministicStructuralEvidence: boolean;
  readonly recordedRuns: readonly CaRecordedRun[];
  readonly trace: {
    readonly kind: string;
    readonly schemaVersion: number;
    readonly scenarioId: string;
    readonly workload: string;
    readonly ruleId: string;
    readonly seed: number;
    readonly bounds: CaBounds;
    readonly neighborhood: string;
    readonly boundary: string;
    readonly materializeEmpty: boolean;
    readonly initialCells: readonly CaSeedCell[];
    readonly initial: {
      readonly initialCaStateHash: string;
      readonly initialCaCumulativeHash: string;
      readonly readout: CaAuthorityReadout;
      readonly projectionChunks: readonly CaMeshChunk[];
      readonly projectionStateHash: string;
      readonly traceHash: string;
    };
    readonly steps: readonly CaSpatialStep[];
  };
}

export interface CaBenchmarkEvidence {
  readonly kind: string;
  readonly schemaVersion: number;
  readonly repositoryCommit: string;
  readonly engineCommit: string;
  readonly environment: {
    readonly operatingSystem: string;
    readonly architecture: string;
    readonly rustcVersion: string;
    readonly buildProfile: string;
    readonly clock: string;
  };
  readonly config: {
    readonly warmupRuns: number;
    readonly recordedRuns: number;
    readonly chunkSize: number;
    readonly maxEditsPerStep: number;
    readonly maxMeshValuesPerStep: number;
  };
  readonly scenarios: readonly CaScenarioEvidence[];
  readonly nonClaims: readonly string[];
}

export interface CompiledCaScenario {
  readonly evidence: CaScenarioEvidence;
  readonly initialFrame: RenderFrameDiff;
  readonly stepFrames: readonly RenderFrameDiff[];
  readonly grid: EditorGridDescriptor;
  readonly camera: {
    readonly position: Coord;
    readonly target: Coord;
    readonly moveSpeed: number;
  };
  readonly chunkCountAfterStep: readonly number[];
}

export function decodeCaBenchmarkEvidence(input: unknown): CaBenchmarkEvidence {
  const root = record(input, 'evidence', [
    'kind', 'schemaVersion', 'repositoryCommit', 'engineCommit', 'environment',
    'config', 'scenarios', 'nonClaims',
  ]);
  exactText(root.kind, 'evidence.kind', CA_BENCHMARK_KIND);
  exactInteger(root.schemaVersion, 'evidence.schemaVersion', 1);
  commit(root.repositoryCommit, 'evidence.repositoryCommit');
  commit(root.engineCommit, 'evidence.engineCommit');
  decodeEnvironment(root.environment);
  const config = decodeConfig(root.config);
  const scenarios = list(root.scenarios, 'evidence.scenarios', 1, MAX_SCENARIOS);
  const ids = new Set<string>();
  for (let index = 0; index < scenarios.length; index += 1) {
    const scenario = decodeScenario(
      scenarios[index],
      `evidence.scenarios[${index}]`,
      config,
    );
    if (ids.has(scenario.scenarioId)) {
      fail(`evidence.scenarios[${index}].scenarioId`, 'is duplicated');
    }
    ids.add(scenario.scenarioId);
  }
  textList(root.nonClaims, 'evidence.nonClaims', 1, 32);
  return input as CaBenchmarkEvidence;
}

export function compileCaScenario(
  scenario: CaScenarioEvidence,
): CompiledCaScenario {
  const facts = allChunkFacts(scenario);
  const orderedFacts = [...facts.values()].sort((left, right) =>
    coordCompare(left.chunk, right.chunk) || left.bufferHash.localeCompare(right.bufferHash));
  const frameByHash = new Map<string, number>();
  for (const fact of orderedFacts) {
    frameByHash.set(fact.bufferHash, frameByHash.size);
  }
  const coordinateKeys = [...new Set(orderedFacts.map((fact) => coordKey(fact.chunk)))]
    .sort((left, right) => coordCompare(parseCoordKey(left), parseCoordKey(right)));
  const handleByCoord = new Map(
    coordinateKeys.map((key, index) => [key, renderHandle(index + 1)]),
  );
  const assetId = `rusty-procgen/ca/${scenario.scenarioId}`;
  const materialOps = MATERIALS.map<RenderDiff>((material) => ({
    op: 'defineMaterial',
    material: {
      schemaVersion: 1,
      id: material.id,
      color: material.color,
      texture: null,
      roughness: material.slot === 4 ? 0.92 : 0.68,
      textureTint: [1, 1, 1, 1],
      emissionColor: material.slot === 2 ? [0.03, 0.16, 0.13] : [0, 0, 0],
      emissionIntensity: material.slot === 2 ? 0.28 : 0,
      uvStrategy: 'flat',
    },
  }));
  const assetOp: RenderDiff = {
    op: 'defineVoxelObject',
    asset: {
      asset: assetId,
      contentHash: scenario.recordedRuns[0]?.structuralHash ?? scenario.trace.initial.traceHash,
      meshes: orderedFacts.map((fact) => ({ payload: meshPayload(fact) })),
      frames: orderedFacts.map((fact, index) => ({
        id: `${coordKey(fact.chunk)}:${fact.bufferHash}`,
        mesh: index,
      })),
      materialSlots: MATERIALS.map((material) => ({
        slot: material.slot,
        material: material.id,
      })),
    },
  };
  const initialInstances = scenario.trace.initial.projectionChunks.map<RenderDiff>((chunk) => ({
    op: 'createVoxelObjectInstance',
    handle: required(handleByCoord, coordKey(chunk.chunk), 'chunk handle'),
    parent: null,
    instance: instance(assetId, required(frameByHash, chunk.bufferHash, 'voxel frame'), chunk),
  }));
  const firstLightHandle = coordinateKeys.length + 1;
  const lights: readonly RenderDiff[] = [
    {
      op: 'createLight',
      handle: renderHandle(firstLightHandle),
      parent: null,
      light: {
        kind: 'ambient',
        color: [0.7, 0.78, 0.86],
        intensity: 0.82,
        enabled: true,
        shadowIntent: 'disabled',
      },
    },
    {
      op: 'createLight',
      handle: renderHandle(firstLightHandle + 1),
      parent: null,
      light: {
        kind: 'directional',
        color: [1, 0.91, 0.76],
        intensity: 1.55,
        enabled: true,
        direction: [-1, -2, -0.65],
        shadowIntent: 'disabled',
      },
    },
  ];
  const initialFrame = decodeRenderFrameDiff({
    schemaVersion: 1,
    ops: [...materialOps, assetOp, ...initialInstances, ...lights],
  });

  const active = new Set(
    scenario.trace.initial.projectionChunks.map((chunk) => coordKey(chunk.chunk)),
  );
  const chunkCountAfterStep = [active.size];
  const stepFrames = scenario.trace.steps.map((step) => {
    const ops = step.projectionOps.map<RenderDiff>((op) => {
      const key = coordKey(op.op === 'delete' ? op.chunk : op.chunk.chunk);
      const handle = required(handleByCoord, key, 'chunk handle');
      if (op.op === 'delete') {
        active.delete(key);
        return { op: 'destroy', handle };
      }
      const frame = required(frameByHash, op.chunk.bufferHash, 'voxel frame');
      if (active.has(key)) {
        return { op: 'setVoxelObjectFrame', handle, frame };
      }
      active.add(key);
      return {
        op: 'createVoxelObjectInstance',
        handle,
        parent: null,
        instance: instance(assetId, frame, op.chunk),
      };
    });
    chunkCountAfterStep.push(active.size);
    return decodeRenderFrameDiff({ schemaVersion: 1, ops });
  });

  const bounds = scenario.trace.bounds;
  const width = bounds.maxExclusive.x - bounds.min.x;
  const height = bounds.maxExclusive.y - bounds.min.y;
  const depth = bounds.maxExclusive.z - bounds.min.z;
  const radius = Math.max(width, height, depth, 8);
  const target = [
    bounds.min.x + width / 2,
    bounds.min.y + height / 2,
    bounds.min.z + depth / 2,
  ] as const;

  return {
    evidence: scenario,
    initialFrame,
    stepFrames,
    grid: {
      visible: true,
      grid: {
        coordinateSystem: 'rightHandedYUp',
        origin: [bounds.min.x, bounds.min.y + 0.002, bounds.min.z],
        spacing: [1, 1, 1],
      },
      plane: 'xz',
      snapAnchor: 'boundary',
      style: {
        minorColor: [0.32, 0.39, 0.46, 0.62],
        majorColor: [0.58, 0.66, 0.73, 0.86],
        xAxisColor: [0.82, 0.28, 0.24, 1],
        yAxisColor: [0.3, 0.76, 0.4, 1],
        zAxisColor: [0.27, 0.5, 0.94, 1],
        majorLineEvery: 5,
        opacity: 0.76,
        fadeStart: Math.max(radius, 16),
        fadeEnd: Math.max(radius * 3, 48),
      },
    },
    camera: {
      position: [
        target[0] + radius * 0.65,
        bounds.maxExclusive.y + radius * 0.72,
        target[2] + radius * 0.82,
      ],
      target,
      moveSpeed: Math.max(4, radius * 0.4),
    },
    chunkCountAfterStep,
  };
}

function decodeEnvironment(input: unknown): void {
  const value = record(input, 'evidence.environment', [
    'operatingSystem', 'architecture', 'rustcVersion', 'buildProfile', 'clock',
  ]);
  for (const key of Object.keys(value)) {
    text(value[key], `evidence.environment.${key}`);
  }
}

function decodeConfig(input: unknown): CaBenchmarkEvidence['config'] {
  const value = record(input, 'evidence.config', [
    'warmupRuns', 'recordedRuns', 'chunkSize', 'maxEditsPerStep',
    'maxMeshValuesPerStep',
  ]);
  integer(value.warmupRuns, 'evidence.config.warmupRuns', 0, 8);
  integer(value.recordedRuns, 'evidence.config.recordedRuns', 1, MAX_RECORDED_RUNS);
  integer(value.chunkSize, 'evidence.config.chunkSize', 1, 64);
  integer(value.maxEditsPerStep, 'evidence.config.maxEditsPerStep', 1, 4_096);
  integer(
    value.maxMeshValuesPerStep,
    'evidence.config.maxMeshValuesPerStep',
    1,
    16_000_000,
  );
  return input as CaBenchmarkEvidence['config'];
}

function decodeScenario(
  input: unknown,
  path: string,
  config: CaBenchmarkEvidence['config'],
): CaScenarioEvidence {
  const value = record(input, path, [
    'scenarioId', 'warmupRuns', 'deterministicStructuralEvidence',
    'recordedRuns', 'trace',
  ]);
  const scenarioId = identity(value.scenarioId, `${path}.scenarioId`);
  exactInteger(value.warmupRuns, `${path}.warmupRuns`, config.warmupRuns);
  exactBoolean(
    value.deterministicStructuralEvidence,
    `${path}.deterministicStructuralEvidence`,
    true,
  );
  const trace = decodeTrace(value.trace, `${path}.trace`, scenarioId, config);
  const expectedStructuralHash = sha256Serialized(serializeTrace(trace));
  const runs = list(value.recordedRuns, `${path}.recordedRuns`, 1, MAX_RECORDED_RUNS);
  if (runs.length !== config.recordedRuns) {
    fail(`${path}.recordedRuns`, `must contain ${config.recordedRuns} runs`);
  }
  let structuralHash: string | null = null;
  for (let index = 0; index < runs.length; index += 1) {
    const run = decodeRecordedRun(runs[index], `${path}.recordedRuns[${index}]`, trace.steps.length);
    exactInteger(run.run, `${path}.recordedRuns[${index}].run`, index + 1);
    if (run.structuralHash !== expectedStructuralHash) {
      fail(
        `${path}.recordedRuns[${index}].structuralHash`,
        `does not match ${expectedStructuralHash}`,
      );
    }
    structuralHash ??= run.structuralHash;
    if (run.structuralHash !== structuralHash) {
      fail(`${path}.recordedRuns[${index}].structuralHash`, 'differs across recorded runs');
    }
  }
  return input as CaScenarioEvidence;
}

function decodeTrace(
  input: unknown,
  path: string,
  scenarioId: string,
  config: CaBenchmarkEvidence['config'],
): CaScenarioEvidence['trace'] {
  const value = record(input, path, [
    'kind', 'schemaVersion', 'scenarioId', 'workload', 'ruleId', 'seed', 'bounds',
    'neighborhood', 'boundary', 'materializeEmpty', 'initialCells', 'initial', 'steps',
  ]);
  exactText(value.kind, `${path}.kind`, CA_TRACE_KIND);
  exactInteger(value.schemaVersion, `${path}.schemaVersion`, 1);
  exactText(value.scenarioId, `${path}.scenarioId`, scenarioId);
  enumeration(value.workload, `${path}.workload`, [
    'sparse_propagation', 'dense_churn', 'cross_boundary',
    'large_resident_small_hot_region', 'high_surface_area',
  ]);
  identity(value.ruleId, `${path}.ruleId`);
  integer(value.seed, `${path}.seed`, 0, Number.MAX_SAFE_INTEGER);
  const bounds = decodeBounds(value.bounds, `${path}.bounds`);
  enumeration(value.neighborhood, `${path}.neighborhood`, ['von_neumann6', 'moore26']);
  enumeration(value.boundary, `${path}.boundary`, ['fixed_empty', 'wrap']);
  booleanValue(value.materializeEmpty, `${path}.materializeEmpty`);
  const initialCells = decodeInitialCells(value.initialCells, `${path}.initialCells`, bounds);
  const initial = decodeInitial(value.initial, `${path}.initial`, config);
  const steps = list(value.steps, `${path}.steps`, 1, MAX_STEPS);
  const cellSteps = boundsVolume(bounds) * steps.length;
  if (!Number.isSafeInteger(cellSteps) || cellSteps > MAX_CELL_STEPS) {
    fail(path, `cell-step workload ${cellSteps} exceeds ${MAX_CELL_STEPS}`);
  }
  const orderedInitialCells = [...initialCells].sort((left, right) =>
    xyzCompare(left.coord, right.coord));
  const initialStateHash = fnv1a64Json(orderedInitialCells);
  if (initial.initialCaStateHash !== initialStateHash) {
    fail(`${path}.initial.initialCaStateHash`, `does not match ${initialStateHash}`);
  }
  const initialCumulativeHash = fnv1a64Json({
    scenario: {
      id: scenarioId,
      workload: value.workload,
      seed: value.seed,
      bounds,
      neighborhood: value.neighborhood,
      boundary: value.boundary,
      rule: value.ruleId,
      steps: steps.length,
      initialCells,
    },
    initialStateHash,
  });
  if (initial.initialCaCumulativeHash !== initialCumulativeHash) {
    fail(
      `${path}.initial.initialCaCumulativeHash`,
      `does not match ${initialCumulativeHash}`,
    );
  }
  const chunks = new Map<string, CaMeshChunk>();
  for (const chunk of initial.projectionChunks) {
    chunks.set(coordKey(chunk.chunk), chunk);
  }
  verifyProjectionState(chunks, initial.projectionStateHash, initial.readout, `${path}.initial`);
  const initialTraceHash = sha256Json([
    'initial',
    scenarioId,
    initial.initialCaStateHash,
    initial.initialCaCumulativeHash,
    initial.readout,
    initial.projectionStateHash,
  ]);
  if (initial.traceHash !== initialTraceHash) {
    fail(`${path}.initial.traceHash`, `does not match ${initialTraceHash}`);
  }

  let revision = initial.readout.sourceRevision;
  let previousTraceHash = initial.traceHash;
  let previousCaCumulativeHash = initial.initialCaCumulativeHash;
  for (let index = 0; index < steps.length; index += 1) {
    const stepPath = `${path}.steps[${index}]`;
    const step = decodeStep(steps[index], stepPath, config, index + 1);
    if (step.revisionBefore !== revision) {
      fail(`${stepPath}.revisionBefore`, `expected ${revision}`);
    }
    if (step.previousTraceHash !== previousTraceHash) {
      fail(`${stepPath}.previousTraceHash`, `expected ${previousTraceHash}`);
    }
    const projectionHash = sha256Serialized(serializeProjectionOps(step.projectionOps));
    if (step.projectionDeltaHash !== projectionHash) {
      fail(`${stepPath}.projectionDeltaHash`, `does not match ${projectionHash}`);
    }
    applyProjectionOps(chunks, step.projectionOps, stepPath);
    verifyProjectionState(chunks, step.projectionStateHash, step.readout, stepPath);
    verifyCaHashes(
      step.ca,
      previousCaCumulativeHash,
      scenarioId,
      value.ruleId as string,
      stepPath,
    );
    const traceHash = sha256Json([
      'step',
      step.previousTraceHash,
      step.ca,
      step.revisionBefore,
      step.acceptedRevision,
      step.canonicalEditCount,
      step.engineDeltaCount,
      step.readout,
      step.projectionDeltaHash,
      step.projectionStateHash,
    ]);
    if (step.traceHash !== traceHash) {
      fail(`${stepPath}.traceHash`, `does not match ${traceHash}`);
    }
    revision = step.acceptedRevision;
    previousTraceHash = step.traceHash;
    previousCaCumulativeHash = step.ca.cumulativeScenarioHash;
  }
  decodeBounds(bounds, `${path}.bounds`);
  return input as CaScenarioEvidence['trace'];
}

function decodeInitial(
  input: unknown,
  path: string,
  config: CaBenchmarkEvidence['config'],
): CaScenarioEvidence['trace']['initial'] {
  const value = record(input, path, [
    'initialCaStateHash', 'initialCaCumulativeHash', 'readout',
    'projectionChunks', 'projectionStateHash', 'traceHash',
  ]);
  hash(value.initialCaStateHash, `${path}.initialCaStateHash`, 'fnv1a64');
  hash(value.initialCaCumulativeHash, `${path}.initialCaCumulativeHash`, 'fnv1a64');
  const readout = decodeReadout(value.readout, `${path}.readout`);
  const chunks = list(value.projectionChunks, `${path}.projectionChunks`, 0, MAX_PROJECTION_CHUNKS);
  let meshValues = 0;
  let previous: Coord | null = null;
  for (let index = 0; index < chunks.length; index += 1) {
    const chunk = decodeChunk(chunks[index], `${path}.projectionChunks[${index}]`);
    if (previous !== null && coordCompare(previous, chunk.chunk) >= 0) {
      fail(`${path}.projectionChunks[${index}].chunk`, 'is not strictly ordered');
    }
    previous = chunk.chunk;
    meshValues = checkedAdd(meshValues, meshValueCount(chunk), `${path}.projectionChunks`);
  }
  if (meshValues > config.maxMeshValuesPerStep) {
    fail(`${path}.projectionChunks`, `mesh values ${meshValues} exceed ${config.maxMeshValuesPerStep}`);
  }
  hash(value.projectionStateHash, `${path}.projectionStateHash`, 'sha256');
  hash(value.traceHash, `${path}.traceHash`, 'sha256');
  return {
    initialCaStateHash: value.initialCaStateHash as string,
    initialCaCumulativeHash: value.initialCaCumulativeHash as string,
    readout,
    projectionChunks: chunks as unknown as readonly CaMeshChunk[],
    projectionStateHash: value.projectionStateHash as string,
    traceHash: value.traceHash as string,
  };
}

function decodeStep(
  input: unknown,
  path: string,
  config: CaBenchmarkEvidence['config'],
  expectedStep: number,
): CaSpatialStep {
  const value = record(input, path, [
    'ca', 'revisionBefore', 'acceptedRevision', 'engineChangedVoxels',
    'canonicalEditCount', 'engineDeltaCount', 'readout', 'projectionOps',
    'projectionDeltaHash', 'projectionStateHash', 'previousTraceHash', 'traceHash',
  ]);
  const ca = decodeCaStep(value.ca, `${path}.ca`, expectedStep);
  const revisionBefore = integer(value.revisionBefore, `${path}.revisionBefore`, 0, Number.MAX_SAFE_INTEGER);
  const acceptedRevision = integer(value.acceptedRevision, `${path}.acceptedRevision`, 0, Number.MAX_SAFE_INTEGER);
  const engineChangedVoxels = integer(
    value.engineChangedVoxels,
    `${path}.engineChangedVoxels`,
    0,
    config.maxEditsPerStep,
  );
  const canonicalEditCount = integer(
    value.canonicalEditCount,
    `${path}.canonicalEditCount`,
    0,
    config.maxEditsPerStep,
  );
  const engineDeltaCount = integer(
    value.engineDeltaCount,
    `${path}.engineDeltaCount`,
    0,
    config.maxEditsPerStep,
  );
  if (
    ca.changedCellCount !== engineChangedVoxels
    || engineChangedVoxels !== canonicalEditCount
    || canonicalEditCount !== engineDeltaCount
  ) {
    fail(path, 'CA, canonical edit, and Engine delta counts disagree');
  }
  const expectedAcceptedRevision = engineDeltaCount === 0
    ? revisionBefore
    : revisionBefore + 1;
  if (acceptedRevision !== expectedAcceptedRevision) {
    fail(`${path}.acceptedRevision`, `expected ${expectedAcceptedRevision}`);
  }
  const readout = decodeReadout(value.readout, `${path}.readout`);
  if (readout.sourceRevision !== acceptedRevision) {
    fail(`${path}.readout.sourceRevision`, `expected ${acceptedRevision}`);
  }
  const projectionOps = list(
    value.projectionOps,
    `${path}.projectionOps`,
    0,
    MAX_PROJECTION_CHUNKS,
  ).map((op, index) => decodeProjectionOp(op, `${path}.projectionOps[${index}]`));
  let meshValues = 0;
  let previous: Coord | null = null;
  for (let index = 0; index < projectionOps.length; index += 1) {
    const op = projectionOps[index];
    const coord = op.op === 'delete' ? op.chunk : op.chunk.chunk;
    if (previous !== null && coordCompare(previous, coord) >= 0) {
      fail(`${path}.projectionOps[${index}]`, 'is not strictly chunk-ordered');
    }
    previous = coord;
    if (op.op === 'upsert') {
      meshValues = checkedAdd(meshValues, meshValueCount(op.chunk), `${path}.projectionOps`);
    }
  }
  if (meshValues > config.maxMeshValuesPerStep) {
    fail(`${path}.projectionOps`, `mesh values ${meshValues} exceed ${config.maxMeshValuesPerStep}`);
  }
  for (const key of ['projectionDeltaHash', 'projectionStateHash', 'previousTraceHash', 'traceHash']) {
    hash(value[key], `${path}.${key}`, 'sha256');
  }
  return {
    ca,
    revisionBefore,
    acceptedRevision,
    engineChangedVoxels,
    canonicalEditCount,
    engineDeltaCount,
    readout,
    projectionOps,
    projectionDeltaHash: value.projectionDeltaHash as string,
    projectionStateHash: value.projectionStateHash as string,
    previousTraceHash: value.previousTraceHash as string,
    traceHash: value.traceHash as string,
  };
}

function decodeCaStep(input: unknown, path: string, expectedStep: number): CaStepEvidence {
  const value = record(input, path, [
    'step', 'activeCellCount', 'changedCellCount', 'evaluatedCellCount',
    'touchedBounds', 'stateCounts', 'deltas', 'deltaHash', 'stateHash',
    'cumulativeScenarioHash',
  ]);
  exactInteger(value.step, `${path}.step`, expectedStep);
  integer(value.activeCellCount, `${path}.activeCellCount`, 0, Number.MAX_SAFE_INTEGER);
  const changed = integer(value.changedCellCount, `${path}.changedCellCount`, 0, Number.MAX_SAFE_INTEGER);
  integer(value.evaluatedCellCount, `${path}.evaluatedCellCount`, 0, Number.MAX_SAFE_INTEGER);
  if (value.touchedBounds !== null) {
    const touched = record(value.touchedBounds, `${path}.touchedBounds`, ['min', 'maxInclusive']);
    xyz(touched.min, `${path}.touchedBounds.min`);
    xyz(touched.maxInclusive, `${path}.touchedBounds.maxInclusive`);
  }
  const counts = record(value.stateCounts, `${path}.stateCounts`, [
    'empty', 'source', 'frontier', 'trail',
  ]);
  for (const key of Object.keys(counts)) {
    integer(counts[key], `${path}.stateCounts.${key}`, 0, Number.MAX_SAFE_INTEGER);
  }
  const countedActive = checkedAdd(
    checkedAdd(counts.source as number, counts.frontier as number, path),
    counts.trail as number,
    path,
  );
  if (value.activeCellCount !== countedActive) {
    fail(`${path}.activeCellCount`, `stateCounts report ${countedActive} active cells`);
  }
  const deltas = list(value.deltas, `${path}.deltas`, 0, changed);
  let previous: { readonly x: number; readonly y: number; readonly z: number } | null = null;
  for (let index = 0; index < deltas.length; index += 1) {
    const delta = record(deltas[index], `${path}.deltas[${index}]`, [
      'coord', 'previous', 'current',
    ]);
    const coord = xyz(delta.coord, `${path}.deltas[${index}].coord`);
    if (previous !== null && xyzCompare(previous, coord) >= 0) {
      fail(`${path}.deltas[${index}].coord`, 'is not strictly ordered');
    }
    previous = coord;
    const prior = enumeration(delta.previous, `${path}.deltas[${index}].previous`, [
      'empty', 'source', 'frontier', 'trail',
    ]);
    const current = enumeration(delta.current, `${path}.deltas[${index}].current`, [
      'empty', 'source', 'frontier', 'trail',
    ]);
    if (prior === current) {
      fail(`${path}.deltas[${index}]`, 'does not change state');
    }
  }
  if (deltas.length !== changed) {
    fail(`${path}.deltas`, `contains ${deltas.length}; changedCellCount is ${changed}`);
  }
  for (const key of ['deltaHash', 'stateHash', 'cumulativeScenarioHash']) {
    hash(value[key], `${path}.${key}`, 'fnv1a64');
  }
  return input as CaStepEvidence;
}

function decodeProjectionOp(input: unknown, path: string): CaProjectionOp {
  const base = looseRecord(input, path);
  const op = enumeration(base.op, `${path}.op`, ['upsert', 'delete']);
  if (op === 'delete') {
    const value = record(input, path, ['op', 'chunk']);
    return { op, chunk: coord(value.chunk, `${path}.chunk`) };
  }
  const value = record(input, path, ['op', 'chunk']);
  return { op, chunk: decodeChunk(value.chunk, `${path}.chunk`) };
}

function decodeChunk(input: unknown, path: string): CaMeshChunk {
  const value = record(input, path, [
    'chunk', 'contentHash', 'bufferHash', 'translation', 'positions', 'normals',
    'indices', 'groups', 'boundsMin', 'boundsMax', 'vertices', 'quads',
    'facesCulled',
  ]);
  const chunk = coord(value.chunk, `${path}.chunk`);
  hash(value.contentHash, `${path}.contentHash`, 'fnv1a64');
  hash(value.bufferHash, `${path}.bufferHash`, 'sha256');
  const translation = floatCoord(value.translation, `${path}.translation`);
  const positions = floatList(value.positions, `${path}.positions`, 0, 6_000_000);
  const normals = floatList(value.normals, `${path}.normals`, 0, 6_000_000);
  const indices = integerList(value.indices, `${path}.indices`, 0, 6_000_000);
  const groups = list(value.groups, `${path}.groups`, 0, 65_536).map((group, index) => {
    const decoded = record(group, `${path}.groups[${index}]`, ['materialSlot', 'start', 'count']);
    integer(decoded.materialSlot, `${path}.groups[${index}].materialSlot`, 1, 4_095);
    integer(decoded.start, `${path}.groups[${index}].start`, 0, indices.length);
    integer(decoded.count, `${path}.groups[${index}].count`, 0, indices.length);
    return group as CaMeshGroup;
  });
  const boundsMin = floatCoord(value.boundsMin, `${path}.boundsMin`);
  const boundsMax = floatCoord(value.boundsMax, `${path}.boundsMax`);
  const vertices = integer(value.vertices, `${path}.vertices`, 0, 2_000_000);
  const quads = integer(value.quads, `${path}.quads`, 0, 2_000_000);
  integer(value.facesCulled, `${path}.facesCulled`, 0, Number.MAX_SAFE_INTEGER);
  if (positions.length !== vertices * 3 || normals.length !== vertices * 3) {
    fail(path, 'position/normal streams do not match vertex count');
  }
  if (indices.some((index) => index >= vertices)) {
    fail(`${path}.indices`, 'references a vertex outside the buffer');
  }
  if (groups.some((group) => group.start + group.count > indices.length)) {
    fail(`${path}.groups`, 'references indices outside the buffer');
  }
  for (let axis = 0; axis < 3; axis += 1) {
    if ((boundsMin[axis] ?? 0) > (boundsMax[axis] ?? 0)) {
      fail(path, 'mesh bounds are inverted');
    }
  }
  const chunkFact = {
    chunk,
    contentHash: value.contentHash as string,
    bufferHash: value.bufferHash as string,
    translation,
    positions,
    normals,
    indices,
    groups,
    boundsMin,
    boundsMax,
    vertices,
    quads,
    facesCulled: value.facesCulled as number,
  };
  const bufferHash = sha256Serialized(serializeBufferHashInput(chunkFact));
  if (chunkFact.bufferHash !== bufferHash) {
    fail(`${path}.bufferHash`, `does not match ${bufferHash}`);
  }
  return chunkFact;
}

function decodeReadout(input: unknown, path: string): CaAuthorityReadout {
  const value = record(input, path, [
    'sourceRevision', 'authorityHash', 'projectionRevisionsCoherent',
    'solidVoxelCount', 'residentChunkCount', 'colliderChunkCount',
    'navigationCellCount', 'navigationHash', 'meshChunkCount',
    'meshVertexCount', 'meshQuadCount', 'meshProjectionHash',
  ]);
  integer(value.sourceRevision, `${path}.sourceRevision`, 0, Number.MAX_SAFE_INTEGER);
  hash(value.authorityHash, `${path}.authorityHash`, 'fnv1a64');
  exactBoolean(value.projectionRevisionsCoherent, `${path}.projectionRevisionsCoherent`, true);
  for (const key of [
    'solidVoxelCount', 'residentChunkCount', 'colliderChunkCount',
    'navigationCellCount', 'meshChunkCount', 'meshVertexCount', 'meshQuadCount',
  ]) {
    integer(value[key], `${path}.${key}`, 0, Number.MAX_SAFE_INTEGER);
  }
  hash(value.navigationHash, `${path}.navigationHash`, 'fnv1a64');
  hash(value.meshProjectionHash, `${path}.meshProjectionHash`, 'sha256');
  return input as CaAuthorityReadout;
}

function decodeRecordedRun(
  input: unknown,
  path: string,
  stepCount: number,
): CaRecordedRun {
  const value = record(input, path, [
    'run', 'structuralHash', 'admissionTiming', 'stepTimings', 'encodedTraceBytes',
  ]);
  integer(value.run, `${path}.run`, 1, MAX_RECORDED_RUNS);
  hash(value.structuralHash, `${path}.structuralHash`, 'sha256');
  const admission = record(value.admissionTiming, `${path}.admissionTiming`, [
    'stateMaterializationNs', 'engineBuildNs', 'evidenceReadbackNs', 'artifactEncodingNs',
  ]);
  for (const key of Object.keys(admission)) {
    integer(admission[key], `${path}.admissionTiming.${key}`, 0, Number.MAX_SAFE_INTEGER);
  }
  const timings = list(value.stepTimings, `${path}.stepTimings`, stepCount, stepCount);
  for (let index = 0; index < timings.length; index += 1) {
    const timing = record(timings[index], `${path}.stepTimings[${index}]`, [
      'caStepNs', 'requestConstructionNs', 'spatialPreviewNs', 'authorityCommitNs',
      'evidenceReadbackNs', 'artifactEncodingNs',
    ]);
    for (const key of Object.keys(timing)) {
      integer(timing[key], `${path}.stepTimings[${index}].${key}`, 0, Number.MAX_SAFE_INTEGER);
    }
  }
  integer(value.encodedTraceBytes, `${path}.encodedTraceBytes`, 1, Number.MAX_SAFE_INTEGER);
  return input as CaRecordedRun;
}

function verifyCaHashes(
  ca: CaStepEvidence,
  previousCumulativeHash: string,
  scenarioId: string,
  ruleId: string,
  path: string,
): void {
  const deltaHash = fnv1a64Json({
    scenarioId,
    ruleId,
    step: ca.step,
    deltas: ca.deltas,
  });
  if (ca.deltaHash !== deltaHash) {
    fail(`${path}.ca.deltaHash`, `does not match ${deltaHash}`);
  }
  const cumulative = fnv1a64Json({
    previousHash: previousCumulativeHash,
    deltaHash: ca.deltaHash,
    stateHash: ca.stateHash,
    step: ca.step,
    activeCellCount: ca.activeCellCount,
    touchedBounds: ca.touchedBounds,
  });
  if (ca.cumulativeScenarioHash !== cumulative) {
    fail(`${path}.ca.cumulativeScenarioHash`, `does not match ${cumulative}`);
  }
}

function applyProjectionOps(
  chunks: Map<string, CaMeshChunk>,
  ops: readonly CaProjectionOp[],
  path: string,
): void {
  for (let index = 0; index < ops.length; index += 1) {
    const op = ops[index];
    if (op.op === 'delete') {
      const key = coordKey(op.chunk);
      if (!chunks.delete(key)) {
        fail(`${path}.projectionOps[${index}]`, `deletes absent chunk ${key}`);
      }
      continue;
    }
    const key = coordKey(op.chunk.chunk);
    const prior = chunks.get(key);
    if (prior !== undefined && serializeChunk(prior) === serializeChunk(op.chunk)) {
      fail(`${path}.projectionOps[${index}]`, `upserts unchanged chunk ${key}`);
    }
    if (prior !== undefined && coordKey(prior.translation) !== coordKey(op.chunk.translation)) {
      fail(`${path}.projectionOps[${index}]`, `changes translation for chunk ${key}`);
    }
    chunks.set(key, op.chunk);
  }
}

function verifyProjectionState(
  chunks: Map<string, CaMeshChunk>,
  expectedHash: string,
  readout: CaAuthorityReadout,
  path: string,
): void {
  const ordered = [...chunks.values()].sort((left, right) => coordCompare(left.chunk, right.chunk));
  const summaries = ordered.map((chunk) => ({
    chunk: chunk.chunk,
    contentHash: chunk.contentHash,
    bufferHash: chunk.bufferHash,
    vertices: chunk.vertices,
    quads: chunk.quads,
    facesCulled: chunk.facesCulled,
  }));
  const actualHash = sha256Json(summaries);
  if (expectedHash !== actualHash || readout.meshProjectionHash !== actualHash) {
    fail(path, `projection state hash does not match ${actualHash}`);
  }
  const vertices = ordered.reduce((total, chunk) => total + chunk.vertices, 0);
  const quads = ordered.reduce((total, chunk) => total + chunk.quads, 0);
  if (
    readout.meshChunkCount !== ordered.length
    || readout.meshVertexCount !== vertices
    || readout.meshQuadCount !== quads
  ) {
    fail(path, 'projection readout does not match retained mesh facts');
  }
}

function allChunkFacts(scenario: CaScenarioEvidence): Map<string, CaMeshChunk> {
  const facts = new Map<string, CaMeshChunk>();
  const add = (fact: CaMeshChunk): void => {
    facts.set(fact.bufferHash, facts.get(fact.bufferHash) ?? fact);
  };
  scenario.trace.initial.projectionChunks.forEach(add);
  for (const step of scenario.trace.steps) {
    for (const op of step.projectionOps) {
      if (op.op === 'upsert') {
        add(op.chunk);
      }
    }
  }
  return facts;
}

function meshPayload(chunk: CaMeshChunk): MeshPayloadDescriptor {
  return {
    layout: {
      vertexCount: chunk.vertices,
      indexCount: chunk.indices.length,
      indexWidth: 'u32',
      attributes: [
        { name: 'position', components: 3, kind: 'f32' },
        { name: 'normal', components: 3, kind: 'f32' },
      ],
    },
    groups: chunk.groups,
    bounds: { min: chunk.boundsMin, max: chunk.boundsMax },
    source: {
      kind: 'inline',
      positions: chunk.positions,
      normals: chunk.normals,
      indices: chunk.indices,
    },
    provenance: 'voxelObject',
  };
}

function instance(
  asset: string,
  frame: number,
  chunk: CaMeshChunk,
): {
  readonly asset: string;
  readonly frame: number;
  readonly transform: {
    readonly translation: Coord;
    readonly rotation: readonly [number, number, number, number];
    readonly scale: Coord;
  };
  readonly visible: true;
  readonly materialOverrides: readonly [];
  readonly metadata: {
    readonly sourceEntity: null;
    readonly sourceSceneNode: null;
    readonly tags: readonly string[];
    readonly label: string;
  };
} {
  return {
    asset,
    frame,
    transform: {
      translation: chunk.translation,
      rotation: [0, 0, 0, 1],
      scale: [1, 1, 1],
    },
    visible: true,
    materialOverrides: [],
    metadata: {
      sourceEntity: null,
      sourceSceneNode: null,
      tags: ['captured-authority-trace', 'rusty-procgen'],
      label: `ca-chunk:${coordKey(chunk.chunk)}:${chunk.bufferHash}`,
    },
  };
}

function serializeBufferHashInput(chunk: CaMeshChunk): string {
  return [
    '[',
    floatArray(chunk.translation),
    ',',
    floatArray(chunk.positions),
    ',',
    floatArray(chunk.normals),
    ',',
    JSON.stringify(chunk.indices),
    ',',
    JSON.stringify(chunk.groups),
    ',',
    floatArray(chunk.boundsMin),
    ',',
    floatArray(chunk.boundsMax),
    ']',
  ].join('');
}

function serializeProjectionOps(ops: readonly CaProjectionOp[]): string {
  return `[${ops.map((op) => op.op === 'delete'
    ? `{"op":"delete","chunk":${JSON.stringify(op.chunk)}}`
    : `{"op":"upsert","chunk":${serializeChunk(op.chunk)}}`).join(',')}]`;
}

function serializeTrace(trace: CaScenarioEvidence['trace']): string {
  return [
    '{"kind":',
    JSON.stringify(trace.kind),
    ',"schemaVersion":',
    String(trace.schemaVersion),
    ',"scenarioId":',
    JSON.stringify(trace.scenarioId),
    ',"workload":',
    JSON.stringify(trace.workload),
    ',"ruleId":',
    JSON.stringify(trace.ruleId),
    ',"seed":',
    String(trace.seed),
    ',"bounds":',
    JSON.stringify({
      min: trace.bounds.min,
      maxExclusive: trace.bounds.maxExclusive,
    }),
    ',"neighborhood":',
    JSON.stringify(trace.neighborhood),
    ',"boundary":',
    JSON.stringify(trace.boundary),
    ',"materializeEmpty":',
    String(trace.materializeEmpty),
    ',"initialCells":',
    serializeSeedCells(trace.initialCells),
    ',"initial":',
    serializeInitial(trace.initial),
    ',"steps":[',
    trace.steps.map(serializeSpatialStep).join(','),
    ']}',
  ].join('');
}

function serializeSeedCells(cells: readonly CaSeedCell[]): string {
  return JSON.stringify(cells.map((cell) => ({
    coord: { x: cell.coord.x, y: cell.coord.y, z: cell.coord.z },
    state: cell.state,
  })));
}

function serializeInitial(initial: CaScenarioEvidence['trace']['initial']): string {
  return [
    '{"initialCaStateHash":',
    JSON.stringify(initial.initialCaStateHash),
    ',"initialCaCumulativeHash":',
    JSON.stringify(initial.initialCaCumulativeHash),
    ',"readout":',
    serializeReadout(initial.readout),
    ',"projectionChunks":[',
    initial.projectionChunks.map(serializeChunk).join(','),
    '],"projectionStateHash":',
    JSON.stringify(initial.projectionStateHash),
    ',"traceHash":',
    JSON.stringify(initial.traceHash),
    '}',
  ].join('');
}

function serializeSpatialStep(step: CaSpatialStep): string {
  return [
    '{"ca":',
    serializeCaStep(step.ca),
    ',"revisionBefore":',
    String(step.revisionBefore),
    ',"acceptedRevision":',
    String(step.acceptedRevision),
    ',"engineChangedVoxels":',
    String(step.engineChangedVoxels),
    ',"canonicalEditCount":',
    String(step.canonicalEditCount),
    ',"engineDeltaCount":',
    String(step.engineDeltaCount),
    ',"readout":',
    serializeReadout(step.readout),
    ',"projectionOps":',
    serializeProjectionOps(step.projectionOps),
    ',"projectionDeltaHash":',
    JSON.stringify(step.projectionDeltaHash),
    ',"projectionStateHash":',
    JSON.stringify(step.projectionStateHash),
    ',"previousTraceHash":',
    JSON.stringify(step.previousTraceHash),
    ',"traceHash":',
    JSON.stringify(step.traceHash),
    '}',
  ].join('');
}

function serializeCaStep(ca: CaStepEvidence): string {
  return JSON.stringify({
    step: ca.step,
    activeCellCount: ca.activeCellCount,
    changedCellCount: ca.changedCellCount,
    evaluatedCellCount: ca.evaluatedCellCount,
    touchedBounds: ca.touchedBounds === null
      ? null
      : {
          min: {
            x: ca.touchedBounds.min.x,
            y: ca.touchedBounds.min.y,
            z: ca.touchedBounds.min.z,
          },
          maxInclusive: {
            x: ca.touchedBounds.maxInclusive.x,
            y: ca.touchedBounds.maxInclusive.y,
            z: ca.touchedBounds.maxInclusive.z,
          },
        },
    stateCounts: {
      empty: ca.stateCounts.empty,
      source: ca.stateCounts.source,
      frontier: ca.stateCounts.frontier,
      trail: ca.stateCounts.trail,
    },
    deltas: ca.deltas.map((delta) => ({
      coord: { x: delta.coord.x, y: delta.coord.y, z: delta.coord.z },
      previous: delta.previous,
      current: delta.current,
    })),
    deltaHash: ca.deltaHash,
    stateHash: ca.stateHash,
    cumulativeScenarioHash: ca.cumulativeScenarioHash,
  });
}

function serializeReadout(readout: CaAuthorityReadout): string {
  return JSON.stringify({
    sourceRevision: readout.sourceRevision,
    authorityHash: readout.authorityHash,
    projectionRevisionsCoherent: readout.projectionRevisionsCoherent,
    solidVoxelCount: readout.solidVoxelCount,
    residentChunkCount: readout.residentChunkCount,
    colliderChunkCount: readout.colliderChunkCount,
    navigationCellCount: readout.navigationCellCount,
    navigationHash: readout.navigationHash,
    meshChunkCount: readout.meshChunkCount,
    meshVertexCount: readout.meshVertexCount,
    meshQuadCount: readout.meshQuadCount,
    meshProjectionHash: readout.meshProjectionHash,
  });
}

function serializeChunk(chunk: CaMeshChunk): string {
  return [
    '{"chunk":',
    JSON.stringify(chunk.chunk),
    ',"contentHash":',
    JSON.stringify(chunk.contentHash),
    ',"bufferHash":',
    JSON.stringify(chunk.bufferHash),
    ',"translation":',
    floatArray(chunk.translation),
    ',"positions":',
    floatArray(chunk.positions),
    ',"normals":',
    floatArray(chunk.normals),
    ',"indices":',
    JSON.stringify(chunk.indices),
    ',"groups":',
    JSON.stringify(chunk.groups),
    ',"boundsMin":',
    floatArray(chunk.boundsMin),
    ',"boundsMax":',
    floatArray(chunk.boundsMax),
    ',"vertices":',
    String(chunk.vertices),
    ',"quads":',
    String(chunk.quads),
    ',"facesCulled":',
    String(chunk.facesCulled),
    '}',
  ].join('');
}

function floatArray(values: readonly number[]): string {
  return `[${values.map((value) => Number.isInteger(value)
    ? `${Object.is(value, -0) ? '-0' : String(value)}.0`
    : JSON.stringify(value)).join(',')}]`;
}

function meshValueCount(chunk: CaMeshChunk): number {
  return chunk.positions.length
    + chunk.normals.length
    + chunk.indices.length
    + chunk.groups.length * 3;
}

function decodeBounds(input: unknown, path: string): CaBounds {
  const value = record(input, path, ['min', 'maxExclusive']);
  const min = xyz(value.min, `${path}.min`);
  const maxExclusive = xyz(value.maxExclusive, `${path}.maxExclusive`);
  if (
    maxExclusive.x <= min.x
    || maxExclusive.y <= min.y
    || maxExclusive.z <= min.z
  ) {
    fail(path, 'has an empty or inverted axis');
  }
  return { min, maxExclusive };
}

function decodeInitialCells(
  input: unknown,
  path: string,
  bounds: CaBounds,
): readonly CaSeedCell[] {
  const cells = list(input, path, 0, MAX_SEED_CELLS);
  const seen = new Set<string>();
  return cells.map((inputCell, index) => {
    const cellPath = `${path}[${index}]`;
    const value = record(inputCell, cellPath, ['coord', 'state']);
    const decodedCoord = xyz(value.coord, `${cellPath}.coord`);
    if (
      decodedCoord.x < bounds.min.x
      || decodedCoord.x >= bounds.maxExclusive.x
      || decodedCoord.y < bounds.min.y
      || decodedCoord.y >= bounds.maxExclusive.y
      || decodedCoord.z < bounds.min.z
      || decodedCoord.z >= bounds.maxExclusive.z
    ) {
      fail(`${cellPath}.coord`, 'lies outside the authored bounds');
    }
    const key = `${decodedCoord.x}:${decodedCoord.y}:${decodedCoord.z}`;
    if (seen.has(key)) {
      fail(`${cellPath}.coord`, `duplicates seed coordinate ${key}`);
    }
    seen.add(key);
    return {
      coord: decodedCoord,
      state: enumeration(value.state, `${cellPath}.state`, [
        'source', 'frontier', 'trail',
      ]),
    };
  });
}

function boundsVolume(bounds: CaBounds): number {
  const dimensions = [
    bounds.maxExclusive.x - bounds.min.x,
    bounds.maxExclusive.y - bounds.min.y,
    bounds.maxExclusive.z - bounds.min.z,
  ];
  const volume = dimensions.reduce((product, dimension) => product * dimension, 1);
  if (!Number.isSafeInteger(volume) || volume > MAX_CELL_STEPS) {
    fail('evidence.trace.bounds', `volume ${volume} exceeds ${MAX_CELL_STEPS}`);
  }
  return volume;
}

function xyz(
  input: unknown,
  path: string,
): { readonly x: number; readonly y: number; readonly z: number } {
  const value = record(input, path, ['x', 'y', 'z']);
  return {
    x: integer(value.x, `${path}.x`, -2_147_483_648, 2_147_483_647),
    y: integer(value.y, `${path}.y`, -2_147_483_648, 2_147_483_647),
    z: integer(value.z, `${path}.z`, -2_147_483_648, 2_147_483_647),
  };
}

function coord(input: unknown, path: string): Coord {
  const values = list(input, path, 3, 3);
  return [
    integer(values[0], `${path}[0]`, Number.MIN_SAFE_INTEGER, Number.MAX_SAFE_INTEGER),
    integer(values[1], `${path}[1]`, Number.MIN_SAFE_INTEGER, Number.MAX_SAFE_INTEGER),
    integer(values[2], `${path}[2]`, Number.MIN_SAFE_INTEGER, Number.MAX_SAFE_INTEGER),
  ];
}

function floatCoord(input: unknown, path: string): Coord {
  const values = list(input, path, 3, 3);
  return [
    finite(values[0], `${path}[0]`),
    finite(values[1], `${path}[1]`),
    finite(values[2], `${path}[2]`),
  ];
}

function floatList(input: unknown, path: string, min: number, max: number): readonly number[] {
  return list(input, path, min, max).map((value, index) => finite(value, `${path}[${index}]`));
}

function integerList(input: unknown, path: string, min: number, max: number): readonly number[] {
  return list(input, path, min, max).map((value, index) =>
    integer(value, `${path}[${index}]`, 0, 4_294_967_295));
}

function record(input: unknown, path: string, keys: readonly string[]): Record<string, unknown> {
  const value = looseRecord(input, path);
  const expected = new Set(keys);
  for (const key of Object.keys(value)) {
    if (!expected.has(key)) {
      fail(`${path}.${key}`, 'is unknown');
    }
  }
  for (const key of keys) {
    if (!(key in value)) {
      fail(`${path}.${key}`, 'is required');
    }
  }
  return value;
}

function looseRecord(input: unknown, path: string): Record<string, unknown> {
  if (typeof input !== 'object' || input === null || Array.isArray(input)) {
    fail(path, 'must be an object');
  }
  return input as Record<string, unknown>;
}

function list(
  input: unknown,
  path: string,
  min: number,
  max: number,
): readonly unknown[] {
  if (!Array.isArray(input) || input.length < min || input.length > max) {
    fail(path, `must be an array with ${min}..=${max} items`);
  }
  return input;
}

function textList(
  input: unknown,
  path: string,
  min: number,
  max: number,
): readonly string[] {
  return list(input, path, min, max).map((value, index) => text(value, `${path}[${index}]`));
}

function text(input: unknown, path: string): string {
  if (typeof input !== 'string' || input.length === 0 || input.length > 1_024) {
    fail(path, 'must be non-empty bounded text');
  }
  return input;
}

function exactText(input: unknown, path: string, expected: string): string {
  const value = text(input, path);
  if (value !== expected) {
    fail(path, `must equal ${expected}`);
  }
  return value;
}

function identity(input: unknown, path: string): string {
  const value = text(input, path);
  if (value.length > 128 || !/^[a-z0-9._-]+$/.test(value)) {
    fail(path, 'must be a stable lowercase identity');
  }
  return value;
}

function commit(input: unknown, path: string): string {
  const value = text(input, path);
  if (!/^[0-9a-f]{40}$/.test(value)) {
    fail(path, 'must be a full lowercase Git SHA');
  }
  return value;
}

function hash(input: unknown, path: string, kind: 'sha256' | 'fnv1a64'): string {
  const value = text(input, path);
  const pattern = kind === 'sha256'
    ? /^sha256:[0-9a-f]{64}$/
    : /^fnv1a64:[0-9a-f]{16}$/;
  if (!pattern.test(value)) {
    fail(path, `must be a ${kind} hash`);
  }
  return value;
}

function integer(input: unknown, path: string, min: number, max: number): number {
  if (!Number.isSafeInteger(input) || (input as number) < min || (input as number) > max) {
    fail(path, `must be a safe integer in ${min}..=${max}`);
  }
  return input as number;
}

function exactInteger(input: unknown, path: string, expected: number): number {
  const value = integer(input, path, expected, expected);
  return value;
}

function finite(input: unknown, path: string): number {
  if (typeof input !== 'number' || !Number.isFinite(input)) {
    fail(path, 'must be finite');
  }
  return input;
}

function booleanValue(input: unknown, path: string): boolean {
  if (typeof input !== 'boolean') {
    fail(path, 'must be boolean');
  }
  return input;
}

function exactBoolean(input: unknown, path: string, expected: boolean): boolean {
  const value = booleanValue(input, path);
  if (value !== expected) {
    fail(path, `must equal ${String(expected)}`);
  }
  return value;
}

function enumeration<T extends string>(
  input: unknown,
  path: string,
  values: readonly T[],
): T {
  if (typeof input !== 'string' || !values.includes(input as T)) {
    fail(path, `must be one of ${values.join(', ')}`);
  }
  return input as T;
}

function coordKey(value: Coord): string {
  return value.join(':');
}

function parseCoordKey(value: string): Coord {
  const parts = value.split(':').map(Number);
  return [parts[0] ?? 0, parts[1] ?? 0, parts[2] ?? 0];
}

function coordCompare(left: Coord, right: Coord): number {
  return (left[0] ?? 0) - (right[0] ?? 0)
    || (left[1] ?? 0) - (right[1] ?? 0)
    || (left[2] ?? 0) - (right[2] ?? 0);
}

function xyzCompare(
  left: { readonly x: number; readonly y: number; readonly z: number },
  right: { readonly x: number; readonly y: number; readonly z: number },
): number {
  return left.x - right.x || left.y - right.y || left.z - right.z;
}

function checkedAdd(left: number, right: number, path: string): number {
  const sum = left + right;
  if (!Number.isSafeInteger(sum)) {
    fail(path, 'aggregate count overflowed safe integer range');
  }
  return sum;
}

function required<K, V>(map: ReadonlyMap<K, V>, key: K, label: string): V {
  const value = map.get(key);
  if (value === undefined) {
    throw new Error(`${label} ${String(key)} is unavailable`);
  }
  return value;
}

function fail(path: string, detail: string): never {
  throw new Error(`${path} ${detail}`);
}
