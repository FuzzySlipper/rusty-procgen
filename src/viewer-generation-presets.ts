export type CorridorRealization = 'catalog' | 'hybrid' | 'procedural';

export interface GenerationConfigSetting<T> {
  value: T;
  readonly defaultValue: T;
}

export interface ViewerGenerationConfig {
  readonly kind: 'rusty_procgen.viewer_generation_config.v2';
  readonly schemaVersion: 2;
  readonly migration: {
    readonly sourceKind: 'rusty_procgen.viewer_generation_config.v1';
    readonly sourceSchemaVersion: 1;
    readonly appliedDefaults: readonly string[];
  } | null;
  readonly geometryLayoutPolicy: {
    readonly initialRoomMargin: GenerationConfigSetting<number>;
    readonly initialColumnGap: GenerationConfigSetting<number>;
    readonly initialRowGap: GenerationConfigSetting<number>;
    readonly roomMarginGrowth: GenerationConfigSetting<number>;
    readonly columnGapGrowth: GenerationConfigSetting<number>;
    readonly rowGapGrowth: GenerationConfigSetting<number>;
    readonly maxSpacingTiers: GenerationConfigSetting<number>;
    readonly roomOrderAttemptsPerTier: GenerationConfigSetting<number>;
    readonly maxSearchAttempts: GenerationConfigSetting<number>;
  };
  readonly placementPolicy: {
    readonly minimumClearanceCells: GenerationConfigSetting<number>;
    readonly wallThicknessCells: GenerationConfigSetting<number>;
  };
  readonly catalogAwareGenerationPolicy: {
    readonly maxGenerationAttempts: GenerationConfigSetting<number>;
    readonly initialRoomCompactionCells: GenerationConfigSetting<number>;
    readonly roomCompactionGrowthCells: GenerationConfigSetting<number>;
    readonly maxRoomCandidates: GenerationConfigSetting<number>;
    readonly maxRoutingStatesPerSection: GenerationConfigSetting<number>;
    readonly routeMarginCells: GenerationConfigSetting<number>;
    readonly guideDistanceWeight: GenerationConfigSetting<number>;
    readonly turnPenalty: GenerationConfigSetting<number>;
    readonly maxPlacementWidthCells: GenerationConfigSetting<number>;
    readonly maxPlacementHeightCells: GenerationConfigSetting<number>;
    readonly maxPlacementAreaCells: GenerationConfigSetting<number>;
    readonly maxRoutedCatalogCells: GenerationConfigSetting<number>;
    readonly primaryMetric: GenerationConfigSetting<
      'placement_span' | 'placement_area' | 'routed_catalog_cells'
    >;
    readonly preferredMaximum: GenerationConfigSetting<number>;
  };
  readonly corridorRealization: GenerationConfigSetting<CorridorRealization>;
}

export interface ViewerGenerationPreset {
  readonly id: 'tight' | 'normal' | 'spread';
  readonly label: string;
  readonly summary: string;
  readonly config: ViewerGenerationConfig;
}

export interface ViewerGenerationPresets {
  readonly kind: 'rusty_procgen.viewer_generation_presets.v1';
  readonly schemaVersion: 1;
  readonly sourceBaseConfigRef: string;
  readonly presets: readonly ViewerGenerationPreset[];
}

const geometryKeys = [
  'initialRoomMargin',
  'initialColumnGap',
  'initialRowGap',
  'roomMarginGrowth',
  'columnGapGrowth',
  'rowGapGrowth',
  'maxSpacingTiers',
  'roomOrderAttemptsPerTier',
  'maxSearchAttempts',
] as const;
const placementKeys = [
  'minimumClearanceCells',
  'wallThicknessCells',
] as const;
const catalogKeys = [
  'maxGenerationAttempts',
  'initialRoomCompactionCells',
  'roomCompactionGrowthCells',
  'maxRoomCandidates',
  'maxRoutingStatesPerSection',
  'routeMarginCells',
  'guideDistanceWeight',
  'turnPenalty',
  'maxPlacementWidthCells',
  'maxPlacementHeightCells',
  'maxPlacementAreaCells',
  'maxRoutedCatalogCells',
  'primaryMetric',
  'preferredMaximum',
] as const;

export function decodeViewerGenerationConfig(input: unknown): ViewerGenerationConfig {
  const root = exactObject(
    input,
    [
      'kind',
      'schemaVersion',
      'migration',
      'geometryLayoutPolicy',
      'placementPolicy',
      'catalogAwareGenerationPolicy',
      'corridorRealization',
    ],
    'generation config',
  );
  if (
    root.kind !== 'rusty_procgen.viewer_generation_config.v2'
    || root.schemaVersion !== 2
  ) {
    throw new Error('generation config uses an unsupported kind or schema');
  }
  decodeMigration(root.migration);
  const geometry = exactObject(
    root.geometryLayoutPolicy,
    geometryKeys,
    'generation config geometry policy',
  );
  const placement = exactObject(
    root.placementPolicy,
    placementKeys,
    'generation config placement policy',
  );
  const catalog = exactObject(
    root.catalogAwareGenerationPolicy,
    catalogKeys,
    'generation config catalog policy',
  );
  for (const key of geometryKeys) {
    decodeSetting(geometry[key], `geometry policy ${key}`, isInteger);
  }
  for (const key of placementKeys) {
    decodeSetting(placement[key], `placement policy ${key}`, isInteger);
  }
  for (const key of catalogKeys) {
    if (key === 'primaryMetric') {
      decodeSetting(catalog[key], `catalog policy ${key}`, isPrimaryMetric);
    } else {
      decodeSetting(catalog[key], `catalog policy ${key}`, isInteger);
    }
  }
  decodeSetting(root.corridorRealization, 'corridor realization', isCorridorRealization);
  return structuredClone(root) as unknown as ViewerGenerationConfig;
}

export function decodeViewerGenerationPresets(input: unknown): ViewerGenerationPresets {
  const root = exactObject(
    input,
    ['kind', 'schemaVersion', 'sourceBaseConfigRef', 'presets'],
    'generation presets',
  );
  if (
    root.kind !== 'rusty_procgen.viewer_generation_presets.v1'
    || root.schemaVersion !== 1
    || typeof root.sourceBaseConfigRef !== 'string'
    || root.sourceBaseConfigRef.length === 0
    || !Array.isArray(root.presets)
    || root.presets.length !== 3
  ) {
    throw new Error('generation presets use an invalid envelope');
  }
  const expectedIds = ['tight', 'normal', 'spread'];
  const presets = root.presets.map((inputPreset, index) => {
    const preset = exactObject(
      inputPreset,
      ['id', 'label', 'summary', 'config'],
      'generation preset',
    );
    if (
      preset.id !== expectedIds[index]
      || typeof preset.label !== 'string'
      || preset.label.length === 0
      || typeof preset.summary !== 'string'
      || preset.summary.length === 0
    ) {
      throw new Error('generation preset identity or copy is invalid');
    }
    return {
      id: preset.id as ViewerGenerationPreset['id'],
      label: preset.label,
      summary: preset.summary,
      config: decodeViewerGenerationConfig(preset.config),
    };
  });
  return {
    kind: root.kind,
    schemaVersion: root.schemaVersion,
    sourceBaseConfigRef: root.sourceBaseConfigRef,
    presets,
  };
}

function decodeMigration(input: unknown): void {
  if (input === null) {
    return;
  }
  const migration = exactObject(
    input,
    ['sourceKind', 'sourceSchemaVersion', 'appliedDefaults'],
    'generation config migration',
  );
  if (
    migration.sourceKind !== 'rusty_procgen.viewer_generation_config.v1'
    || migration.sourceSchemaVersion !== 1
    || !Array.isArray(migration.appliedDefaults)
    || migration.appliedDefaults.some((value) => typeof value !== 'string')
  ) {
    throw new Error('generation config migration is invalid');
  }
}

function decodeSetting<T>(
  input: unknown,
  label: string,
  predicate: (value: unknown) => value is T,
): void {
  const setting = exactObject(input, ['value', 'defaultValue'], label);
  if (!predicate(setting.value) || !predicate(setting.defaultValue)) {
    throw new Error(`${label} has an invalid value`);
  }
}

function exactObject(
  input: unknown,
  keys: readonly string[],
  label: string,
): Record<string, unknown> {
  if (input === null || typeof input !== 'object' || Array.isArray(input)) {
    throw new Error(`${label} must be an object`);
  }
  const object = input as Record<string, unknown>;
  if (
    JSON.stringify(Object.keys(object).sort())
    !== JSON.stringify([...keys].sort())
  ) {
    throw new Error(`${label} has unexpected fields`);
  }
  return object;
}

function isInteger(value: unknown): value is number {
  return Number.isSafeInteger(value);
}

function isPrimaryMetric(
  value: unknown,
): value is 'placement_span' | 'placement_area' | 'routed_catalog_cells' {
  return [
    'placement_span',
    'placement_area',
    'routed_catalog_cells',
  ].includes(String(value));
}

function isCorridorRealization(value: unknown): value is CorridorRealization {
  return ['catalog', 'hybrid', 'procedural'].includes(String(value));
}
