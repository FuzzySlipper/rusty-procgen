import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { decodeCatalogGenerationRun } from '../dist/ts/src/catalog-generation-trace.js';

const host = '127.0.0.1';
const port = Number(process.env.GENERATION_CONFIG_SMOKE_PORT ?? 5195);
const baseUrl = `http://${host}:${port}`;
const tempDir = await mkdtemp(join(tmpdir(), 'rusty-procgen-generation-config-smoke-'));
const configPath = join(tempDir, 'viewer-generation.json');
const legacyConfigBytes = await readFile(
  'fixtures/policies/catalog-aware-coverage-config.v1.json',
  'utf8',
);
await writeFile(configPath, legacyConfigBytes, 'utf8');

const server = spawn(
  process.execPath,
  ['scripts/serve-viewer.mjs', '--host', host, '--port', String(port)],
  {
    cwd: process.cwd(),
    env: {
      ...process.env,
      RUSTY_PROCGEN_GENERATION_CONFIG_PATH: configPath,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  },
);

let serverLog = '';
server.stdout.on('data', (chunk) => {
  serverLog += chunk.toString();
});
server.stderr.on('data', (chunk) => {
  serverLog += chunk.toString();
});

try {
  await waitForHealth();
  const batch = await fetchJson('/api/batches/v2');
  const candidateId = batch.accepted?.find(
    (entry) => entry.profileSequence === 'lock-key-baseline',
  )?.candidateId ?? batch.accepted?.[0]?.candidateId;
  if (typeof candidateId !== 'string') {
    throw new Error('generation config smoke requires one accepted batch candidate');
  }

  const defaults = await fetchJson('/api/generation-config');
  assertConfigEnvelope(defaults);
  if (await readFile(configPath, 'utf8') !== legacyConfigBytes) {
    throw new Error('reading the legacy config mutated it before an accepted rebuild');
  }
  const configured = structuredClone(defaults);
  configured.geometryLayoutPolicy.initialColumnGap.value = 160;
  configured.placementPolicy.minimumClearanceCells.value = 5;
  configured.placementPolicy.wallThicknessCells.value = 1;
  configured.corridorRealization.value = 'procedural';

  const first = await postRebuild({ candidateId, config: configured }, 200);
  const repeated = await postRebuild({ candidateId, config: configured }, 200);
  if (
    first.kind !== 'rusty_procgen.viewer_generation_rebuild.v1'
    || first.buildId !== repeated.buildId
    || JSON.stringify(first.geometry) !== JSON.stringify(repeated.geometry)
    || JSON.stringify(first.placement) !== JSON.stringify(repeated.placement)
    || JSON.stringify(first.builtFlowValidation) !== JSON.stringify(repeated.builtFlowValidation)
    || first.config.geometryLayoutPolicy.initialColumnGap.value !== 160
    || first.config.placementPolicy.minimumClearanceCells.value !== 5
    || first.placement.corridorRealization !== 'procedural'
    || first.geometryValidation?.ok !== true
    || first.placementValidation?.ok !== true
    || first.builtFlowValidation?.ok !== true
    || first.metrics?.corridorPrefabInstances !== 0
    || first.persisted !== true
    || first.nativeAuthority !== false
  ) {
    throw new Error('combined configuration rebuild was not deterministic, complete, and persisted');
  }
  const persistedConfigured = await readConfigFile();
  if (JSON.stringify(persistedConfigured) !== JSON.stringify(persistableConfig(configured))) {
    throw new Error('successful rebuild did not atomically persist the submitted configuration');
  }

  const pureCatalog = structuredClone(defaults);
  pureCatalog.corridorRealization.value = 'catalog';
  const pureCatalogResult = await postRebuild({ candidateId, config: pureCatalog }, 200);
  if (
    pureCatalogResult.placement?.corridorRealization !== 'catalog'
    || pureCatalogResult.placement?.connectionCells?.length !== 0
    || pureCatalogResult.geometryValidation?.ok !== true
    || pureCatalogResult.placementValidation?.ok !== true
    || pureCatalogResult.builtFlowValidation?.ok !== true
  ) {
    throw new Error('catalog-aware generation did not produce an exact validated catalog build');
  }
  const acceptedTrace = decodeCatalogGenerationRun(
    pureCatalogResult.catalogAwareGeneration?.trace,
    pureCatalogResult.catalogAwareGeneration?.result,
  );
  if (
    acceptedTrace.selectedAttempt !== pureCatalogResult.catalogAwareGeneration.selectedAttempt
    || acceptedTrace.candidateId !== candidateId
  ) {
    throw new Error('successful catalog rebuild did not return its exact verified decision trace');
  }
  const repeatedPureCatalog = await postRebuild(
    { candidateId, config: pureCatalog },
    200,
  );
  if (
    repeatedPureCatalog.buildId !== pureCatalogResult.buildId
    || JSON.stringify(repeatedPureCatalog.catalogAwareGeneration)
      !== JSON.stringify(pureCatalogResult.catalogAwareGeneration)
  ) {
    throw new Error('identical catalog rebuilds did not return an exact deterministic trace pair');
  }
  if (JSON.stringify(await readConfigFile()) !== JSON.stringify(persistableConfig(pureCatalog))) {
    throw new Error('successful pure catalog rebuild did not persist the unified configuration');
  }

  const selectedOutcome = pureCatalogResult.catalogAwareGeneration?.attempts?.find(
    (attempt) => attempt.attempt === pureCatalogResult.catalogAwareGeneration.selectedAttempt,
  )?.outcome;
  const selectedWidth = selectedOutcome?.metrics?.placementWidthCells;
  if (!Number.isInteger(selectedWidth) || selectedWidth <= 1) {
    throw new Error(`catalog rebuild omitted selected outcome metrics: ${JSON.stringify(selectedOutcome)}`);
  }
  const exactWidthCatalog = structuredClone(pureCatalog);
  exactWidthCatalog.catalogAwareGenerationPolicy.maxGenerationAttempts.value = 1;
  exactWidthCatalog.catalogAwareGenerationPolicy.maxPlacementWidthCells.value = selectedWidth;
  const exactWidthResult = await postRebuild(
    { candidateId, config: exactWidthCatalog },
    200,
  );
  const exactSelectedOutcome = exactWidthResult.catalogAwareGeneration?.attempts?.find(
    (attempt) => attempt.attempt === exactWidthResult.catalogAwareGeneration.selectedAttempt,
  )?.outcome;
  if (
    exactSelectedOutcome?.metrics?.placementWidthCells !== selectedWidth
    || exactSelectedOutcome.constraintMisses.length !== 0
  ) {
    throw new Error(
      `exact hard placement-width boundary did not admit: ${JSON.stringify(exactSelectedOutcome)}`,
    );
  }
  const persistedExactWidth = await readConfigFile();
  if (
    JSON.stringify(persistedExactWidth)
      !== JSON.stringify(persistableConfig(exactWidthCatalog))
  ) {
    throw new Error('exact hard-limit rebuild did not persist its accepted configuration');
  }
  const oneUnderWidthCatalog = structuredClone(exactWidthCatalog);
  oneUnderWidthCatalog.catalogAwareGenerationPolicy.maxPlacementWidthCells.value =
    selectedWidth - 1;
  const hardLimitFailure = await postRebuild(
    { candidateId, config: oneUnderWidthCatalog },
    422,
    'catalog_aware_outcome_constraint_miss',
  );
  const widthMisses = hardLimitFailure.evidence?.attempts?.map((attempt) =>
    attempt.outcome?.constraintMisses?.find(
      (miss) => miss.metric === 'placement_width_cells',
    ));
  if (
    hardLimitFailure.evidence?.classification !== 'outcome_constraint_miss'
    || widthMisses?.length !== 1
    || widthMisses.some((miss) =>
      miss?.limit !== selectedWidth - 1
      || !Number.isInteger(miss.actual)
      || miss.actual <= miss.limit)
  ) {
    throw new Error(
      `one-under hard placement-width evidence was incomplete: ${
        JSON.stringify(hardLimitFailure.evidence)
      }`,
    );
  }
  if (JSON.stringify(await readConfigFile()) !== JSON.stringify(persistedExactWidth)) {
    throw new Error('hard outcome rejection changed the persisted configuration');
  }

  const constrainedCatalog = structuredClone(pureCatalog);
  constrainedCatalog.catalogAwareGenerationPolicy.maxGenerationAttempts.value = 1;
  constrainedCatalog.catalogAwareGenerationPolicy.maxRoutingStatesPerSection.value = 100;
  const pureCatalogFailure = await postRebuild(
    { candidateId, config: constrainedCatalog },
    422,
    'catalog_aware_search_budget_exhaustion',
  );
  assertCatalogAwareExhaustionEvidence(pureCatalogFailure.evidence);
  const exhaustedTrace = decodeCatalogGenerationRun(
    pureCatalogFailure.evidence.trace,
    pureCatalogFailure.evidence.result,
  );
  if (exhaustedTrace.selectedAttempt !== null || exhaustedTrace.candidateId !== candidateId) {
    throw new Error('exhausted catalog rebuild did not return its exact verified decision trace');
  }
  if (JSON.stringify(await readConfigFile()) !== JSON.stringify(persistedExactWidth)) {
    throw new Error('failed catalog-aware rebuild changed the persisted configuration');
  }

  const compact = structuredClone(defaults);
  for (const key of ['initialRoomMargin', 'initialColumnGap', 'initialRowGap']) {
    compact.geometryLayoutPolicy[key].value = 32;
  }
  for (const key of ['roomMarginGrowth', 'columnGapGrowth', 'rowGapGrowth']) {
    compact.geometryLayoutPolicy[key].value = 8;
  }
  compact.corridorRealization.value = 'procedural';
  const compactResult = await postRebuild({ candidateId, config: compact }, 200);
  if (
    compactResult.placement?.realizationSearch?.realizationScaleTier !== 0
    || compactResult.placement.realizationSearch.realizationAttempts !== 1
    || compactResult.metrics?.footprintWidth > 59
    || compactResult.metrics?.footprintHeight > 30
    || compactResult.metrics?.routedCorridorCells > 88
    || compactResult.geometryValidation?.ok !== true
    || compactResult.placementValidation?.ok !== true
    || compactResult.builtFlowValidation?.ok !== true
  ) {
    throw new Error(
      `compact-first physical realization regressed for ${candidateId}: `
        + JSON.stringify({
          search: compactResult.placement?.realizationSearch,
          metrics: compactResult.metrics,
      }),
    );
  }
  if (JSON.stringify(await readConfigFile()) !== JSON.stringify(persistableConfig(compact))) {
    throw new Error('successful compact-first rebuild did not persist the unified configuration');
  }

  const constrained = structuredClone(configured);
  for (const key of ['initialRoomMargin', 'initialColumnGap', 'initialRowGap']) {
    constrained.geometryLayoutPolicy[key].value = 32;
  }
  for (const key of ['roomMarginGrowth', 'columnGapGrowth', 'rowGapGrowth']) {
    constrained.geometryLayoutPolicy[key].value = 0;
  }
  constrained.geometryLayoutPolicy.maxSpacingTiers.value = 1;
  constrained.geometryLayoutPolicy.roomOrderAttemptsPerTier.value = 1;
  constrained.geometryLayoutPolicy.maxSearchAttempts.value = 1;
  await postRebuild(
    { candidateId, config: constrained },
    422,
    'geometry_search_exhausted',
  );
  if (JSON.stringify(await readConfigFile()) !== JSON.stringify(persistableConfig(compact))) {
    throw new Error('failed pipeline rebuild changed the persisted configuration');
  }

  await postRebuild(
    { candidateId, config: { ...configured, unexpected: true } },
    400,
    'invalid_generationConfig_fields',
  );
  if (JSON.stringify(await readConfigFile()) !== JSON.stringify(persistableConfig(compact))) {
    throw new Error('invalid configuration request changed the persisted configuration');
  }

  const reset = withDefaultValues(configured);
  const resetResult = await postRebuild({ candidateId, config: reset }, 200);
  if (
    resetResult.buildId === first.buildId
    || resetResult.config.geometryLayoutPolicy.initialColumnGap.value !== 144
    || resetResult.config.placementPolicy.minimumClearanceCells.value !== 3
    || resetResult.placement.corridorRealization !== 'hybrid'
    || resetResult.geometryValidation?.ok !== true
    || resetResult.placementValidation?.ok !== true
    || resetResult.builtFlowValidation?.ok !== true
  ) {
    throw new Error('default reset did not rebuild the complete configured pipeline');
  }
  const persistedReset = await readConfigFile();
  const fetchedReset = await fetchJson('/api/generation-config');
  if (
    JSON.stringify(persistedReset) !== JSON.stringify(persistableConfig(reset))
    || JSON.stringify(fetchedReset) !== JSON.stringify(persistableConfig(reset))
  ) {
    throw new Error('default reset was not visible in persisted storage and the config API');
  }

  const methodResponse = await fetch(`${baseUrl}/api/generation-config/rebuild`);
  if (methodResponse.status !== 405) {
    throw new Error(`generation config rebuild GET expected 405, received ${methodResponse.status}`);
  }

  console.log(
    `generation config smoke passed; ${candidateId} compact physical scale 1 at `
      + `${compactResult.metrics.footprintWidth}x${compactResult.metrics.footprintHeight}/`
      + `${compactResult.metrics.routedCorridorCells} routed cells; combined build `
      + `${first.buildId.slice(0, 12)}, rollback preserved config, defaults reset in `
      + `${resetResult.buildId.slice(0, 12)}`,
  );
} finally {
  server.kill('SIGTERM');
  await waitForChildExit(server);
  await rm(tempDir, { recursive: true, force: true });
}

function assertConfigEnvelope(config) {
  if (
    config.kind !== 'rusty_procgen.viewer_generation_config.v2'
    || config.schemaVersion !== 2
    || config.migration?.sourceKind !== 'rusty_procgen.viewer_generation_config.v1'
    || config.migration?.sourceSchemaVersion !== 1
    || !config.migration.appliedDefaults.includes('initialRoomCompactionCells=0')
    || !config.migration.appliedDefaults.includes('roomCompactionGrowthCells=1')
    || !config.migration.appliedDefaults.includes('preferredMaximum=286')
  ) {
    throw new Error(`unexpected generation config envelope: ${JSON.stringify(config)}`);
  }
}

function assertCatalogAwareExhaustionEvidence(evidence) {
  if (
    evidence?.kind !== 'rusty_procgen.catalog_aware_generation_exhaustion.v2'
    || evidence.schemaVersion !== 2
    || evidence.classification !== 'search_budget_exhaustion'
    || !Array.isArray(evidence.attempts)
    || evidence.attempts.length !== 1
    || evidence.attempts[0]?.classification !== 'search_budget_exhaustion'
    || !Number.isInteger(evidence.attempts[0]?.routingStates)
    || evidence.result?.kind !== 'rusty_procgen.catalog_aware_generation.v2'
    || evidence.trace?.kind !== 'rusty_procgen.catalog_generation_trace.v2'
  ) {
    throw new Error(`catalog-aware exhaustion evidence was incomplete: ${JSON.stringify(evidence)}`);
  }
}

function persistableConfig(config) {
  const persisted = structuredClone(config);
  persisted.migration = null;
  return persisted;
}

function withDefaultValues(config) {
  const reset = structuredClone(config);
  for (const setting of Object.values(reset.geometryLayoutPolicy)) {
    setting.value = setting.defaultValue;
  }
  for (const setting of Object.values(reset.placementPolicy)) {
    setting.value = setting.defaultValue;
  }
  for (const setting of Object.values(reset.catalogAwareGenerationPolicy)) {
    setting.value = setting.defaultValue;
  }
  reset.corridorRealization.value = reset.corridorRealization.defaultValue;
  return reset;
}

async function postRebuild(payload, expectedStatus, expectedError) {
  const response = await fetch(`${baseUrl}/api/generation-config/rebuild`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  const result = await response.json();
  if (response.status !== expectedStatus) {
    throw new Error(
      `generation config expected ${expectedStatus}, received ${response.status}: ${JSON.stringify(result)}`,
    );
  }
  if (expectedError !== undefined && result.error !== expectedError) {
    throw new Error(`generation config expected ${expectedError}, received ${JSON.stringify(result)}`);
  }
  return result;
}

async function fetchJson(path) {
  const response = await fetch(`${baseUrl}${path}`);
  if (!response.ok) {
    throw new Error(`failed to fetch ${path}: ${response.status}`);
  }
  return await response.json();
}

async function readConfigFile() {
  return JSON.parse(await readFile(configPath, 'utf8'));
}

async function waitForHealth() {
  const started = Date.now();
  while (Date.now() - started < 10_000) {
    try {
      const response = await fetch(`${baseUrl}/health`);
      if (response.ok) {
        return;
      }
    } catch {
      // Server is still starting.
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`generation config smoke server did not start:\n${serverLog}`);
}

async function waitForChildExit(child) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  await new Promise((resolve) => child.once('exit', resolve));
}
