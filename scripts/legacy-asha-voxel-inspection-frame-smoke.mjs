import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import { ASHA_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS } from '@asha/renderer-host';

import { buildVoxelInspectionProjection } from '../dist/ts/src/voxel-inspection-projection.js';
import { compilePlacementExtrusion } from '../dist/ts/src/voxel-extrusion.js';

const repoRoot = resolve(import.meta.dirname, '..');
const selection = await readJson('artifacts/samples/batch-v2/selection-report.json');
const acceptedEntries = selection.accepted.filter((entry) => typeof entry.piecePlacementRef === 'string');
const firstEntry = acceptedEntries[0];
const distinctEntry = acceptedEntries.find((entry) => (
  entry.topologyFingerprint !== firstEntry?.topologyFingerprint
));
const entries = firstEntry === undefined || distinctEntry === undefined
  ? acceptedEntries
  : [firstEntry, distinctEntry, ...acceptedEntries.filter((entry) => (
      entry !== firstEntry && entry !== distinctEntry
    ))];
if (entries.length === 0) {
  throw new Error('voxel inspection smoke requires an accepted piece placement');
}

const projections = [];
for (const entry of entries) {
  const placement = await readJson(entry.piecePlacementRef);
  const plan = compilePlacementExtrusion(placement);
  const projection = buildVoxelInspectionProjection(plan);
  const repeated = buildVoxelInspectionProjection(plan);
  if (JSON.stringify(projection.frame) !== JSON.stringify(repeated.frame)) {
    throw new Error(`inspection frame for ${plan.placementId} is not deterministic`);
  }
  assertProjectionExactlyRepresentsPlan(plan, projection);
  if (projection.frame.ops.length > ASHA_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS) {
    throw new Error(
      `inspection frame for ${plan.placementId} has ${projection.frame.ops.length} ops; limit is ${ASHA_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS}`,
    );
  }
  if (projection.omittedCeilingVoxelCount <= 0) {
    throw new Error(`inspection frame for ${plan.placementId} omitted no ceiling voxels`);
  }
  if (projection.projectedVoxelCount + projection.omittedCeilingVoxelCount !== plan.solidVoxelCount) {
    throw new Error(`inspection voxel accounting does not match ${plan.placementId}`);
  }
  const creates = projection.frame.ops.filter((op) => op.op === 'create');
  if (creates.length !== projection.projectedNodeCount + projection.doorNodeCount) {
    throw new Error(`inspection frame for ${plan.placementId} has unexpected create count`);
  }
  const voxelCreates = creates.filter((op) => op.node.metadata.label?.startsWith('procgen-voxel-box:'));
  const doorCreates = creates.filter((op) => op.node.metadata.label?.startsWith('procgen-door:'));
  if (doorCreates.length !== plan.doorPortals.reduce((count, portal) => count + portal.cells.length, 0)) {
    throw new Error(`inspection frame for ${plan.placementId} did not project every verified door cell`);
  }
  if (doorCreates.some((op) => op.node.material.color[3] >= 1)) {
    throw new Error(`inspection frame for ${plan.placementId} contains an opaque door`);
  }
  for (const portal of plan.doorPortals) {
    const portalCreates = doorCreates.filter((op) => op.node.metadata.label?.includes(`:${portal.id}:`));
    if (portalCreates.length !== portal.cells.length) {
      throw new Error(`portal ${portal.id} did not create one door cuboid per width cell`);
    }
    const expectedColor = portal.requiredItem === null
      ? [0.16, 0.48, 0.98, 0.48]
      : [0.93, 0.16, 0.2, 0.56];
    const expectedScale = portal.orientation === 'east' || portal.orientation === 'west'
      ? [0.18, portal.maxExclusiveY - portal.minY, 1]
      : [1, portal.maxExclusiveY - portal.minY, 0.18];
    if (portalCreates.some((op) => (
      JSON.stringify(op.node.material.color) !== JSON.stringify(expectedColor)
      || JSON.stringify(op.node.transform.scale) !== JSON.stringify(expectedScale)
    ))) {
      throw new Error(`portal ${portal.id} has incorrect RGBA, orientation, width, or height`);
    }
  }
  const portalIds = new Set(plan.doorPortals.map((portal) => portal.id));
  const allUnlocked = buildVoxelInspectionProjection(plan, {
    includedPortalIds: portalIds,
    openPortalIds: portalIds,
  });
  if (
    allUnlocked.lockedDoorCount !== 0
    || allUnlocked.unlockedDoorCount !== plan.doorPortals.length
    || JSON.stringify(allUnlocked.frame) === JSON.stringify(projection.frame)
  ) {
    throw new Error(`inspection frame for ${plan.placementId} did not transition every door to blue`);
  }
  const hidden = buildVoxelInspectionProjection(plan, {
    includedPortalIds: new Set(),
    openPortalIds: new Set(),
  });
  if (hidden.doorNodeCount !== 0 || hidden.frame.ops.some((op) => op.op === 'create' && op.node.metadata.label?.startsWith('procgen-door:'))) {
    throw new Error(`inspection frame for ${plan.placementId} retained stale door overlays`);
  }
  for (const op of voxelCreates) {
    const maxY = op.node.transform.translation[1] + op.node.transform.scale[1] / 2;
    if (maxY > projection.ceilingY) {
      throw new Error(`inspection frame for ${plan.placementId} contains a ceiling voxel`);
    }
  }
  if (projection.frame.ops.filter((op) => op.op === 'createLight').length !== 2) {
    throw new Error(`inspection frame for ${plan.placementId} is missing engine light descriptors`);
  }
  if (
    projection.grid.visible !== true
    || projection.grid.plane !== 'xz'
    || projection.grid.grid.coordinateSystem !== 'rightHandedYUp'
    || projection.grid.grid.spacing.some((spacing) => spacing !== 1)
  ) {
    throw new Error(`inspection projection for ${plan.placementId} has an invalid engine grid descriptor`);
  }
  projections.push(projection);
}

const focusedProjection = buildVoxelInspectionProjection(fragmentedMultiMaterialPlan());
assertProjectionExactlyRepresentsPlan(fragmentedMultiMaterialPlan(), focusedProjection);
if (focusedProjection.projectedNodeCount !== 5) {
  throw new Error(
    `focused cuboid compaction expected 5 nodes, received ${focusedProjection.projectedNodeCount}`,
  );
}

if (projections.length > 1) {
  if (
    projections[0].placementId === projections[1].placementId
    || JSON.stringify(projections[0].frame) === JSON.stringify(projections[1].frame)
  ) {
    throw new Error('candidate switching did not produce a distinct deterministic inspection frame');
  }
}

const viewerSource = await readFile(resolve(repoRoot, 'viewer/app.ts'), 'utf8');
const projectionSource = await readFile(resolve(repoRoot, 'src/voxel-inspection-projection.ts'), 'utf8');
if (!/from '@asha\/renderer-host'/.test(viewerSource)) {
  throw new Error('viewer does not import the engine renderer host from its package root');
}
if (!/initialGrid: projection\.grid/.test(viewerSource) || !/surface\.setGrid\(projection\.grid\)/.test(viewerSource)) {
  throw new Error('viewer does not mount and replace the public engine grid with its voxel projection');
}
const forbiddenRendererImport = /(?:from\s+|import\()['"](?:three(?:\/|['"])|@asha\/renderer-three)/;
if (forbiddenRendererImport.test(`${viewerSource}\n${projectionSource}`)) {
  throw new Error('voxel inspection source contains a direct renderer implementation import');
}

console.log(
  `voxel inspection frame smoke passed; ${projections.length} layouts, max ${Math.max(...projections.map((projection) => projection.frame.ops.length))}/${ASHA_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS} frame ops; largest layout ${projections[0].projectedVoxelCount} voxels in ${projections[0].projectedNodeCount} nodes`,
);

function assertProjectionExactlyRepresentsPlan(plan, projection) {
  const expected = new Map();
  for (const voxel of plan.solidVoxels) {
    if (voxel.coord.y !== projection.ceilingY) {
      expected.set(cellKey(voxel.coord.x, voxel.coord.y, voxel.coord.z), voxel.material);
    }
  }

  const represented = new Map();
  for (const op of projection.frame.ops) {
    if (op.op !== 'create') {
      continue;
    }
    if (!op.node.metadata.label?.startsWith('procgen-voxel-box:')) {
      continue;
    }
    const match = /:material-(\d+):voxels-(\d+)$/.exec(op.node.metadata.label ?? '');
    if (match === null) {
      throw new Error(`inspection node ${op.handle} has no material/voxel evidence label`);
    }
    const material = Number(match[1]);
    const labelledVoxelCount = Number(match[2]);
    const min = op.node.transform.translation.map((value, axis) =>
      value - op.node.transform.scale[axis] / 2);
    const maxExclusive = op.node.transform.translation.map((value, axis) =>
      value + op.node.transform.scale[axis] / 2);
    if (
      min.some((value) => !Number.isInteger(value))
      || maxExclusive.some((value) => !Number.isInteger(value))
      || op.node.transform.scale.some((value) => !Number.isInteger(value) || value <= 0)
    ) {
      throw new Error(`inspection node ${op.handle} is not an integer-aligned positive cuboid`);
    }
    let actualVoxelCount = 0;
    for (let x = min[0]; x < maxExclusive[0]; x += 1) {
      for (let y = min[1]; y < maxExclusive[1]; y += 1) {
        for (let z = min[2]; z < maxExclusive[2]; z += 1) {
          const key = cellKey(x, y, z);
          if (represented.has(key)) {
            throw new Error(`inspection cuboids overlap at ${key}`);
          }
          if (expected.get(key) !== material) {
            throw new Error(
              `inspection cuboid covers absent or wrong-material cell ${key}; expected ${expected.get(key)}, received ${material}`,
            );
          }
          represented.set(key, material);
          actualVoxelCount += 1;
        }
      }
    }
    if (actualVoxelCount !== labelledVoxelCount) {
      throw new Error(
        `inspection node ${op.handle} label counts ${labelledVoxelCount} voxels but represents ${actualVoxelCount}`,
      );
    }
  }
  if (represented.size !== expected.size || represented.size !== projection.projectedVoxelCount) {
    throw new Error(
      `inspection projection for ${plan.placementId} represents ${represented.size}/${expected.size} expected voxels`,
    );
  }
  for (const [key, material] of expected) {
    if (represented.get(key) !== material) {
      throw new Error(`inspection projection omitted ${key} material ${material}`);
    }
  }
}

function fragmentedMultiMaterialPlan() {
  const solids = [];
  const add = (x, y, z, material) => solids.push({ coord: { x, y, z }, material });
  for (let x = 0; x < 3; x += 1) {
    for (let z = 0; z < 2; z += 1) {
      add(x, 0, z, 2);
    }
  }
  add(0, 1, 0, 5);
  add(1, 1, 0, 5);
  add(0, 1, 1, 5);
  add(3, 0, 0, 8);
  for (let x = 4; x < 6; x += 1) {
    for (let y = 0; y < 2; y += 1) {
      for (let z = 0; z < 2; z += 1) {
        add(x, y, z, 7);
      }
    }
  }
  add(0, 3, 0, 3);
  return {
    placementId: 'focused.fragmented-multi-material',
    coordinateMapping: 'placement_x_y_to_voxel_x_z',
    commands: [],
    solidVoxels: solids,
    walkableCellCount: 0,
    openingCellCount: 0,
    boundaryCellCount: 0,
    solidVoxelCount: solids.length,
    residentChunkCount: 0,
    doorPortals: [],
    buildBounds: {
      min: { x: 0, y: 0, z: 0 },
      maxExclusive: { x: 6, y: 4, z: 2 },
    },
  };
}

function cellKey(x, y, z) {
  return `${x}:${y}:${z}`;
}

async function readJson(relativePath) {
  return JSON.parse(await readFile(resolve(repoRoot, relativePath), 'utf8'));
}
