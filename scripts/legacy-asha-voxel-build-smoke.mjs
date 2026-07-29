#!/usr/bin/env node
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { createNativeRuntimeBridge, createRuntimeSessionFacade } from '@asha/runtime-bridge';
import { compilePlacementExtrusion } from '../dist/ts/src/voxel-extrusion.js';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const placementPath = path.join(
  repoRoot,
  process.argv[2] ?? 'artifacts/samples/batch-v2/candidate-006/piece-placement.json',
);
const placement = JSON.parse(await readFile(placementPath, 'utf8'));
const plan = compilePlacementExtrusion(placement);
const engineRoot = path.resolve(repoRoot, '../asha-engine');
const engineCommit = execFileSync('git', ['rev-parse', 'HEAD'], {
  cwd: engineRoot,
  encoding: 'utf8',
}).trim();
const source = conversionSource(plan);
const request = conversionRequest(plan, source.source);

const first = runDirectAuthorityBuild('rusty-procgen.voxel-extrusion.first', plan.commands);
const second = runDirectAuthorityBuild('rusty-procgen.voxel-extrusion.repeat', plan.commands);
assert.equal(second.history.cursor.voxelStateHash, first.history.cursor.voxelStateHash);

const beforeRejected = first.history;
const rejected = first.session.submitCommands({
  commands: [{
    op: 'setVoxel',
    grid: 1,
    coord: plan.solidVoxels[0].coord,
    value: { kind: 'solid', material: 65535 },
  }],
});
assert.equal(rejected.result.accepted, 0);
assert.equal(rejected.result.rejected, 1);
assert.deepEqual(rejected.result.rejections, [{
  reason: 'unknownMaterial',
  material: 65535,
}]);
const afterRejected = readHistory(first.session);
assert.equal(afterRejected.cursor.voxelStateHash, beforeRejected.cursor.voxelStateHash);
assert.equal(afterRejected.cursor.entryCount, beforeRejected.cursor.entryCount);
assert.equal(afterRejected.historyHash, beforeRejected.historyHash);

const comparison = runConversionComparison(source, request);
assert.equal(comparison.registration.registered, true, diagnosticText(comparison.registration));
assert.equal(comparison.plan.diagnostics.length, 0, diagnosticText(comparison.plan));
assert.equal(comparison.preview.diagnostics.length, 0, diagnosticText(comparison.preview));
assert.equal(comparison.preview.outputVoxelCount, plan.solidVoxelCount);
assert.equal(comparison.receipt.applied, true, diagnosticText(comparison.receipt));
assert.equal(comparison.receipt.outputVoxelCount, plan.solidVoxelCount);
assert.equal(comparison.model.resident, true, diagnosticText(comparison.model));
assert.equal(comparison.model.voxelCount, plan.solidVoxelCount);
const expectedMaterialCounts = materialCounts(plan.solidVoxels);
assert.deepEqual(comparison.model.materialCounts, expectedMaterialCounts);
const readbackSamples = [1, 2, 3].map((material) => {
  const voxel = plan.solidVoxels.find((candidate) => candidate.material === material);
  assert.ok(voxel, `extrusion must contain material ${material}`);
  const readout = comparison.session.readVoxelModelWindow({
    grid: request.target.grid,
    volumeAssetId: request.target.volumeAssetId,
    bounds: { min: voxel.coord, max: voxel.coord },
    includeEmpty: false,
    materialFilter: [material],
    maxSamples: 1,
  });
  assert.equal(readout.resident, true, diagnosticText(readout));
  assert.deepEqual(readout.samples, [{ coord: voxel.coord, occupied: true, material }]);
  return readout.samples[0];
});

const staleConversion = comparison.session.applyVoxelConversion({
  planId: comparison.plan.planId,
  expectedPlanHash: comparison.plan.planHash,
  expectedPreviewHash: 'fnv1a64:stale-preview-proof',
});
assert.equal(staleConversion.applied, false);
const comparisonAfterRejected = comparison.session.readVoxelModelInfo(modelRequest(request));
assert.equal(comparisonAfterRejected.latestOutputHash, comparison.model.latestOutputHash);
assert.equal(comparisonAfterRejected.sessionHash, comparison.model.sessionHash);

const evidence = {
  kind: 'rusty_procgen.evidence.native_voxel_extrusion.v1',
  sourcePlacement: path.relative(repoRoot, placementPath),
  placementId: plan.placementId,
  backend: 'native_rust_runtime_session_voxel_commands',
  ashaEngineCommit: engineCommit,
  coordinateMapping: plan.coordinateMapping,
  placementPolicy: placement.placementPolicy,
  enclosure: {
    floorY: 0,
    wallY: [1, 3],
    ceilingY: 4,
    floorMaterial: 2,
    wallMaterial: 1,
    ceilingMaterial: 3,
  },
  counts: {
    walkableCells: plan.walkableCellCount,
    declaredOpeningCells: plan.openingCellCount,
    boundaryCells: plan.boundaryCellCount,
    solidVoxels: plan.solidVoxelCount,
    requiredChunksForDirectCommandLane: plan.residentChunkCount,
    sourceVertices: source.positions.length,
    sourceTriangles: source.triangles.length,
  },
  buildBounds: plan.buildBounds,
  authority: {
    mutationPath: 'RuntimeSessionFacade.submitCommands',
    phases: first.phases,
    submittedCommands: plan.commands.length,
    acceptedCommands: first.phases.reduce((total, phase) => total + phase.accepted, 0),
    rejectedCommands: first.phases.reduce((total, phase) => total + phase.rejected, 0),
    historyEntryCount: first.history.cursor.entryCount,
    voxelStateHash: first.history.cursor.voxelStateHash,
    deterministicRepeatHash: second.history.cursor.voxelStateHash,
    deterministic: second.history.cursor.voxelStateHash === first.history.cursor.voxelStateHash,
    rejectedUnknownMaterialWithoutMutation:
      afterRejected.cursor.voxelStateHash === beforeRejected.cursor.voxelStateHash
      && afterRejected.historyHash === beforeRejected.historyHash,
    unknownMaterialRejection: rejected.result.rejections[0],
  },
  conversionComparison: {
    role: 'bounded_model_readback_comparison_only',
    registered: comparison.registration.registered,
    plannedOutputVoxels: comparison.plan.estimatedOutputVoxels,
    previewVoxelCount: comparison.preview.outputVoxelCount,
    appliedVoxelCount: comparison.receipt.outputVoxelCount,
    residentVoxelCount: comparison.model.voxelCount,
    materialCounts: comparison.model.materialCounts,
    boundedReadbackSamples: readbackSamples,
    outputHash: comparison.receipt.outputHash,
    rejectedStalePreviewWithoutMutation:
      comparisonAfterRejected.latestOutputHash === comparison.model.latestOutputHash
      && comparisonAfterRejected.sessionHash === comparison.model.sessionHash,
  },
  nonClaims: [
    'not_3d_piece_placement',
    'not_exit_socket_alignment_proof',
    'not_renderer_proof',
    'not_navigation_proof',
    'not_performance_proof',
    'not_mesh_fidelity_proof',
  ],
};
const evidencePath = path.join(repoRoot, 'artifacts/evidence/native-voxel-extrusion.json');
await mkdir(path.dirname(evidencePath), { recursive: true });
await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
console.log(JSON.stringify({ ...evidence, evidencePath: path.relative(repoRoot, evidencePath) }, null, 2));

function runDirectAuthorityBuild(sessionId, commands) {
  const session = createSession(sessionId);
  const commandPhases = [
    ['generate_chunks', commands.filter((command) => command.op === 'generateChunk')],
    ['clear_chunks', commands.filter((command) => command.op === 'fillRegion')],
    ['set_solids', commands.filter((command) => command.op === 'setVoxel')],
  ].flatMap(([name, phaseCommands]) => chunkCommands(name, phaseCommands, 5_000));
  const phases = commandPhases.map(([name, phaseCommands]) => {
    assert.ok(phaseCommands.length > 0, `${name} phase must not be empty`);
    const receipt = session.submitCommands({ commands: phaseCommands });
    assert.deepEqual(receipt.result, {
      accepted: phaseCommands.length,
      rejected: 0,
      rejections: [],
    });
    return {
      name,
      submitted: phaseCommands.length,
      accepted: receipt.result.accepted,
      rejected: receipt.result.rejected,
    };
  });
  const history = readHistory(session);
  assert.equal(history.cursor.entryCount, commandPhases.length);
  assert.deepEqual(history.entries.map((entry) => entry.commandCount), commandPhases.map(([, batch]) => batch.length));
  return { session, phases, history };
}

function chunkCommands(name, commands, limit) {
  const chunks = [];
  for (let index = 0; index < commands.length; index += limit) {
    chunks.push([`${name}_${chunks.length + 1}`, commands.slice(index, index + limit)]);
  }
  return chunks;
}

function runConversionComparison(sourceInput, planRequest) {
  const session = createSession('rusty-procgen.voxel-extrusion.conversion-comparison');
  const registration = session.registerVoxelConversionSource(sourceInput);
  const authorityPlan = session.planVoxelConversion(planRequest);
  const preview = session.previewVoxelConversion({
    planId: authorityPlan.planId,
    expectedPlanHash: authorityPlan.planHash,
  });
  const receipt = session.applyVoxelConversion({
    planId: authorityPlan.planId,
    expectedPlanHash: authorityPlan.planHash,
    expectedPreviewHash: preview.outputHash,
  });
  const model = session.readVoxelModelInfo(modelRequest(planRequest));
  return { session, registration, plan: authorityPlan, preview, receipt, model };
}

function createSession(sessionId) {
  const bridge = createNativeRuntimeBridge();
  const session = createRuntimeSessionFacade({ bridge, mode: 'rust' });
  session.initialize({
    sessionId,
    seed: 5201,
    project: { gameId: 'rusty-procgen', workspaceId: 'workspace.local' },
  });
  return session;
}

function readHistory(session) {
  return session.readVoxelEditHistory({
    historyId: 'history/default',
    cursorId: null,
    maxEntries: 8,
    includeRedoTail: true,
    expectedHistoryHash: null,
  });
}

function conversionSource(extrusion) {
  const positions = [];
  const triangles = [];
  const materialSlots = new Map([[1, 0], [2, 1], [3, 2]]);
  for (const voxel of extrusion.solidVoxels) {
    const base = positions.length;
    const { x, y, z } = voxel.coord;
    positions.push(
      [x + 0.2, y + 0.2, z + 0.2],
      [x + 0.4, y + 0.2, z + 0.2],
      [x + 0.2, y + 0.4, z + 0.2],
    );
    triangles.push({
      indices: [base, base + 1, base + 2],
      sourceMaterialSlot: materialSlots.get(voxel.material),
    });
  }
  const sourceHash = `sha256:${createHash('sha256').update(JSON.stringify({ positions, triangles })).digest('hex')}`;
  return {
    source: {
      assetId: 'mesh/rusty-procgen-first-extrusion',
      assetKind: 'mesh',
      assetVersion: 1,
      sourceHash,
      meshPrimitive: 'voxel-cell-surface-samples',
    },
    positions,
    triangles,
    materialSlots: [
      { sourceMaterialSlot: 0, sourceMaterialId: 'material/wall' },
      { sourceMaterialSlot: 1, sourceMaterialId: 'material/floor' },
      { sourceMaterialSlot: 2, sourceMaterialId: 'material/ceiling' },
    ],
  };
}

function conversionRequest(extrusion, sourceRef) {
  const min = extrusion.buildBounds.min;
  const max = extrusion.buildBounds.maxExclusive;
  return {
    source: sourceRef,
    target: {
      grid: 1,
      volumeAssetId: 'voxel/generated',
      origin: min,
    },
    settings: {
      mode: 'surface',
      fitPolicy: 'stretch',
      originPolicy: 'target_min',
      resolution: [max.x - min.x, max.y - min.y, max.z - min.z],
      voxelSize: 1,
      maxOutputVoxels: extrusion.solidVoxelCount,
      transform: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
      materialMap: {
        entries: [
          { sourceMaterialSlot: 0, sourceMaterialId: 'material/wall', voxelMaterial: 1 },
          { sourceMaterialSlot: 1, sourceMaterialId: 'material/floor', voxelMaterial: 2 },
          { sourceMaterialSlot: 2, sourceMaterialId: 'material/ceiling', voxelMaterial: 3 },
        ],
        textureAssets: [],
        textureBindings: [],
        defaultVoxelMaterial: null,
      },
    },
  };
}

function modelRequest(planRequest) {
  return {
    grid: planRequest.target.grid,
    volumeAssetId: planRequest.target.volumeAssetId,
    includeMaterialCounts: true,
  };
}

function diagnosticText(value) {
  return JSON.stringify(value.diagnostics ?? []);
}

function materialCounts(voxels) {
  const counts = new Map();
  for (const voxel of voxels) {
    counts.set(voxel.material, (counts.get(voxel.material) ?? 0) + 1);
  }
  return [...counts.entries()]
    .sort(([left], [right]) => left - right)
    .map(([material, voxelCount]) => ({ material, voxelCount }));
}
