#!/usr/bin/env node
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  ASHA_PROJECT_BUNDLE_MANIFEST_PATH,
  createMemoryAshaProjectSource,
  serializeAshaPrefabRegistrySource,
} from '@asha/game-workspace';
import { createNativeRuntimeBridge, createRuntimeSessionFacade } from '@asha/runtime-bridge';
import {
  ProcgenPublishError,
  compileProcgenPrefabPublication,
} from '../dist/ts/src/prefab-publishing.js';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const readJson = async (relativePath) => JSON.parse(await readFile(path.join(repoRoot, relativePath), 'utf8'));
const catalog = await readJson('fixtures/shape-catalogs/2d-basic.json');
const shapeMatch = await readJson('artifacts/samples/batch-v2/candidate-006/piece-shape-match.json');
const placement = await readJson('artifacts/samples/batch-v2/candidate-006/piece-placement.json');
const configuration = await readJson('fixtures/prefab-mappings/first-slice.json');
const sourceAssetBodies = new Map(configuration.sourceAssets.map((source) => [
  source.artifact,
  `${JSON.stringify({
    kind: 'asha_procgen.voxel_object_source.v1',
    assetId: source.assetId,
    voxels: [],
  }, null, 2)}\n`,
]));
const canonicalConfiguration = {
  ...configuration,
  sourceAssets: configuration.sourceAssets.map((source) => ({
    ...source,
    contentHash: contentHash(sourceAssetBodies.get(source.artifact)),
  })),
};
const compile = (overrides = {}) => compileProcgenPrefabPublication({
  catalog: overrides.catalog ?? catalog,
  shapeMatch: overrides.shapeMatch ?? shapeMatch,
  placement: overrides.placement ?? placement,
  configuration: overrides.configuration ?? canonicalConfiguration,
});

const publication = compile();
assert.equal(publication.prefabRegistry.definitions.length, 2);
assert.equal(publication.prefabInstancesArtifact.prefabInstances.length, 2);
assert.deepEqual(
  publication.prefabInstancesArtifact.prefabInstances.map((instance) => [instance.instance, instance.prefab]),
  [[2001, 1003], [2002, 1002]],
);
assert.deepEqual(publication.prefabInstancesArtifact.prefabInstances[1].transform.rotation, [0, 1, 0, 0]);
assert.equal(publication.sceneArtifact.nodes.length, 2);
assert.equal(publication.sceneArtifact.nodes[0].kind.kind, 'voxelVolume');
assert.equal(publication.manifest.bundleSchemaVersion, 2);
assert.equal(publication.manifest.entryScene, publication.sceneArtifact.id);
assert.deepEqual(publication.manifest.scenes.map((scene) => scene.id), [publication.sceneArtifact.id]);
assert.equal(publication.manifest.generationProvenance?.provider, 'asha-procgen.prefab-publisher');
assert.equal(publication.manifest.artifacts.find((artifact) => artifact.role === 'prefabRegistry')?.path, 'prefabs/registry.json');
assert.equal(
  publication.manifest.artifacts.filter((artifact) => artifact.role === 'resource:procgen-prefab-source').length,
  2,
);
assert.equal(publication.provenance.instances[0].shapeId, 'shape.room.flow_junction.spaced_8_exit');
assert.equal(publication.provenance.instances[0].matchScore, 1035);
assert.match(serializeAshaPrefabRegistrySource(publication.prefabRegistry), /"schemaVersion": 1/);

const runtimeSession = createRuntimeSessionFacade({
  bridge: createNativeRuntimeBridge(),
  mode: 'rust',
});
runtimeSession.initialize({
  sessionId: 'asha-procgen.prefab-publication-admission',
  seed: 5201,
  project: { gameId: 'asha-procgen', workspaceId: 'prefab-publication' },
});
const legacyReceipt = await runtimeSession.loadProject({
  source: createMemoryAshaProjectSource(
    'memory:asha-procgen-prefab-publication-v1-rejection',
    publicationProjectFiles(
      { ...publication, manifest: { ...publication.manifest, bundleSchemaVersion: 1 } },
      canonicalConfiguration,
      sourceAssetBodies,
    ),
  ),
});
assert.equal(legacyReceipt.accepted, false);
assert.equal(legacyReceipt.diagnostics[0]?.phase, 'sourceBatch');
assert.match(legacyReceipt.diagnostics[0]?.message ?? '', /unsupported bundle schema version 1/);
const projectReceipt = await runtimeSession.loadProject({
  source: createMemoryAshaProjectSource(
    'memory:asha-procgen-prefab-publication',
    publicationProjectFiles(publication, canonicalConfiguration, sourceAssetBodies),
  ),
});
assert.equal(projectReceipt.accepted, false);
assert.deepEqual(
  projectReceipt.diagnostics.map(({ phase, code }) => ({ phase, code })),
  [{ phase: 'lifecycle', code: 'missingStaticComposition' }],
  JSON.stringify(projectReceipt.diagnostics),
);

const repeatedShapeConfiguration = {
  ...configuration,
  selectedInstanceIds: [
    'instance.piece_room_room_region_gate_locked_1',
    'instance.piece_room_room_region_key_gate_1',
  ],
  mappings: [{
    ...configuration.mappings[0],
    shapeId: 'shape.room.flow_junction.36_exit',
    prefabId: 1003,
    source: { kind: 'voxelObject', asset: 'voxel-object/procgen-standard-room' },
  }],
  instanceIdentities: [
    { procgenInstanceId: 'instance.piece_room_room_region_gate_locked_1', prefabInstanceId: 2010 },
    { procgenInstanceId: 'instance.piece_room_room_region_key_gate_1', prefabInstanceId: 2009 },
  ],
};
const repeatedShapePublication = compile({ configuration: repeatedShapeConfiguration });
assert.equal(repeatedShapePublication.prefabRegistry.definitions.length, 1);
assert.equal(repeatedShapePublication.sceneArtifact.dependencies.length, 1);
const expectedRepeatedShapeTransforms = repeatedShapeConfiguration.instanceIdentities.map((identity) => {
  const instance = placement.instances.find(
    (candidate) => candidate.instanceId === identity.procgenInstanceId,
  );
  assert.ok(instance, `missing repeated-shape placement ${identity.procgenInstanceId}`);
  return [identity.prefabInstanceId, [instance.origin.x, 0, instance.origin.y]];
});
assert.deepEqual(
  repeatedShapePublication.sceneArtifact.nodes.map((node) => [node.id, node.transform.translation]),
  expectedRepeatedShapeTransforms,
);

expectFailure('missingPrefabMapping', () => compile({
  configuration: {
    ...configuration,
    mappings: configuration.mappings.filter(
      (mapping) => mapping.shapeId !== 'shape.room.flow_junction.spaced_8_exit',
    ),
  },
}));
expectFailure('missingStableRole', () => compile({
  configuration: {
    ...configuration,
    mappings: configuration.mappings.map((mapping) =>
      mapping.shapeId === 'shape.room.flow_junction.spaced_8_exit'
        ? { ...mapping, stableRole: '' }
        : mapping),
  },
}));
expectFailure('incompatibleSourceAsset', () => compile({
  configuration: {
    ...configuration,
    mappings: configuration.mappings.map((mapping) =>
      mapping.shapeId === 'shape.room.flow_junction.spaced_8_exit'
        ? { ...mapping, source: { kind: 'voxelObject', asset: 'scene/procgen-standard-room' } }
        : mapping),
  },
}));
expectFailure('duplicateInstanceIdentity', () => compile({
  configuration: {
    ...configuration,
    instanceIdentities: configuration.instanceIdentities.map((identity) => ({
      ...identity,
      prefabInstanceId: 2001,
    })),
  },
}));
expectFailure('invalidTransform', () => compile({
  placement: {
    ...placement,
    instances: placement.instances.map((instance) =>
      instance.instanceId === configuration.selectedInstanceIds[0]
        ? { ...instance, origin: { ...instance.origin, x: Number.NaN } }
        : instance),
  },
}));

const evidence = {
  ...publication,
  proof: {
    adapter: 'compileProcgenPrefabPublication',
    publicPackages: ['@asha/contracts', '@asha/game-workspace', '@asha/runtime-bridge'],
    canonicalSourceAdmission: {
      schemaVersion: publication.manifest.bundleSchemaVersion,
      reachedRuntimeComposition: projectReceipt.diagnostics[0]?.phase === 'lifecycle',
      runtimeActivationNonClaim: projectReceipt.diagnostics[0]?.code ?? null,
      legacySchemaRejectedAt: legacyReceipt.diagnostics[0]?.phase ?? null,
    },
    failClosedCases: [
      'missingPrefabMapping',
      'missingStableRole',
      'incompatibleSourceAsset',
      'duplicateInstanceIdentity',
      'invalidTransform',
    ],
  },
};
const evidencePath = path.join(repoRoot, 'artifacts/evidence/prefab-project-bundle-publication.json');
await mkdir(path.dirname(evidencePath), { recursive: true });
await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
console.log(JSON.stringify({
  evidencePath: path.relative(repoRoot, evidencePath),
  prefabDefinitions: publication.prefabRegistry.definitions.length,
  prefabInstances: publication.prefabInstancesArtifact.prefabInstances.length,
  projectBundleArtifacts: publication.manifest.artifacts.length,
  failClosedCases: evidence.proof.failClosedCases,
}, null, 2));

function expectFailure(code, callback) {
  assert.throws(callback, (error) => error instanceof ProcgenPublishError && error.code === code);
}

function publicationProjectFiles(publication, configuration, assetBodies) {
  const text = (value) => new TextEncoder().encode(value);
  const files = new Map([
    [ASHA_PROJECT_BUNDLE_MANIFEST_PATH, text(`${JSON.stringify(publication.manifest)}\n`)],
    [configuration.assetLockArtifact, text(prettyJson(publication.assetLockArtifact))],
    [configuration.prefabRegistryArtifact, text(serializeAshaPrefabRegistrySource(publication.prefabRegistry))],
    [configuration.prefabInstancesArtifact, text(prettyJson(publication.prefabInstancesArtifact))],
    [configuration.scene.artifact, text(prettyJson(publication.sceneArtifact))],
  ]);
  for (const source of configuration.sourceAssets) {
    const body = assetBodies.get(source.artifact);
    assert.notEqual(body, undefined, `missing publication source body for ${source.artifact}`);
    files.set(source.artifact, text(body));
  }
  return files;
}

function contentHash(value) {
  const bytes = new TextEncoder().encode(value ?? '');
  let hash = 0xcbf29ce484222325n;
  for (const byte of bytes) {
    hash ^= BigInt(byte);
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return hash.toString(16).padStart(16, '0');
}

function prettyJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}
