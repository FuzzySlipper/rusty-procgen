import {
  decodeRenderFrameDiff,
  renderHandle,
  type EditorGridDescriptor,
  type RenderDiff,
  type RenderFrameDiff,
} from '@rusty-engine/render-contracts';

import type { VoxelExtrusionPlan } from './voxel-extrusion.js';

export interface VoxelInspectionProjection {
  readonly frame: RenderFrameDiff;
  readonly placementId: string;
  readonly ceilingY: number;
  readonly projectedVoxelCount: number;
  readonly projectedNodeCount: number;
  readonly omittedCeilingVoxelCount: number;
  readonly doorNodeCount: number;
  readonly lockedDoorCount: number;
  readonly unlockedDoorCount: number;
  readonly grid: EditorGridDescriptor;
  readonly camera: {
    readonly position: readonly [number, number, number];
    readonly target: readonly [number, number, number];
    readonly moveSpeed: number;
  };
}

export function decodeVoxelInspectionFrame(input: unknown): RenderFrameDiff {
  return decodeRenderFrameDiff(input);
}

export interface VoxelInspectionDoorState {
  readonly openPortalIds: ReadonlySet<string>;
  readonly includedPortalIds?: ReadonlySet<string>;
}

const MATERIAL_COLORS: Readonly<Record<number, readonly [number, number, number, number]>> = {
  1: [0.2, 0.24, 0.28, 1],
  2: [0.23, 0.58, 0.39, 1],
};
const UNKNOWN_MATERIAL_COLOR = [0.72, 0.31, 0.68, 1] as const;

interface VoxelInspectionBox {
  readonly min: readonly [number, number, number];
  readonly maxExclusive: readonly [number, number, number];
  readonly material: number;
  readonly voxelCount: number;
}

/**
 * Creates a projection-only inspection frame from the canonical extrusion plan.
 * The top enclosure layer is intentionally excluded from this presentation frame;
 * the source plan and its native evidence remain fully enclosed.
 */
export function buildVoxelInspectionProjection(
  plan: VoxelExtrusionPlan,
  doorState: VoxelInspectionDoorState = { openPortalIds: new Set() },
): VoxelInspectionProjection {
  const ceilingY = plan.buildBounds.maxExclusive.y - 1;
  const projectedVoxels = plan.solidVoxels.filter((voxel) => voxel.coord.y !== ceilingY);
  const omittedCeilingVoxelCount = plan.solidVoxels.length - projectedVoxels.length;
  if (omittedCeilingVoxelCount === 0) {
    throw new Error(`extrusion ${plan.placementId} has no ceiling layer at y=${ceilingY}`);
  }

  const boxes = compactInspectionVoxels(projectedVoxels);
  const voxelOps: RenderDiff[] = boxes.map((box, index) => ({
    op: 'create',
    handle: renderHandle(index + 1),
    parent: null,
    node: {
      geometry: { kind: 'cube' },
      material: {
        color: MATERIAL_COLORS[box.material] ?? UNKNOWN_MATERIAL_COLOR,
        wireframe: false,
      },
      transform: {
        translation: [
          (box.min[0] + box.maxExclusive[0]) / 2,
          (box.min[1] + box.maxExclusive[1]) / 2,
          (box.min[2] + box.maxExclusive[2]) / 2,
        ],
        rotation: [0, 0, 0, 1],
        scale: [
          box.maxExclusive[0] - box.min[0],
          box.maxExclusive[1] - box.min[1],
          box.maxExclusive[2] - box.min[2],
        ],
      },
      visible: true,
      layer: 'scene',
      metadata: {
        sourceEntity: null,
        sourceSceneNode: null,
        tags: [],
        label: `procgen-voxel-box:${box.min.join(',')}..${box.maxExclusive.join(',')}:material-${box.material}:voxels-${box.voxelCount}`,
      },
    },
  }));
  const doorOps: RenderDiff[] = [];
  let lockedDoorCount = 0;
  let unlockedDoorCount = 0;
  for (const portal of plan.doorPortals) {
    if (doorState.includedPortalIds !== undefined && !doorState.includedPortalIds.has(portal.id)) {
      continue;
    }
    const unlocked = portal.requiredItem === null || doorState.openPortalIds.has(portal.id);
    if (unlocked) {
      unlockedDoorCount += 1;
    } else {
      lockedDoorCount += 1;
    }
    for (const [cellIndex, cell] of portal.cells.entries()) {
      const eastWest = portal.orientation === 'east' || portal.orientation === 'west';
      doorOps.push({
        op: 'create',
        handle: renderHandle(voxelOps.length + doorOps.length + 1),
        parent: null,
        node: {
          geometry: { kind: 'cube' },
          material: {
            color: unlocked ? [0.16, 0.48, 0.98, 0.48] : [0.93, 0.16, 0.2, 0.56],
            wireframe: false,
          },
          transform: {
            translation: [
              cell.x + 0.5,
              (portal.minY + portal.maxExclusiveY) / 2,
              cell.y + 0.5,
            ],
            rotation: [0, 0, 0, 1],
            scale: eastWest
              ? [0.18, portal.maxExclusiveY - portal.minY, 1]
              : [1, portal.maxExclusiveY - portal.minY, 0.18],
          },
          visible: true,
          layer: 'scene',
          metadata: {
            sourceEntity: null,
            sourceSceneNode: null,
            tags: [],
            label: `procgen-door:${portal.sourceEdge}:${portal.requiredItem ?? 'unlocked'}:${portal.id}:cell-${cellIndex + 1}`,
          },
        },
      });
    }
  }
  const firstLightHandle = voxelOps.length + doorOps.length + 1;
  const lightOps: readonly RenderDiff[] = [
    {
      op: 'createLight',
      handle: renderHandle(firstLightHandle),
      parent: null,
      light: {
        kind: 'ambient',
        color: [0.72, 0.78, 0.85],
        intensity: 0.8,
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
        color: [1, 0.93, 0.82],
        intensity: 1.6,
        enabled: true,
        direction: [-1, -2, -0.75],
        shadowIntent: 'disabled',
      },
    },
  ];

  const width = plan.buildBounds.maxExclusive.x - plan.buildBounds.min.x;
  const depth = plan.buildBounds.maxExclusive.z - plan.buildBounds.min.z;
  const height = plan.buildBounds.maxExclusive.y - plan.buildBounds.min.y;
  const centerX = plan.buildBounds.min.x + width / 2;
  const centerZ = plan.buildBounds.min.z + depth / 2;
  const radius = Math.max(width, depth, height, 8);
  const target = [centerX, plan.buildBounds.min.y + Math.min(height / 2, 2), centerZ] as const;
  const gridFadeEnd = Math.max(radius * 3, 48);

  return {
    frame: { schemaVersion: 1, ops: [...voxelOps, ...doorOps, ...lightOps] },
    placementId: plan.placementId,
    ceilingY,
    projectedVoxelCount: projectedVoxels.length,
    projectedNodeCount: boxes.length,
    omittedCeilingVoxelCount,
    doorNodeCount: doorOps.length,
    lockedDoorCount,
    unlockedDoorCount,
    grid: {
      visible: true,
      grid: {
        coordinateSystem: 'rightHandedYUp',
        // Keep the presentation grid just above the top of the floor cubes so it remains legible.
        origin: [plan.buildBounds.min.x, plan.buildBounds.min.y + 1.002, plan.buildBounds.min.z],
        spacing: [1, 1, 1],
      },
      plane: 'xz',
      snapAnchor: 'boundary',
      style: {
        minorColor: [0.36, 0.42, 0.48, 0.7],
        majorColor: [0.58, 0.64, 0.7, 0.9],
        xAxisColor: [0.8, 0.25, 0.22, 1],
        yAxisColor: [0.28, 0.75, 0.38, 1],
        zAxisColor: [0.25, 0.48, 0.9, 1],
        majorLineEvery: 5,
        opacity: 0.78,
        fadeStart: Math.max(radius, 16),
        fadeEnd: gridFadeEnd,
      },
    },
    camera: {
      position: [
        centerX + radius * 0.5,
        plan.buildBounds.maxExclusive.y + radius * 0.8,
        centerZ + radius * 0.75,
      ],
      target,
      moveSpeed: Math.max(4, radius * 0.4),
    },
  };
}

function compactInspectionVoxels(
  voxels: VoxelExtrusionPlan['solidVoxels'],
): readonly VoxelInspectionBox[] {
  const materialByCell = new Map<string, number>();
  for (const voxel of voxels) {
    materialByCell.set(inspectionCellKey(voxel.coord.x, voxel.coord.y, voxel.coord.z), voxel.material);
  }

  const remaining = new Set(materialByCell.keys());
  const seeds = [...voxels].sort((left, right) =>
    left.coord.x - right.coord.x
    || left.coord.y - right.coord.y
    || left.coord.z - right.coord.z
    || left.material - right.material);
  const boxes: VoxelInspectionBox[] = [];
  for (const seed of seeds) {
    const seedKey = inspectionCellKey(seed.coord.x, seed.coord.y, seed.coord.z);
    if (!remaining.has(seedKey)) {
      continue;
    }

    const candidates = INSPECTION_GROWTH_ORDERS.map((order) => {
      let candidate = inspectionUnitBox(seed);
      for (const axis of order) {
        while (canGrowInspectionBox(candidate, axis, seed.material, materialByCell, remaining)) {
          candidate = growInspectionBox(candidate, axis);
        }
      }
      return candidate;
    });
    const selected = candidates.reduce((best, candidate) =>
      candidate.voxelCount > best.voxelCount ? candidate : best);
    forEachInspectionCell(selected, (x, y, z) => {
      remaining.delete(inspectionCellKey(x, y, z));
    });
    boxes.push(selected);
  }
  if (remaining.size !== 0) {
    throw new Error(`inspection compaction left ${remaining.size} voxels unrepresented`);
  }
  return boxes.sort(compareInspectionBoxes);
}

type InspectionAxis = 0 | 1 | 2;

const INSPECTION_GROWTH_ORDERS: readonly (readonly InspectionAxis[])[] = [
  [0, 2, 1],
  [2, 0, 1],
  [1, 0, 2],
  [0, 1, 2],
  [2, 1, 0],
  [1, 2, 0],
];

function inspectionUnitBox(
  voxel: VoxelExtrusionPlan['solidVoxels'][number],
): VoxelInspectionBox {
  return inspectionBox(
    [voxel.coord.x, voxel.coord.y, voxel.coord.z],
    [voxel.coord.x + 1, voxel.coord.y + 1, voxel.coord.z + 1],
    voxel.material,
  );
}

function canGrowInspectionBox(
  box: VoxelInspectionBox,
  axis: InspectionAxis,
  material: number,
  materialByCell: ReadonlyMap<string, number>,
  remaining: ReadonlySet<string>,
): boolean {
  const layerMin = [...box.min] as [number, number, number];
  const layerMax = [...box.maxExclusive] as [number, number, number];
  layerMin[axis] = box.maxExclusive[axis];
  layerMax[axis] = box.maxExclusive[axis] + 1;
  let valid = true;
  forEachInspectionBounds(layerMin, layerMax, (x, y, z) => {
    const key = inspectionCellKey(x, y, z);
    if (!remaining.has(key) || materialByCell.get(key) !== material) {
      valid = false;
    }
  });
  return valid;
}

function growInspectionBox(
  box: VoxelInspectionBox,
  axis: InspectionAxis,
): VoxelInspectionBox {
  const maxExclusive = [...box.maxExclusive] as [number, number, number];
  maxExclusive[axis] += 1;
  return inspectionBox(box.min, maxExclusive, box.material);
}

function inspectionBox(
  min: readonly [number, number, number],
  maxExclusive: readonly [number, number, number],
  material: number,
): VoxelInspectionBox {
  return {
    min,
    maxExclusive,
    material,
    voxelCount:
      (maxExclusive[0] - min[0])
      * (maxExclusive[1] - min[1])
      * (maxExclusive[2] - min[2]),
  };
}

function forEachInspectionCell(
  box: VoxelInspectionBox,
  visit: (x: number, y: number, z: number) => void,
): void {
  forEachInspectionBounds(box.min, box.maxExclusive, visit);
}

function forEachInspectionBounds(
  min: readonly [number, number, number],
  maxExclusive: readonly [number, number, number],
  visit: (x: number, y: number, z: number) => void,
): void {
  for (let x = min[0]; x < maxExclusive[0]; x += 1) {
    for (let y = min[1]; y < maxExclusive[1]; y += 1) {
      for (let z = min[2]; z < maxExclusive[2]; z += 1) {
        visit(x, y, z);
      }
    }
  }
}

function inspectionCellKey(x: number, y: number, z: number): string {
  return `${x}:${y}:${z}`;
}

function compareInspectionBoxes(left: VoxelInspectionBox, right: VoxelInspectionBox): number {
  return left.min[0] - right.min[0]
    || left.min[1] - right.min[1]
    || left.min[2] - right.min[2]
    || left.maxExclusive[0] - right.maxExclusive[0]
    || left.maxExclusive[1] - right.maxExclusive[1]
    || left.maxExclusive[2] - right.maxExclusive[2]
    || left.material - right.material;
}
