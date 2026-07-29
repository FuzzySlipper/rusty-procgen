#!/usr/bin/env node
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

import {
  decodeEngineSpatialEvidence,
  verifyEngineSpatialPlan,
} from '../dist/ts/src/engine-spatial-evidence.js';
import { compilePlacementExtrusion } from '../dist/ts/src/voxel-extrusion.js';

const evidence = decodeEngineSpatialEvidence(await readJson(
  'artifacts/evidence/engine-spatial-extrusion.json',
));
const placement = await readJson(evidence.sourcePlacement);
const plan = compilePlacementExtrusion(placement);

assert.equal(verifyEngineSpatialPlan(evidence, plan), evidence.planSha256);
assert.equal(plan.solidVoxelCount, evidence.authority.readout.solidVoxelCount);

const tamperedPlacement = structuredClone(placement);
const distant = tamperedPlacement.occupiedCells.reduce(
  (maximum, cell) => ({
    x: Math.max(maximum.x, cell.x),
    y: Math.max(maximum.y, cell.y),
  }),
  { x: 0, y: 0 },
);
tamperedPlacement.occupiedCells.push({
  instanceId: tamperedPlacement.occupiedCells[0].instanceId,
  x: distant.x + 100,
  y: distant.y + 100,
});
const tamperedPlan = compilePlacementExtrusion(tamperedPlacement);
assert.equal(tamperedPlan.placementId, plan.placementId);
assert.notEqual(tamperedPlan.solidVoxelCount, plan.solidVoxelCount);
assert.throws(
  () => verifyEngineSpatialPlan(evidence, tamperedPlan),
  /plan SHA .* does not match native/,
);

const staleCounts = structuredClone(evidence);
staleCounts.counts.solidVoxels += 1;
assert.throws(
  () => verifyEngineSpatialPlan(staleCounts, plan),
  /solid voxels .* do not match native/,
);

for (const invalid of [
  {
    ...structuredClone(evidence),
    kind: evidence.kind.replace(/v2$/, 'v1'),
  },
  { ...structuredClone(evidence), planSha256: 'sha256:invalid' },
  {
    ...structuredClone(evidence),
    authority: {
      ...structuredClone(evidence.authority),
      readout: {
        ...structuredClone(evidence.authority.readout),
        materialCounts: [{ material: -1, voxels: 1 }],
      },
    },
  },
]) {
  assert.throws(() => decodeEngineSpatialEvidence(invalid));
}

console.log(
  `Engine spatial evidence smoke passed; Rust/TypeScript plan ${evidence.planSha256}, same-ID ${plan.solidVoxelCount}->${tamperedPlan.solidVoxelCount} tamper rejected`,
);

async function readJson(path) {
  return JSON.parse(await readFile(path, 'utf8'));
}
