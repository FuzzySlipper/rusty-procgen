import {
  PREFAB_DEFINITION_SCHEMA_VERSION,
  PREFAB_REGISTRY_SCHEMA_VERSION,
  prefabId,
  prefabInstanceId,
  prefabPartId,
  projectId,
  sceneId,
  sceneNodeId,
} from '@asha/contracts';
import type {
  FlatSceneDocument,
  PrefabDefinition,
  PrefabInstanceRecord,
  PrefabPartSource,
  PrefabRegistry,
  PrefabTransform,
  ProjectBundleManifest,
} from '@asha/contracts';
import {
  decodeAndValidateAshaPrefabRegistrySourceDocument,
  serializeAshaPrefabRegistrySource,
} from '@asha/game-workspace';

export type ProcgenPublishDiagnosticCode =
  | 'invalidProvenance'
  | 'missingShape'
  | 'missingPrefabMapping'
  | 'missingStableRole'
  | 'incompatibleSourceAsset'
  | 'duplicateInstanceIdentity'
  | 'invalidTransform';

export class ProcgenPublishError extends Error {
  constructor(
    readonly code: ProcgenPublishDiagnosticCode,
    message: string,
  ) {
    super(message);
    this.name = 'ProcgenPublishError';
  }
}

export interface ProcgenShapeCatalogForPublish {
  readonly kind: string;
  readonly schemaVersion: number;
  readonly catalogId: string;
  readonly shapes: readonly {
    readonly shapeId: string;
    readonly label: string;
    readonly allowedTransforms: readonly string[];
  }[];
}

export interface ProcgenShapeMatchForPublish {
  readonly kind: string;
  readonly schemaVersion: number;
  readonly matchId: string;
  readonly planId: string;
  readonly catalogId: string;
  readonly seed: number;
  readonly sourcePlanRef: string;
  readonly sourceCatalogRef: string;
  readonly ok: boolean;
  readonly unmatchedCount: number;
  readonly matches: readonly {
    readonly pieceId: string;
    readonly shapeId: string;
    readonly transform: string;
    readonly score: number;
    readonly sourceRequirementRef: string;
  }[];
}

export interface ProcgenPiecePlacementForPublish {
  readonly kind: string;
  readonly schemaVersion: number;
  readonly placementId: string;
  readonly planId: string;
  readonly catalogId: string;
  readonly matchId: string;
  readonly sourcePlanRef: string;
  readonly sourceCatalogRef: string;
  readonly sourceMatchRef: string;
  readonly cellSize: number;
  readonly instances: readonly {
    readonly instanceId: string;
    readonly pieceId: string;
    readonly shapeId: string;
    readonly transform: string;
    readonly origin: { readonly x: number; readonly y: number };
    readonly sourceRequirementRef: string;
  }[];
}

export interface ProcgenPrefabPublishMapping {
  readonly shapeId: string;
  readonly prefabId: number;
  readonly partId: number;
  readonly partNamespace: string;
  readonly stableRole: string;
  readonly source: PrefabPartSource;
}

export interface ProcgenPrefabPublishConfiguration {
  readonly kind: 'rusty_procgen.prefab_publish_mapping.v1';
  readonly candidateRef: string;
  readonly selectedInstanceIds: readonly string[];
  readonly mappings: readonly ProcgenPrefabPublishMapping[];
  readonly instanceIdentities: readonly {
    readonly procgenInstanceId: string;
    readonly prefabInstanceId: number;
  }[];
  readonly sourceAssets: readonly {
    readonly assetId: string;
    readonly artifact: string;
    readonly contentHash: string;
  }[];
  readonly project: { readonly id: number; readonly name: string };
  readonly scene: { readonly id: number; readonly artifact: string };
  readonly prefabRegistryArtifact: string;
  readonly prefabInstancesArtifact: string;
  readonly assetLockArtifact: string;
}

export interface ProcgenPrefabPublication {
  readonly kind: 'rusty_procgen.prefab_project_bundle_publication.v1';
  readonly manifest: ProjectBundleManifest;
  readonly prefabRegistry: PrefabRegistry;
  readonly sceneArtifact: FlatSceneDocument;
  readonly prefabInstancesArtifact: {
    readonly kind: 'rusty_procgen.published_prefab_instances.v1';
    readonly schemaVersion: 1;
    readonly scene: ReturnType<typeof sceneId>;
    readonly prefabInstances: readonly PrefabInstanceRecord[];
  };
  readonly assetLockArtifact: {
    readonly kind: 'rusty_procgen.asset_lock_references.v1';
    readonly schemaVersion: 1;
    readonly assets: ProcgenPrefabPublishConfiguration['sourceAssets'];
  };
  readonly sourceAssets: ProcgenPrefabPublishConfiguration['sourceAssets'];
  readonly provenance: {
    readonly candidateRef: string;
    readonly catalogId: string;
    readonly catalogRef: string;
    readonly planId: string;
    readonly planRef: string;
    readonly matchId: string;
    readonly matchRef: string;
    readonly placementId: string;
    readonly instances: readonly {
      readonly procgenInstanceId: string;
      readonly prefabInstanceId: number;
      readonly pieceId: string;
      readonly shapeId: string;
      readonly prefabId: number;
      readonly matchScore: number;
      readonly sourceRequirementRef: string;
    }[];
  };
  readonly nonClaims: readonly [
    'not_live_prefab_instantiation',
    'not_renderer_proof',
    'not_navigation_proof',
    'not_collision_proof',
  ];
}

export function compileProcgenPrefabPublication(input: {
  readonly catalog: ProcgenShapeCatalogForPublish;
  readonly shapeMatch: ProcgenShapeMatchForPublish;
  readonly placement: ProcgenPiecePlacementForPublish;
  readonly configuration: ProcgenPrefabPublishConfiguration;
}): ProcgenPrefabPublication {
  const { catalog, shapeMatch, placement, configuration } = input;
  validateProvenance(catalog, shapeMatch, placement, configuration);

  const shapes = uniqueBy(catalog.shapes, (shape) => shape.shapeId, 'missingShape', 'shape catalog');
  const matches = uniqueBy(shapeMatch.matches, (match) => match.pieceId, 'invalidProvenance', 'shape match');
  const placements = uniqueBy(
    placement.instances,
    (instance) => instance.instanceId,
    'duplicateInstanceIdentity',
    'piece placement',
  );
  const mappings = uniqueBy(
    configuration.mappings,
    (mapping) => mapping.shapeId,
    'missingPrefabMapping',
    'prefab mapping',
  );
  const instanceIdentities = uniqueBy(
    configuration.instanceIdentities,
    (identity) => identity.procgenInstanceId,
    'duplicateInstanceIdentity',
    'prefab instance identity mapping',
  );
  rejectDuplicateNumbers(
    configuration.instanceIdentities.map((identity) => identity.prefabInstanceId),
    'prefab instance identity',
  );
  rejectDuplicateNumbers(configuration.mappings.map((mapping) => mapping.prefabId), 'prefab identity');

  const selected = configuration.selectedInstanceIds.map((instanceId) => {
    const instance = placements.get(instanceId);
    if (instance === undefined) {
      throw new ProcgenPublishError(
        'duplicateInstanceIdentity',
        `selected Procgen instance ${instanceId} is missing from the placement`,
      );
    }
    const shape = shapes.get(instance.shapeId);
    if (shape === undefined) {
      throw new ProcgenPublishError('missingShape', `placement references missing shape ${instance.shapeId}`);
    }
    const match = matches.get(instance.pieceId);
    if (
      match === undefined
      || match.shapeId !== instance.shapeId
      || match.transform !== instance.transform
      || match.sourceRequirementRef !== instance.sourceRequirementRef
    ) {
      throw new ProcgenPublishError(
        'invalidProvenance',
        `placement instance ${instance.instanceId} does not match its shape-match provenance`,
      );
    }
    const mapping = mappings.get(instance.shapeId);
    if (mapping === undefined) {
      throw new ProcgenPublishError(
        'missingPrefabMapping',
        `shape ${instance.shapeId} has no stable ASHA prefab mapping`,
      );
    }
    validateStableRole(mapping.stableRole, instance.shapeId);
    const identity = instanceIdentities.get(instance.instanceId);
    if (identity === undefined) {
      throw new ProcgenPublishError(
        'duplicateInstanceIdentity',
        `Procgen instance ${instance.instanceId} has no stable ASHA prefab instance identity`,
      );
    }
    return { instance, shape, match, mapping, identity };
  });

  if (new Set(configuration.selectedInstanceIds).size !== configuration.selectedInstanceIds.length) {
    throw new ProcgenPublishError('duplicateInstanceIdentity', 'selected Procgen instance identities must be unique');
  }

  const selectedMappings = new Map<string, ProcgenPrefabPublishMapping>();
  for (const { mapping } of selected) {
    selectedMappings.set(mapping.shapeId, mapping);
  }
  const definitions = [...selectedMappings.values()]
    .map((mapping) => buildPrefabDefinition(mapping, shapes.get(mapping.shapeId)))
    .sort((left, right) => left.id - right.id);
  const prefabRegistry: PrefabRegistry = {
    schemaVersion: PREFAB_REGISTRY_SCHEMA_VERSION,
    definitions,
  };
  validatePrefabRegistry(prefabRegistry, configuration);

  const prefabInstances = selected
    .map(({ instance, shape, mapping, identity }) => ({
      instance: prefabInstanceId(identity.prefabInstanceId),
      prefab: prefabId(mapping.prefabId),
      seed: shapeMatch.seed,
      transform: placementTransform(instance, shape.allowedTransforms, placement.cellSize),
      overrides: [],
    }))
    .sort((left, right) => left.instance - right.instance);

  const prefabInstancesArtifact = {
    kind: 'rusty_procgen.published_prefab_instances.v1' as const,
    schemaVersion: 1 as const,
    scene: sceneId(configuration.scene.id),
    prefabInstances,
  };
  const assetLockArtifact = {
    kind: 'rusty_procgen.asset_lock_references.v1' as const,
    schemaVersion: 1 as const,
    assets: configuration.sourceAssets,
  };
  const sceneArtifact = buildSceneArtifact(configuration, selected, prefabInstances);
  const provenance = {
    candidateRef: configuration.candidateRef,
    catalogId: catalog.catalogId,
    catalogRef: placement.sourceCatalogRef,
    planId: placement.planId,
    planRef: placement.sourcePlanRef,
    matchId: placement.matchId,
    matchRef: placement.sourceMatchRef,
    placementId: placement.placementId,
    instances: selected.map(({ instance, match, mapping, identity }) => ({
      procgenInstanceId: instance.instanceId,
      prefabInstanceId: identity.prefabInstanceId,
      pieceId: instance.pieceId,
      shapeId: instance.shapeId,
      prefabId: mapping.prefabId,
      matchScore: match.score,
      sourceRequirementRef: instance.sourceRequirementRef,
    })),
  };
  const manifest = buildManifest(
    configuration,
    prefabRegistry,
    prefabInstancesArtifact,
    assetLockArtifact,
    sceneArtifact,
    provenance,
    shapeMatch.seed,
  );

  return {
    kind: 'rusty_procgen.prefab_project_bundle_publication.v1',
    manifest,
    prefabRegistry,
    sceneArtifact,
    prefabInstancesArtifact,
    assetLockArtifact,
    sourceAssets: configuration.sourceAssets,
    provenance,
    nonClaims: [
      'not_live_prefab_instantiation',
      'not_renderer_proof',
      'not_navigation_proof',
      'not_collision_proof',
    ],
  };
}

function validateProvenance(
  catalog: ProcgenShapeCatalogForPublish,
  shapeMatch: ProcgenShapeMatchForPublish,
  placement: ProcgenPiecePlacementForPublish,
  configuration: ProcgenPrefabPublishConfiguration,
): void {
  const validKinds = catalog.kind === 'rusty_procgen.shape_catalog.v1'
    && shapeMatch.kind === 'rusty_procgen.piece_shape_match.v1'
    && placement.kind === 'rusty_procgen.piece_placement.v1'
    && configuration.kind === 'rusty_procgen.prefab_publish_mapping.v1';
  const aligned = shapeMatch.ok
    && shapeMatch.unmatchedCount === 0
    && catalog.catalogId === shapeMatch.catalogId
    && shapeMatch.catalogId === placement.catalogId
    && shapeMatch.matchId === placement.matchId
    && shapeMatch.planId === placement.planId
    && shapeMatch.sourcePlanRef === placement.sourcePlanRef
    && shapeMatch.sourceCatalogRef === placement.sourceCatalogRef
    && configuration.candidateRef.length > 0;
  if (!validKinds || !aligned) {
    throw new ProcgenPublishError('invalidProvenance', 'catalog, match, placement, and candidate provenance are not aligned');
  }
  if (!Number.isFinite(placement.cellSize) || placement.cellSize <= 0) {
    throw new ProcgenPublishError('invalidTransform', 'placement cellSize must be finite and positive');
  }
  uniqueBy(
    configuration.sourceAssets,
    (source) => source.assetId,
    'incompatibleSourceAsset',
    'source asset inventory',
  );
  uniqueBy(
    configuration.sourceAssets,
    (source) => source.artifact,
    'incompatibleSourceAsset',
    'source asset artifact inventory',
  );
  for (const source of configuration.sourceAssets) {
    if (!/^[0-9a-f]{16}$/.test(source.contentHash)) {
      throw new ProcgenPublishError(
        'incompatibleSourceAsset',
        `source asset ${source.assetId} requires a 16-digit ASHA BundleHash`,
      );
    }
  }
}

function buildPrefabDefinition(
  mapping: ProcgenPrefabPublishMapping,
  shape: ProcgenShapeCatalogForPublish['shapes'][number] | undefined,
): PrefabDefinition {
  if (shape === undefined) {
    throw new ProcgenPublishError('missingShape', `prefab mapping references missing shape ${mapping.shapeId}`);
  }
  return {
    id: prefabId(mapping.prefabId),
    schemaVersion: PREFAB_DEFINITION_SCHEMA_VERSION,
    displayName: shape.label,
    parts: [{
      id: prefabPartId(mapping.partId),
      namespace: mapping.partNamespace,
      displayName: `${shape.label} source`,
      parent: null,
      transform: identityTransform(),
      source: mapping.source,
    }],
    partRoles: [{ role: mapping.stableRole, part: prefabPartId(mapping.partId) }],
    variant: null,
  };
}

function validatePrefabRegistry(
  registry: PrefabRegistry,
  configuration: ProcgenPrefabPublishConfiguration,
): void {
  const result = decodeAndValidateAshaPrefabRegistrySourceDocument(registry, {
    assetIds: configuration.sourceAssets.map((source) => source.assetId),
    entityDefinitionIds: [],
  });
  if (!result.ok) {
    const diagnostics = result.diagnostics.map((diagnostic) => `${diagnostic.code}@${diagnostic.path}`).join(', ');
    throw new ProcgenPublishError(
      'incompatibleSourceAsset',
      `ASHA prefab source validation rejected the publication registry: ${diagnostics}`,
    );
  }
}

function placementTransform(
  instance: ProcgenPiecePlacementForPublish['instances'][number],
  allowedTransforms: readonly string[],
  cellSize: number,
): PrefabTransform {
  if (!allowedTransforms.includes(instance.transform)) {
    throw new ProcgenPublishError(
      'invalidTransform',
      `shape ${instance.shapeId} does not allow transform ${instance.transform}`,
    );
  }
  if (!Number.isFinite(instance.origin.x) || !Number.isFinite(instance.origin.y)) {
    throw new ProcgenPublishError('invalidTransform', `instance ${instance.instanceId} has a non-finite origin`);
  }
  const halfSqrt = Math.SQRT1_2;
  const rotations: Readonly<Record<string, readonly [number, number, number, number]>> = {
    identity: [0, 0, 0, 1],
    rotate90: [0, halfSqrt, 0, halfSqrt],
    rotate180: [0, 1, 0, 0],
    rotate270: [0, -halfSqrt, 0, halfSqrt],
  };
  const rotation = rotations[instance.transform];
  if (rotation === undefined) {
    throw new ProcgenPublishError(
      'invalidTransform',
      `publication adapter does not support transform ${instance.transform}`,
    );
  }
  return {
    translation: [instance.origin.x * cellSize, 0, instance.origin.y * cellSize],
    rotation,
    scale: [1, 1, 1],
  };
}

function buildManifest(
  configuration: ProcgenPrefabPublishConfiguration,
  registry: PrefabRegistry,
  prefabInstancesArtifact: ProcgenPrefabPublication['prefabInstancesArtifact'],
  assetLockArtifact: ProcgenPrefabPublication['assetLockArtifact'],
  sceneArtifact: ProcgenPrefabPublication['sceneArtifact'],
  provenance: ProcgenPrefabPublication['provenance'],
  seed: number,
): ProjectBundleManifest {
  return {
    bundleSchemaVersion: 2,
    protocolVersion: 1,
    project: { id: projectId(configuration.project.id), name: configuration.project.name },
    entryScene: sceneArtifact.id,
    scenes: [{ id: sceneArtifact.id, schemaVersion: 1, artifact: configuration.scene.artifact }],
    assetLock: { artifact: configuration.assetLockArtifact, assetCount: configuration.sourceAssets.length },
    generationProvenance: {
      provider: 'rusty-procgen.prefab-publisher',
      seed,
      version: 1,
      params: JSON.stringify({
        candidateRef: provenance.candidateRef,
        catalogId: provenance.catalogId,
        matchId: provenance.matchId,
        placementId: provenance.placementId,
      }),
    },
    artifacts: [
      {
        path: configuration.assetLockArtifact,
        class: 'durable',
        role: 'assetLock',
        contentHash: contentHash(prettyJson(assetLockArtifact)),
      },
      {
        path: configuration.prefabRegistryArtifact,
        class: 'durable',
        role: 'prefabRegistry',
        contentHash: contentHash(serializeAshaPrefabRegistrySource(registry)),
      },
      {
        path: configuration.prefabInstancesArtifact,
        class: 'durable',
        role: 'resource:prefab-instances',
        contentHash: contentHash(prettyJson(prefabInstancesArtifact)),
      },
      {
        path: configuration.scene.artifact,
        class: 'durable',
        role: 'sceneDocument',
        contentHash: contentHash(prettyJson(sceneArtifact)),
      },
      ...configuration.sourceAssets.map((source) => ({
        path: source.artifact,
        class: 'durable' as const,
        role: 'resource:procgen-prefab-source',
        contentHash: source.contentHash,
      })),
    ],
  };
}

function buildSceneArtifact(
  configuration: ProcgenPrefabPublishConfiguration,
  selected: readonly {
    readonly shape: ProcgenShapeCatalogForPublish['shapes'][number];
    readonly mapping: ProcgenPrefabPublishMapping;
    readonly identity: ProcgenPrefabPublishConfiguration['instanceIdentities'][number];
  }[],
  prefabInstances: readonly PrefabInstanceRecord[],
): FlatSceneDocument {
  const sourceAssets = new Map(configuration.sourceAssets.map((source) => [source.assetId, source]));
  const dependencies = selected.map(({ mapping }) => {
    if (mapping.source.kind !== 'voxelObject') {
      throw new ProcgenPublishError(
        'incompatibleSourceAsset',
        `first publication proof requires voxelObject scene sources, got ${mapping.source.kind}`,
      );
    }
    const source = sourceAssets.get(mapping.source.asset);
    if (source === undefined) {
      throw new ProcgenPublishError(
        'incompatibleSourceAsset',
        `prefab source ${mapping.source.asset} is absent from the asset lock`,
      );
    }
    return {
      id: source.assetId,
      version: { req: 'any' as const },
      hash: source.contentHash,
    };
  });
  const dependenciesById = new Map(dependencies.map((dependency) => [dependency.id, dependency]));
  const uniqueDependencies = [...dependenciesById.values()];
  const instancesById = new Map(prefabInstances.map((record) => [Number(record.instance), record]));
  return {
    schemaVersion: 1,
    id: sceneId(configuration.scene.id),
    metadata: {
      name: `${configuration.project.name} generated layout`,
      authoringFormatVersion: 1,
    },
    dependencies: uniqueDependencies,
    nodes: selected.map(({ shape, mapping, identity }, index) => {
      const record = instancesById.get(identity.prefabInstanceId);
      if (record === undefined || mapping.source.kind !== 'voxelObject') {
        throw new ProcgenPublishError('invalidProvenance', 'prefab instance and scene projection diverged');
      }
      return {
        id: sceneNodeId(identity.prefabInstanceId),
        parent: null,
        childOrder: index,
        label: shape.label,
        tags: [
          'rusty-procgen',
          `prefab-${mapping.prefabId}`,
          `prefab-instance-${identity.prefabInstanceId}`,
        ],
        transform: record.transform,
        kind: {
          kind: 'voxelVolume' as const,
          asset: {
            id: mapping.source.asset,
            version: { req: 'any' as const },
            hash: sourceAssets.get(mapping.source.asset)?.contentHash ?? null,
          },
        },
      };
    }),
  };
}

function validateStableRole(role: string, shapeId: string): void {
  const valid = role.length > 0
    && role.split('/').every((segment) => /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(segment));
  if (!valid) {
    throw new ProcgenPublishError(
      'missingStableRole',
      `shape ${shapeId} requires a slash-scoped stable prefab part role`,
    );
  }
}

function uniqueBy<T>(
  values: readonly T[],
  key: (value: T) => string,
  code: ProcgenPublishDiagnosticCode,
  label: string,
): ReadonlyMap<string, T> {
  const result = new Map<string, T>();
  for (const value of values) {
    const identity = key(value);
    if (result.has(identity)) {
      throw new ProcgenPublishError(code, `${label} contains duplicate identity ${identity}`);
    }
    result.set(identity, value);
  }
  return result;
}

function rejectDuplicateNumbers(values: readonly number[], label: string): void {
  const seen = new Set<number>();
  for (const value of values) {
    if (!Number.isSafeInteger(value) || value <= 0) {
      throw new ProcgenPublishError('duplicateInstanceIdentity', `${label} ${value} must be a positive safe integer`);
    }
    if (seen.has(value)) {
      throw new ProcgenPublishError('duplicateInstanceIdentity', `${label} ${value} is duplicated`);
    }
    seen.add(value);
  }
}

function identityTransform(): PrefabTransform {
  return { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] };
}

function contentHash(value: unknown): string {
  const bytes = new TextEncoder().encode(typeof value === 'string' ? value : JSON.stringify(value));
  let hash = 0xcbf29ce484222325n;
  for (const byte of bytes) {
    hash ^= BigInt(byte);
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return hash.toString(16).padStart(16, '0');
}

function prettyJson(value: unknown): string {
  return `${JSON.stringify(value, null, 2)}\n`;
}
