import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';

const host = '127.0.0.1';
const port = Number(process.env.CATALOG_AWARE_COVERAGE_PORT ?? 5218);
const baseUrl = `http://${host}:${port}`;
const outputPath = resolve(
  process.env.CATALOG_AWARE_COVERAGE_OUT
    ?? 'artifacts/evidence/catalog-aware-generation-coverage.json',
);
const tempDir = await mkdtemp(join(tmpdir(), 'rusty-procgen-catalog-aware-coverage-'));
const configPath = join(tempDir, 'viewer-generation.json');
await writeFile(configPath, await readFile('config/viewer-generation.json', 'utf8'), 'utf8');

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
  const [selection, config] = await Promise.all([
    fetchJson('/api/batches/v2'),
    fetchJson('/api/generation-config'),
  ]);
  config.corridorRealization.value = 'catalog';
  const outcomes = [];
  for (const entry of selection.accepted ?? []) {
    const first = await rebuild(entry.candidateId, config);
    if (first.response.ok) {
      const repeated = await rebuild(entry.candidateId, config);
      if (!repeated.response.ok || first.result.buildId !== repeated.result.buildId) {
        throw new Error(`catalog-aware rebuild was not deterministic for ${entry.candidateId}`);
      }
      assertExactCatalogBuild(entry.candidateId, first.result);
      outcomes.push({
        candidateId: entry.candidateId,
        topologyFingerprint: entry.topologyFingerprint,
        status: 'success',
        classification: 'catalog_aware_exact_assembly',
        buildId: first.result.buildId,
        instanceCount: first.result.placement.instances.length,
        connectionCellCount: first.result.placement.connectionCells.length,
        placementValidationOk: first.result.placementValidation.ok,
        builtFlowValidationOk: first.result.builtFlowValidation.ok,
        catalogAwareGeneration: first.result.catalogAwareGeneration,
      });
    } else {
      if (
        first.response.status !== 422
        || !String(first.result.error).startsWith('catalog_aware_')
        || first.result.evidence?.kind
          !== 'rusty_procgen.catalog_aware_generation_exhaustion.v1'
      ) {
        throw new Error(
          `unexpected catalog-aware outcome for ${entry.candidateId}:`
          + ` ${first.response.status} ${JSON.stringify(first.result)}`,
        );
      }
      outcomes.push({
        candidateId: entry.candidateId,
        topologyFingerprint: entry.topologyFingerprint,
        status: 'rejected',
        classification: first.result.evidence.classification,
        evidence: first.result.evidence,
      });
    }
  }
  const successes = outcomes.filter((outcome) => outcome.status === 'success');
  const report = {
    kind: 'rusty_procgen.evidence.catalog_aware_generation_coverage.v1',
    schemaVersion: 1,
    sourceSelectionRef: 'artifacts/samples/batch-v2/selection-report.json',
    sourceCatalogRef: 'fixtures/shape-catalogs/2d-basic.json',
    sourcePolicyRef: 'config/viewer-generation.json:catalogAwareGenerationPolicy',
    summary: {
      candidateCount: outcomes.length,
      successCount: successes.length,
      rejectionCount: outcomes.length - successes.length,
      successfulTopologyCount: new Set(
        successes.map((outcome) => outcome.topologyFingerprint),
      ).size,
      uniqueTopologyCount: new Set(
        outcomes.map((outcome) => outcome.topologyFingerprint),
      ).size,
    },
    outcomes,
  };
  await mkdir(dirname(outputPath), { recursive: true });
  await writeFile(outputPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(
    `catalog-aware coverage: ${report.summary.successCount}/${report.summary.candidateCount}`
    + ` candidates, ${report.summary.successfulTopologyCount}/${report.summary.uniqueTopologyCount}`
    + ` topologies; wrote ${outputPath}`,
  );
} catch (error) {
  throw new Error(`${error.message}\nViewer log:\n${serverLog}`);
} finally {
  server.kill('SIGTERM');
  await waitForChildExit(server);
  await rm(tempDir, { recursive: true, force: true });
}

function assertExactCatalogBuild(candidateId, result) {
  if (
    result.kind !== 'rusty_procgen.viewer_generation_rebuild.v1'
    || result.placement?.corridorRealization !== 'catalog'
    || result.placement?.connectionCells?.length !== 0
    || result.geometryValidation?.ok !== true
    || result.placementValidation?.ok !== true
    || result.builtFlowValidation?.ok !== true
    || result.catalogAwareGeneration?.attempts?.at(-1)?.classification !== 'success'
  ) {
    throw new Error(
      `catalog-aware success was not an exact validated assembly for ${candidateId}:`
      + ` ${JSON.stringify(result)}`,
    );
  }
}

async function rebuild(candidateId, config) {
  const response = await fetch(`${baseUrl}/api/generation-config/rebuild`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ candidateId, config }),
  });
  return { response, result: await response.json() };
}

async function fetchJson(path) {
  const response = await fetch(`${baseUrl}${path}`);
  if (!response.ok) {
    throw new Error(`failed to fetch ${path}: ${response.status}`);
  }
  return await response.json();
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
      // The server is still starting.
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 100));
  }
  throw new Error('catalog-aware coverage server did not start');
}

async function waitForChildExit(child) {
  if (child.exitCode !== null) {
    return;
  }
  await new Promise((resolvePromise) => {
    child.once('exit', resolvePromise);
    setTimeout(resolvePromise, 2_000);
  });
}
