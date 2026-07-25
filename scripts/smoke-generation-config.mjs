import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const host = '127.0.0.1';
const port = Number(process.env.GENERATION_CONFIG_SMOKE_PORT ?? 5195);
const baseUrl = `http://${host}:${port}`;
const tempDir = await mkdtemp(join(tmpdir(), 'asha-procgen-generation-config-smoke-'));
const configPath = join(tempDir, 'viewer-generation.json');
await writeFile(configPath, await readFile('config/viewer-generation.json', 'utf8'), 'utf8');

const server = spawn(
  process.execPath,
  ['scripts/serve-viewer.mjs', '--host', host, '--port', String(port)],
  {
    cwd: process.cwd(),
    env: {
      ...process.env,
      ASHA_PROCGEN_GENERATION_CONFIG_PATH: configPath,
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
  const candidateId = batch.accepted?.[0]?.candidateId;
  if (typeof candidateId !== 'string') {
    throw new Error('generation config smoke requires one accepted batch candidate');
  }

  const defaults = await fetchJson('/api/generation-config');
  assertConfigEnvelope(defaults);
  const configured = structuredClone(defaults);
  configured.geometryLayoutPolicy.initialColumnGap.value = 160;
  configured.placementPolicy.minimumClearanceCells.value = 5;
  configured.placementPolicy.wallThicknessCells.value = 1;
  configured.corridorRealization.value = 'procedural';

  const first = await postRebuild({ candidateId, config: configured }, 200);
  const repeated = await postRebuild({ candidateId, config: configured }, 200);
  if (
    first.kind !== 'asha_procgen.viewer_generation_rebuild.v1'
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
  if (JSON.stringify(persistedConfigured) !== JSON.stringify(configured)) {
    throw new Error('successful rebuild did not atomically persist the submitted configuration');
  }

  const pureCatalog = structuredClone(defaults);
  pureCatalog.corridorRealization.value = 'catalog';
  const pureCatalogFailure = await postRebuild(
    { candidateId, config: pureCatalog },
    422,
    'pure_catalog_search_exhausted',
  );
  assertPureCatalogExhaustionEvidence(pureCatalogFailure.evidence);
  if (JSON.stringify(await readConfigFile()) !== JSON.stringify(configured)) {
    throw new Error('failed pure catalog rebuild changed the persisted configuration');
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
  if (JSON.stringify(await readConfigFile()) !== JSON.stringify(configured)) {
    throw new Error('failed pipeline rebuild changed the persisted configuration');
  }

  await postRebuild(
    { candidateId, config: { ...configured, unexpected: true } },
    400,
    'invalid_generationConfig_fields',
  );
  if (JSON.stringify(await readConfigFile()) !== JSON.stringify(configured)) {
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
    JSON.stringify(persistedReset) !== JSON.stringify(reset)
    || JSON.stringify(fetchedReset) !== JSON.stringify(reset)
  ) {
    throw new Error('default reset was not visible in persisted storage and the config API');
  }

  const methodResponse = await fetch(`${baseUrl}/api/generation-config/rebuild`);
  if (methodResponse.status !== 405) {
    throw new Error(`generation config rebuild GET expected 405, received ${methodResponse.status}`);
  }

  console.log(
    `generation config smoke passed; ${candidateId} combined build ${first.buildId.slice(0, 12)}, rollback preserved config, defaults reset in ${resetResult.buildId.slice(0, 12)}`,
  );
} finally {
  server.kill('SIGTERM');
  await waitForChildExit(server);
  await rm(tempDir, { recursive: true, force: true });
}

function assertConfigEnvelope(config) {
  if (
    config.kind !== 'asha_procgen.viewer_generation_config.v1'
    || config.schemaVersion !== 1
  ) {
    throw new Error(`unexpected generation config envelope: ${JSON.stringify(config)}`);
  }
}

function assertPureCatalogExhaustionEvidence(evidence) {
  const failure = evidence?.failure;
  const budgets = evidence?.budgets;
  if (
    evidence?.kind !== 'asha_procgen.pure_catalog_exhaustion.v1'
    || evidence.schemaVersion !== 1
    || !Array.isArray(failure?.requiredEndpoints)
    || failure.requiredEndpoints.length === 0
    || typeof failure.fixedPort?.neighborPieceId !== 'string'
    || (failure.originBounds == null && failure.laneEnvelope == null)
    || !Array.isArray(failure.exhaustedFamilies)
    || failure.exhaustedFamilies.length === 0
    || !Number.isInteger(budgets?.decisions)
    || budgets.decisions > budgets.maxDecisions
    || !Number.isInteger(budgets?.backtracks)
    || budgets.backtracks > budgets.maxBacktracks
  ) {
    throw new Error(`combined pure catalog rejection evidence was incomplete: ${JSON.stringify(evidence)}`);
  }
}

function withDefaultValues(config) {
  const reset = structuredClone(config);
  for (const setting of Object.values(reset.geometryLayoutPolicy)) {
    setting.value = setting.defaultValue;
  }
  for (const setting of Object.values(reset.placementPolicy)) {
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
