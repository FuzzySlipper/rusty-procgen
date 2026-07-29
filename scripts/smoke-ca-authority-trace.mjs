import { readFile } from 'node:fs/promises';

import {
  compileCaScenario,
  decodeCaBenchmarkEvidence,
} from '../dist/ts/src/ca-authority-trace.js';
import {
  fnv1a64Json,
  sha256Json,
  sha256Text,
} from '../dist/ts/src/ca-trace-hash.js';

const artifactPath = 'artifacts/evidence/engine-ca-benchmark.json';
const source = await readFile(artifactPath, 'utf8');
const input = JSON.parse(source);

if (sha256Text('abc') !== 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad') {
  throw new Error('browser-safe SHA-256 implementation failed the canonical abc vector');
}

const evidence = decodeCaBenchmarkEvidence(input);
const expectedScenarios = [
  'sparse-propagation',
  'dense-churn',
  'cross-boundary',
  'large-resident-small-hot-region',
  'high-surface-area',
];
if (JSON.stringify(evidence.scenarios.map((scenario) => scenario.scenarioId)) !== JSON.stringify(expectedScenarios)) {
  throw new Error('captured CA scenario set does not match the accepted benchmark corpus');
}

const compiled = evidence.scenarios.map(compileCaScenario);
for (const scenario of compiled) {
  const voxelAsset = scenario.initialFrame.ops.find((op) => op.op === 'defineVoxelObject');
  if (
    voxelAsset === undefined
    || voxelAsset.asset.meshes.length === 0
    || voxelAsset.asset.meshes.some((mesh) => mesh.payload.source.kind !== 'inline')
  ) {
    throw new Error(`${scenario.evidence.scenarioId} does not retain real public Engine mesh buffers`);
  }
  if (
    scenario.initialFrame.ops.some((op) => op.op === 'createMeshInstance')
    || scenario.stepFrames.some((frame) => frame.ops.some((op) => op.op === 'createMeshInstance'))
  ) {
    throw new Error(`${scenario.evidence.scenarioId} regenerated generic mesh instances instead of retaining voxel facts`);
  }
  const expectedCounts = [
    scenario.evidence.trace.initial.readout.meshChunkCount,
    ...scenario.evidence.trace.steps.map((step) => step.readout.meshChunkCount),
  ];
  if (JSON.stringify(scenario.chunkCountAfterStep) !== JSON.stringify(expectedCounts)) {
    throw new Error(`${scenario.evidence.scenarioId} compiled chunk counts disagree with Engine readouts`);
  }
  const repeated = compileCaScenario(scenario.evidence);
  if (
    JSON.stringify(repeated.initialFrame) !== JSON.stringify(scenario.initialFrame)
    || JSON.stringify(repeated.stepFrames) !== JSON.stringify(scenario.stepFrames)
  ) {
    throw new Error(`${scenario.evidence.scenarioId} retained-frame compilation is not deterministic`);
  }
}

expectRejected('schema version', (candidate) => {
  candidate.schemaVersion = 2;
});
expectRejected('unknown root field', (candidate) => {
  candidate.browserAuthority = true;
});
expectRejected('initial trace hash', (candidate) => {
  candidate.scenarios[0].trace.initial.traceHash = 'sha256:0000000000000000000000000000000000000000000000000000000000000000';
});
expectRejected('mesh buffer mutation', (candidate) => {
  candidate.scenarios[0].trace.initial.projectionChunks[0].positions[0] += 0.25;
});
expectRejected('projection diff mutation', (candidate) => {
  candidate.scenarios[0].trace.steps[0].projectionOps.pop();
});
expectRejected('trace-chain mutation', (candidate) => {
  candidate.scenarios[0].trace.steps[1].previousTraceHash =
    'sha256:1111111111111111111111111111111111111111111111111111111111111111';
});
expectRejected('arbitrary equal structural hashes', (candidate) => {
  for (const run of candidate.scenarios[0].recordedRuns) {
    run.structuralHash =
      'sha256:2222222222222222222222222222222222222222222222222222222222222222';
  }
}, /recordedRuns\[0\]\.structuralHash/);
expectRejected('initial CA cumulative root', (candidate) => {
  candidate.scenarios[0].trace.initial.initialCaCumulativeHash =
    'fnv1a64:3333333333333333';
}, /initial\.initialCaCumulativeHash/);
expectRejected('re-chained CA state count mutation', (candidate) => {
  const trace = candidate.scenarios[0].trace;
  trace.steps[0].ca.activeCellCount += 1;
  trace.steps[0].ca.stateCounts.frontier += 1;
  rechainTrace(trace);
}, /recordedRuns\[0\]\.structuralHash/);

const inspectedSources = await Promise.all([
  readFile('src/ca-authority-trace.ts', 'utf8'),
  readFile('src/ca-trace-hash.ts', 'utf8'),
  readFile('viewer/ca-trace-viewer.ts', 'utf8'),
]);
const inspected = inspectedSources.join('\n');
for (const [label, pattern] of [
  ['Asha import', /@asha|asha-engine/i],
  ['renderer backend import', /@rusty-engine\/renderer-three|from ['"][^'"]*renderer-three/i],
  ['deep Engine import', /@rusty-engine\/[^'"]+\/(?:src|dist|backend)\//i],
  ['browser-owned CA authority', /\bRuntimeSession\b|\blocalAuthority\b|\brunCellularAutomata\b/],
  ['raw renderer transport', /\bWebGLRenderer\b|\bTHREE\b|from ['"]three['"]/],
]) {
  if (pattern.test(inspected)) {
    throw new Error(`CA trace viewer crossed the ${label} boundary`);
  }
}

console.log(JSON.stringify({
  artifact: artifactPath,
  scenarios: compiled.map((scenario) => ({
    id: scenario.evidence.scenarioId,
    steps: scenario.stepFrames.length,
    retainedMeshFrames: scenario.initialFrame.ops
      .find((op) => op.op === 'defineVoxelObject').asset.meshes.length,
    finalChunks: scenario.chunkCountAfterStep.at(-1),
  })),
  tamperCases: 9,
  authority: 'captured_hash_chained_engine_evidence',
  timingRole: 'observational_non_gating',
}, null, 2));

function expectRejected(label, mutate, expected = null) {
  const candidate = structuredClone(input);
  mutate(candidate);
  try {
    decodeCaBenchmarkEvidence(candidate);
  } catch (error) {
    if (expected !== null && !expected.test(String(error))) {
      throw new Error(`${label} rejected for the wrong reason: ${String(error)}`);
    }
    return;
  }
  throw new Error(`${label} tamper was accepted`);
}

function rechainTrace(trace) {
  let previousCaHash = trace.initial.initialCaCumulativeHash;
  let previousTraceHash = trace.initial.traceHash;
  for (const step of trace.steps) {
    step.ca.cumulativeScenarioHash = fnv1a64Json({
      previousHash: previousCaHash,
      deltaHash: step.ca.deltaHash,
      stateHash: step.ca.stateHash,
      step: step.ca.step,
      activeCellCount: step.ca.activeCellCount,
      touchedBounds: step.ca.touchedBounds,
    });
    step.previousTraceHash = previousTraceHash;
    step.traceHash = sha256Json([
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
    previousCaHash = step.ca.cumulativeScenarioHash;
    previousTraceHash = step.traceHash;
  }
}
