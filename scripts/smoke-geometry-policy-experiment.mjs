import { spawn } from 'node:child_process';

const host = '127.0.0.1';
const port = Number(process.env.GEOMETRY_POLICY_SMOKE_PORT ?? 5297);
const baseUrl = `http://${host}:${port}`;
const server = spawn(process.execPath, ['scripts/serve-viewer.mjs', '--host', host, '--port', String(port)], {
  cwd: process.cwd(),
  stdio: ['ignore', 'pipe', 'pipe'],
});

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
  const candidateId = batch.accepted?.find((entry) => entry.profileSequence === 'lock-key-baseline')?.candidateId
    ?? batch.accepted?.[0]?.candidateId;
  if (typeof candidateId !== 'string') {
    throw new Error('geometry policy smoke requires one accepted batch candidate');
  }
  const policy = geometryPolicy();
  const first = await postExperiment({ candidateId, geometryLayoutPolicy: policy }, 200);
  const repeated = await postExperiment({ candidateId, geometryLayoutPolicy: policy }, 200);
  if (
    first.kind !== 'rusty_procgen.geometry_layout_policy_experiment.v1'
    || first.experimentId !== repeated.experimentId
    || JSON.stringify(first.geometry) !== JSON.stringify(repeated.geometry)
    || JSON.stringify(first.placement) !== JSON.stringify(repeated.placement)
    || first.geometryValidation?.ok !== true
    || first.placementValidation?.ok !== true
    || first.builtFlowValidation?.ok !== true
    || first.persisted !== false
    || first.nativeAuthority !== false
  ) {
    throw new Error('valid geometry-policy experiment was not deterministic, validated, and explicitly temporary');
  }
  if (
    first.geometry?.layoutPolicy?.initialColumnGap !== policy.initialColumnGap
    || !Number.isInteger(first.geometry?.layoutSearch?.spacingTier)
    || JSON.stringify(first).includes('/tmp/')
  ) {
    throw new Error('geometry experiment did not carry policy/search evidence or leaked temporary paths');
  }

  await postExperiment({
    candidateId,
    geometryLayoutPolicy: { ...policy, initialColumnGap: 145 },
  }, 400, 'invalid_initialColumnGap');
  await postExperiment({
    candidateId,
    geometryLayoutPolicy: { ...policy, maxSearchAttempts: 81 },
  }, 400, 'invalid_maxSearchAttempts');
  await postExperiment({
    candidateId: 'candidate.unknown',
    geometryLayoutPolicy: policy,
  }, 404, 'candidate_not_found');
  await postExperiment({
    candidateId,
    geometryLayoutPolicy: policy,
    path: '/etc/passwd',
  }, 400, 'invalid_request_fields');

  const methodResponse = await fetch(`${baseUrl}/api/experiments/geometry-layout-policy`);
  if (methodResponse.status !== 405) {
    throw new Error(`geometry policy endpoint GET expected 405, received ${methodResponse.status}`);
  }
  console.log(
    `geometry policy experiment smoke passed; ${candidateId}, deterministic ${first.experimentId.slice(0, 12)}, tier ${first.geometry.layoutSearch.spacingTier + 1}`,
  );
} finally {
  server.kill('SIGTERM');
  await waitForChildExit(server);
}

function geometryPolicy() {
  return {
    kind: 'rusty_procgen.geometry_layout_policy.v1',
    schemaVersion: 1,
    initialRoomMargin: 96,
    initialColumnGap: 144,
    initialRowGap: 64,
    roomMarginGrowth: 48,
    columnGapGrowth: 72,
    rowGapGrowth: 40,
    maxSpacingTiers: 5,
    roomOrderAttemptsPerTier: 4,
    maxSearchAttempts: 80,
  };
}

async function postExperiment(payload, expectedStatus, expectedError) {
  const response = await fetch(`${baseUrl}/api/experiments/geometry-layout-policy`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  const result = await response.json();
  if (response.status !== expectedStatus) {
    throw new Error(
      `geometry policy endpoint expected ${expectedStatus}, received ${response.status}: ${JSON.stringify(result)}`,
    );
  }
  if (expectedError !== undefined && result.error !== expectedError) {
    throw new Error(`geometry policy endpoint expected ${expectedError}, received ${JSON.stringify(result)}`);
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
  throw new Error(`geometry policy smoke server did not start:\n${serverLog}`);
}

async function waitForChildExit(child) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  await new Promise((resolve) => child.once('exit', resolve));
}
