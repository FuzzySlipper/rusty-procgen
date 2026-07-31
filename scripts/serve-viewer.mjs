import { createReadStream } from 'node:fs';
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, rename, rm, stat, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { dirname, extname, join, resolve, sep } from 'node:path';
import { tmpdir } from 'node:os';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const execFileAsync = promisify(execFile);
const selectionReportPath = join(repoRoot, 'artifacts/samples/batch-v2/selection-report.json');
const generationConfigPath = process.env.RUSTY_PROCGEN_GENERATION_CONFIG_PATH === undefined
  ? join(repoRoot, 'config/viewer-generation.json')
  : resolve(process.env.RUSTY_PROCGEN_GENERATION_CONFIG_PATH);
const generationPresetDefinitionsPath =
  process.env.RUSTY_PROCGEN_GENERATION_PRESETS_PATH === undefined
    ? join(repoRoot, 'fixtures/policies/viewer-generation-presets.v1.json')
    : resolve(process.env.RUSTY_PROCGEN_GENERATION_PRESETS_PATH);
const generationPresetBaseConfigRef =
  'fixtures/policies/viewer-generation-default.v2.json';
const args = parseArgs(process.argv.slice(2));
const host = args.host ?? process.env.HOST ?? process.env.npm_config_host ?? '0.0.0.0';
const port = Number(args.port ?? process.env.PORT ?? process.env.npm_config_port ?? 5183);
const PROCGEN_COMMAND_TIMEOUT_MS = 120_000;
const catalogGenerationRuns = [
  {
    id: 'accepted-default',
    label: 'Accepted · default bounded policy',
    result: join(repoRoot, 'fixtures/catalog-generation/candidate-000-result.v1.json'),
    trace: join(repoRoot, 'fixtures/catalog-generation/candidate-000-trace.v1.json'),
  },
  {
    id: 'exhausted-route-budget',
    label: 'Exhausted · 100-state route budget',
    result: join(repoRoot, 'fixtures/catalog-generation/candidate-000-exhausted-result.v1.json'),
    trace: join(repoRoot, 'fixtures/catalog-generation/candidate-000-exhausted-trace.v1.json'),
  },
  {
    id: 'best-admissible-selection',
    label: 'Accepted · exhausted preference selects best admissible',
    result: join(repoRoot, 'fixtures/catalog-generation/candidate-000-selection-result.v1.json'),
    trace: join(repoRoot, 'fixtures/catalog-generation/candidate-000-selection-trace.v1.json'),
  },
  {
    id: 'control-tight-5201-rejected',
    label: 'Rejected · 5201 · tight initial spacing',
    result: join(repoRoot, 'fixtures/catalog-generation/control-sprawling-5201-tight-result.v1.json'),
    trace: join(repoRoot, 'fixtures/catalog-generation/control-sprawling-5201-tight-trace.v1.json'),
  },
  {
    id: 'control-tight-5801-accepted',
    label: 'Compact · 5801 · tight initial spacing',
    result: join(repoRoot, 'fixtures/catalog-generation/control-compact-5801-tight-result.v1.json'),
    trace: join(repoRoot, 'fixtures/catalog-generation/control-compact-5801-tight-trace.v1.json'),
  },
];

const routes = new Map([
  ['/', join(repoRoot, 'viewer/index.html')],
  ['/viewer/index.html', join(repoRoot, 'viewer/index.html')],
  ['/viewer/styles.css', join(repoRoot, 'viewer/styles.css')],
  ['/viewer/app.js', join(repoRoot, 'dist/ts/viewer/app.js')],
  ['/api/artifacts/first-run', join(repoRoot, 'artifacts/samples/first-run/accepted.json')],
  ['/api/batches/v2', join(repoRoot, 'artifacts/samples/batch-v2/selection-report.json')],
  ['/api/evidence/engine-spatial-extrusion', join(repoRoot, 'artifacts/evidence/engine-spatial-extrusion.json')],
  ['/api/evidence/engine-ca-benchmark', join(repoRoot, 'artifacts/evidence/engine-ca-benchmark.json')],
  ['/src/voxel-extrusion.js', join(repoRoot, 'dist/ts/src/voxel-extrusion.js')],
]);

const server = createServer(async (request, response) => {
  response.setHeader('X-Den-Project', 'rusty-procgen');
  const url = new URL(request.url ?? '/', `http://${request.headers.host ?? 'localhost'}`);
  if (url.pathname === '/health') {
    sendJson(response, 200, { ok: true, project: 'rusty-procgen' });
    return;
  }

  if (url.pathname === '/api/evidence/catalog-generation-runs') {
    if (request.method !== 'GET') {
      response.setHeader('Allow', 'GET');
      sendJson(response, 405, { error: 'method_not_allowed', detail: 'Use GET.' });
      return;
    }
    try {
      sendJson(response, 200, {
        kind: 'rusty_procgen.catalog_generation_trace_bundle.v1',
        schemaVersion: 1,
        runs: await Promise.all(catalogGenerationRuns.map(async (run) => ({
          id: run.id,
          label: run.label,
          result: JSON.parse(await readFile(run.result, 'utf8')),
          trace: JSON.parse(await readFile(run.trace, 'utf8')),
        }))),
      });
    } catch (error) {
      sendJson(response, 500, {
        error: 'catalog_generation_trace_read_failed',
        detail: error instanceof Error ? error.message : String(error),
      });
    }
    return;
  }

  if (url.pathname === '/api/generation-config') {
    if (request.method !== 'GET') {
      response.setHeader('Allow', 'GET');
      sendJson(response, 405, { error: 'method_not_allowed', detail: 'Use GET.' });
      return;
    }
    try {
      sendJson(response, 200, await readGenerationConfig());
    } catch (error) {
      const statusCode = error instanceof ExperimentError ? error.statusCode : 500;
      sendJson(response, statusCode, {
        error: error instanceof ExperimentError ? error.code : 'config_read_failed',
        detail: error instanceof Error ? error.message : String(error),
      });
    }
    return;
  }

  if (url.pathname === '/api/generation-presets') {
    if (request.method !== 'GET') {
      response.setHeader('Allow', 'GET');
      sendJson(response, 405, { error: 'method_not_allowed', detail: 'Use GET.' });
      return;
    }
    try {
      sendJson(response, 200, await readGenerationPresets());
    } catch (error) {
      const statusCode = error instanceof ExperimentError ? error.statusCode : 500;
      sendJson(response, statusCode, {
        error: error instanceof ExperimentError ? error.code : 'generation_presets_read_failed',
        detail: error instanceof Error ? error.message : String(error),
      });
    }
    return;
  }

  if (url.pathname === '/api/generation-presets/rebuild') {
    if (request.method !== 'POST') {
      response.setHeader('Allow', 'POST');
      sendJson(response, 405, { error: 'method_not_allowed', detail: 'Use POST.' });
      return;
    }
    try {
      const payload = await readJsonRequest(request, 4_096);
      sendJson(response, 200, await runGenerationPresetRebuild(payload));
    } catch (error) {
      const statusCode = error instanceof ExperimentError ? error.statusCode : 500;
      sendJson(response, statusCode, {
        error: error instanceof ExperimentError ? error.code : 'generation_preset_rebuild_failed',
        detail: error instanceof Error ? error.message : String(error),
        ...(error instanceof ExperimentError && error.evidence !== undefined
          ? { evidence: error.evidence }
          : {}),
      });
    }
    return;
  }

  if (url.pathname === '/api/generation-config/rebuild') {
    if (request.method !== 'POST') {
      response.setHeader('Allow', 'POST');
      sendJson(response, 405, { error: 'method_not_allowed', detail: 'Use POST.' });
      return;
    }
    try {
      const payload = await readJsonRequest(request, 32_768);
      sendJson(response, 200, await runGenerationConfigRebuild(payload));
    } catch (error) {
      const statusCode = error instanceof ExperimentError ? error.statusCode : 500;
      sendJson(response, statusCode, {
        error: error instanceof ExperimentError ? error.code : 'generation_config_rebuild_failed',
        detail: error instanceof Error ? error.message : String(error),
        ...(error instanceof ExperimentError && error.evidence !== undefined
          ? { evidence: error.evidence }
          : {}),
      });
    }
    return;
  }

  if (url.pathname === '/api/experiments/placement-policy') {
    if (request.method !== 'POST') {
      response.setHeader('Allow', 'POST');
      sendJson(response, 405, { error: 'method_not_allowed', detail: 'Use POST.' });
      return;
    }
    try {
      const payload = await readJsonRequest(request, 16_384);
      const result = await runPlacementPolicyExperiment(payload);
      sendJson(response, 200, result);
    } catch (error) {
      const statusCode = error instanceof ExperimentError ? error.statusCode : 500;
      sendJson(response, statusCode, {
        error: error instanceof ExperimentError ? error.code : 'experiment_failed',
        detail: error instanceof Error ? error.message : String(error),
        ...(error instanceof ExperimentError && error.evidence !== undefined
          ? { evidence: error.evidence }
          : {}),
      });
    }
    return;
  }

  if (url.pathname === '/api/experiments/geometry-layout-policy') {
    if (request.method !== 'POST') {
      response.setHeader('Allow', 'POST');
      sendJson(response, 405, { error: 'method_not_allowed', detail: 'Use POST.' });
      return;
    }
    try {
      const payload = await readJsonRequest(request, 16_384);
      const result = await runGeometryLayoutPolicyExperiment(payload);
      sendJson(response, 200, result);
    } catch (error) {
      const statusCode = error instanceof ExperimentError ? error.statusCode : 500;
      sendJson(response, statusCode, {
        error: error instanceof ExperimentError ? error.code : 'experiment_failed',
        detail: error instanceof Error ? error.message : String(error),
        ...(error instanceof ExperimentError && error.evidence !== undefined
          ? { evidence: error.evidence }
          : {}),
      });
    }
    return;
  }

  if (url.pathname === '/api/experiments/corridor-realization') {
    if (request.method !== 'POST') {
      response.setHeader('Allow', 'POST');
      sendJson(response, 405, { error: 'method_not_allowed', detail: 'Use POST.' });
      return;
    }
    try {
      const payload = await readJsonRequest(request, 16_384);
      const result = await runCorridorRealizationExperiment(payload);
      sendJson(response, 200, result);
    } catch (error) {
      const statusCode = error instanceof ExperimentError ? error.statusCode : 500;
      sendJson(response, statusCode, {
        error: error instanceof ExperimentError ? error.code : 'experiment_failed',
        detail: error instanceof Error ? error.message : String(error),
        ...(error instanceof ExperimentError && error.evidence !== undefined
          ? { evidence: error.evidence }
          : {}),
      });
    }
    return;
  }

  if (url.pathname === '/api/artifacts/by-path') {
    const requestedPath = url.searchParams.get('path');
    const filePath = requestedPath === null ? null : resolve(repoRoot, requestedPath);
    const allowedRoots = [
      resolve(repoRoot, 'artifacts/samples'),
      resolve(repoRoot, 'fixtures'),
    ];
    if (filePath === null || !allowedRoots.some((root) => isInside(filePath, root))) {
      response.writeHead(400);
      response.end('Invalid artifact path');
      return;
    }
    await sendFile(response, filePath);
    return;
  }

  if (url.pathname.startsWith('/fixtures/')) {
    const filePath = resolve(repoRoot, url.pathname.slice(1));
    const fixtureRoot = resolve(repoRoot, 'fixtures');
    if (!isInside(filePath, fixtureRoot)) {
      response.writeHead(400);
      response.end('Invalid fixture path');
      return;
    }
    await sendFile(response, filePath);
    return;
  }

  const filePath = routes.get(url.pathname);
  if (filePath === undefined) {
    response.writeHead(404);
    response.end('Not found');
    return;
  }
  await sendFile(response, filePath);
});

server.listen(port, host, () => {
  const address = server.address();
  const selectedPort = typeof address === 'object' && address !== null ? address.port : port;
  console.log(`rusty-procgen viewer listening at http://${host}:${selectedPort}`);
  console.log('"project": "rusty-procgen"');
});

process.on('SIGTERM', () => server.close(() => process.exit(0)));
process.on('SIGINT', () => server.close(() => process.exit(0)));

async function sendFile(response, filePath) {
  try {
    const fileStat = await stat(filePath);
    if (!fileStat.isFile()) {
      throw new Error('not a file');
    }
    response.writeHead(200, {
      'Content-Type': contentType(filePath),
      'Cache-Control': 'no-store',
    });
    createReadStream(filePath).pipe(response);
  } catch {
    response.writeHead(404);
    response.end('Not found');
  }
}

function isInside(filePath, rootPath) {
  return filePath === rootPath || filePath.startsWith(`${rootPath}${sep}`);
}

function sendJson(response, statusCode, value) {
  response.writeHead(statusCode, { 'Content-Type': 'application/json; charset=utf-8' });
  response.end(`${JSON.stringify(value, null, 2)}\n`);
}

class ExperimentError extends Error {
  constructor(statusCode, code, message, evidence = undefined) {
    super(message);
    this.statusCode = statusCode;
    this.code = code;
    this.evidence = evidence;
  }
}

function pureCatalogExhaustionEvidence(detail) {
  const marker = 'evidence=';
  const markerIndex = detail.lastIndexOf(marker);
  if (markerIndex < 0) {
    return undefined;
  }
  try {
    const evidence = JSON.parse(detail.slice(markerIndex + marker.length).trim());
    if (
      evidence?.kind !== 'rusty_procgen.pure_catalog_exhaustion.v1'
      || evidence.schemaVersion !== 1
      || typeof evidence.failure !== 'object'
      || typeof evidence.budgets !== 'object'
    ) {
      return undefined;
    }
    return evidence;
  } catch {
    return undefined;
  }
}

async function readJsonRequest(request, maxBytes) {
  let size = 0;
  const chunks = [];
  for await (const chunk of request) {
    size += chunk.length;
    if (size > maxBytes) {
      throw new ExperimentError(413, 'request_too_large', `Request body exceeds ${maxBytes} bytes.`);
    }
    chunks.push(chunk);
  }
  try {
    return JSON.parse(Buffer.concat(chunks).toString('utf8'));
  } catch {
    throw new ExperimentError(400, 'invalid_json', 'Request body must be valid JSON.');
  }
}

async function readGenerationConfig() {
  try {
    return validateGenerationConfig(JSON.parse(await readFile(generationConfigPath, 'utf8')));
  } catch (error) {
    if (error instanceof ExperimentError) {
      throw error;
    }
    throw new ExperimentError(
      500,
      'config_read_failed',
      `Failed to read generation config: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

async function readGenerationPresets() {
  let definitions;
  let baseConfig;
  try {
    [definitions, baseConfig] = await Promise.all([
      readFile(generationPresetDefinitionsPath, 'utf8').then(JSON.parse),
      readFile(resolve(repoRoot, generationPresetBaseConfigRef), 'utf8').then(JSON.parse),
    ]);
  } catch (error) {
    throw new ExperimentError(
      500,
      'generation_presets_read_failed',
      `Failed to read generation presets: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
  return materializeGenerationPresets(
    validateGenerationPresetDefinitions(definitions),
    validateGenerationConfig(baseConfig),
  );
}

async function persistGenerationConfig(config) {
  const temporaryPath = `${generationConfigPath}.${process.pid}.tmp`;
  try {
    await writeFile(temporaryPath, `${JSON.stringify(config, null, 2)}\n`, 'utf8');
    await rename(temporaryPath, generationConfigPath);
  } catch (error) {
    await rm(temporaryPath, { force: true });
    throw new ExperimentError(
      500,
      'config_persist_failed',
      `Failed to persist generation config: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

async function runGenerationPresetRebuild(payload) {
  assertExactKeys(payload, ['candidateId', 'presetId'], 'request');
  if (typeof payload.candidateId !== 'string' || payload.candidateId.length === 0) {
    throw new ExperimentError(400, 'invalid_candidate', 'candidateId must be a non-empty string.');
  }
  if (typeof payload.presetId !== 'string' || payload.presetId.length === 0) {
    throw new ExperimentError(400, 'invalid_generation_preset_id', 'presetId must be a non-empty string.');
  }
  const presets = await readGenerationPresets();
  const preset = presets.presets.find((entry) => entry.id === payload.presetId);
  if (preset === undefined) {
    throw new ExperimentError(
      400,
      'unknown_generation_preset',
      `Unknown generation preset ${payload.presetId}.`,
    );
  }
  return {
    kind: 'rusty_procgen.viewer_generation_preset_rebuild.v1',
    schemaVersion: 1,
    presetId: preset.id,
    rebuild: await runGenerationConfigRebuild({
      candidateId: payload.candidateId,
      config: preset.config,
    }),
  };
}

async function runGenerationConfigRebuild(payload) {
  assertExactKeys(payload, ['candidateId', 'config'], 'request');
  if (typeof payload.candidateId !== 'string' || payload.candidateId.length === 0) {
    throw new ExperimentError(400, 'invalid_candidate', 'candidateId must be a non-empty string.');
  }
  const config = finalizeGenerationConfig(validateGenerationConfig(payload.config));
  const geometryLayoutPolicy = materializeGeometryLayoutPolicy(config, 'value');
  const placementPolicy = materializePlacementPolicy(config, 'value');
  const catalogAwareGenerationPolicy = materializeCatalogAwareGenerationPolicy(config, 'value');
  const corridorRealization = config.corridorRealization.value;
  const selection = JSON.parse(await readFile(selectionReportPath, 'utf8'));
  const entry = selection.accepted?.find((candidate) => candidate.candidateId === payload.candidateId);
  if (entry === undefined) {
    throw new ExperimentError(404, 'candidate_not_found', `Unknown accepted candidate ${payload.candidateId}.`);
  }
  for (const ref of [
    'artifactRef',
    'intermediateBreakdownRef',
    'physicalConnectionPlanRef',
    'geometryRef',
    'shapeCatalogRef',
    'shapeMatchRef',
  ]) {
    if (typeof entry[ref] !== 'string') {
      throw new ExperimentError(422, 'candidate_missing_build_refs', `Selected candidate has no ${ref}.`);
    }
  }

  const acceptedPath = safeExperimentSourcePath(entry.artifactRef, 'artifacts/samples');
  const intermediatePath = safeExperimentSourcePath(entry.intermediateBreakdownRef, 'artifacts/samples');
  const connectionPlanPath = safeExperimentSourcePath(entry.physicalConnectionPlanRef, 'artifacts/samples');
  const committedGeometryPath = safeExperimentSourcePath(entry.geometryRef, 'artifacts/samples');
  const committedCatalogPath = safeExperimentSourcePath(entry.shapeCatalogRef, 'fixtures');
  const committedMatchPath = safeExperimentSourcePath(entry.shapeMatchRef, 'artifacts/samples');
  const accepted = JSON.parse(await readFile(acceptedPath, 'utf8'));
  const committedGeometry = JSON.parse(await readFile(committedGeometryPath, 'utf8'));
  const committedCatalog = JSON.parse(await readFile(committedCatalogPath, 'utf8'));
  const committedMatch = JSON.parse(await readFile(committedMatchPath, 'utf8'));
  if (accepted?.candidate?.candidateId !== payload.candidateId) {
    throw new ExperimentError(422, 'candidate_artifact_mismatch', 'Accepted artifact does not contain the selected candidate.');
  }
  if (!Number.isInteger(committedGeometry.seed) || !Number.isInteger(committedMatch.seed)) {
    throw new ExperimentError(422, 'candidate_missing_seeds', 'Committed geometry or shape match has no deterministic seed.');
  }

  const buildDir = await mkdtemp(join(tmpdir(), 'rusty-procgen-generation-config-'));
  const candidatePath = join(buildDir, 'candidate.json');
  const geometryPolicyPath = join(buildDir, 'geometry-layout-policy.json');
  const catalogAwarePolicyPath = join(buildDir, 'catalog-aware-generation-policy.json');
  const catalogAwareResultPath = join(buildDir, 'catalog-aware-generation.json');
  const catalogAwareTracePath = join(buildDir, 'catalog-aware-generation.trace.json');
  const catalogPath = join(buildDir, 'shape-catalog.json');
  const geometryPath = join(buildDir, 'geometry-2d.json');
  const geometryValidationPath = join(buildDir, 'geometry-2d.validation.json');
  const piecePlanPath = join(buildDir, 'piece-plan.json');
  const shapeMatchPath = join(buildDir, 'piece-shape-match.json');
  const placementPath = join(buildDir, 'piece-placement.json');
  const placementValidationPath = join(buildDir, 'piece-placement.validation.json');
  const builtFlowValidationPath = join(buildDir, 'built-flow.validation.json');
  const configRef = 'config/viewer-generation.json';
  try {
    let catalogAwareGeneration = null;
    committedCatalog.placementPolicy = placementPolicy;
    await Promise.all([
      writeFile(candidatePath, `${JSON.stringify(accepted.candidate, null, 2)}\n`, 'utf8'),
      writeFile(geometryPolicyPath, `${JSON.stringify(geometryLayoutPolicy, null, 2)}\n`, 'utf8'),
      writeFile(catalogAwarePolicyPath, `${JSON.stringify(catalogAwareGenerationPolicy, null, 2)}\n`, 'utf8'),
      writeFile(catalogPath, `${JSON.stringify(committedCatalog, null, 2)}\n`, 'utf8'),
    ]);
    await runProcgen([
      'geometry', 'emit-2d',
      '--candidate', candidatePath,
      '--intermediate', intermediatePath,
      '--connection-plan', connectionPlanPath,
      '--layout-policy', geometryPolicyPath,
      '--seed', String(committedGeometry.seed),
      '--out', geometryPath,
    ]);
    await runProcgen([
      'build', 'emit-piece-plan',
      '--candidate', candidatePath,
      '--intermediate', intermediatePath,
      '--geometry', geometryPath,
      '--corridor-realization', corridorRealization,
      '--out', piecePlanPath,
    ]);
    if (corridorRealization === 'catalog') {
      const [catalogSourceGeometry, catalogSourcePlan] = await Promise.all([
        readFile(geometryPath, 'utf8').then(JSON.parse),
        readFile(piecePlanPath, 'utf8').then(JSON.parse),
      ]);
      catalogSourceGeometry.sourceCandidateRef = entry.artifactRef;
      catalogSourceGeometry.sourceIntermediateRef = entry.intermediateBreakdownRef;
      catalogSourceGeometry.sourceConnectionPlanRef = entry.physicalConnectionPlanRef;
      catalogSourcePlan.sourceCandidateRef = entry.artifactRef;
      catalogSourcePlan.sourceIntermediateRef = entry.intermediateBreakdownRef;
      catalogSourcePlan.sourceGeometryRef = `${configRef}:${payload.candidateId}:geometry`;
      await Promise.all([
        writeFile(geometryPath, `${JSON.stringify(catalogSourceGeometry, null, 2)}\n`, 'utf8'),
        writeFile(piecePlanPath, `${JSON.stringify(catalogSourcePlan, null, 2)}\n`, 'utf8'),
      ]);
      await runProcgen([
        'build', 'realize-catalog-aware',
        '--candidate', 'candidate.json',
        '--geometry', 'geometry-2d.json',
        '--piece-plan', 'piece-plan.json',
        '--catalog', 'shape-catalog.json',
        '--policy', 'catalog-aware-generation-policy.json',
        '--seed', String(committedMatch.seed),
        '--out', 'catalog-aware-generation.json',
        '--trace-out', 'catalog-aware-generation.trace.json',
      ], buildDir);
      const [catalogAwareResult, catalogAwareTrace] = await Promise.all([
        readFile(catalogAwareResultPath, 'utf8').then(JSON.parse),
        readFile(catalogAwareTracePath, 'utf8').then(JSON.parse),
      ]);
      if (catalogAwareResult.ok !== true) {
        const finalAttempt = catalogAwareResult.attempts?.at(-1);
        const classification = catalogAwareResult.exhaustedClassification
          ?? finalAttempt?.classification
          ?? 'generation_infeasibility';
        throw new ExperimentError(
          422,
          `catalog_aware_${classification}`,
          finalAttempt?.detail ?? 'Catalog-aware generation exhausted without a successful attempt.',
          {
            kind: 'rusty_procgen.catalog_aware_generation_exhaustion.v2',
            schemaVersion: 2,
            classification,
            attempts: catalogAwareResult.attempts ?? [],
            result: catalogAwareResult,
            trace: catalogAwareTrace,
          },
        );
      }
      catalogAwareGeneration = {
        policy: catalogAwareResult.policy,
        attempts: catalogAwareResult.attempts,
        selectedAttempt: catalogAwareResult.selectedAttempt,
        result: catalogAwareResult,
        trace: catalogAwareTrace,
      };
      await Promise.all([
        writeFile(geometryPath, `${JSON.stringify(catalogAwareResult.geometry, null, 2)}\n`, 'utf8'),
        writeFile(piecePlanPath, `${JSON.stringify(catalogAwareResult.piecePlan, null, 2)}\n`, 'utf8'),
        writeFile(shapeMatchPath, `${JSON.stringify(catalogAwareResult.shapeMatch, null, 2)}\n`, 'utf8'),
        writeFile(placementPath, `${JSON.stringify(catalogAwareResult.placement, null, 2)}\n`, 'utf8'),
      ]);
    } else {
      await runProcgen([
        'build', 'match-shapes',
        '--catalog', catalogPath,
        '--piece-plan', piecePlanPath,
        '--seed', String(committedMatch.seed),
        '--out', shapeMatchPath,
      ]);
      await runProcgen([
        'build', 'assemble',
        '--catalog', catalogPath,
        '--piece-plan', piecePlanPath,
        '--shape-match', shapeMatchPath,
        '--connectivity', 'four-way',
        '--out', placementPath,
      ]);
    }
    await runProcgen([
      'geometry', 'validate-2d',
      '--state', geometryPath,
      '--out', geometryValidationPath,
    ]);
    const placement = JSON.parse(await readFile(placementPath, 'utf8'));
    placement.sourcePlanRef = `${configRef}:${payload.candidateId}:${corridorRealization}:piece-plan`;
    placement.sourceCatalogRef = `${configRef}:placement-policy`;
    placement.sourceMatchRef = `${configRef}:${payload.candidateId}:${corridorRealization}:shape-match`;
    await writeFile(placementPath, `${JSON.stringify(placement, null, 2)}\n`, 'utf8');
    await runProcgen([
      'build', 'validate-placement',
      '--state', placementPath,
      '--out', placementValidationPath,
    ]);
    await runProcgen([
      'build', 'validate-flow',
      '--candidate', candidatePath,
      '--geometry', geometryPath,
      '--piece-plan', piecePlanPath,
      '--piece-placement', placementPath,
      '--out', builtFlowValidationPath,
    ]);

    const geometry = JSON.parse(await readFile(geometryPath, 'utf8'));
    const geometryValidation = JSON.parse(await readFile(geometryValidationPath, 'utf8'));
    const placementValidation = JSON.parse(await readFile(placementValidationPath, 'utf8'));
    const builtFlowValidation = JSON.parse(await readFile(builtFlowValidationPath, 'utf8'));
    if (
      geometryValidation.ok !== true
      || placementValidation.ok !== true
      || builtFlowValidation.ok !== true
    ) {
      throw new ExperimentError(
        422,
        'generation_validation_failed',
        'Generation config rebuild did not pass geometry, placement, and built-flow validation.',
      );
    }

    geometry.sourceCandidateRef = entry.artifactRef;
    geometry.sourceIntermediateRef = entry.intermediateBreakdownRef;
    geometry.sourceConnectionPlanRef = entry.physicalConnectionPlanRef;
    placement.sourcePlanRef = `${configRef}:${payload.candidateId}:${corridorRealization}:piece-plan`;
    placement.sourceCatalogRef = `${configRef}:placement-policy`;
    placement.sourceMatchRef = `${configRef}:${payload.candidateId}:${corridorRealization}:shape-match`;
    builtFlowValidation.candidateRef = entry.artifactRef;
    builtFlowValidation.geometryRef = `${configRef}:${payload.candidateId}:geometry`;
    builtFlowValidation.piecePlanRef = placement.sourcePlanRef;
    builtFlowValidation.piecePlacementRef = `${configRef}:${payload.candidateId}:placement`;
    const buildId = createHash('sha256')
      .update(JSON.stringify({
        candidateId: payload.candidateId,
        config,
        geometry,
        placement,
        builtFlowValidation,
      }))
      .digest('hex');
    await persistGenerationConfig(config);
    return {
      kind: 'rusty_procgen.viewer_generation_rebuild.v1',
      buildId,
      candidateId: payload.candidateId,
      config,
      geometry,
      geometryValidation,
      placement,
      placementValidation,
      builtFlowValidation,
      catalogAwareGeneration,
      metrics: placementMetrics(placement),
      persisted: true,
      nativeAuthority: false,
    };
  } catch (error) {
    if (error instanceof ExperimentError) {
      throw error;
    }
    const detail = error?.stderr?.trim() || error?.stdout?.trim()
      || (error instanceof Error ? error.message : String(error));
    const code = detail.includes('geometry search exhausted')
      ? 'geometry_search_exhausted'
      : detail.includes('pure catalog coverage rejected')
        ? 'pure_catalog_coverage_rejected'
        : detail.includes('pure catalog search exhausted')
          ? 'pure_catalog_search_exhausted'
          : 'generation_config_rebuild_failed';
    throw new ExperimentError(422, code, detail, pureCatalogExhaustionEvidence(detail));
  } finally {
    await rm(buildDir, { recursive: true, force: true });
  }
}

async function runPlacementPolicyExperiment(payload) {
  assertExactKeys(payload, ['candidateId', 'placementPolicy'], 'request');
  if (typeof payload.candidateId !== 'string' || payload.candidateId.length === 0) {
    throw new ExperimentError(400, 'invalid_candidate', 'candidateId must be a non-empty string.');
  }
  const policy = validatePlacementPolicy(payload.placementPolicy);
  const selection = JSON.parse(await readFile(selectionReportPath, 'utf8'));
  const entry = selection.accepted?.find((candidate) => candidate.candidateId === payload.candidateId);
  if (entry === undefined) {
    throw new ExperimentError(404, 'candidate_not_found', `Unknown accepted candidate ${payload.candidateId}.`);
  }
  if (
    typeof entry.piecePlanRef !== 'string'
    || typeof entry.shapeMatchRef !== 'string'
    || typeof entry.shapeCatalogRef !== 'string'
  ) {
    throw new ExperimentError(422, 'candidate_missing_build_refs', 'Selected candidate has no complete piece-build references.');
  }

  const piecePlanPath = safeExperimentSourcePath(entry.piecePlanRef, 'artifacts/samples');
  const shapeMatchPath = safeExperimentSourcePath(entry.shapeMatchRef, 'artifacts/samples');
  const catalogPath = safeExperimentSourcePath(entry.shapeCatalogRef, 'fixtures');
  const catalog = JSON.parse(await readFile(catalogPath, 'utf8'));
  catalog.placementPolicy = policy;

  const experimentDir = await mkdtemp(join(tmpdir(), 'rusty-procgen-policy-'));
  const experimentCatalogPath = join(experimentDir, 'shape-catalog.json');
  const placementPath = join(experimentDir, 'piece-placement.json');
  const validationPath = join(experimentDir, 'piece-placement.validation.json');
  try {
    await writeFile(experimentCatalogPath, `${JSON.stringify(catalog, null, 2)}\n`, 'utf8');
    await runProcgen([
      'build', 'assemble',
      '--catalog', experimentCatalogPath,
      '--piece-plan', piecePlanPath,
      '--shape-match', shapeMatchPath,
      '--connectivity', 'four-way',
      '--out', placementPath,
    ]);
    await runProcgen([
      'build', 'validate-placement',
      '--state', placementPath,
      '--out', validationPath,
    ]);
    const placement = JSON.parse(await readFile(placementPath, 'utf8'));
    const validation = JSON.parse(await readFile(validationPath, 'utf8'));
    if (validation.ok !== true) {
      const diagnosticSummary = Array.isArray(validation.diagnostics)
        ? validation.diagnostics.slice(0, 4).map((diagnostic) =>
          `${diagnostic.code ?? 'unknown'}: ${diagnostic.detail ?? 'no detail'}`).join('; ')
        : 'no structured diagnostics';
      throw new ExperimentError(
        422,
        'placement_validation_failed',
        `Experimental placement has ${validation.fatalCount ?? 'unknown'} fatal diagnostic(s): ${diagnosticSummary}`,
      );
    }
    placement.sourcePlanRef = entry.piecePlanRef;
    placement.sourceCatalogRef = `experiment:${entry.shapeCatalogRef}`;
    placement.sourceMatchRef = entry.shapeMatchRef;
    const experimentId = createHash('sha256')
      .update(JSON.stringify({ candidateId: payload.candidateId, policy, placement }))
      .digest('hex');
    return {
      kind: 'rusty_procgen.placement_policy_experiment.v1',
      experimentId,
      candidateId: payload.candidateId,
      placementPolicy: policy,
      placement,
      validation,
      persisted: false,
      nativeAuthority: false,
    };
  } catch (error) {
    if (error instanceof ExperimentError) {
      throw error;
    }
    const detail = error?.stderr?.trim() || error?.stdout?.trim() || (error instanceof Error ? error.message : String(error));
    throw new ExperimentError(422, 'placement_assembly_failed', detail);
  } finally {
    await rm(experimentDir, { recursive: true, force: true });
  }
}

async function runGeometryLayoutPolicyExperiment(payload) {
  assertExactKeys(payload, ['candidateId', 'geometryLayoutPolicy'], 'request');
  if (typeof payload.candidateId !== 'string' || payload.candidateId.length === 0) {
    throw new ExperimentError(400, 'invalid_candidate', 'candidateId must be a non-empty string.');
  }
  const policy = validateGeometryLayoutPolicy(payload.geometryLayoutPolicy);
  const selection = JSON.parse(await readFile(selectionReportPath, 'utf8'));
  const entry = selection.accepted?.find((candidate) => candidate.candidateId === payload.candidateId);
  if (entry === undefined) {
    throw new ExperimentError(404, 'candidate_not_found', `Unknown accepted candidate ${payload.candidateId}.`);
  }
  for (const ref of [
    'artifactRef',
    'intermediateBreakdownRef',
    'physicalConnectionPlanRef',
    'geometryRef',
    'shapeCatalogRef',
    'shapeMatchRef',
  ]) {
    if (typeof entry[ref] !== 'string') {
      throw new ExperimentError(422, 'candidate_missing_geometry_refs', `Selected candidate has no ${ref}.`);
    }
  }

  const acceptedPath = safeExperimentSourcePath(entry.artifactRef, 'artifacts/samples');
  const intermediatePath = safeExperimentSourcePath(entry.intermediateBreakdownRef, 'artifacts/samples');
  const connectionPlanPath = safeExperimentSourcePath(entry.physicalConnectionPlanRef, 'artifacts/samples');
  const committedGeometryPath = safeExperimentSourcePath(entry.geometryRef, 'artifacts/samples');
  const catalogPath = safeExperimentSourcePath(entry.shapeCatalogRef, 'fixtures');
  const committedMatchPath = safeExperimentSourcePath(entry.shapeMatchRef, 'artifacts/samples');
  const accepted = JSON.parse(await readFile(acceptedPath, 'utf8'));
  const committedGeometry = JSON.parse(await readFile(committedGeometryPath, 'utf8'));
  const committedMatch = JSON.parse(await readFile(committedMatchPath, 'utf8'));
  if (accepted?.candidate?.candidateId !== payload.candidateId) {
    throw new ExperimentError(422, 'candidate_artifact_mismatch', 'Accepted artifact does not contain the selected candidate.');
  }
  if (!Number.isInteger(committedGeometry.seed) || !Number.isInteger(committedMatch.seed)) {
    throw new ExperimentError(422, 'candidate_missing_seeds', 'Committed geometry or shape match has no deterministic seed.');
  }

  const experimentDir = await mkdtemp(join(tmpdir(), 'rusty-procgen-geometry-policy-'));
  const candidatePath = join(experimentDir, 'candidate.json');
  const policyPath = join(experimentDir, 'geometry-layout-policy.json');
  const geometryPath = join(experimentDir, 'geometry-2d.json');
  const geometryValidationPath = join(experimentDir, 'geometry-2d.validation.json');
  const piecePlanPath = join(experimentDir, 'piece-plan.json');
  const shapeMatchPath = join(experimentDir, 'piece-shape-match.json');
  const placementPath = join(experimentDir, 'piece-placement.json');
  const placementValidationPath = join(experimentDir, 'piece-placement.validation.json');
  const builtFlowValidationPath = join(experimentDir, 'built-flow.validation.json');
  try {
    await writeFile(candidatePath, `${JSON.stringify(accepted.candidate, null, 2)}\n`, 'utf8');
    await writeFile(policyPath, `${JSON.stringify(policy, null, 2)}\n`, 'utf8');
    await runProcgen([
      'geometry', 'emit-2d',
      '--candidate', candidatePath,
      '--intermediate', intermediatePath,
      '--connection-plan', connectionPlanPath,
      '--layout-policy', policyPath,
      '--seed', String(committedGeometry.seed),
      '--out', geometryPath,
    ]);
    await runProcgen([
      'geometry', 'validate-2d',
      '--state', geometryPath,
      '--out', geometryValidationPath,
    ]);
    await runProcgen([
      'build', 'emit-piece-plan',
      '--candidate', candidatePath,
      '--intermediate', intermediatePath,
      '--geometry', geometryPath,
      '--out', piecePlanPath,
    ]);
    await runProcgen([
      'build', 'match-shapes',
      '--catalog', catalogPath,
      '--piece-plan', piecePlanPath,
      '--seed', String(committedMatch.seed),
      '--out', shapeMatchPath,
    ]);
    await runProcgen([
      'build', 'assemble',
      '--catalog', catalogPath,
      '--piece-plan', piecePlanPath,
      '--shape-match', shapeMatchPath,
      '--connectivity', 'four-way',
      '--out', placementPath,
    ]);
    await runProcgen([
      'build', 'validate-placement',
      '--state', placementPath,
      '--out', placementValidationPath,
    ]);
    await runProcgen([
      'build', 'validate-flow',
      '--candidate', candidatePath,
      '--geometry', geometryPath,
      '--piece-plan', piecePlanPath,
      '--piece-placement', placementPath,
      '--out', builtFlowValidationPath,
    ]);
    const geometry = JSON.parse(await readFile(geometryPath, 'utf8'));
    const geometryValidation = JSON.parse(await readFile(geometryValidationPath, 'utf8'));
    const placement = JSON.parse(await readFile(placementPath, 'utf8'));
    const placementValidation = JSON.parse(await readFile(placementValidationPath, 'utf8'));
    const builtFlowValidation = JSON.parse(await readFile(builtFlowValidationPath, 'utf8'));
    geometry.sourceCandidateRef = entry.artifactRef;
    geometry.sourceIntermediateRef = entry.intermediateBreakdownRef;
    geometry.sourceConnectionPlanRef = entry.physicalConnectionPlanRef;
    placement.sourcePlanRef = `experiment:${entry.candidateId}`;
    placement.sourceCatalogRef = entry.shapeCatalogRef;
    placement.sourceMatchRef = `experiment:${entry.candidateId}`;
    builtFlowValidation.candidateRef = entry.artifactRef;
    builtFlowValidation.geometryRef = `experiment:${entry.candidateId}:geometry`;
    builtFlowValidation.piecePlanRef = `experiment:${entry.candidateId}:piece-plan`;
    builtFlowValidation.piecePlacementRef = `experiment:${entry.candidateId}:piece-placement`;
    const experimentId = createHash('sha256')
      .update(JSON.stringify({ candidateId: payload.candidateId, policy, geometry, placement }))
      .digest('hex');
    return {
      kind: 'rusty_procgen.geometry_layout_policy_experiment.v1',
      experimentId,
      candidateId: payload.candidateId,
      geometryLayoutPolicy: policy,
      geometry,
      geometryValidation,
      placement,
      placementValidation,
      builtFlowValidation,
      persisted: false,
      nativeAuthority: false,
    };
  } catch (error) {
    if (error instanceof ExperimentError) {
      throw error;
    }
    const detail = error?.stderr?.trim() || error?.stdout?.trim() || (error instanceof Error ? error.message : String(error));
    const code = detail.includes('geometry search exhausted')
      ? 'geometry_search_exhausted'
      : 'geometry_generation_failed';
    throw new ExperimentError(422, code, detail, pureCatalogExhaustionEvidence(detail));
  } finally {
    await rm(experimentDir, { recursive: true, force: true });
  }
}

async function runCorridorRealizationExperiment(payload) {
  assertExactKeys(payload, ['candidateId', 'corridorRealization'], 'request');
  if (typeof payload.candidateId !== 'string' || payload.candidateId.length === 0) {
    throw new ExperimentError(400, 'invalid_candidate', 'candidateId must be a non-empty string.');
  }
  if (!['catalog', 'hybrid', 'procedural'].includes(payload.corridorRealization)) {
    throw new ExperimentError(
      400,
      'invalid_corridor_realization',
      'corridorRealization must be catalog, hybrid, or procedural.',
    );
  }
  const selection = JSON.parse(await readFile(selectionReportPath, 'utf8'));
  const entry = selection.accepted?.find((candidate) => candidate.candidateId === payload.candidateId);
  if (entry === undefined) {
    throw new ExperimentError(404, 'candidate_not_found', `Unknown accepted candidate ${payload.candidateId}.`);
  }
  for (const ref of [
    'artifactRef',
    'intermediateBreakdownRef',
    'geometryRef',
    'shapeCatalogRef',
    'shapeMatchRef',
  ]) {
    if (typeof entry[ref] !== 'string') {
      throw new ExperimentError(422, 'candidate_missing_build_refs', `Selected candidate has no ${ref}.`);
    }
  }
  const acceptedPath = safeExperimentSourcePath(entry.artifactRef, 'artifacts/samples');
  const intermediatePath = safeExperimentSourcePath(entry.intermediateBreakdownRef, 'artifacts/samples');
  const geometryPath = safeExperimentSourcePath(entry.geometryRef, 'artifacts/samples');
  const catalogPath = safeExperimentSourcePath(entry.shapeCatalogRef, 'fixtures');
  const committedMatchPath = safeExperimentSourcePath(entry.shapeMatchRef, 'artifacts/samples');
  const accepted = JSON.parse(await readFile(acceptedPath, 'utf8'));
  const committedMatch = JSON.parse(await readFile(committedMatchPath, 'utf8'));
  if (accepted?.candidate?.candidateId !== payload.candidateId) {
    throw new ExperimentError(422, 'candidate_artifact_mismatch', 'Accepted artifact does not contain the selected candidate.');
  }
  if (!Number.isInteger(committedMatch.seed)) {
    throw new ExperimentError(422, 'candidate_missing_seed', 'Committed shape match has no deterministic seed.');
  }

  const experimentDir = await mkdtemp(join(tmpdir(), 'rusty-procgen-corridor-realization-'));
  const candidatePath = join(experimentDir, 'candidate.json');
  const piecePlanPath = join(experimentDir, 'piece-plan.json');
  const shapeMatchPath = join(experimentDir, 'piece-shape-match.json');
  const placementPath = join(experimentDir, 'piece-placement.json');
  const placementValidationPath = join(experimentDir, 'piece-placement.validation.json');
  const builtFlowValidationPath = join(experimentDir, 'built-flow.validation.json');
  try {
    await writeFile(candidatePath, `${JSON.stringify(accepted.candidate, null, 2)}\n`, 'utf8');
    await runProcgen([
      'build', 'emit-piece-plan',
      '--candidate', candidatePath,
      '--intermediate', intermediatePath,
      '--geometry', geometryPath,
      '--corridor-realization', payload.corridorRealization,
      '--out', piecePlanPath,
    ]);
    await runProcgen([
      'build', 'match-shapes',
      '--catalog', catalogPath,
      '--piece-plan', piecePlanPath,
      '--seed', String(committedMatch.seed),
      '--out', shapeMatchPath,
    ]);
    await runProcgen([
      'build', 'assemble',
      '--catalog', catalogPath,
      '--piece-plan', piecePlanPath,
      '--shape-match', shapeMatchPath,
      '--connectivity', 'four-way',
      '--out', placementPath,
    ]);
    const placement = JSON.parse(await readFile(placementPath, 'utf8'));
    placement.sourcePlanRef = `experiment:${entry.candidateId}:${payload.corridorRealization}`;
    placement.sourceCatalogRef = entry.shapeCatalogRef;
    placement.sourceMatchRef = `experiment:${entry.candidateId}:${payload.corridorRealization}`;
    await writeFile(placementPath, `${JSON.stringify(placement, null, 2)}\n`, 'utf8');
    await runProcgen([
      'build', 'validate-placement',
      '--state', placementPath,
      '--out', placementValidationPath,
    ]);
    await runProcgen([
      'build', 'validate-flow',
      '--candidate', candidatePath,
      '--geometry', geometryPath,
      '--piece-plan', piecePlanPath,
      '--piece-placement', placementPath,
      '--out', builtFlowValidationPath,
    ]);
    const placementValidation = JSON.parse(await readFile(placementValidationPath, 'utf8'));
    const builtFlowValidation = JSON.parse(await readFile(builtFlowValidationPath, 'utf8'));
    builtFlowValidation.candidateRef = entry.artifactRef;
    builtFlowValidation.geometryRef = entry.geometryRef;
    builtFlowValidation.piecePlanRef = `experiment:${entry.candidateId}:${payload.corridorRealization}:piece-plan`;
    builtFlowValidation.piecePlacementRef = `experiment:${entry.candidateId}:${payload.corridorRealization}:placement`;
    const corridorInstances = placement.instances.filter((instance) =>
      ['connector', 'corridor', 'bend', 'junction'].includes(instance.requirementKind));
    const footprintCells = [...placement.occupiedCells, ...placement.connectionCells];
    const xs = footprintCells.map((cell) => cell.x);
    const ys = footprintCells.map((cell) => cell.y);
    const metrics = {
      prefabInstances: placement.instances.length,
      corridorPrefabInstances: corridorInstances.length,
      corridorPrefabCells: corridorInstances.reduce(
        (total, instance) => total + instance.occupiedCells.length,
        0,
      ),
      routedCorridorCells: placement.connectionCells.length,
      footprintWidth: xs.length === 0 ? 0 : Math.max(...xs) - Math.min(...xs) + 1,
      footprintHeight: ys.length === 0 ? 0 : Math.max(...ys) - Math.min(...ys) + 1,
    };
    const experimentId = createHash('sha256')
      .update(JSON.stringify({
        candidateId: payload.candidateId,
        corridorRealization: payload.corridorRealization,
        placement,
        builtFlowValidation,
      }))
      .digest('hex');
    return {
      kind: 'rusty_procgen.corridor_realization_experiment.v1',
      experimentId,
      candidateId: payload.candidateId,
      corridorRealization: payload.corridorRealization,
      placement,
      placementValidation,
      builtFlowValidation,
      metrics,
      persisted: false,
      nativeAuthority: false,
    };
  } catch (error) {
    const detail = error?.stderr?.trim() || error?.stdout?.trim() || (error instanceof Error ? error.message : String(error));
    const code = detail.includes('pure catalog coverage rejected')
      ? 'pure_catalog_coverage_rejected'
      : detail.includes('pure catalog search exhausted')
        ? 'pure_catalog_search_exhausted'
        : 'corridor_realization_failed';
    throw new ExperimentError(422, code, detail, pureCatalogExhaustionEvidence(detail));
  } finally {
    await rm(experimentDir, { recursive: true, force: true });
  }
}

function validatePlacementPolicy(value) {
  assertExactKeys(
    value,
    ['schemaVersion', 'minimumClearanceCells', 'contactPolicy', 'wallThicknessCells', 'doorwayWidthCells', 'preservePieceBoundaries'],
    'placementPolicy',
  );
  if (value.schemaVersion !== 1) {
    throw new ExperimentError(400, 'unsupported_policy_schema', 'Only placement-policy schemaVersion 1 is supported.');
  }
  if (value.contactPolicy !== 'glued_exits_only') {
    throw new ExperimentError(400, 'unsupported_contact_policy', 'contactPolicy must be glued_exits_only.');
  }
  if (value.doorwayWidthCells !== 1) {
    throw new ExperimentError(400, 'unsupported_doorway_width', 'doorwayWidthCells must remain 1 in schema v1.');
  }
  if (value.preservePieceBoundaries !== true) {
    throw new ExperimentError(400, 'unsupported_boundary_policy', 'preservePieceBoundaries must remain true in schema v1.');
  }
  assertBoundedInteger(value.wallThicknessCells, 1, 8, 'wallThicknessCells');
  assertBoundedInteger(value.minimumClearanceCells, 3, 64, 'minimumClearanceCells');
  const requiredClearance = value.wallThicknessCells * 2 + 1;
  if (value.minimumClearanceCells < requiredClearance) {
    throw new ExperimentError(
      400,
      'clearance_too_small_for_walls',
      `minimumClearanceCells must be at least ${requiredClearance} for wallThicknessCells=${value.wallThicknessCells}.`,
    );
  }
  return {
    schemaVersion: 1,
    minimumClearanceCells: value.minimumClearanceCells,
    contactPolicy: 'glued_exits_only',
    wallThicknessCells: value.wallThicknessCells,
    doorwayWidthCells: 1,
    preservePieceBoundaries: true,
  };
}

function validateGenerationConfig(value) {
  if (
    value?.kind === 'rusty_procgen.viewer_generation_config.v1'
    && value.schemaVersion === 1
  ) {
    return validateGenerationConfig(migrateLegacyGenerationConfig(value));
  }
  assertExactKeys(
    value,
    [
      'kind',
      'schemaVersion',
      'migration',
      'geometryLayoutPolicy',
      'placementPolicy',
      'catalogAwareGenerationPolicy',
      'corridorRealization',
    ],
    'generationConfig',
  );
  if (value.kind !== 'rusty_procgen.viewer_generation_config.v2' || value.schemaVersion !== 2) {
    throw new ExperimentError(
      400,
      'unsupported_generation_config_schema',
      'Only viewer-generation-config schemaVersion 2 or explicit schemaVersion 1 migration is supported.',
    );
  }
  validateGenerationConfigMigration(value.migration);
  assertExactKeys(
    value.geometryLayoutPolicy,
    [
      'initialRoomMargin',
      'initialColumnGap',
      'initialRowGap',
      'roomMarginGrowth',
      'columnGapGrowth',
      'rowGapGrowth',
      'maxSpacingTiers',
      'roomOrderAttemptsPerTier',
      'maxSearchAttempts',
    ],
    'generationConfig.geometryLayoutPolicy',
  );
  assertExactKeys(
    value.placementPolicy,
    ['minimumClearanceCells', 'wallThicknessCells'],
    'generationConfig.placementPolicy',
  );
  assertExactKeys(
    value.catalogAwareGenerationPolicy,
    [
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
    ],
    'generationConfig.catalogAwareGenerationPolicy',
  );
  for (const [label, setting] of Object.entries({
    ...value.geometryLayoutPolicy,
    ...value.placementPolicy,
    ...value.catalogAwareGenerationPolicy,
    corridorRealization: value.corridorRealization,
  })) {
    assertExactKeys(setting, ['value', 'defaultValue'], `generationConfig.${label}`);
  }
  for (const field of ['value', 'defaultValue']) {
    validateGeometryLayoutPolicy(materializeGeometryLayoutPolicy(value, field));
    validatePlacementPolicy(materializePlacementPolicy(value, field));
    validateCatalogAwareGenerationPolicy(materializeCatalogAwareGenerationPolicy(value, field));
    const corridorRealization = value.corridorRealization[field];
    if (!['catalog', 'hybrid', 'procedural'].includes(corridorRealization)) {
      throw new ExperimentError(
        400,
        `invalid_corridorRealization_${field}`,
        `corridorRealization.${field} must be catalog, hybrid, or procedural.`,
      );
    }
  }
  return JSON.parse(JSON.stringify(value));
}

function validateGenerationPresetDefinitions(value) {
  assertExactKeys(
    value,
    ['kind', 'schemaVersion', 'sourceBaseConfigRef', 'presets'],
    'generationPresetDefinitions',
  );
  if (
    value.kind !== 'rusty_procgen.viewer_generation_preset_definitions.v1'
    || value.schemaVersion !== 1
    || value.sourceBaseConfigRef !== generationPresetBaseConfigRef
    || !Array.isArray(value.presets)
    || value.presets.length !== 3
  ) {
    throw new ExperimentError(
      400,
      'invalid_generation_preset_definitions',
      'Generation preset definitions must use schema 1, the canonical base config, and exactly three presets.',
    );
  }
  const expectedIds = ['tight', 'normal', 'spread'];
  if (
    JSON.stringify(value.presets.map((preset) => preset?.id))
      !== JSON.stringify(expectedIds)
  ) {
    throw new ExperimentError(
      400,
      'invalid_generation_preset_ids',
      `Generation preset ids must be exactly: ${expectedIds.join(', ')}.`,
    );
  }
  for (const preset of value.presets) {
    assertExactKeys(preset, ['id', 'label', 'summary', 'values'], 'generationPreset');
    if (
      typeof preset.label !== 'string'
      || preset.label.length === 0
      || typeof preset.summary !== 'string'
      || preset.summary.length === 0
    ) {
      throw new ExperimentError(
        400,
        'invalid_generation_preset_copy',
        `Generation preset ${preset.id} must have a non-empty label and summary.`,
      );
    }
    validateGenerationPresetValues(preset.values, preset.id);
  }
  return JSON.parse(JSON.stringify(value));
}

function validateGenerationPresetValues(value, presetId) {
  assertExactKeys(
    value,
    [
      'geometryLayoutPolicy',
      'placementPolicy',
      'catalogAwareGenerationPolicy',
      'corridorRealization',
    ],
    `generationPreset_${presetId}_values`,
  );
  assertExactKeys(
    value.geometryLayoutPolicy,
    [
      'initialRoomMargin',
      'initialColumnGap',
      'initialRowGap',
      'roomMarginGrowth',
      'columnGapGrowth',
      'rowGapGrowth',
      'maxSpacingTiers',
      'roomOrderAttemptsPerTier',
      'maxSearchAttempts',
    ],
    `generationPreset_${presetId}_geometry`,
  );
  assertExactKeys(
    value.placementPolicy,
    ['minimumClearanceCells', 'wallThicknessCells'],
    `generationPreset_${presetId}_placement`,
  );
  assertExactKeys(
    value.catalogAwareGenerationPolicy,
    [
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
    ],
    `generationPreset_${presetId}_catalogAware`,
  );
}

function materializeGenerationPresets(definitions, baseConfig) {
  return {
    kind: 'rusty_procgen.viewer_generation_presets.v1',
    schemaVersion: 1,
    sourceBaseConfigRef: definitions.sourceBaseConfigRef,
    presets: definitions.presets.map((definition) => {
      const config = structuredClone(baseConfig);
      config.migration = null;
      for (const [key, setting] of Object.entries(config.geometryLayoutPolicy)) {
        setting.value = definition.values.geometryLayoutPolicy[key];
      }
      for (const [key, setting] of Object.entries(config.placementPolicy)) {
        setting.value = definition.values.placementPolicy[key];
      }
      for (const [key, setting] of Object.entries(config.catalogAwareGenerationPolicy)) {
        setting.value = definition.values.catalogAwareGenerationPolicy[key];
      }
      config.corridorRealization.value = definition.values.corridorRealization;
      return {
        id: definition.id,
        label: definition.label,
        summary: definition.summary,
        config: validateGenerationConfig(config),
      };
    }),
  };
}

function migrateLegacyGenerationConfig(value) {
  assertExactKeys(
    value,
    [
      'kind',
      'schemaVersion',
      'geometryLayoutPolicy',
      'placementPolicy',
      'catalogAwareGenerationPolicy',
      'corridorRealization',
    ],
    'generationConfig',
  );
  assertExactKeys(
    value.catalogAwareGenerationPolicy,
    [
      'maxGenerationAttempts',
      'initialRoomSlackCells',
      'roomSlackGrowthCells',
      'maxRoomCandidates',
      'maxRoutingStatesPerSection',
      'routeMarginCells',
      'guideDistanceWeight',
      'turnPenalty',
    ],
    'generationConfig.catalogAwareGenerationPolicy',
  );
  return {
    ...JSON.parse(JSON.stringify(value)),
    kind: 'rusty_procgen.viewer_generation_config.v2',
    schemaVersion: 2,
    migration: {
      sourceKind: 'rusty_procgen.viewer_generation_config.v1',
      sourceSchemaVersion: 1,
      appliedDefaults: [
        'initialRoomCompactionCells=0',
        'roomCompactionGrowthCells=1',
        'maxPlacementWidthCells=4096',
        'maxPlacementHeightCells=4096',
        'maxPlacementAreaCells=16777216',
        'maxRoutedCatalogCells=1048576',
        'primaryMetric=placement_span',
        'preferredMaximum=286',
      ],
    },
    catalogAwareGenerationPolicy: {
      maxGenerationAttempts: JSON.parse(
        JSON.stringify(value.catalogAwareGenerationPolicy.maxGenerationAttempts),
      ),
      initialRoomCompactionCells: { value: 0, defaultValue: 0 },
      roomCompactionGrowthCells: { value: 1, defaultValue: 1 },
      maxRoomCandidates: JSON.parse(
        JSON.stringify(value.catalogAwareGenerationPolicy.maxRoomCandidates),
      ),
      maxRoutingStatesPerSection: JSON.parse(
        JSON.stringify(value.catalogAwareGenerationPolicy.maxRoutingStatesPerSection),
      ),
      routeMarginCells: JSON.parse(
        JSON.stringify(value.catalogAwareGenerationPolicy.routeMarginCells),
      ),
      guideDistanceWeight: JSON.parse(
        JSON.stringify(value.catalogAwareGenerationPolicy.guideDistanceWeight),
      ),
      turnPenalty: JSON.parse(
        JSON.stringify(value.catalogAwareGenerationPolicy.turnPenalty),
      ),
      maxPlacementWidthCells: { value: 4_096, defaultValue: 4_096 },
      maxPlacementHeightCells: { value: 4_096, defaultValue: 4_096 },
      maxPlacementAreaCells: { value: 16_777_216, defaultValue: 16_777_216 },
      maxRoutedCatalogCells: { value: 1_048_576, defaultValue: 1_048_576 },
      primaryMetric: { value: 'placement_span', defaultValue: 'placement_span' },
      preferredMaximum: { value: 286, defaultValue: 286 },
    },
  };
}

function validateGenerationConfigMigration(value) {
  if (value === null) {
    return;
  }
  assertExactKeys(
    value,
    ['sourceKind', 'sourceSchemaVersion', 'appliedDefaults'],
    'generationConfig.migration',
  );
  if (
    value.sourceKind !== 'rusty_procgen.viewer_generation_config.v1'
    || value.sourceSchemaVersion !== 1
    || JSON.stringify(value.appliedDefaults) !== JSON.stringify([
      'initialRoomCompactionCells=0',
      'roomCompactionGrowthCells=1',
      'maxPlacementWidthCells=4096',
      'maxPlacementHeightCells=4096',
      'maxPlacementAreaCells=16777216',
      'maxRoutedCatalogCells=1048576',
      'primaryMetric=placement_span',
      'preferredMaximum=286',
    ])
  ) {
    throw new ExperimentError(
      400,
      'invalid_generation_config_migration',
      'Generation config migration marker does not match the supported schema-1 defaults.',
    );
  }
}

function finalizeGenerationConfig(config) {
  return {
    ...JSON.parse(JSON.stringify(config)),
    migration: null,
  };
}

function materializeGeometryLayoutPolicy(config, field) {
  return {
    kind: 'rusty_procgen.geometry_layout_policy.v1',
    schemaVersion: 1,
    initialRoomMargin: config.geometryLayoutPolicy.initialRoomMargin[field],
    initialColumnGap: config.geometryLayoutPolicy.initialColumnGap[field],
    initialRowGap: config.geometryLayoutPolicy.initialRowGap[field],
    roomMarginGrowth: config.geometryLayoutPolicy.roomMarginGrowth[field],
    columnGapGrowth: config.geometryLayoutPolicy.columnGapGrowth[field],
    rowGapGrowth: config.geometryLayoutPolicy.rowGapGrowth[field],
    maxSpacingTiers: config.geometryLayoutPolicy.maxSpacingTiers[field],
    roomOrderAttemptsPerTier: config.geometryLayoutPolicy.roomOrderAttemptsPerTier[field],
    maxSearchAttempts: config.geometryLayoutPolicy.maxSearchAttempts[field],
  };
}

function materializePlacementPolicy(config, field) {
  return {
    schemaVersion: 1,
    minimumClearanceCells: config.placementPolicy.minimumClearanceCells[field],
    contactPolicy: 'glued_exits_only',
    wallThicknessCells: config.placementPolicy.wallThicknessCells[field],
    doorwayWidthCells: 1,
    preservePieceBoundaries: true,
  };
}

function materializeCatalogAwareGenerationPolicy(config, field) {
  return {
    kind: 'rusty_procgen.catalog_aware_generation_policy.v2',
    schemaVersion: 2,
    maxGenerationAttempts: config.catalogAwareGenerationPolicy.maxGenerationAttempts[field],
    initialRoomCompactionCells:
      config.catalogAwareGenerationPolicy.initialRoomCompactionCells[field],
    roomCompactionGrowthCells:
      config.catalogAwareGenerationPolicy.roomCompactionGrowthCells[field],
    maxRoomCandidates: config.catalogAwareGenerationPolicy.maxRoomCandidates[field],
    maxRoutingStatesPerSection:
      config.catalogAwareGenerationPolicy.maxRoutingStatesPerSection[field],
    routeMarginCells: config.catalogAwareGenerationPolicy.routeMarginCells[field],
    guideDistanceWeight: config.catalogAwareGenerationPolicy.guideDistanceWeight[field],
    turnPenalty: config.catalogAwareGenerationPolicy.turnPenalty[field],
    outcomeConstraints: {
      maxPlacementWidthCells:
        config.catalogAwareGenerationPolicy.maxPlacementWidthCells[field],
      maxPlacementHeightCells:
        config.catalogAwareGenerationPolicy.maxPlacementHeightCells[field],
      maxPlacementAreaCells:
        config.catalogAwareGenerationPolicy.maxPlacementAreaCells[field],
      maxRoutedCatalogCells:
        config.catalogAwareGenerationPolicy.maxRoutedCatalogCells[field],
    },
    outcomePreferences: {
      primaryMetric: config.catalogAwareGenerationPolicy.primaryMetric[field],
      preferredMaximum: config.catalogAwareGenerationPolicy.preferredMaximum[field],
    },
  };
}

function validateCatalogAwareGenerationPolicy(policy) {
  assertBoundedInteger(policy.maxGenerationAttempts, 1, 16, 'maxGenerationAttempts');
  assertBoundedInteger(
    policy.initialRoomCompactionCells,
    0,
    128,
    'initialRoomCompactionCells',
  );
  assertBoundedInteger(
    policy.roomCompactionGrowthCells,
    0,
    128,
    'roomCompactionGrowthCells',
  );
  const maximumCompaction = policy.initialRoomCompactionCells
    + policy.roomCompactionGrowthCells * (policy.maxGenerationAttempts - 1);
  if (maximumCompaction > 128) {
    throw new ExperimentError(
      400,
      'catalog_aware_compaction_too_large',
      'Catalog-aware room compaction must remain at most 128 cells across all attempts.',
    );
  }
  assertBoundedInteger(policy.maxRoomCandidates, 1, 64, 'maxRoomCandidates');
  assertBoundedInteger(
    policy.maxRoutingStatesPerSection,
    100,
    1_000_000,
    'maxRoutingStatesPerSection',
  );
  assertBoundedInteger(policy.routeMarginCells, 8, 256, 'routeMarginCells');
  assertBoundedInteger(policy.guideDistanceWeight, 0, 1_000, 'guideDistanceWeight');
  assertBoundedInteger(policy.turnPenalty, 0, 1_000, 'turnPenalty');
  assertBoundedInteger(
    policy.outcomeConstraints.maxPlacementWidthCells,
    1,
    4_294_967_296,
    'maxPlacementWidthCells',
  );
  assertBoundedInteger(
    policy.outcomeConstraints.maxPlacementHeightCells,
    1,
    4_294_967_296,
    'maxPlacementHeightCells',
  );
  assertBoundedInteger(
    policy.outcomeConstraints.maxPlacementAreaCells,
    1,
    Number.MAX_SAFE_INTEGER,
    'maxPlacementAreaCells',
  );
  assertBoundedInteger(
    policy.outcomeConstraints.maxRoutedCatalogCells,
    1,
    1_048_576,
    'maxRoutedCatalogCells',
  );
  if (!['placement_span', 'placement_area', 'routed_catalog_cells'].includes(
    policy.outcomePreferences.primaryMetric,
  )) {
    throw new ExperimentError(
      400,
      'invalid_primaryMetric',
      'primaryMetric must be placement_span, placement_area, or routed_catalog_cells.',
    );
  }
  assertBoundedInteger(
    policy.outcomePreferences.preferredMaximum,
    1,
    Number.MAX_SAFE_INTEGER,
    'preferredMaximum',
  );
}

function placementMetrics(placement) {
  const corridorInstances = placement.instances.filter((instance) =>
    ['connector', 'corridor', 'bend', 'junction'].includes(instance.requirementKind));
  const footprintCells = [...placement.occupiedCells, ...placement.connectionCells];
  const xs = footprintCells.map((cell) => cell.x);
  const ys = footprintCells.map((cell) => cell.y);
  return {
    prefabInstances: placement.instances.length,
    corridorPrefabInstances: corridorInstances.length,
    corridorPrefabCells: corridorInstances.reduce(
      (total, instance) => total + instance.occupiedCells.length,
      0,
    ),
    routedCorridorCells: placement.connectionCells.length,
    footprintWidth: xs.length === 0 ? 0 : Math.max(...xs) - Math.min(...xs) + 1,
    footprintHeight: ys.length === 0 ? 0 : Math.max(...ys) - Math.min(...ys) + 1,
  };
}

function validateGeometryLayoutPolicy(value) {
  assertExactKeys(
    value,
    [
      'kind',
      'schemaVersion',
      'initialRoomMargin',
      'initialColumnGap',
      'initialRowGap',
      'roomMarginGrowth',
      'columnGapGrowth',
      'rowGapGrowth',
      'maxSpacingTiers',
      'roomOrderAttemptsPerTier',
      'maxSearchAttempts',
    ],
    'geometryLayoutPolicy',
  );
  if (value.kind !== 'rusty_procgen.geometry_layout_policy.v1' || value.schemaVersion !== 1) {
    throw new ExperimentError(400, 'unsupported_geometry_policy_schema', 'Only geometry-layout-policy schemaVersion 1 is supported.');
  }
  for (const [label, minimum, maximum] of [
    ['initialRoomMargin', 32, 1_024],
    ['initialColumnGap', 32, 1_024],
    ['initialRowGap', 32, 1_024],
    ['roomMarginGrowth', 0, 512],
    ['columnGapGrowth', 0, 512],
    ['rowGapGrowth', 0, 512],
  ]) {
    assertBoundedInteger(value[label], minimum, maximum, label);
    if (value[label] % 8 !== 0) {
      throw new ExperimentError(400, `invalid_${label}`, `${label} must align to the 8-unit route grid.`);
    }
  }
  assertBoundedInteger(value.maxSpacingTiers, 1, 8, 'maxSpacingTiers');
  assertBoundedInteger(value.roomOrderAttemptsPerTier, 1, 32, 'roomOrderAttemptsPerTier');
  const availableAttempts = value.maxSpacingTiers * value.roomOrderAttemptsPerTier * 4;
  assertBoundedInteger(value.maxSearchAttempts, 1, availableAttempts, 'maxSearchAttempts');
  const finalTier = value.maxSpacingTiers - 1;
  for (const [initial, growth, label] of [
    ['initialRoomMargin', 'roomMarginGrowth', 'roomMargin'],
    ['initialColumnGap', 'columnGapGrowth', 'columnGap'],
    ['initialRowGap', 'rowGapGrowth', 'rowGap'],
  ]) {
    if (value[initial] + value[growth] * finalTier > 2_048) {
      throw new ExperimentError(400, `invalid_${label}`, `${label} exceeds 2048 units at the final tier.`);
    }
  }
  return { ...value };
}

function assertExactKeys(value, expected, label) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new ExperimentError(400, `invalid_${label}`, `${label} must be an object.`);
  }
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    throw new ExperimentError(400, `invalid_${label}_fields`, `${label} must contain exactly: ${wanted.join(', ')}.`);
  }
}

function assertBoundedInteger(value, min, max, label) {
  if (!Number.isInteger(value) || value < min || value > max) {
    throw new ExperimentError(400, `invalid_${label}`, `${label} must be an integer from ${min} through ${max}.`);
  }
}

function safeExperimentSourcePath(relativePath, allowedRelativeRoot) {
  const filePath = resolve(repoRoot, relativePath);
  const allowedRoot = resolve(repoRoot, allowedRelativeRoot);
  if (!isInside(filePath, allowedRoot)) {
    throw new ExperimentError(422, 'unsafe_artifact_reference', `Candidate contains an out-of-scope ${allowedRelativeRoot} reference.`);
  }
  return filePath;
}

async function runProcgen(args, cwd = repoRoot) {
  await execFileAsync('cargo', [
    'run', '--quiet', '--release',
    '--manifest-path', join(repoRoot, 'procgen-rs/Cargo.toml'),
    '--bin', 'rusty-procgen',
    '--',
    ...args,
  ], {
    cwd,
    encoding: 'utf8',
    maxBuffer: 1024 * 1024,
    timeout: PROCGEN_COMMAND_TIMEOUT_MS,
  });
}

function contentType(filePath) {
  switch (extname(filePath)) {
    case '.css':
      return 'text/css; charset=utf-8';
    case '.html':
      return 'text/html; charset=utf-8';
    case '.js':
      return 'text/javascript; charset=utf-8';
    case '.json':
      return 'application/json; charset=utf-8';
    default:
      return 'application/octet-stream';
  }
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--host') {
      parsed.host = argv[index + 1];
      index += 1;
    } else if (arg === '--port') {
      parsed.port = argv[index + 1];
      index += 1;
    }
  }
  return parsed;
}
