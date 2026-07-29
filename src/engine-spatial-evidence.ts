import { sha256Json } from './ca-trace-hash.js';
import type { VoxelExtrusionPlan } from './voxel-extrusion.js';

export interface EngineSpatialEvidence {
  readonly kind: 'rusty_procgen.evidence.engine_spatial_extrusion.v2';
  readonly schemaVersion: 2;
  readonly sourcePlacement: string;
  readonly placementId: string;
  readonly planSha256: string;
  readonly engineCommit: string;
  readonly coordinateMapping: 'placement_x_y_to_voxel_x_z';
  readonly counts: {
    readonly walkableCells: number;
    readonly declaredOpeningCells: number;
    readonly boundaryCells: number;
    readonly solidVoxels: number;
    readonly residentChunks: number;
  };
  readonly authority: {
    readonly changedVoxels: number;
    readonly transactionCount: number;
    readonly maxEditsPerTransaction: number;
    readonly deterministic: boolean;
    readonly readout: {
      readonly authorityHash: string;
      readonly projectionRevisionsCoherent: boolean;
      readonly solidVoxelCount: number;
      readonly residentChunkCount: number;
      readonly meshProjectionHash: string;
      readonly materialCounts: readonly {
        readonly material: number;
        readonly voxels: number;
      }[];
    };
  };
}

export function decodeEngineSpatialEvidence(value: unknown): EngineSpatialEvidence {
  const evidence = record(value, 'engine spatial evidence');
  if (
    evidence.kind !== 'rusty_procgen.evidence.engine_spatial_extrusion.v2'
    || evidence.schemaVersion !== 2
  ) {
    throw new Error('engine spatial evidence has an unsupported contract');
  }
  const sourcePlacement = string(valueAt(evidence, 'sourcePlacement'), 'sourcePlacement');
  if (!sourcePlacement.startsWith('artifacts/samples/') || sourcePlacement.includes('..')) {
    throw new Error('engine spatial evidence has an unsafe source placement');
  }
  const placementId = string(valueAt(evidence, 'placementId'), 'placementId');
  const planSha256 = hash(valueAt(evidence, 'planSha256'), 'planSha256', /^sha256:[0-9a-f]{64}$/);
  const engineCommit = hash(valueAt(evidence, 'engineCommit'), 'engineCommit', /^[0-9a-f]{40}$/);
  if (evidence.coordinateMapping !== 'placement_x_y_to_voxel_x_z') {
    throw new Error('engine spatial evidence has an unsupported coordinate mapping');
  }

  const counts = record(valueAt(evidence, 'counts'), 'counts');
  const authority = record(valueAt(evidence, 'authority'), 'authority');
  const readout = record(valueAt(authority, 'readout'), 'authority.readout');
  const materialCounts = array(valueAt(readout, 'materialCounts'), 'authority.readout.materialCounts')
    .map((entry, index) => {
      const count = record(entry, `authority.readout.materialCounts[${index}]`);
      return {
        material: integer(valueAt(count, 'material'), `materialCounts[${index}].material`),
        voxels: integer(valueAt(count, 'voxels'), `materialCounts[${index}].voxels`),
      };
    });

  return {
    kind: evidence.kind,
    schemaVersion: evidence.schemaVersion,
    sourcePlacement,
    placementId,
    planSha256,
    engineCommit,
    coordinateMapping: evidence.coordinateMapping,
    counts: {
      walkableCells: integer(valueAt(counts, 'walkableCells'), 'counts.walkableCells'),
      declaredOpeningCells: integer(
        valueAt(counts, 'declaredOpeningCells'),
        'counts.declaredOpeningCells',
      ),
      boundaryCells: integer(valueAt(counts, 'boundaryCells'), 'counts.boundaryCells'),
      solidVoxels: integer(valueAt(counts, 'solidVoxels'), 'counts.solidVoxels'),
      residentChunks: integer(valueAt(counts, 'residentChunks'), 'counts.residentChunks'),
    },
    authority: {
      changedVoxels: integer(valueAt(authority, 'changedVoxels'), 'authority.changedVoxels'),
      transactionCount: integer(
        valueAt(authority, 'transactionCount'),
        'authority.transactionCount',
      ),
      maxEditsPerTransaction: integer(
        valueAt(authority, 'maxEditsPerTransaction'),
        'authority.maxEditsPerTransaction',
      ),
      deterministic: boolean(valueAt(authority, 'deterministic'), 'authority.deterministic'),
      readout: {
        authorityHash: hash(
          valueAt(readout, 'authorityHash'),
          'authority.readout.authorityHash',
          /^fnv1a64:[0-9a-f]{16}$/,
        ),
        projectionRevisionsCoherent: boolean(
          valueAt(readout, 'projectionRevisionsCoherent'),
          'authority.readout.projectionRevisionsCoherent',
        ),
        solidVoxelCount: integer(
          valueAt(readout, 'solidVoxelCount'),
          'authority.readout.solidVoxelCount',
        ),
        residentChunkCount: integer(
          valueAt(readout, 'residentChunkCount'),
          'authority.readout.residentChunkCount',
        ),
        meshProjectionHash: hash(
          valueAt(readout, 'meshProjectionHash'),
          'authority.readout.meshProjectionHash',
          /^fnv1a64:[0-9a-f]{16}$/,
        ),
        materialCounts,
      },
    },
  };
}

export function verifyEngineSpatialPlan(
  evidence: EngineSpatialEvidence,
  plan: VoxelExtrusionPlan,
): string {
  if (plan.placementId !== evidence.placementId) {
    throw new Error(
      `placement identity ${plan.placementId} does not match native ${evidence.placementId}`,
    );
  }
  if (plan.coordinateMapping !== evidence.coordinateMapping) {
    throw new Error('coordinate mapping does not match native evidence');
  }
  const planSha256 = sha256Json(plan);
  if (planSha256 !== evidence.planSha256) {
    throw new Error(
      `plan SHA ${planSha256} does not match native ${evidence.planSha256}`,
    );
  }
  const expectedCounts = [
    [plan.walkableCellCount, evidence.counts.walkableCells, 'walkable cells'],
    [plan.openingCellCount, evidence.counts.declaredOpeningCells, 'opening cells'],
    [plan.boundaryCellCount, evidence.counts.boundaryCells, 'boundary cells'],
    [plan.solidVoxelCount, evidence.counts.solidVoxels, 'solid voxels'],
    [plan.residentChunkCount, evidence.counts.residentChunks, 'resident chunks'],
    [plan.solidVoxelCount, evidence.authority.changedVoxels, 'changed voxels'],
    [plan.solidVoxelCount, evidence.authority.readout.solidVoxelCount, 'authority voxels'],
    [
      plan.residentChunkCount,
      evidence.authority.readout.residentChunkCount,
      'authority resident chunks',
    ],
  ] as const;
  for (const [actual, expected, label] of expectedCounts) {
    if (actual !== expected) {
      throw new Error(`${label} ${actual} do not match native ${expected}`);
    }
  }

  const planMaterialCounts = new Map<number, number>();
  for (const voxel of plan.solidVoxels) {
    planMaterialCounts.set(
      voxel.material,
      (planMaterialCounts.get(voxel.material) ?? 0) + 1,
    );
  }
  const evidenceMaterialCounts = new Map(
    evidence.authority.readout.materialCounts
      .map((entry) => [entry.material, entry.voxels] as const),
  );
  if (
    planMaterialCounts.size !== evidenceMaterialCounts.size
    || [...planMaterialCounts].some(
      ([material, voxels]) => evidenceMaterialCounts.get(material) !== voxels,
    )
  ) {
    throw new Error('material counts do not match native authority');
  }
  if (
    !evidence.authority.deterministic
    || !evidence.authority.readout.projectionRevisionsCoherent
  ) {
    throw new Error('native authority evidence is not deterministic and coherent');
  }
  return planSha256;
}

function record(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(`${path} must be an object`);
  }
  return value as Record<string, unknown>;
}

function valueAt(value: Record<string, unknown>, key: string): unknown {
  if (!(key in value)) {
    throw new Error(`engine spatial evidence is missing ${key}`);
  }
  return value[key];
}

function string(value: unknown, path: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${path} must be a non-empty string`);
  }
  return value;
}

function hash(value: unknown, path: string, pattern: RegExp): string {
  const parsed = string(value, path);
  if (!pattern.test(parsed)) {
    throw new Error(`${path} has an invalid hash`);
  }
  return parsed;
}

function integer(value: unknown, path: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) {
    throw new Error(`${path} must be a non-negative safe integer`);
  }
  return value as number;
}

function boolean(value: unknown, path: string): boolean {
  if (typeof value !== 'boolean') {
    throw new Error(`${path} must be a boolean`);
  }
  return value;
}

function array(value: unknown, path: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new Error(`${path} must be an array`);
  }
  return value;
}
