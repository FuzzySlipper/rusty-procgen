import { spawn } from 'node:child_process';

const host = '127.0.0.1';
const port = Number(process.env.POLICY_SMOKE_PORT ?? 5196);
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
  const candidateId = batch.accepted?.[0]?.candidateId;
  if (typeof candidateId !== 'string') {
    throw new Error('policy smoke requires one accepted batch candidate');
  }
  const policy = placementPolicy(5, 2);
  const first = await postExperiment({ candidateId, placementPolicy: policy }, 200);
  const repeated = await postExperiment({ candidateId, placementPolicy: policy }, 200);
  if (
    first.kind !== 'rusty_procgen.placement_policy_experiment.v1'
    || first.experimentId !== repeated.experimentId
    || JSON.stringify(first.placement) !== JSON.stringify(repeated.placement)
    || first.validation?.ok !== true
    || first.persisted !== false
    || first.nativeAuthority !== false
  ) {
    throw new Error('valid placement-policy experiment was not deterministic and explicitly temporary');
  }
  if (
    first.placement?.placementPolicy?.minimumClearanceCells !== 5
    || first.placement?.placementPolicy?.wallThicknessCells !== 2
    || String(first.placement?.sourceCatalogRef).includes('/tmp/')
  ) {
    throw new Error('experiment response did not carry the requested policy or leaked its temporary catalog path');
  }

  await postExperiment({ candidateId, placementPolicy: placementPolicy(3, 2) }, 400, 'clearance_too_small_for_walls');
  await postExperiment({ candidateId, placementPolicy: placementPolicy(1, 1) }, 400, 'invalid_minimumClearanceCells');
  await postExperiment({ candidateId: 'candidate.unknown', placementPolicy: policy }, 404, 'candidate_not_found');
  await postExperiment({
    candidateId,
    path: '/etc/passwd',
    placementPolicy: policy,
  }, 400, 'invalid_request_fields');
  await postExperiment({
    candidateId,
    placementPolicy: { ...policy, doorwayWidthCells: 3 },
  }, 400, 'unsupported_doorway_width');

  const methodResponse = await fetch(`${baseUrl}/api/experiments/placement-policy`);
  if (methodResponse.status !== 405) {
    throw new Error(`policy endpoint GET expected 405, received ${methodResponse.status}`);
  }
  console.log(
    `placement policy experiment smoke passed; ${candidateId}, deterministic ${first.experimentId.slice(0, 12)}, clearance 5, wall 2`,
  );
} finally {
  server.kill('SIGTERM');
  await waitForChildExit(server);
}

function placementPolicy(minimumClearanceCells, wallThicknessCells) {
  return {
    schemaVersion: 1,
    minimumClearanceCells,
    contactPolicy: 'glued_exits_only',
    wallThicknessCells,
    doorwayWidthCells: 1,
    preservePieceBoundaries: true,
  };
}

async function postExperiment(payload, expectedStatus, expectedError) {
  const response = await fetch(`${baseUrl}/api/experiments/placement-policy`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  const result = await response.json();
  if (response.status !== expectedStatus) {
    throw new Error(
      `policy endpoint expected ${expectedStatus}, received ${response.status}: ${JSON.stringify(result)}`,
    );
  }
  if (expectedError !== undefined && result.error !== expectedError) {
    throw new Error(`policy endpoint expected ${expectedError}, received ${JSON.stringify(result)}`);
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
  throw new Error(`placement policy smoke server did not start:\n${serverLog}`);
}

async function waitForChildExit(child) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  await new Promise((resolve) => child.once('exit', resolve));
}
