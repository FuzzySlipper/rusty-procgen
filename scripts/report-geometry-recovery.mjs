import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const repoRoot = resolve(import.meta.dirname, '..');
const fixtureRef = 'fixtures/geometry-recovery/v1.json';
const selectionRef = 'artifacts/samples/batch-v2/selection-report.json';
const outputRef = 'artifacts/evidence/geometry-recovery-v1.json';
const checkOnly = process.argv.includes('--check');

const readJson = async (ref) =>
  JSON.parse(await readFile(resolve(repoRoot, ref), 'utf8'));

const stableValue = (value) => {
  if (Array.isArray(value)) {
    return value.map(stableValue);
  }
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, stableValue(value[key])]),
    );
  }
  return value;
};

const normalizedPlanHash = (plan) => {
  const normalized = structuredClone(plan);
  for (const key of [
    'candidateId',
    'planId',
    'sourceCandidateRef',
    'sourceIntermediateRef',
  ]) {
    delete normalized[key];
  }
  return createHash('sha256')
    .update(JSON.stringify(stableValue(normalized)))
    .digest('hex');
};

const corridorCenterlineLength = (geometry) =>
  geometry.corridors.reduce(
    (total, corridor) =>
      total
      + corridor.points.slice(1).reduce(
        (length, point, index) =>
          length
          + Math.abs(point.x - corridor.points[index].x)
          + Math.abs(point.y - corridor.points[index].y),
        0,
      ),
    0,
  );

const roomEnvelopeArea = (geometry) => {
  const minX = Math.min(...geometry.rooms.map((room) => room.rect.x));
  const minY = Math.min(...geometry.rooms.map((room) => room.rect.y));
  const maxX = Math.max(
    ...geometry.rooms.map((room) => room.rect.x + room.rect.width),
  );
  const maxY = Math.max(
    ...geometry.rooms.map((room) => room.rect.y + room.rect.height),
  );
  return (maxX - minX) * (maxY - minY);
};

const median = (values) => {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? Math.floor((sorted[middle - 1] + sorted[middle]) / 2)
    : sorted[middle];
};

const reductionBasisPoints = (baseline, current) =>
  Math.floor(((baseline - current) * 10_000) / baseline);

const fixture = await readJson(fixtureRef);
const selection = await readJson(selectionRef);
const acceptedById = new Map(
  selection.accepted.map((entry) => [entry.candidateId, entry]),
);
const rejectedById = new Map(
  selection.rejected.map((entry) => [entry.candidateId, entry]),
);

if (selection.seed !== fixture.batchSeed) {
  throw new Error(
    `selection seed ${selection.seed} does not match corpus seed ${fixture.batchSeed}`,
  );
}
if (selection.requestedCount !== fixture.batchCount) {
  throw new Error(
    `selection count ${selection.requestedCount} does not match corpus count ${fixture.batchCount}`,
  );
}
if (selection.accepted.length < fixture.minimumAccepted) {
  throw new Error(
    `recovery corpus accepted ${selection.accepted.length}/${fixture.batchCount}; expected at least ${fixture.minimumAccepted}`,
  );
}

const compactnessLayouts = [];
for (const baseline of fixture.compactnessBaseline) {
  const candidateId = `candidate.first_slice.${baseline.candidateSeed}`;
  const accepted = acceptedById.get(candidateId);
  if (!accepted) {
    throw new Error(
      `${candidateId} was accepted at compactness baseline ${fixture.compactnessBaselineCommit} but is not accepted now`,
    );
  }
  const geometry = await readJson(accepted.geometryRef);
  const placement = await readJson(accepted.piecePlacementRef);
  const current = {
    roomEnvelopeArea: roomEnvelopeArea(geometry),
    corridorCenterlineLength: corridorCenterlineLength(geometry),
    routedShellCount: placement.connectionCells.length,
    occupiedCellCount: placement.occupiedCells.length,
  };
  const reductions = {
    roomEnvelopeAreaBasisPoints: reductionBasisPoints(
      baseline.roomEnvelopeArea,
      current.roomEnvelopeArea,
    ),
    corridorCenterlineLengthBasisPoints: reductionBasisPoints(
      baseline.corridorCenterlineLength,
      current.corridorCenterlineLength,
    ),
    routedShellCountBasisPoints: reductionBasisPoints(
      baseline.routedShellCount,
      current.routedShellCount,
    ),
    occupiedCellCountBasisPoints: reductionBasisPoints(
      baseline.occupiedCellCount,
      current.occupiedCellCount,
    ),
  };
  for (const metric of [
    'roomEnvelopeAreaBasisPoints',
    'corridorCenterlineLengthBasisPoints',
    'routedShellCountBasisPoints',
  ]) {
    if (
      reductions[metric]
      < -fixture.compactnessTargets.maximumPerLayoutRegressionBasisPoints
    ) {
      throw new Error(
        `${candidateId} compactness ${metric} regressed by ${-reductions[metric]} basis points`,
      );
    }
  }
  compactnessLayouts.push({
    candidateId,
    candidateIndex: baseline.candidateIndex,
    candidateSeed: baseline.candidateSeed,
    baseline: {
      roomEnvelopeArea: baseline.roomEnvelopeArea,
      corridorCenterlineLength: baseline.corridorCenterlineLength,
      routedShellCount: baseline.routedShellCount,
      occupiedCellCount: baseline.occupiedCellCount,
    },
    current,
    reductions,
    search: {
      validLayoutCandidates: geometry.layoutSearch.validLayoutCandidates,
      selectedEmbedding: geometry.layoutSearch.embeddingId,
      spacingTier: geometry.layoutSearch.spacingTier,
    },
    refs: {
      geometry: accepted.geometryRef,
      placement: accepted.piecePlacementRef,
    },
  });
}

const compactnessAggregate = {
  baselineMedianRoomEnvelopeArea: median(
    compactnessLayouts.map((layout) => layout.baseline.roomEnvelopeArea),
  ),
  currentMedianRoomEnvelopeArea: median(
    compactnessLayouts.map((layout) => layout.current.roomEnvelopeArea),
  ),
  baselineMedianRoutedShellCount: median(
    compactnessLayouts.map((layout) => layout.baseline.routedShellCount),
  ),
  currentMedianRoutedShellCount: median(
    compactnessLayouts.map((layout) => layout.current.routedShellCount),
  ),
};
compactnessAggregate.roomEnvelopeReductionBasisPoints = reductionBasisPoints(
  compactnessAggregate.baselineMedianRoomEnvelopeArea,
  compactnessAggregate.currentMedianRoomEnvelopeArea,
);
compactnessAggregate.routedShellReductionBasisPoints = reductionBasisPoints(
  compactnessAggregate.baselineMedianRoutedShellCount,
  compactnessAggregate.currentMedianRoutedShellCount,
);
if (
  compactnessAggregate.roomEnvelopeReductionBasisPoints
    < fixture.compactnessTargets.minimumMedianReductionBasisPoints
  && compactnessAggregate.routedShellReductionBasisPoints
    < fixture.compactnessTargets.minimumMedianReductionBasisPoints
) {
  throw new Error(
    `compactness median reductions ${compactnessAggregate.roomEnvelopeReductionBasisPoints}/${compactnessAggregate.routedShellReductionBasisPoints} basis points miss target ${fixture.compactnessTargets.minimumMedianReductionBasisPoints}`,
  );
}

const families = [];
for (const family of fixture.families) {
  const candidateId = `candidate.first_slice.${family.candidateSeed}`;
  const candidateDir = `artifacts/samples/batch-v2/candidate-${String(
    family.candidateIndex,
  ).padStart(3, '0')}`;
  const planRef = `${candidateDir}/physical-connection-plan.json`;
  const plan = await readJson(planRef);
  const actualPlanHash = normalizedPlanHash(plan);
  if (actualPlanHash !== family.normalizedPlanSha256) {
    throw new Error(
      `${family.profileSequence} normalized plan hash ${actualPlanHash} does not match ${family.normalizedPlanSha256}`,
    );
  }

  const accepted = acceptedById.get(candidateId);
  const rejected = rejectedById.get(candidateId);
  if (Boolean(accepted) === Boolean(rejected)) {
    throw new Error(
      `${candidateId} must occur exactly once in accepted or rejected selection entries`,
    );
  }

  if (accepted) {
    if (accepted.profileSequence !== family.profileSequence) {
      throw new Error(
        `${candidateId} profile ${accepted.profileSequence} does not match ${family.profileSequence}`,
      );
    }
    const geometry = await readJson(accepted.geometryRef);
    const geometryValidation = await readJson(accepted.geometryValidationRef);
    const placement = await readJson(accepted.piecePlacementRef);
    const placementValidation = await readJson(
      accepted.piecePlacementValidationRef,
    );
    const builtFlow = await readJson(accepted.builtFlowValidationRef);
    if (!geometryValidation.ok || !placementValidation.ok || !builtFlow.ok) {
      throw new Error(`${candidateId} did not pass every physical pipeline gate`);
    }
    families.push({
      profileSequence: family.profileSequence,
      candidateIndex: family.candidateIndex,
      candidateSeed: family.candidateSeed,
      baselineGeometryOutcome: family.baselineGeometryOutcome,
      normalizedPlanSha256: actualPlanHash,
      currentOutcome: 'accepted',
      stages: {
        feasibility: 'rotation_witness',
        geometry: 'accepted',
        configuredRealization: placement.corridorRealization,
        placementValidation: 'accepted',
        builtFlowValidation: 'accepted',
      },
      embedding: {
        kind: geometry.layoutSearch.embeddingKind,
        id: geometry.layoutSearch.embeddingId,
        faces: geometry.layoutSearch.embeddingFaces,
        targetFaces: geometry.layoutSearch.embeddingTargetFaces,
        searchSteps: geometry.layoutSearch.embeddingSearchSteps,
      },
      geometrySearch: {
        attempts: geometry.layoutSearch.searchAttempts,
        spacingTier: geometry.layoutSearch.spacingTier,
        roomOrderAttempt: geometry.layoutSearch.roomOrderAttempt,
        portOrderAttempt: geometry.layoutSearch.portOrderAttempt,
        routeOrderAttempt: geometry.layoutSearch.routeOrderAttempt,
        routedSections: geometry.corridors.length,
        decisions: geometry.layoutSearch.routeDecisions,
        backtracks: geometry.layoutSearch.routeBacktracks,
        pathAlternatives: geometry.layoutSearch.routePathAlternatives,
        repairs: geometry.layoutSearch.routeRepairs,
        gridExpansions: geometry.layoutSearch.routeGridExpansions,
        pathExpansionExhaustions:
          geometry.layoutSearch.routePathExpansionExhaustions,
        blockingOwners: geometry.layoutSearch.routeBlockingOwners,
        budgetExhausted: null,
      },
      compactness: {
        validLayoutCandidates: geometry.layoutSearch.validLayoutCandidates,
        portalCapacityPenalty:
          geometry.layoutSearch.compactnessPortalCapacityPenalty,
        buildEnvelopeArea: geometry.layoutSearch.compactnessEnvelopeArea,
        corridorCenterlineLength:
          geometry.layoutSearch.compactnessCorridorCenterlineLength,
        routedShellCost: geometry.layoutSearch.compactnessRoutedShellCost,
        bendCount: geometry.layoutSearch.compactnessBendCount,
        routedShellCount: placement.connectionCells.length,
      },
      realizationSearch: {
        scaleTier: placement.realizationSearch.realizationScaleTier,
        attempts: placement.realizationSearch.realizationAttempts,
        routeOrderAttempt: placement.realizationSearch.routeOrderAttempt,
        routeAttempts: placement.realizationSearch.routeAttempts,
        decisions: placement.realizationSearch.routeDecisions,
        backtracks: placement.realizationSearch.routeBacktracks,
        pathAlternatives:
          placement.realizationSearch.routePathAlternatives,
        repairs: placement.realizationSearch.routeRepairs,
        blockingOwners:
          placement.realizationSearch.routeBlockingOwners ?? [],
        budgetExhausted:
          placement.realizationSearch.routeBudgetExhausted ?? null,
      },
      refs: {
        physicalPlan: planRef,
        geometry: accepted.geometryRef,
        geometryValidation: accepted.geometryValidationRef,
        placement: accepted.piecePlacementRef,
        placementValidation: accepted.piecePlacementValidationRef,
        builtFlowValidation: accepted.builtFlowValidationRef,
      },
    });
    continue;
  }

  if (rejected.profileSequence !== family.profileSequence) {
    throw new Error(
      `${candidateId} profile ${rejected.profileSequence} does not match ${family.profileSequence}`,
    );
  }
  const diagnostic = rejected.diagnostics.find(
    (entry) => entry.severity === 'fatal',
  );
  const embeddingId =
    diagnostic?.detail.match(/rotation\.v1\.[A-Za-z0-9:]+/)?.[0] ?? null;
  const necessaryCondition = diagnostic?.detail.includes(
    'necessary-condition failed',
  );
  if (!embeddingId && !necessaryCondition) {
    throw new Error(
      `${candidateId} rejection lacks a rotation identifier or certified necessary-condition failure`,
    );
  }
  families.push({
    profileSequence: family.profileSequence,
    candidateIndex: family.candidateIndex,
    candidateSeed: family.candidateSeed,
    baselineGeometryOutcome: family.baselineGeometryOutcome,
    normalizedPlanSha256: actualPlanHash,
    currentOutcome: 'rejected',
    stages: {
      feasibility: necessaryCondition
        ? 'necessary_condition_obstruction'
        : 'bounded_embedding_search_exhausted',
      geometry: 'rejected',
      configuredRealization: 'not_run',
      placementValidation: 'not_run',
      builtFlowValidation: 'not_run',
    },
    embedding: {
      kind: necessaryCondition ? 'necessary_condition' : 'planar_rotation',
      id: embeddingId,
    },
    geometrySearch: {
      diagnosticCode: diagnostic.code,
      detail: diagnostic.detail,
      budgetExhausted: 'bounded_embedding_search',
    },
    realizationSearch: null,
    refs: {
      physicalPlan: planRef,
      candidate: rejected.candidateRef,
    },
  });
}

const report = {
  kind: 'asha_procgen.geometry_recovery_report.v1',
  schemaVersion: 1,
  corpusRef: fixtureRef,
  selectionRef,
  batchSeed: fixture.batchSeed,
  batchCount: fixture.batchCount,
  acceptedCount: selection.accepted.length,
  rejectedCount: selection.rejected.length,
  minimumAccepted: fixture.minimumAccepted,
  compactness: {
    baselineCommit: fixture.compactnessBaselineCommit,
    targets: fixture.compactnessTargets,
    aggregate: compactnessAggregate,
    layouts: compactnessLayouts,
  },
  families,
};
const encoded = `${JSON.stringify(report, null, 2)}\n`;

if (checkOnly) {
  const committed = await readFile(resolve(repoRoot, outputRef), 'utf8');
  if (committed !== encoded) {
    throw new Error(
      `${outputRef} is stale; run npm run geometry:recovery:report`,
    );
  }
  console.log(
    `geometry recovery report is byte-stable at ${selection.accepted.length}/${fixture.batchCount} accepted`,
  );
} else {
  await writeFile(resolve(repoRoot, outputRef), encoded);
  console.log(
    `geometry recovery report wrote ${families.length} unique families with ${selection.accepted.length}/${fixture.batchCount} accepted to ${outputRef}`,
  );
}
