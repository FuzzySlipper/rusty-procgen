#!/usr/bin/env node
import assert from 'node:assert/strict';

import { compilePlacementExtrusion } from '../dist/ts/src/voxel-extrusion.js';

const policy = {
  schemaVersion: 1,
  minimumClearanceCells: 3,
  contactPolicy: 'glued_exits_only',
  wallThicknessCells: 1,
  doorwayWidthCells: 1,
  preservePieceBoundaries: true,
};
const occupiedCells = [
  ...ownedSquare('instance.room_a', 0, 0),
  ...ownedSquare('instance.room_b', 5, 0),
];
const connectionOwner = 'connection.glue_room_a_room_b';
const placement = {
  kind: 'rusty_procgen.piece_placement.v1',
  placementId: 'piece_placement.boundary_smoke',
  gridConnectivity: 'four_way',
  placementPolicy: policy,
  occupiedCells,
  reservedCells: [],
  connectionCells: [2, 3, 4].map((x) => ({ instanceId: connectionOwner, x, y: 0 })),
  gluedExits: [{
    id: 'glue.room_a.room_b',
    fromInstance: 'instance.room_a',
    fromCell: { x: 2, y: 0 },
    fromDirection: 'east',
    fromWidth: 1,
    toInstance: 'instance.room_b',
    toCell: { x: 4, y: 0 },
    toDirection: 'west',
    toWidth: 1,
    sourceSection: 'section.room_a_room_b',
  }],
  gatePortals: [],
};

const plan = compilePlacementExtrusion(placement);
assert.equal(plan.openingCellCount, 3);
assert.equal(plan.walkableCellCount, 11);
for (const x of [2, 3, 4]) {
  assert.ok(hasSolid(plan, x, 0, 0, 2), `doorway route ${x},0 is missing its floor`);
  assert.ok(hasSolid(plan, x, 1, 1, 1), `wall line beside doorway route ${x},0 is missing`);
  assert.equal(hasSolid(plan, x, 1, 0, 1), false, `doorway route ${x},0 was closed by a wall`);
}
assert.ok(hasSolid(plan, 2, 1, 1, 1), 'room-facing wall is missing outside the declared opening');
assert.ok(hasSolid(plan, 4, 1, 1, 1), 'second room-facing wall is missing outside the declared opening');

const offsetPlacement = {
  ...placement,
  placementId: 'piece_placement.offset_exit_smoke',
  occupiedCells: [
    ...ownedSquare('instance.room_a', 0, 0),
    ...ownedSquare('instance.room_b', 5, 3),
  ],
  connectionCells: [
    { instanceId: connectionOwner, x: 2, y: 0 },
    { instanceId: connectionOwner, x: 3, y: 0 },
    { instanceId: connectionOwner, x: 3, y: 1 },
    { instanceId: connectionOwner, x: 3, y: 2 },
    { instanceId: connectionOwner, x: 3, y: 3 },
    { instanceId: connectionOwner, x: 3, y: 4 },
    { instanceId: connectionOwner, x: 4, y: 4 },
  ],
  gluedExits: [{
    ...placement.gluedExits[0],
    toCell: { x: 4, y: 4 },
  }],
  gatePortals: [],
};
const offsetPlan = compilePlacementExtrusion(offsetPlacement);
assert.equal(offsetPlan.openingCellCount, 7);
assert.ok(hasSolid(offsetPlan, 2, 1, 1, 1), 'nearer non-exit wall on room A was opened');
assert.ok(hasSolid(offsetPlan, 4, 1, 3, 1), 'nearer non-exit wall on room B was opened');

const turningProceduralPlacement = {
  ...placement,
  placementId: 'piece_placement.turning_procedural_smoke',
  corridorRealization: 'procedural',
  occupiedCells: [
    ...ownedSquare('instance.room_a', 0, 0),
    ...ownedSquare('instance.room_b', 5, 5),
  ],
  connectionCells: [
    { instanceId: connectionOwner, x: 2, y: 0 },
    { instanceId: connectionOwner, x: 3, y: 0 },
    { instanceId: connectionOwner, x: 4, y: 0 },
    { instanceId: connectionOwner, x: 4, y: 1 },
    { instanceId: connectionOwner, x: 4, y: 2 },
    { instanceId: connectionOwner, x: 4, y: 3 },
    { instanceId: connectionOwner, x: 4, y: 4 },
    { instanceId: connectionOwner, x: 5, y: 4 },
  ],
  gluedExits: [{
    ...placement.gluedExits[0],
    toCell: { x: 5, y: 4 },
    toDirection: 'north',
  }],
};
const turningProceduralPlan = compilePlacementExtrusion(turningProceduralPlacement);
assert.equal(turningProceduralPlan.openingCellCount, 8);
const turningHybridPlan = compilePlacementExtrusion({
  ...turningProceduralPlacement,
  placementId: 'piece_placement.turning_hybrid_smoke',
  corridorRealization: 'hybrid',
  gluedExits: [{
    ...turningProceduralPlacement.gluedExits[0],
    routePoints: [
      { x: 2, y: 0 },
      { x: 4, y: 0 },
      { x: 4, y: 4 },
      { x: 5, y: 4 },
    ],
  }],
});
assert.equal(turningHybridPlan.openingCellCount, 8);
assert.throws(
  () => compilePlacementExtrusion({
    ...turningProceduralPlacement,
    corridorRealization: 'hybrid',
  }),
  /incompatible directions/,
);

const pureCatalogPlan = compilePlacementExtrusion({
  ...placement,
  placementId: 'piece_placement.pure_catalog_smoke',
  corridorRealization: 'catalog',
  occupiedCells: [
    { instanceId: 'instance.room_a', x: 0, y: 0 },
    { instanceId: 'instance.room_b', x: 1, y: 0 },
  ],
  connectionCells: [],
  gluedExits: [{
    ...placement.gluedExits[0],
    fromCell: { x: 1, y: 0 },
    toCell: { x: 0, y: 0 },
  }],
});
assert.equal(pureCatalogPlan.openingCellCount, 0);
assert.equal(pureCatalogPlan.walkableCellCount, 2);
assert.throws(
  () => compilePlacementExtrusion({
    ...placement,
    corridorRealization: 'catalog',
  }),
  /must not contain generated connection cells/,
);

assert.throws(
  () => compilePlacementExtrusion({
    ...placement,
    corridorRealization: 'procedural',
    occupiedCells: [
      ...ownedSquare('instance.room_a', 0, 0),
      ...ownedSquare('instance.room_b', 3, 0),
    ],
  }),
  /piece boundary clearance 3 violated/,
);
assert.throws(
  () => compilePlacementExtrusion({
    ...placement,
    placementPolicy: { ...policy, preservePieceBoundaries: false },
  }),
  /requires preservePieceBoundaries=true/,
);
assert.throws(
  () => compilePlacementExtrusion({
    ...placement,
    placementPolicy: { ...policy, wallThicknessCells: 2, minimumClearanceCells: 4 },
  }),
  /at least twice wallThicknessCells plus one/,
);
assert.throws(
  () => compilePlacementExtrusion({
    ...placement,
    placementPolicy: { ...policy, doorwayWidthCells: 3 },
  }),
  /supports doorwayWidthCells=1 only/,
);
assert.throws(
  () => compilePlacementExtrusion({
    ...placement,
    connectionCells: [{ instanceId: 'connection.undeclared', x: 2, y: 0 }],
  }),
  /not owned by a declared glued exit/,
);
assert.throws(
  () => compilePlacementExtrusion({
    ...placement,
    connectionCells: [
      ...placement.connectionCells,
      { instanceId: connectionOwner, x: 2, y: 1 },
    ],
  }),
  /enters non-exit wall clearance/,
);

console.log(
  `voxel boundary smoke passed; ${plan.walkableCellCount} walkable cells, ${plan.openingCellCount} declared opening cells, ${plan.boundaryCellCount} boundary cells; offset exits kept nearer walls closed`,
);

function ownedSquare(instanceId, minX, minY) {
  return [
    { instanceId, x: minX, y: minY },
    { instanceId, x: minX + 1, y: minY },
    { instanceId, x: minX, y: minY + 1 },
    { instanceId, x: minX + 1, y: minY + 1 },
  ];
}

function hasSolid(plan, x, y, z, material) {
  return plan.solidVoxels.some((voxel) => (
    voxel.coord.x === x
    && voxel.coord.y === y
    && voxel.coord.z === z
    && voxel.material === material
  ));
}
