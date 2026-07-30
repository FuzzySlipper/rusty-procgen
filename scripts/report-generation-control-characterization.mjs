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

import { decodeCatalogGenerationRun } from '../dist/ts/src/catalog-generation-trace.js';

const repoRoot = resolve(import.meta.dirname, '..');
const suiteRef = 'fixtures/generation-control/characterization-v1.json';
const outputRef = 'artifacts/evidence/generation-control-characterization.v1.json';
const checkOnly = process.argv.includes('--check');
const host = '127.0.0.1';
const port = Number(process.env.GENERATION_CONTROL_REPORT_PORT ?? 5222);
const baseUrl = `http://${host}:${port}`;
const tempDir = await mkdtemp(join(tmpdir(), 'rusty-procgen-generation-control-'));
const configPath = join(tempDir, 'viewer-generation.json');
const suite = await readJson(suiteRef);
assertSuite(suite);
const baseConfig = await readJson(suite.sourceBaseConfigRef);
assertMatrixCoverage(suite, baseConfig);
await writeFile(configPath, encode(baseConfig));

const server = spawn(
  process.execPath,
  ['scripts/serve-viewer.mjs', '--host', host, '--port', String(port)],
  {
    cwd: repoRoot,
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
  const selection = await fetchJson('/api/batches/v2');
  const selectionOwner = await readJson(suite.sourceSelectionRef);
  if (JSON.stringify(selection) !== JSON.stringify(selectionOwner)) {
    throw new Error(
      `viewer selection differs from tracked owner ${suite.sourceSelectionRef}`,
    );
  }
  const candidateIds = new Set(selection.accepted.map((entry) => entry.candidateId));
  for (const run of suite.namedRuns) {
    if (!candidateIds.has(run.candidateId)) {
      throw new Error(`named run ${run.id} references missing ${run.candidateId}`);
    }
  }
  if (!candidateIds.has(suite.influenceMatrix.candidateId)) {
    throw new Error(`matrix references missing ${suite.influenceMatrix.candidateId}`);
  }

  const namedRuns = new Map();
  for (const definition of suite.namedRuns) {
    if (definition.generateFromConfig) {
      const config = applyOverrides(baseConfig, definition.overrides);
      const first = await rebuild(definition.candidateId, config);
      const second = await rebuild(definition.candidateId, config);
      const firstRun = responseTracePair(definition.id, first);
      const secondRun = responseTracePair(`${definition.id} repeat`, second);
      assertNamedOutcome(definition, firstRun.result);
      if (
        first.response.status !== second.response.status
        || JSON.stringify(firstRun) !== JSON.stringify(secondRun)
      ) {
        throw new Error(`${definition.id} was not byte-exact across two runs`);
      }
      namedRuns.set(definition.id, {
        ...firstRun,
        decoded: decodeCatalogGenerationRun(firstRun.trace, firstRun.result),
        fixtureRole: 'checked_live_config_fixture',
      });
    } else {
      const fixture = await readFixtureRun(definition);
      namedRuns.set(definition.id, {
        ...fixture,
        decoded: decodeCatalogGenerationRun(fixture.trace, fixture.result),
        fixtureRole: 'existing_checked_cli_fixture',
      });
      assertNamedOutcome(definition, fixture.result);
    }
  }

  const baselineDefinition = requiredRun(suite, suite.influenceMatrix.baselineRunId);
  const baselineFixture = requiredNamedRun(namedRuns, baselineDefinition.id);
  const baselineConfig = applyOverrides(baseConfig, baselineDefinition.overrides);
  const baselineResponse = await rebuild(baselineDefinition.candidateId, baselineConfig);
  assertSuccessfulCatalogRun('matrix baseline', baselineResponse);
  const checkedBaselineMetrics = metricsForRun(
    baselineFixture.result,
    baselineFixture.trace,
  );
  const baselineMetrics = metricsForResponse(baselineResponse);
  assertSameLayoutMetrics(
    'checked sprawling trace and live baseline',
    checkedBaselineMetrics,
    baselineMetrics,
  );

  const probes = [];
  for (const definition of suite.influenceMatrix.probes) {
    const config = applyOverrides(baseConfig, [definition]);
    assertSingleValueChange(baseConfig, config, definition.path);
    const response = await rebuild(suite.influenceMatrix.candidateId, config);
    probes.push(summarizeProbe(definition, baselineResponse, response));
  }

  const report = {
    kind: 'rusty_procgen.evidence.generation_control_characterization.v1',
    schemaVersion: 1,
    sourceSuiteRef: suiteRef,
    sourceSelectionRef: suite.sourceSelectionRef,
    sourceBaseConfigRef: suite.sourceBaseConfigRef,
    sourceHashes: {
      suite: sha256Json(suite),
      selection: sha256Json(selectionOwner),
      baseConfig: sha256Json(baseConfig),
    },
    behaviorChanges: true,
    namedRuns: suite.namedRuns.map((definition) => {
      const run = requiredNamedRun(namedRuns, definition.id);
      return summarizeNamedRun(
        definition,
        run.result,
        run.trace,
        run.decoded,
        run.fixtureRole,
      );
    }),
    influenceMatrix: {
      candidateId: suite.influenceMatrix.candidateId,
      baselineRunId: suite.influenceMatrix.baselineRunId,
      baseline: summarizeResponse(baselineResponse),
      probes,
    },
  };

  const generated = new Map([[outputRef, encode(report)]]);
  for (const definition of suite.namedRuns.filter((run) => run.generateFromConfig)) {
    const run = requiredNamedRun(namedRuns, definition.id);
    generated.set(definition.resultRef, encode(run.result));
    generated.set(definition.traceRef, encode(run.trace));
  }
  for (const [ref, bytes] of generated) {
    if (checkOnly) {
      const expected = await readFile(resolve(repoRoot, ref));
      if (!expected.equals(bytes)) {
        throw new Error(`${ref} is stale; run pnpm run generation-control:report`);
      }
    } else {
      const target = resolve(repoRoot, ref);
      await mkdir(dirname(target), { recursive: true });
      await writeFile(target, bytes);
    }
  }
  const effects = Object.fromEntries(
    probes.map((probe) => [
      probe.settingPath,
      probe.observed.effect,
    ]),
  );
  console.log(JSON.stringify({
    namedRuns: report.namedRuns.map((run) => ({
      id: run.id,
      candidateId: run.candidateId,
      outcome: run.outcome,
      outputHash: run.outputHash,
      placementSpan: run.metrics?.placementSpan ?? null,
      roomEnvelopeArea: run.metrics?.roomEnvelopeArea ?? null,
      routedCells: run.metrics?.routedCatalogCells ?? null,
      failure: run.failure,
    })),
    probeCount: probes.length,
    effects,
    mode: checkOnly ? 'checked' : 'written',
  }, null, 2));
} catch (error) {
  throw new Error(`${error.message}\nViewer server log:\n${serverLog}`);
} finally {
  server.kill('SIGTERM');
  await waitForChildExit(server);
  await rm(tempDir, { recursive: true, force: true });
}

function summarizeNamedRun(definition, result, trace, decoded, fixtureRole) {
  const accepted = result.ok === true;
  return {
    id: definition.id,
    label: definition.label,
    candidateId: definition.candidateId,
    fixtureRole,
    resultRef: definition.resultRef,
    traceRef: definition.traceRef,
    outputHash: decoded.outputHash,
    traceRootHash: trace.rootHash,
    selectedAttempt: decoded.selectedAttempt,
    outcome: accepted ? 'accepted' : 'rejected',
    classification: accepted ? 'success' : result.exhaustedClassification,
    metrics: accepted ? metricsForRun(result, trace) : null,
    failure: accepted ? null : {
      attemptCount: result.attempts.length,
      classifications: [...new Set(
        result.attempts.map((attempt) => attempt.classification),
      )],
      maximumRoomsPlaced: Math.max(
        ...result.attempts.map((attempt) => attempt.roomsPlaced),
      ),
      maximumSectionsRouted: Math.max(
        ...result.attempts.map((attempt) => attempt.sectionsRouted),
      ),
      totalRoutingStates: result.attempts.reduce(
        (total, attempt) => total + attempt.routingStates,
        0,
      ),
    },
  };
}

function summarizeProbe(definition, baseline, response) {
  const baselineSummary = summarizeResponse(baseline);
  const summary = summarizeResponse(response);
  const changedStages = Object.keys(baselineSummary.stageHashes)
    .filter((stage) =>
      baselineSummary.stageHashes[stage] !== summary.stageHashes[stage]);
  const metricDeltas = summary.metrics === null
    ? null
    : numericDeltas(baselineSummary.metrics, summary.metrics);
  const firstChangedEvent = firstTraceDifference(
    traceFromResponse(baseline),
    traceFromResponse(response),
  );
  const effect = (() => {
    if (!response.response.ok) {
      return response.body.evidence === undefined
        ? 'configuration_rejected'
        : 'generation_rejected';
    }
    if (firstChangedEvent !== null) {
      return 'decision_changed';
    }
    if (changedStages.length === 0 && allZero(metricDeltas)) {
      return 'no_observed_semantic_effect';
    }
    return 'projection_changed';
  })();
  return {
    settingPath: definition.path,
    baselineValue: valueAtPath(baseConfig, definition.path),
    probeValue: definition.value,
    owningStage: settingStage(definition.path),
    observed: {
      effect,
      classification: summary.classification,
      changedStages,
      firstChangedEvent,
      metricDeltas,
      outputHash: summary.outputHash,
      traceRootHash: summary.traceRootHash,
    },
  };
}

function summarizeResponse(response) {
  if (!response.response.ok) {
    const evidence = response.body.evidence;
    return {
      status: 'rejected',
      classification: response.body.error ?? 'generation_rejected',
      outputHash: evidence?.trace?.finalOutputHash ?? null,
      traceRootHash: evidence?.trace?.rootHash ?? null,
      metrics: null,
      stageHashes: {
        geometry: null,
        roomPlacement: null,
        routing: traceRoutesHash(evidence?.trace),
        validation: null,
      },
    };
  }
  const generation = response.body.catalogAwareGeneration;
  return {
    status: 'accepted',
    classification: generation === null
      ? response.body.placement.corridorRealization
      : 'catalog_aware_exact_assembly',
    outputHash: generation?.trace.finalOutputHash ?? null,
    traceRootHash: generation?.trace.rootHash ?? null,
    metrics: metricsForResponse(response),
    stageHashes: {
      geometry: geometryProjectionHash(response.body.geometry),
      roomPlacement: sha256Json(roomInstances(response.body.placement)),
      routing: generation === null
        ? sha256Json({
          corridors: response.body.geometry.corridors,
          connectionCells: response.body.placement.connectionCells,
        })
        : traceRoutesHash(generation.trace),
      validation: validationOutcomeHash(response.body),
    },
  };
}

function metricsForResponse(response) {
  if (!response.response.ok) {
    return null;
  }
  return metricsForRun(
    {
      geometry: response.body.geometry,
      placement: response.body.placement,
      attempts: response.body.catalogAwareGeneration?.attempts ?? [],
      selectedAttempt: response.body.catalogAwareGeneration?.selectedAttempt ?? null,
    },
    response.body.catalogAwareGeneration?.trace ?? null,
  );
}

function metricsForRun(result, trace) {
  const geometry = result.geometry;
  const placement = result.placement;
  const roomBounds = boundsOfRects(geometry.rooms.map((room) => room.rect));
  const placementBounds = boundsOfCells(placement.occupiedCells);
  const selectedAttempt = result.selectedAttempt;
  const selectedEvidence = selectedAttempt === null
    ? null
    : result.attempts[selectedAttempt];
  const routes = trace === null
    ? []
    : trace.events
      .filter((event) =>
        event.attempt === selectedAttempt
        && event.body.type === 'section_routing_finished'
        && event.body.status === 'found')
      .map((event) => event.body.cells);
  const occupied = new Set(
    placement.occupiedCells.map((cell) => `${cell.x},${cell.y}`),
  ).size;
  const placementArea = placementBounds.width * placementBounds.height;
  return {
    geometryWidth: geometry.bounds.width,
    geometryHeight: geometry.bounds.height,
    geometrySpan: geometry.bounds.width + geometry.bounds.height,
    roomEnvelopeWidth: roomBounds.width,
    roomEnvelopeHeight: roomBounds.height,
    roomEnvelopeArea: roomBounds.width * roomBounds.height,
    placementSpan: {
      width: placementBounds.width,
      height: placementBounds.height,
      total: placementBounds.width + placementBounds.height,
    },
    placementArea,
    occupiedCells: occupied,
    occupiedFillBasisPoints: placementArea === 0
      ? 0
      : Math.floor((occupied * 10_000) / placementArea),
    corridorCenterline: corridorCenterlineLength(geometry),
    routedShellCells: placement.connectionCells.length,
    routedCatalogCells: routes.reduce((total, cells) => total + cells.length, 0),
    routeBends: routes.reduce((total, cells) => total + countBends(cells), 0),
    routingStates: selectedEvidence?.routingStates ?? 0,
    roomsPlaced: selectedEvidence?.roomsPlaced ?? roomInstances(placement).length,
    sectionsRouted: selectedEvidence?.sectionsRouted ?? 0,
    attemptCount: result.attempts.length,
    selectedAttempt,
    layoutSpacingTier: geometry.layoutSearch.spacingTier,
    layoutEmbeddingId: geometry.layoutSearch.embeddingId,
    validLayoutCandidates: geometry.layoutSearch.validLayoutCandidates,
  };
}

function numericDeltas(baseline, current) {
  const deltas = {};
  for (const key of [
    'geometryWidth',
    'geometryHeight',
    'geometrySpan',
    'roomEnvelopeWidth',
    'roomEnvelopeHeight',
    'roomEnvelopeArea',
    'placementArea',
    'occupiedCells',
    'occupiedFillBasisPoints',
    'corridorCenterline',
    'routedShellCells',
    'routedCatalogCells',
    'routeBends',
    'routingStates',
    'roomsPlaced',
    'sectionsRouted',
    'attemptCount',
    'validLayoutCandidates',
  ]) {
    deltas[key] = current[key] - baseline[key];
  }
  deltas.placementWidth = current.placementSpan.width - baseline.placementSpan.width;
  deltas.placementHeight = current.placementSpan.height - baseline.placementSpan.height;
  deltas.placementSpan = current.placementSpan.total - baseline.placementSpan.total;
  return deltas;
}

function allZero(deltas) {
  return deltas !== null && Object.values(deltas).every((value) => value === 0);
}

function firstTraceDifference(baseline, probe) {
  if (baseline === null && probe === null) {
    return null;
  }
  if (baseline === null || probe === null) {
    return {
      index: null,
      baselineType: baseline === null ? null : 'trace_present',
      probeType: probe === null ? null : 'trace_present',
    };
  }
  const length = Math.max(baseline.events.length, probe.events.length);
  for (let index = 0; index < length; index += 1) {
    const left = baseline.events[index];
    const right = probe.events[index];
    if (
      left?.body.type === right?.body.type
      && ['input_bound', 'validation_completed', 'run_finished']
        .includes(left?.body.type)
    ) {
      continue;
    }
    if (JSON.stringify(left?.body) !== JSON.stringify(right?.body)) {
      return {
        index,
        baselineType: left?.body.type ?? null,
        probeType: right?.body.type ?? null,
      };
    }
  }
  return null;
}

function traceFromResponse(response) {
  return response.body.catalogAwareGeneration?.trace
    ?? response.body.evidence?.trace
    ?? null;
}

function traceRoutesHash(trace) {
  if (trace === undefined || trace === null) {
    return null;
  }
  return sha256Json(trace.events
    .filter((event) =>
      event.body.type === 'section_routing_started'
      || event.body.type === 'section_routing_finished')
    .map((event) => ({ attempt: event.attempt, body: event.body })));
}

function geometryProjectionHash(geometry) {
  return sha256Json({
    bounds: geometry.bounds,
    rooms: geometry.rooms,
    corridors: geometry.corridors,
    contents: geometry.contents,
    skippedConnectors: geometry.skippedConnectors,
    selectedEmbedding: geometry.layoutSearch.embeddingId,
    spacingTier: geometry.layoutSearch.spacingTier,
  });
}

function validationOutcomeHash(response) {
  const project = (report) => ({
    ok: report.ok,
    diagnostics: report.diagnostics.map((diagnostic) => ({
      code: diagnostic.code,
      severity: diagnostic.severity,
    })),
  });
  return sha256Json({
    geometry: project(response.geometryValidation),
    placement: project(response.placementValidation),
    builtFlow: project(response.builtFlowValidation),
  });
}

function roomInstances(placement) {
  return placement.instances.filter((instance) =>
    !['connector', 'corridor', 'bend', 'junction']
      .includes(instance.requirementKind));
}

function corridorCenterlineLength(geometry) {
  return geometry.corridors.reduce(
    (total, corridor) =>
      total + corridor.points.slice(1).reduce(
        (length, point, index) =>
          length
          + Math.abs(point.x - corridor.points[index].x)
          + Math.abs(point.y - corridor.points[index].y),
        0,
      ),
    0,
  );
}

function countBends(cells) {
  let bends = 0;
  for (let index = 2; index < cells.length; index += 1) {
    const first = cells[index - 2];
    const middle = cells[index - 1];
    const last = cells[index];
    const priorDirection = `${middle.x - first.x},${middle.y - first.y}`;
    const nextDirection = `${last.x - middle.x},${last.y - middle.y}`;
    if (priorDirection !== nextDirection) {
      bends += 1;
    }
  }
  return bends;
}

function boundsOfRects(rects) {
  return boundsFromExtents(
    rects.map((rect) => ({
      minX: rect.x,
      maxX: rect.x + rect.width,
      minY: rect.y,
      maxY: rect.y + rect.height,
    })),
  );
}

function boundsOfCells(cells) {
  return boundsFromExtents(
    cells.map((cell) => ({
      minX: cell.x,
      maxX: cell.x + 1,
      minY: cell.y,
      maxY: cell.y + 1,
    })),
  );
}

function boundsFromExtents(extents) {
  if (extents.length === 0) {
    return { width: 0, height: 0 };
  }
  const minX = Math.min(...extents.map((extent) => extent.minX));
  const maxX = Math.max(...extents.map((extent) => extent.maxX));
  const minY = Math.min(...extents.map((extent) => extent.minY));
  const maxY = Math.max(...extents.map((extent) => extent.maxY));
  return { width: maxX - minX, height: maxY - minY };
}

function settingStage(path) {
  if (path.startsWith('geometryLayoutPolicy.')) {
    return 'geometry_embedding';
  }
  if (path.startsWith('placementPolicy.')) {
    return 'placement_validation';
  }
  if (path.startsWith('catalogAwareGenerationPolicy.')) {
    return 'catalog_room_and_route_search';
  }
  return 'corridor_realization';
}

function assertSameLayoutMetrics(label, left, right) {
  const selected = [
    'geometryWidth',
    'geometryHeight',
    'roomEnvelopeArea',
    'placementArea',
    'occupiedCells',
    'corridorCenterline',
    'routedCatalogCells',
    'routeBends',
    'routingStates',
    'roomsPlaced',
    'sectionsRouted',
  ];
  for (const metric of selected) {
    if (left[metric] !== right[metric]) {
      throw new Error(`${label} differs at ${metric}: ${left[metric]} != ${right[metric]}`);
    }
  }
}

function decodeResponseRun(response) {
  return decodeCatalogGenerationRun(
    response.body.catalogAwareGeneration.trace,
    response.body.catalogAwareGeneration.result,
  );
}

function responseTracePair(label, response) {
  const owner = response.response.ok
    ? response.body.catalogAwareGeneration
    : response.body.evidence;
  if (
    owner?.result?.kind !== 'rusty_procgen.catalog_aware_generation.v2'
    || owner?.trace?.kind !== 'rusty_procgen.catalog_generation_trace.v2'
  ) {
    throw new Error(`${label} did not return a complete catalog result/trace pair`);
  }
  decodeCatalogGenerationRun(owner.trace, owner.result);
  return { result: owner.result, trace: owner.trace };
}

function assertNamedOutcome(definition, result) {
  const outcome = result.ok === true ? 'accepted' : 'rejected';
  if (outcome !== definition.expectedOutcome) {
    throw new Error(
      `${definition.id} expected ${definition.expectedOutcome}, got ${outcome}`,
    );
  }
}

function assertSuccessfulCatalogRun(label, response) {
  if (
    !response.response.ok
    || response.body.kind !== 'rusty_procgen.viewer_generation_rebuild.v1'
    || response.body.catalogAwareGeneration === null
    || response.body.placement?.corridorRealization !== 'catalog'
  ) {
    throw new Error(`${label} did not produce an accepted catalog trace: ${JSON.stringify(response.body)}`);
  }
  decodeResponseRun(response);
}

function assertSingleValueChange(baseline, probe, expectedPath) {
  const paths = configValuePaths(baseline)
    .filter((path) =>
      JSON.stringify(valueAtPath(baseline, path))
      !== JSON.stringify(valueAtPath(probe, path)));
  if (paths.length !== 1 || paths[0] !== expectedPath) {
    throw new Error(`probe ${expectedPath} changed ${JSON.stringify(paths)}`);
  }
}

function configValuePaths(config) {
  return [
    ...Object.keys(config.geometryLayoutPolicy)
      .map((key) => `geometryLayoutPolicy.${key}.value`),
    ...Object.keys(config.placementPolicy)
      .map((key) => `placementPolicy.${key}.value`),
    ...Object.keys(config.catalogAwareGenerationPolicy)
      .map((key) => `catalogAwareGenerationPolicy.${key}.value`),
    'corridorRealization.value',
  ];
}

function applyOverrides(config, overrides) {
  const value = structuredClone(config);
  for (const override of overrides) {
    setAtPath(value, override.path, override.value);
  }
  return value;
}

function setAtPath(value, path, next) {
  const parts = path.split('.');
  let target = value;
  for (const part of parts.slice(0, -1)) {
    if (target?.[part] === undefined) {
      throw new Error(`unknown override path ${path}`);
    }
    target = target[part];
  }
  target[parts.at(-1)] = next;
}

function valueAtPath(value, path) {
  return path.split('.').reduce((current, part) => current?.[part], value);
}

function requiredRun(value, id) {
  const run = value.namedRuns.find((candidate) => candidate.id === id);
  if (run === undefined) {
    throw new Error(`missing named run ${id}`);
  }
  return run;
}

function requiredNamedRun(runs, id) {
  const run = runs.get(id);
  if (run === undefined) {
    throw new Error(`missing generated named run ${id}`);
  }
  return run;
}

function assertSuite(value) {
  if (
    value.kind !== 'rusty_procgen.generation_control_characterization_suite.v1'
    || value.schemaVersion !== 1
    || typeof value.sourceSelectionRef !== 'string'
    || value.sourceSelectionRef.length === 0
    || typeof value.sourceBaseConfigRef !== 'string'
    || value.sourceBaseConfigRef.length === 0
    || !Array.isArray(value.namedRuns)
    || value.namedRuns.length !== 3
    || typeof value.influenceMatrix?.candidateId !== 'string'
    || typeof value.influenceMatrix?.baselineRunId !== 'string'
    || !Array.isArray(value.influenceMatrix?.probes)
    || value.influenceMatrix.probes.length !== 26
  ) {
    throw new Error(`${suiteRef} has an invalid characterization contract`);
  }
  const runIds = value.namedRuns.map((run) => run.id);
  if (new Set(runIds).size !== runIds.length) {
    throw new Error(`${suiteRef} has duplicate named run ids`);
  }
  for (const run of value.namedRuns) {
    if (
      typeof run.id !== 'string'
      || run.id.length === 0
      || typeof run.label !== 'string'
      || run.label.length === 0
      || typeof run.candidateId !== 'string'
      || run.candidateId.length === 0
      || typeof run.generateFromConfig !== 'boolean'
      || !['accepted', 'rejected'].includes(run.expectedOutcome)
      || !Array.isArray(run.overrides)
      || typeof run.resultRef !== 'string'
      || run.resultRef.length === 0
      || typeof run.traceRef !== 'string'
      || run.traceRef.length === 0
      || run.overrides.some((override) =>
        typeof override?.path !== 'string'
        || override.path.length === 0
        || !Object.hasOwn(override, 'value'))
      || (run.generateFromConfig && run.overrides.length === 0)
      || (!run.generateFromConfig && run.overrides.length !== 0)
    ) {
      throw new Error(`${suiteRef} has an invalid named run ${JSON.stringify(run?.id)}`);
    }
  }
  const probes = value.influenceMatrix.probes.map((probe) => probe.path);
  if (value.influenceMatrix.probes.some((probe) =>
    typeof probe?.path !== 'string'
    || probe.path.length === 0
    || !Object.hasOwn(probe, 'value'))
  ) {
    throw new Error(`${suiteRef} has an invalid influence probe`);
  }
  if (new Set(probes).size !== probes.length) {
    throw new Error(`${suiteRef} has duplicate probes`);
  }
  requiredRun(value, value.influenceMatrix.baselineRunId);
}

function assertMatrixCoverage(value, config) {
  const expected = configValuePaths(config).sort();
  const actual = value.influenceMatrix.probes
    .map((probe) => probe.path)
    .sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${suiteRef} does not probe every editable config value exactly once`);
  }
}

async function readFixtureRun(definition) {
  const [result, trace] = await Promise.all([
    readJson(definition.resultRef),
    readJson(definition.traceRef),
  ]);
  if (result.candidateId !== definition.candidateId) {
    throw new Error(`${definition.resultRef} does not belong to ${definition.candidateId}`);
  }
  return { result, trace };
}

async function rebuild(candidateId, config) {
  const response = await fetch(`${baseUrl}/api/generation-config/rebuild`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ candidateId, config }),
  });
  return { response, body: await response.json() };
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
      // The server is still starting.
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 100));
  }
  throw new Error('generation control report server did not start');
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
