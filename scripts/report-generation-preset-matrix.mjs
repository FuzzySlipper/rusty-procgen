import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import {
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';

const repoRoot = resolve(import.meta.dirname, '..');
const suiteRef = 'fixtures/generation-control/preset-matrix-v1.json';
const outputRef = 'artifacts/evidence/generation-preset-matrix.v1.json';
const checkOnly = process.argv.includes('--check');
const host = '127.0.0.1';
const port = Number(process.env.GENERATION_PRESET_REPORT_PORT ?? 5244);
const baseUrl = `http://${host}:${port}`;
const tempDir = await mkdtemp(join(tmpdir(), 'rusty-procgen-generation-presets-'));
const configPath = join(tempDir, 'viewer-generation.json');
const suite = await readJson(suiteRef);
assertSuite(suite);
const definitions = await readJson(suite.sourcePresetsRef);
const selection = await readJson(suite.sourceSelectionRef);
const baseConfig = await readJson(definitions.sourceBaseConfigRef);
await writeFile(configPath, encode(baseConfig));

const server = spawn(
  process.execPath,
  ['scripts/serve-viewer.mjs', '--host', host, '--port', String(port)],
  {
    cwd: repoRoot,
    env: {
      ...process.env,
      RUSTY_PROCGEN_GENERATION_CONFIG_PATH: configPath,
      RUSTY_PROCGEN_GENERATION_PRESETS_PATH: resolve(
        repoRoot,
        suite.sourcePresetsRef,
      ),
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
  const presets = await fetchJson('/api/generation-presets');
  assertPresets(presets, definitions);
  assertCandidates(selection, suite);

  const matrix = [];
  for (const candidate of suite.candidates) {
    const selectionEntry = selection.accepted.find(
      (entry) => entry.candidateId === candidate.candidateId,
    );
    for (const preset of presets.presets) {
      const beforeFirst = await readFile(configPath);
      const first = await rebuild(candidate.candidateId, preset.id);
      const afterFirst = await readFile(configPath);
      assertPublication(
        candidate.candidateId,
        preset,
        first.response.ok,
        beforeFirst,
        afterFirst,
      );
      const beforeSecond = afterFirst;
      const second = await rebuild(candidate.candidateId, preset.id);
      const afterSecond = await readFile(configPath);
      assertPublication(
        candidate.candidateId,
        preset,
        second.response.ok,
        beforeSecond,
        afterSecond,
      );
      if (
        first.status !== second.status
        || JSON.stringify(first.body) !== JSON.stringify(second.body)
      ) {
        throw new Error(
          `${candidate.candidateId}/${preset.id} was not byte-exact across repeated runs`,
        );
      }
      const actual = first.response.ok ? 'accepted' : 'rejected';
      if (actual !== candidate.expected[preset.id]) {
        throw new Error(
          `${candidate.candidateId}/${preset.id} expected ${
            candidate.expected[preset.id]
          }, received ${actual}: ${JSON.stringify(first.body)}`,
        );
      }
      matrix.push(summarize(
        candidate,
        selectionEntry,
        preset,
        first.status,
        first.body,
      ));
    }
  }

  const report = {
    kind: 'rusty_procgen.evidence.generation_preset_matrix.v1',
    schemaVersion: 1,
    sourceSuiteRef: suiteRef,
    sourceSelectionRef: suite.sourceSelectionRef,
    sourcePresetsRef: suite.sourcePresetsRef,
    sourceBaseConfigRef: definitions.sourceBaseConfigRef,
    sourceHashes: {
      suite: sha256Json(suite),
      selection: sha256Json(selection),
      presetDefinitions: sha256Json(definitions),
      baseConfig: sha256Json(baseConfig),
    },
    presetCount: presets.presets.length,
    candidateCount: suite.candidates.length,
    runCount: matrix.length,
    presets: presets.presets.map((preset) => ({
      id: preset.id,
      label: preset.label,
      summary: preset.summary,
      configHash: sha256Json(preset.config),
    })),
    candidates: suite.candidates.map((candidate) => ({
      candidateId: candidate.candidateId,
      role: candidate.role,
      profileSequence: selection.accepted.find(
        (entry) => entry.candidateId === candidate.candidateId,
      ).profileSequence,
    })),
    matrix,
  };

  const bytes = encode(report);
  if (checkOnly) {
    const expected = await readFile(resolve(repoRoot, outputRef));
    if (!expected.equals(bytes)) {
      throw new Error(
        `${outputRef} is stale; run pnpm run generation-presets:report`,
      );
    }
  } else {
    const target = resolve(repoRoot, outputRef);
    await mkdir(dirname(target), { recursive: true });
    await writeFile(target, bytes);
  }
  console.log(JSON.stringify({
    mode: checkOnly ? 'checked' : 'written',
    candidates: report.candidateCount,
    presets: report.presetCount,
    runs: report.runCount,
    matrix: report.matrix.map((entry) => ({
      candidateId: entry.candidateId,
      presetId: entry.presetId,
      outcome: entry.outcome,
      placementSpan: entry.metrics?.placementSpanCells ?? null,
      routedCatalogCells: entry.metrics?.routedCatalogCells ?? null,
      classification: entry.classification,
    })),
  }, null, 2));
} catch (error) {
  throw new Error(`${error.message}\nViewer server log:\n${serverLog}`);
} finally {
  server.kill('SIGTERM');
  await waitForChildExit(server);
  await rm(tempDir, { recursive: true, force: true });
}

function assertPublication(candidateId, preset, accepted, before, after) {
  if (accepted) {
    const expected = Buffer.from(encode(preset.config));
    if (!after.equals(expected)) {
      throw new Error(
        `${candidateId}/${preset.id} accepted without persisting its exact canonical config`,
      );
    }
    return;
  }
  if (!after.equals(before)) {
    throw new Error(
      `${candidateId}/${preset.id} rejection changed the prior persisted config`,
    );
  }
}

function summarize(candidate, selectionEntry, preset, status, body) {
  if (status === 200) {
    if (
      body.kind !== 'rusty_procgen.viewer_generation_preset_rebuild.v1'
      || body.schemaVersion !== 1
      || body.presetId !== preset.id
      || body.rebuild?.kind !== 'rusty_procgen.viewer_generation_rebuild.v1'
      || body.rebuild.persisted !== true
      || body.rebuild.catalogAwareGeneration === null
    ) {
      throw new Error(
        `${candidate.candidateId}/${preset.id} returned an invalid accepted envelope`,
      );
    }
    const rebuild = body.rebuild;
    const generation = rebuild.catalogAwareGeneration;
    const attempt = generation.attempts.find(
      (entry) => entry.attempt === generation.selectedAttempt,
    );
    if (attempt?.outcome?.admissible !== true) {
      throw new Error(
        `${candidate.candidateId}/${preset.id} omitted its selected admissible outcome`,
      );
    }
    return {
      candidateId: candidate.candidateId,
      profileSequence: selectionEntry.profileSequence,
      role: candidate.role,
      presetId: preset.id,
      outcome: 'accepted',
      classification: 'catalog_aware_exact_assembly',
      buildId: rebuild.buildId,
      outputHash: generation.trace.finalOutputHash,
      selectedAttempt: generation.selectedAttempt,
      attemptCount: generation.attempts.length,
      metrics: {
        ...attempt.outcome.metrics,
        geometryWidth: rebuild.geometry.bounds.width,
        geometryHeight: rebuild.geometry.bounds.height,
        roomEnvelopeArea: roomEnvelopeArea(rebuild.geometry.rooms),
        occupiedFillBasisPoints: occupiedFillBasisPoints(rebuild.placement),
      },
      failure: null,
    };
  }
  const evidence = body.evidence;
  if (
    status !== 422
    || typeof body.error !== 'string'
    || evidence?.kind !== 'rusty_procgen.catalog_aware_generation_exhaustion.v2'
    || evidence.schemaVersion !== 2
    || !Array.isArray(evidence.attempts)
    || evidence.result?.ok !== false
  ) {
    throw new Error(
      `${candidate.candidateId}/${preset.id} returned an invalid rejection envelope`,
    );
  }
  return {
    candidateId: candidate.candidateId,
    profileSequence: selectionEntry.profileSequence,
    role: candidate.role,
    presetId: preset.id,
    outcome: 'rejected',
    classification: evidence.classification,
    buildId: null,
    outputHash: evidence.trace?.finalOutputHash ?? null,
    selectedAttempt: null,
    attemptCount: evidence.attempts.length,
    metrics: null,
    failure: {
      code: body.error,
      maximumRoomsPlaced: Math.max(
        ...evidence.attempts.map((attempt) => attempt.roomsPlaced),
      ),
      maximumSectionsRouted: Math.max(
        ...evidence.attempts.map((attempt) => attempt.sectionsRouted),
      ),
      totalRoutingStates: evidence.attempts.reduce(
        (total, attempt) => total + attempt.routingStates,
        0,
      ),
    },
  };
}

function roomEnvelopeArea(rooms) {
  if (!Array.isArray(rooms) || rooms.length === 0) {
    return 0;
  }
  let minX = Number.POSITIVE_INFINITY;
  let minY = Number.POSITIVE_INFINITY;
  let maxX = Number.NEGATIVE_INFINITY;
  let maxY = Number.NEGATIVE_INFINITY;
  for (const room of rooms) {
    minX = Math.min(minX, room.rect.x);
    minY = Math.min(minY, room.rect.y);
    maxX = Math.max(maxX, room.rect.x + room.rect.width);
    maxY = Math.max(maxY, room.rect.y + room.rect.height);
  }
  return (maxX - minX) * (maxY - minY);
}

function occupiedFillBasisPoints(placement) {
  const cells = placement.occupiedCells;
  if (!Array.isArray(cells) || cells.length === 0) {
    return 0;
  }
  let minX = Number.POSITIVE_INFINITY;
  let minY = Number.POSITIVE_INFINITY;
  let maxX = Number.NEGATIVE_INFINITY;
  let maxY = Number.NEGATIVE_INFINITY;
  const occupied = new Set();
  for (const cell of cells) {
    minX = Math.min(minX, cell.x);
    minY = Math.min(minY, cell.y);
    maxX = Math.max(maxX, cell.x);
    maxY = Math.max(maxY, cell.y);
    occupied.add(`${cell.x},${cell.y}`);
  }
  const area = (maxX - minX + 1) * (maxY - minY + 1);
  return Math.floor((occupied.size * 10_000) / area);
}

function assertSuite(value) {
  if (
    value.kind !== 'rusty_procgen.generation_preset_matrix_suite.v1'
    || value.schemaVersion !== 1
    || value.sourceSelectionRef
      !== 'artifacts/samples/batch-v2/selection-report.json'
    || value.sourcePresetsRef
      !== 'fixtures/policies/viewer-generation-presets.v1.json'
    || !Array.isArray(value.candidates)
    || value.candidates.length !== 5
  ) {
    throw new Error(`${suiteRef} has an invalid preset matrix contract`);
  }
  const ids = value.candidates.map((candidate) => candidate?.candidateId);
  if (
    new Set(ids).size !== ids.length
    || !ids.includes('candidate.first_slice.5501')
  ) {
    throw new Error(`${suiteRef} must contain unique candidates including 5501`);
  }
  for (const candidate of value.candidates) {
    if (
      typeof candidate.candidateId !== 'string'
      || candidate.candidateId.length === 0
      || typeof candidate.role !== 'string'
      || candidate.role.length === 0
      || JSON.stringify(Object.keys(candidate.expected).sort())
        !== JSON.stringify(['normal', 'spread', 'tight'])
      || Object.values(candidate.expected).some(
        (outcome) => !['accepted', 'rejected'].includes(outcome),
      )
    ) {
      throw new Error(`${suiteRef} contains an invalid candidate`);
    }
  }
}

function assertPresets(presets, source) {
  if (
    presets.kind !== 'rusty_procgen.viewer_generation_presets.v1'
    || presets.schemaVersion !== 1
    || presets.sourceBaseConfigRef !== source.sourceBaseConfigRef
    || !Array.isArray(presets.presets)
    || JSON.stringify(presets.presets.map((preset) => preset.id))
      !== JSON.stringify(['tight', 'normal', 'spread'])
  ) {
    throw new Error('generation preset endpoint did not return the canonical catalog');
  }
}

function assertCandidates(selectionReport, presetSuite) {
  for (const candidate of presetSuite.candidates) {
    if (!selectionReport.accepted.some(
      (entry) => entry.candidateId === candidate.candidateId,
    )) {
      throw new Error(
        `${candidate.candidateId} is absent from ${presetSuite.sourceSelectionRef}`,
      );
    }
  }
}

async function rebuild(candidateId, presetId) {
  const response = await fetch(`${baseUrl}/api/generation-presets/rebuild`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ candidateId, presetId }),
  });
  return {
    response,
    status: response.status,
    body: await response.json(),
  };
}

async function fetchJson(path) {
  const response = await fetch(`${baseUrl}${path}`);
  if (!response.ok) {
    throw new Error(`${path} returned ${response.status}`);
  }
  return await response.json();
}

async function readJson(ref) {
  return JSON.parse(await readFile(resolve(repoRoot, ref), 'utf8'));
}

function encode(value) {
  return Buffer.from(`${JSON.stringify(value, null, 2)}\n`);
}

function sha256Json(value) {
  return `sha256:${createHash('sha256').update(JSON.stringify(value)).digest('hex')}`;
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
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 100));
  }
  throw new Error('generation preset report server did not start');
}

async function waitForChildExit(child) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  await new Promise((resolvePromise) => {
    child.once('exit', resolvePromise);
    setTimeout(resolvePromise, 2_000);
  });
}
