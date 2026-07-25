import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';

const host = '127.0.0.1';
const port = Number(process.env.PURE_CATALOG_COVERAGE_PORT ?? 5217);
const baseUrl = `http://${host}:${port}`;
const outputPath = resolve(
  process.env.PURE_CATALOG_COVERAGE_OUT
    ?? 'artifacts/evidence/pure-catalog-coverage.json',
);

const server = spawn(
  process.execPath,
  ['scripts/serve-viewer.mjs', '--host', host, '--port', String(port)],
  {
    cwd: process.cwd(),
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
  const [selection, catalog] = await Promise.all([
    fetchJson('/api/batches/v2'),
    fetchJson('/fixtures/shape-catalogs/2d-basic.json'),
  ]);
  const accepted = selection.accepted ?? [];
  if (accepted.length === 0) {
    throw new Error('pure catalog coverage requires at least one accepted candidate');
  }

  const outcomes = [];
  for (const entry of accepted) {
    const response = await fetch(`${baseUrl}/api/experiments/corridor-realization`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        candidateId: entry.candidateId,
        corridorRealization: 'catalog',
      }),
    });
    const result = await response.json();
    if (response.ok) {
      assertPureCatalogSuccess(entry.candidateId, result);
      outcomes.push({
        candidateId: entry.candidateId,
        topologyFingerprint: entry.topologyFingerprint,
        status: 'success',
        classification: 'catalog_exact_assembly',
        placementId: result.placement.placementId,
        instanceCount: result.placement.instances.length,
        connectionCellCount: result.placement.connectionCells.length,
        placementValidationOk: result.placementValidation.ok,
        builtFlowValidationOk: result.builtFlowValidation.ok,
        usedShapeIds: [...new Set(result.placement.instances.map((instance) => instance.shapeId))]
          .sort(),
        search: result.placement.catalogSearch ?? null,
      });
      continue;
    }
    if (
      response.status !== 422
      || !['pure_catalog_search_exhausted', 'pure_catalog_coverage_rejected']
        .includes(result.error)
    ) {
      throw new Error(
        `unexpected pure catalog outcome for ${entry.candidateId}:`
        + ` ${response.status} ${JSON.stringify(result)}`,
      );
    }
    assertPureCatalogFailure(entry.candidateId, result.evidence);
    const signature = failureSignature(result.evidence);
    outcomes.push({
      candidateId: entry.candidateId,
      topologyFingerprint: entry.topologyFingerprint,
      status: 'rejected',
      classification: classifyFailure(result.evidence),
      failureSignatureId: stableId('failure', signature),
      failureSignature: signature,
      evidence: result.evidence,
    });
  }

  const failureGroups = groupFailures(outcomes);
  const successes = outcomes.filter((outcome) => outcome.status === 'success');
  const rejections = outcomes.filter((outcome) => outcome.status === 'rejected');
  const report = {
    kind: 'asha_procgen.evidence.pure_catalog_coverage.v1',
    schemaVersion: 1,
    sourceSelectionRef: 'artifacts/samples/batch-v2/selection-report.json',
    sourceCatalogRef: 'fixtures/shape-catalogs/2d-basic.json',
    catalogId: catalog.catalogId,
    summary: {
      candidateCount: outcomes.length,
      successCount: successes.length,
      rejectionCount: rejections.length,
      uniqueTopologyCount: new Set(outcomes.map((outcome) => outcome.topologyFingerprint)).size,
      successfulTopologyCount: new Set(
        successes.map((outcome) => outcome.topologyFingerprint),
      ).size,
      failureGroupCount: failureGroups.length,
    },
    outcomes,
    failureGroups,
    recommendations: failureGroups.map((group) => ({
      failureSignatureId: group.failureSignatureId,
      action: group.classification === 'catalog_aware_generation_alignment'
        ? 'retry geometry with catalog-aligned room ports and corridor envelopes'
        : 'add a reusable catalog family matching the recorded endpoint signature',
      followUpTaskId: group.classification === 'catalog_aware_generation_alignment'
        ? 6196
        : null,
    })),
  };
  await mkdir(dirname(outputPath), { recursive: true });
  await writeFile(outputPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.log(
    `pure catalog coverage: ${successes.length}/${outcomes.length} candidates,`
    + ` ${report.summary.successfulTopologyCount}/${report.summary.uniqueTopologyCount}`
    + ` topologies; ${failureGroups.length} grouped failure(s); wrote ${outputPath}`,
  );
} catch (error) {
  throw new Error(`${error.message}\nViewer log:\n${serverLog}`);
} finally {
  server.kill('SIGTERM');
  await waitForChildExit(server);
}

function assertPureCatalogSuccess(candidateId, result) {
  if (
    result.kind !== 'asha_procgen.corridor_realization_experiment.v1'
    || result.corridorRealization !== 'catalog'
    || result.placement?.corridorRealization !== 'catalog'
    || !Array.isArray(result.placement?.instances)
    || result.placement.instances.length === 0
    || !Array.isArray(result.placement?.connectionCells)
    || result.placement.connectionCells.length !== 0
    || result.placementValidation?.ok !== true
    || result.builtFlowValidation?.ok !== true
  ) {
    throw new Error(
      `pure catalog success was not an exact validated assembly for ${candidateId}:`
      + ` ${JSON.stringify(result)}`,
    );
  }
}

function assertPureCatalogFailure(candidateId, evidence) {
  const failure = evidence?.failure;
  const budgets = evidence?.budgets;
  if (
    evidence?.kind !== 'asha_procgen.pure_catalog_exhaustion.v1'
    || evidence.schemaVersion !== 1
    || typeof failure?.reason !== 'string'
    || typeof failure?.pieceId !== 'string'
    || !Array.isArray(failure.requiredEndpoints)
    || failure.requiredEndpoints.length === 0
    || !Array.isArray(failure.exhaustedFamilies)
    || failure.exhaustedFamilies.length === 0
    || !Number.isInteger(budgets?.decisions)
    || !Number.isInteger(budgets?.maxDecisions)
    || budgets.decisions > budgets.maxDecisions
    || !Number.isInteger(budgets?.backtracks)
    || !Number.isInteger(budgets?.maxBacktracks)
    || budgets.backtracks > budgets.maxBacktracks
  ) {
    throw new Error(
      `pure catalog rejection lacks structured evidence for ${candidateId}:`
      + ` ${JSON.stringify(evidence)}`,
    );
  }
}

function failureSignature(evidence) {
  const failure = evidence.failure;
  return {
    reason: failure.reason,
    requirementKind: failure.requirementKind,
    requiredDirections: failure.requiredEndpoints
      .map((endpoint) => endpoint.direction)
      .sort(),
    fixedPortDirection: failure.fixedPort?.direction ?? null,
    requiredOppositeDirection:
      failure.fixedPort?.requiredOppositeDirection ?? null,
    offsetFromEnvelopeAnchor:
      failure.fixedPort?.offsetFromEnvelopeAnchor ?? null,
    originBounds: failure.originBounds ?? null,
    laneEnvelope: failure.laneEnvelope === null
      ? null
      : {
          envelopeCells: failure.laneEnvelope.envelopeCells,
          from: failure.laneEnvelope.from,
          to: failure.laneEnvelope.to,
        },
    exhaustedFamilies: [...failure.exhaustedFamilies].sort(),
  };
}

function classifyFailure(evidence) {
  const failure = evidence.failure;
  const offset = failure.fixedPort?.offsetFromEnvelopeAnchor;
  const envelope = failure.laneEnvelope?.envelopeCells;
  if (
    failure.reason === 'geometry_constraint_rejected'
    && offset !== null
    && Number.isInteger(envelope)
    && (Math.abs(offset.x) > envelope || Math.abs(offset.y) > envelope)
  ) {
    return 'catalog_aware_generation_alignment';
  }
  if (
    failure.reason === 'geometry_constraint_rejected'
    && failure.originBounds !== null
    && (
      failure.originBounds.minX > failure.originBounds.maxX
      || failure.originBounds.minY > failure.originBounds.maxY
    )
  ) {
    return 'catalog_aware_generation_alignment';
  }
  return 'catalog_vocabulary_gap';
}

function groupFailures(outcomes) {
  const groups = new Map();
  for (const outcome of outcomes) {
    if (outcome.status !== 'rejected') {
      continue;
    }
    const existing = groups.get(outcome.failureSignatureId);
    if (existing === undefined) {
      groups.set(outcome.failureSignatureId, {
        failureSignatureId: outcome.failureSignatureId,
        classification: outcome.classification,
        candidateIds: [outcome.candidateId],
        topologyFingerprints: [outcome.topologyFingerprint],
        signature: outcome.failureSignature,
      });
      continue;
    }
    existing.candidateIds.push(outcome.candidateId);
    if (!existing.topologyFingerprints.includes(outcome.topologyFingerprint)) {
      existing.topologyFingerprints.push(outcome.topologyFingerprint);
    }
  }
  return [...groups.values()]
    .map((group) => ({
      ...group,
      candidateIds: group.candidateIds.sort(),
      topologyFingerprints: group.topologyFingerprints.sort(),
    }))
    .sort((left, right) =>
      left.failureSignatureId.localeCompare(right.failureSignatureId));
}

function stableId(prefix, value) {
  return `${prefix}.${createHash('sha256')
    .update(JSON.stringify(value))
    .digest('hex')
    .slice(0, 16)}`;
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
  while (Date.now() - started < 30_000) {
    try {
      const response = await fetch(`${baseUrl}/api/batches/v2`);
      if (
        response.ok
        && response.headers.get('x-den-project') === 'asha-procgen'
      ) {
        return;
      }
    } catch {
      // Server is still starting.
    }
    await new Promise((resolveWait) => setTimeout(resolveWait, 100));
  }
  throw new Error('timed out waiting for pure catalog coverage viewer');
}

async function waitForChildExit(child) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  await new Promise((resolveExit) => {
    child.once('exit', resolveExit);
  });
}
