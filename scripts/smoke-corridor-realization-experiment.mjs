import { spawn } from 'node:child_process';

const host = '127.0.0.1';
const port = Number(process.env.CORRIDOR_SMOKE_PORT ?? 5194);
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
  const candidateIds = batch.accepted?.map((entry) => entry.candidateId) ?? [];
  const candidateId = candidateIds[0];
  if (typeof candidateId !== 'string') {
    throw new Error('corridor realization smoke requires one accepted batch candidate');
  }
  const procedural = await postExperiment({ candidateId, corridorRealization: 'procedural' }, 200);
  const repeatedProcedural = await postExperiment({
    candidateId,
    corridorRealization: 'procedural',
  }, 200);
  if (
    procedural.kind !== 'asha_procgen.corridor_realization_experiment.v1'
    || procedural.experimentId !== repeatedProcedural.experimentId
    || JSON.stringify(procedural.placement) !== JSON.stringify(repeatedProcedural.placement)
    || JSON.stringify(procedural.builtFlowValidation)
      !== JSON.stringify(repeatedProcedural.builtFlowValidation)
    || procedural.placement?.corridorRealization !== 'procedural'
    || procedural.placementValidation?.ok !== true
    || procedural.builtFlowValidation?.ok !== true
    || typeof procedural.builtFlowValidation?.validationId !== 'string'
    || procedural.builtFlowValidation.validationId.length === 0
    || procedural.metrics?.corridorPrefabInstances !== 0
    || procedural.metrics?.routedCorridorCells < 1
    || procedural.persisted !== false
    || procedural.nativeAuthority !== false
  ) {
    throw new Error('procedural corridor realization was not deterministic, validated, and prefab-free');
  }
  const proceduralMetrics = new Map([[candidateId, procedural.metrics]]);
  const proceduralIdentities = new Map([[candidateId, {
    planId: procedural.placement.planId,
    matchId: procedural.placement.matchId,
    placementId: procedural.placement.placementId,
    validationId: procedural.builtFlowValidation.validationId,
    experimentId: procedural.experimentId,
  }]]);
  for (const acceptedCandidateId of candidateIds.slice(1)) {
    const candidate = await postExperiment({
      candidateId: acceptedCandidateId,
      corridorRealization: 'procedural',
    }, 200);
    if (
      candidate.placement?.corridorRealization !== 'procedural'
      || candidate.metrics?.corridorPrefabInstances !== 0
      || candidate.placementValidation?.ok !== true
      || candidate.builtFlowValidation?.ok !== true
      || typeof candidate.builtFlowValidation?.validationId !== 'string'
      || candidate.builtFlowValidation.validationId.length === 0
    ) {
      throw new Error(`procedural corridor realization failed accepted candidate ${acceptedCandidateId}`);
    }
    proceduralMetrics.set(acceptedCandidateId, candidate.metrics);
    proceduralIdentities.set(acceptedCandidateId, {
      planId: candidate.placement.planId,
      matchId: candidate.placement.matchId,
      placementId: candidate.placement.placementId,
      validationId: candidate.builtFlowValidation.validationId,
      experimentId: candidate.experimentId,
    });
  }
  const hybridMetrics = new Map();
  for (const acceptedCandidateId of candidateIds) {
    const hybrid = await postExperiment({
      candidateId: acceptedCandidateId,
      corridorRealization: 'hybrid',
    }, 200);
    if (
      hybrid.placement?.corridorRealization !== 'hybrid'
      || !Number.isInteger(hybrid.metrics?.corridorPrefabInstances)
      || hybrid.metrics.corridorPrefabInstances < 0
      || !Number.isInteger(hybrid.metrics?.corridorPrefabCells)
      || hybrid.metrics?.corridorPrefabCells < hybrid.metrics.corridorPrefabInstances
      || !Number.isInteger(hybrid.metrics?.routedCorridorCells)
      || hybrid.metrics.routedCorridorCells < 0
      || hybrid.metrics?.footprintWidth < 1
      || hybrid.metrics?.footprintHeight < 1
      || hybrid.placementValidation?.ok !== true
      || hybrid.builtFlowValidation?.ok !== true
      || typeof hybrid.builtFlowValidation?.validationId !== 'string'
      || hybrid.builtFlowValidation.validationId.length === 0
    ) {
      throw new Error(
        `hybrid corridor realization failed accepted candidate ${acceptedCandidateId}: `
          + JSON.stringify({
            realization: hybrid.placement?.corridorRealization,
            metrics: hybrid.metrics,
            placementOk: hybrid.placementValidation?.ok,
            builtFlowOk: hybrid.builtFlowValidation?.ok,
            validationId: hybrid.builtFlowValidation?.validationId,
          }),
      );
    }
    if (acceptedCandidateId === candidateId) {
      const repeatedHybrid = await postExperiment({
        candidateId: acceptedCandidateId,
        corridorRealization: 'hybrid',
      }, 200);
      if (
        hybrid.experimentId !== repeatedHybrid.experimentId
        || JSON.stringify(hybrid.placement) !== JSON.stringify(repeatedHybrid.placement)
        || JSON.stringify(hybrid.builtFlowValidation)
          !== JSON.stringify(repeatedHybrid.builtFlowValidation)
      ) {
        throw new Error('hybrid corridor realization was not deterministic');
      }
    }
    const proceduralIdentity = proceduralIdentities.get(acceptedCandidateId);
    if (
      hybrid.placement.planId === proceduralIdentity.planId
      || hybrid.placement.matchId === proceduralIdentity.matchId
      || hybrid.placement.placementId === proceduralIdentity.placementId
      || hybrid.builtFlowValidation.validationId === proceduralIdentity.validationId
      || hybrid.experimentId === proceduralIdentity.experimentId
    ) {
      throw new Error(
        `hybrid and procedural identities collided for ${acceptedCandidateId}`,
      );
    }
    hybridMetrics.set(acceptedCandidateId, hybrid.metrics);
  }
  const proceduralRoutedCells = candidateIds.reduce(
    (total, id) => total + proceduralMetrics.get(id).routedCorridorCells,
    0,
  );
  const hybridRoutedCells = candidateIds.reduce(
    (total, id) => total + hybridMetrics.get(id).routedCorridorCells,
    0,
  );
  const layoutsWithReducedJoins = candidateIds.filter(
    (id) =>
      hybridMetrics.get(id).routedCorridorCells
      < proceduralMetrics.get(id).routedCorridorCells,
  ).length;
  const hybridPrefabInstances = candidateIds.reduce(
    (total, id) => total + hybridMetrics.get(id).corridorPrefabInstances,
    0,
  );
  // Hybrid intentionally falls back to procedural routing for bend-only
  // sections. Compact layouts can therefore have no eligible prefab span, and
  // stitching eligible spans can add a small join cost. Guard corpus coverage
  // and bounded aggregate overhead instead of requiring every layout to win.
  if (
    hybridPrefabInstances === 0
    || layoutsWithReducedJoins === 0
    || hybridRoutedCells * 10_000 > proceduralRoutedCells * 11_000
  ) {
    throw new Error(
      `hybrid corridor coverage was absent or exceeded its bounded routed-join overhead: `
        + `${hybridRoutedCells} hybrid versus ${proceduralRoutedCells} procedural; `
        + `${layoutsWithReducedJoins}/${candidateIds.length} layouts improved; `
        + `${hybridPrefabInstances} corridor prefabs`,
    );
  }
  const catalogOutcomes = new Map();
  for (const acceptedCandidateId of candidateIds) {
    const first = await postExperimentOutcome({
      candidateId: acceptedCandidateId,
      corridorRealization: 'catalog',
    });
    const repeated = await postExperimentOutcome({
      candidateId: acceptedCandidateId,
      corridorRealization: 'catalog',
    });
    if (first.status !== repeated.status || JSON.stringify(first.result) !== JSON.stringify(repeated.result)) {
      throw new Error(`pure catalog outcome was not deterministic for ${acceptedCandidateId}`);
    }
    if (first.status === 200) {
      const catalog = first.result;
      if (
        catalog.placement?.corridorRealization !== 'catalog'
        || catalog.placement?.connectionCells?.length !== 0
        || catalog.metrics?.routedCorridorCells !== 0
        || catalog.metrics?.corridorPrefabInstances < 1
        || catalog.placement?.catalogSearch === undefined
        || catalog.placementValidation?.ok !== true
        || catalog.builtFlowValidation?.ok !== true
      ) {
        throw new Error(`pure catalog success violated prefab-only invariants for ${acceptedCandidateId}`);
      }
      catalogOutcomes.set(acceptedCandidateId, 'success');
    } else if (
      first.status === 422
      && ['pure_catalog_coverage_rejected', 'pure_catalog_search_exhausted']
        .includes(first.result.error)
      && typeof first.result.detail === 'string'
      && first.result.detail.includes('pure catalog')
    ) {
      if (first.result.error === 'pure_catalog_search_exhausted') {
        assertPureCatalogExhaustionEvidence(first.result.evidence, acceptedCandidateId);
      }
      catalogOutcomes.set(acceptedCandidateId, 'stable rejection');
    } else {
      throw new Error(`unexpected pure catalog outcome for ${acceptedCandidateId}: ${JSON.stringify(first)}`);
    }
  }
  await postExperiment({ candidateId, corridorRealization: 'automatic' }, 400, 'invalid_corridor_realization');
  await postExperiment({ candidateId: 'candidate.unknown', corridorRealization: 'procedural' }, 404, 'candidate_not_found');
  await postExperiment({
    candidateId,
    corridorRealization: 'procedural',
    path: '/etc/passwd',
  }, 400, 'invalid_request_fields');
  const methodResponse = await fetch(`${baseUrl}/api/experiments/corridor-realization`);
  if (methodResponse.status !== 405) {
    throw new Error(`corridor realization GET expected 405, received ${methodResponse.status}`);
  }
  console.log(
    `corridor realization smoke passed; ${candidateIds.map((id) => {
      const proceduralResult = proceduralMetrics.get(id);
      const hybridResult = hybridMetrics.get(id);
      return `${id}: procedural ${proceduralResult.routedCorridorCells} routed; hybrid ${hybridResult.corridorPrefabInstances} prefabs/${hybridResult.routedCorridorCells} routed; pure catalog ${catalogOutcomes.get(id)}`;
    }).join(', ')}`,
  );
} finally {
  server.kill('SIGTERM');
  await waitForChildExit(server);
}

function assertPureCatalogExhaustionEvidence(evidence, candidateId) {
  const failure = evidence?.failure;
  const budgets = evidence?.budgets;
  if (
    evidence?.kind !== 'asha_procgen.pure_catalog_exhaustion.v1'
    || evidence.schemaVersion !== 1
    || typeof failure?.reason !== 'string'
    || typeof failure?.pieceId !== 'string'
    || !Array.isArray(failure.requiredEndpoints)
    || failure.requiredEndpoints.length === 0
    || failure.requiredEndpoints.some((endpoint) =>
      typeof endpoint?.id !== 'string' || typeof endpoint?.direction !== 'string')
    || typeof failure.fixedPort?.neighborPieceId !== 'string'
    || typeof failure.fixedPort?.neighborExitId !== 'string'
    || !Number.isInteger(failure.fixedPort?.cell?.x)
    || !Number.isInteger(failure.fixedPort?.cell?.y)
    || typeof failure.fixedPort?.requiredOppositeDirection !== 'string'
    || (failure.originBounds === undefined && failure.laneEnvelope === undefined)
    || !Array.isArray(failure.exhaustedFamilies)
    || failure.exhaustedFamilies.length === 0
    || !Number.isInteger(failure.candidateCount)
    || !Number.isInteger(budgets?.decisions)
    || !Number.isInteger(budgets?.maxDecisions)
    || budgets.decisions > budgets.maxDecisions
    || !Number.isInteger(budgets?.backtracks)
    || !Number.isInteger(budgets?.maxBacktracks)
    || budgets.backtracks > budgets.maxBacktracks
    || !Number.isInteger(budgets?.chainExpansions)
    || !Number.isInteger(budgets?.maxChainExpansionsPerSection)
  ) {
    throw new Error(
      `pure catalog rejection lacked actionable structured exhaustion evidence for ${candidateId}:`
      + ` ${JSON.stringify(evidence)}`,
    );
  }
}

async function postExperiment(payload, expectedStatus, expectedError) {
  const response = await fetch(`${baseUrl}/api/experiments/corridor-realization`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  const result = await response.json();
  if (response.status !== expectedStatus) {
    throw new Error(
      `corridor realization expected ${expectedStatus}, received ${response.status}: ${JSON.stringify(result)}`,
    );
  }
  if (expectedError !== undefined && result.error !== expectedError) {
    throw new Error(`corridor realization expected ${expectedError}, received ${JSON.stringify(result)}`);
  }
  return result;
}

async function postExperimentOutcome(payload) {
  const response = await fetch(`${baseUrl}/api/experiments/corridor-realization`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  return { status: response.status, result: await response.json() };
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
  throw new Error(`corridor realization smoke server did not start:\n${serverLog}`);
}

async function waitForChildExit(child) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  await new Promise((resolve) => child.once('exit', resolve));
}
