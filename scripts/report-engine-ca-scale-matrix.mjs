#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile, rename, writeFile } from 'node:fs/promises';
import { arch, platform, tmpdir } from 'node:os';
import { join } from 'node:path';

import { decodeCaBenchmarkEvidence } from '../dist/ts/src/ca-authority-trace.js';

const check = process.argv.slice(2).includes('--check');
const benchmarkPath = 'artifacts/evidence/engine-ca-benchmark.json';
const outputPath = 'artifacts/evidence/engine-ca-scale-matrix.json';
const reportPath = 'docs/rusty-engine-ca-scale-baseline.md';
const viewerReportPath = process.env.VIEWER_SMOKE_OUT === undefined
  ? join(tmpdir(), 'rusty-procgen-viewer-smoke', 'viewer-smoke-report.json')
  : join(process.env.VIEWER_SMOKE_OUT, 'viewer-smoke-report.json');
const benchmarkBytes = await readFile(benchmarkPath);
const benchmark = decodeCaBenchmarkEvidence(JSON.parse(benchmarkBytes));
const fixtures = JSON.parse(await readFile('fixtures/ca/scenarios.v1.json', 'utf8'));

if (check) {
  const scale = JSON.parse(await readFile(outputPath, 'utf8'));
  validateScaleEvidence(scale, benchmark, benchmarkBytes, fixtures);
  const expectedReport = renderReport(scale);
  const actualReport = await readFile(reportPath, 'utf8');
  if (actualReport !== expectedReport) {
    fail(`${reportPath} does not match the checked scale evidence`);
  }
  console.log(
    `Engine CA scale matrix check passed (${scale.matrix.scenarios.length} scenarios, `
      + `${scale.source.benchmarkSha256}, ${scale.browser.chromiumVersion})`,
  );
  process.exit(0);
}

const viewerReport = JSON.parse(await readFile(viewerReportPath, 'utf8'));
const scale = buildScaleEvidence(
  benchmark,
  benchmarkBytes,
  fixtures,
  viewerReport,
  chromiumVersion(),
);
validateScaleEvidence(scale, benchmark, benchmarkBytes, fixtures);
await writeAtomic(outputPath, `${JSON.stringify(scale, null, 2)}\n`);
await writeAtomic(reportPath, renderReport(scale));
console.log(
  `Engine CA scale matrix: ${scale.matrix.scenarios.length} scenarios; wrote `
    + `${outputPath} and ${reportPath}`,
);

function buildScaleEvidence(evidence, bytes, scenarioSuite, viewerReport, chromium) {
  const fixturesById = new Map(
    scenarioSuite.scenarios.map((scenario) => [scenario.id, scenario]),
  );
  const browserById = browserMeasurements(viewerReport.caTraceTab);
  const artifactBytes = bytes.length;
  const scenarios = evidence.scenarios.map((scenario) => {
    const fixture = fixturesById.get(scenario.scenarioId);
    if (fixture === undefined) {
      fail(`missing fixture for ${scenario.scenarioId}`);
    }
    const browser = browserById.get(scenario.scenarioId);
    if (browser === undefined) {
      fail(`browser report is missing ${scenario.scenarioId}`);
    }
    return buildScenarioRow(scenario, fixture, artifactBytes, browser);
  });
  return {
    kind: 'rusty_procgen.evidence.engine_ca_scale_matrix.v1',
    schemaVersion: 1,
    source: {
      benchmarkKind: evidence.kind,
      benchmarkSha256: `sha256:${sha256(bytes)}`,
      benchmarkRepositoryCommit: evidence.repositoryCommit,
      viewerRepositoryCommit: gitHead(),
      engineCommit: evidence.engineCommit,
      benchmarkEnvironment: evidence.environment,
      benchmarkConfig: evidence.config,
    },
    matrix: {
      dimensions: [
        'resident_volume',
        'initial_active_density',
        'changed_cell_density',
        'step_count',
        'boundary_policy',
        'mesh_surface_complexity',
      ],
      scenarios,
    },
    browser: {
      chromiumVersion: chromium,
      operatingSystem: platform(),
      architecture: arch(),
      viewport: { width: 1_600, height: 860, deviceScaleFactor: 1 },
      measurementPosture:
        'three real Chromium first-step interaction samples per scenario; descriptive and non-gating',
      representativePlayback: [
        'sparse-propagation',
        'dense-churn',
        'cross-boundary',
        'large-resident-small-hot-region',
        'high-surface-area',
      ],
      deterministicReset: viewerReport.caTraceTab.deterministicReset === true,
      obsoleteResourcesReleased:
        viewerReport.caTraceTab.obsoleteResourcesReleased === true,
      boundedStepSelection: viewerReport.caTraceTab.boundedStepSelection,
      visualSmoothness: {
        status: 'not_measured',
        detail:
          'The trace has 4-6 discrete steps and no frame-pacing sampler; successful interaction is not a smoothness claim.',
      },
    },
    findings: findings(scenarios),
    nonClaims: [
      'Timing samples are observations from the declared hosts, never equality gates.',
      'The five fixtures vary multiple dimensions and do not establish single-factor causality.',
      'No memory allocation, dirty-region, GPU utilization, transfer-network, or frame-pacing measurement was taken.',
      'Browser presentation time is interaction-to-two-animation-frames, not GPU completion time.',
      'This bounded matrix is neither an Engine scale ceiling nor a gameplay runtime benchmark.',
    ],
  };
}

function buildScenarioRow(scenario, fixture, artifactBytes, browser) {
  const volume = boundsVolume(scenario.trace.bounds);
  const steps = scenario.trace.steps;
  const initialCells = fixture.initialCells.length;
  const changed = steps.map((step) => step.ca.changedCellCount);
  const evaluated = steps.map((step) => step.ca.evaluatedCellCount);
  const active = steps.map((step) => step.ca.activeCellCount);
  const projectionOps = steps.map((step) => step.projectionOps.length);
  const readouts = [scenario.trace.initial.readout, ...steps.map((step) => step.readout)];
  const facesCulled = [
    ...scenario.trace.initial.projectionChunks.map((chunk) => chunk.facesCulled),
    ...steps.flatMap((step) => step.projectionOps
      .filter((op) => op.op === 'upsert')
      .map((op) => op.chunk.facesCulled)),
  ];
  const admissionPhaseNames = [
    'stateMaterializationNs',
    'engineBuildNs',
    'evidenceReadbackNs',
    'artifactEncodingNs',
  ];
  const stepPhaseNames = [
    'caStepNs',
    'requestConstructionNs',
    'spatialPreviewNs',
    'authorityCommitNs',
    'evidenceReadbackNs',
    'artifactEncodingNs',
  ];
  const admission = Object.fromEntries(admissionPhaseNames.map((phase) => [
    phase,
    distribution(scenario.recordedRuns.map((run) => run.admissionTiming[phase])),
  ]));
  const stepTimings = Object.fromEntries(stepPhaseNames.map((phase) => [
    phase,
    distribution(scenario.recordedRuns.flatMap((run) =>
      run.stepTimings.map((timing) => timing[phase]))),
  ]));
  const endToEndSamples = scenario.recordedRuns.flatMap((run) =>
    run.stepTimings.map((timing) =>
      stepPhaseNames.reduce((total, phase) => total + timing[phase], 0)));
  const throughputSamples = scenario.recordedRuns.flatMap((run) =>
    run.stepTimings.flatMap((timing, index) => {
      const duration = stepPhaseNames.reduce(
        (total, phase) => total + timing[phase],
        0,
      );
      const changedCells = changed[index] ?? 0;
      return duration === 0 || changedCells === 0
        ? []
        : [changedCells * 1_000_000_000 / duration];
    }));
  const encodedTraceBytes = scenario.recordedRuns[0]?.encodedTraceBytes ?? 0;
  if (
    scenario.recordedRuns.some((run) =>
      run.encodedTraceBytes !== encodedTraceBytes)
  ) {
    fail(`${scenario.scenarioId} encoded trace bytes differ across repeats`);
  }
  return {
    scenarioId: scenario.scenarioId,
    workload: scenario.trace.workload,
    ruleId: scenario.trace.ruleId,
    bounds: scenario.trace.bounds,
    volume,
    steps: steps.length,
    neighborhood: scenario.trace.neighborhood,
    boundary: scenario.trace.boundary,
    materializeEmpty: scenario.trace.materializeEmpty,
    ca: {
      initialCells,
      initialActiveDensity: ratio(initialCells, volume),
      maximumActiveCells: Math.max(initialCells, ...active),
      maximumActiveDensity: ratio(Math.max(initialCells, ...active), volume),
      changedCells: distribution(changed),
      changedCellDensity: distribution(changed.map((count) => ratio(count, volume))),
      evaluatedCells: distribution(evaluated),
      totalChangedCells: changed.reduce((total, count) => total + count, 0),
    },
    engine: {
      initialAuthorityVoxels: scenario.trace.initial.readout.solidVoxelCount,
      maximumAuthorityVoxels: Math.max(...readouts.map((readout) => readout.solidVoxelCount)),
      maximumResidentChunks: Math.max(...readouts.map((readout) => readout.residentChunkCount)),
      maximumMeshChunks: Math.max(...readouts.map((readout) => readout.meshChunkCount)),
      maximumMeshVertices: Math.max(...readouts.map((readout) => readout.meshVertexCount)),
      maximumMeshQuads: Math.max(...readouts.map((readout) => readout.meshQuadCount)),
      maximumFacesCulledInPublishedChunk: Math.max(0, ...facesCulled),
      projectionOperations: distribution(projectionOps),
      finalAuthorityHash: readouts.at(-1).authorityHash,
      finalProjectionHash: readouts.at(-1).meshProjectionHash,
    },
    transfer: {
      encodedTraceBytes,
      shareOfBenchmarkArtifact: ratio(encodedTraceBytes, artifactBytes),
    },
    timings: {
      posture: 'same_host_observation_non_gating',
      admission,
      step: stepTimings,
      endToEndNs: distribution(endToEndSamples),
      changedCellsPerSecond: distribution(throughputSamples),
    },
    browser,
    structuralHash: scenario.recordedRuns[0]?.structuralHash,
    deterministicAcrossRecordedRuns: scenario.deterministicStructuralEvidence,
  };
}

function browserMeasurements(traceReport) {
  if (
    traceReport?.authority !== 'captured_engine_trace'
    || traceReport?.timingRole !== 'observational_non_gating'
  ) {
    fail('viewer report is not captured non-gating CA evidence');
  }
  const values = new Map();
  values.set('sparse-propagation', browserRow(
    traceReport.sparse,
    traceReport.sparse.stepTraceHash,
  ));
  for (const [scenarioId, readout] of Object.entries(traceReport.scenarios ?? {})) {
    values.set(scenarioId, browserRow(readout, readout.stepTraceHash));
  }
  return values;
}

function browserRow(readout, traceHash) {
  if (!Array.isArray(readout?.timingSamples) || readout.timingSamples.length !== 3) {
    fail('viewer report must contain three browser timing samples');
  }
  const observations = readout.timingSamples.map((sample) => {
    for (const field of [
      'rendererApplicationMs',
      'renderSubmissionMs',
      'browserPresentationMs',
    ]) {
      if (!Number.isFinite(sample?.[field]) || sample[field] < 0) {
        fail(`viewer report has invalid ${field}`);
      }
    }
    return sample;
  });
  return {
    sampledStep: 1,
    traceHash,
    observations,
    rendererApplicationMs: distribution(
      observations.map((sample) => sample.rendererApplicationMs),
    ),
    renderSubmissionMs: distribution(
      observations.map((sample) => sample.renderSubmissionMs),
    ),
    browserPresentationMs: distribution(
      observations.map((sample) => sample.browserPresentationMs),
    ),
  };
}

function validateScaleEvidence(scale, benchmark, benchmarkBytes, scenarioSuite) {
  if (
    scale?.kind !== 'rusty_procgen.evidence.engine_ca_scale_matrix.v1'
    || scale.schemaVersion !== 1
  ) {
    fail('scale evidence identity is invalid');
  }
  const expectedBenchmarkHash = `sha256:${sha256(benchmarkBytes)}`;
  const expectedDimensions = [
    'resident_volume',
    'initial_active_density',
    'changed_cell_density',
    'step_count',
    'boundary_policy',
    'mesh_surface_complexity',
  ];
  const expectedPlayback = benchmark.scenarios.map((scenario) => scenario.scenarioId);
  if (
    scale.source?.benchmarkSha256 !== expectedBenchmarkHash
    || scale.source.benchmarkKind !== benchmark.kind
    || scale.source.benchmarkRepositoryCommit !== benchmark.repositoryCommit
    || scale.source.engineCommit !== benchmark.engineCommit
    || JSON.stringify(scale.source.benchmarkEnvironment) !== JSON.stringify(benchmark.environment)
    || JSON.stringify(scale.source.benchmarkConfig) !== JSON.stringify(benchmark.config)
    || !/^[0-9a-f]{40}$/.test(scale.source.viewerRepositoryCommit)
    || !isAncestor(scale.source.viewerRepositoryCommit)
  ) {
    fail('scale evidence source pins do not match the benchmark artifact');
  }
  if (
    !Array.isArray(scale.matrix?.scenarios)
    || scale.matrix.scenarios.length !== benchmark.scenarios.length
    || JSON.stringify(scale.matrix.dimensions) !== JSON.stringify(expectedDimensions)
    || JSON.stringify(scale.browser?.representativePlayback) !== JSON.stringify(expectedPlayback)
    || scale.browser?.visualSmoothness?.status !== 'not_measured'
    || scale.browser.deterministicReset !== true
    || scale.browser.obsoleteResourcesReleased !== true
  ) {
    fail('scale evidence matrix or browser proof is incomplete');
  }
  const fixturesById = new Map(
    scenarioSuite.scenarios.map((scenario) => [scenario.id, scenario]),
  );
  for (const row of scale.matrix.scenarios) {
    const benchmarkScenario = benchmark.scenarios.find(
      (scenario) => scenario.scenarioId === row.scenarioId,
    );
    const fixture = fixturesById.get(row.scenarioId);
    if (
      benchmarkScenario === undefined
      || fixture === undefined
    ) {
      fail(`scale evidence row ${row.scenarioId} diverges from benchmark structure`);
    }
    validateBrowserRow(row.browser, benchmarkScenario.trace.steps[0]?.traceHash);
    const expected = buildScenarioRow(
      benchmarkScenario,
      fixture,
      benchmarkBytes.length,
      row.browser,
    );
    if (JSON.stringify(row) !== JSON.stringify(expected)) {
      fail(`scale evidence row ${row.scenarioId} diverges from benchmark derivation`);
    }
    for (const value of numericLeaves({ ...row, bounds: undefined })) {
      if (!Number.isFinite(value) || value < 0) {
        fail(`scale evidence row ${row.scenarioId} has invalid numeric data`);
      }
    }
  }
  if (JSON.stringify(scale.findings) !== JSON.stringify(findings(scale.matrix.scenarios))) {
    fail('scale findings diverge from the checked matrix');
  }
}

function validateBrowserRow(row, traceHash) {
  if (
    row?.sampledStep !== 1
    || row.traceHash !== traceHash
    || !Array.isArray(row.observations)
    || row.observations.length !== 3
  ) {
    fail('browser timing row is incomplete');
  }
  const expected = browserRow({ timingSamples: row.observations }, traceHash);
  if (JSON.stringify(row) !== JSON.stringify(expected)) {
    fail('browser timing distributions diverge from their observations');
  }
}

function findings(scenarios) {
  const slowest = maximumBy(
    scenarios,
    (scenario) => scenario.timings.endToEndNs.median,
  );
  const largestTransfer = maximumBy(
    scenarios,
    (scenario) => scenario.transfer.encodedTraceBytes,
  );
  const largestResident = maximumBy(
    scenarios,
    (scenario) => scenario.volume,
  );
  const highestSurface = maximumBy(
    scenarios,
    (scenario) => scenario.engine.maximumMeshQuads,
  );
  return {
    slowestMedianStep: {
      scenarioId: slowest.scenarioId,
      medianNs: slowest.timings.endToEndNs.median,
      dominantMeasuredPhase: dominantPhase(slowest.timings.step),
    },
    largestEncodedTrace: {
      scenarioId: largestTransfer.scenarioId,
      bytes: largestTransfer.transfer.encodedTraceBytes,
    },
    largestResidentScope: {
      scenarioId: largestResident.scenarioId,
      volume: largestResident.volume,
      residentChunks: largestResident.engine.maximumResidentChunks,
    },
    highestPublishedMeshSurface: {
      scenarioId: highestSurface.scenarioId,
      quads: highestSurface.engine.maximumMeshQuads,
    },
    optimizationDisposition:
      'No optimization task created: this first matrix exposes associations but does not isolate a production bottleneck from benchmark readback/encoding overhead.',
  };
}

function renderReport(scale) {
  const rows = scale.matrix.scenarios.map((scenario) => [
    scenario.scenarioId,
    String(scenario.volume),
    percent(scenario.ca.maximumActiveDensity),
    `${scenario.ca.changedCells.median}/${scenario.ca.changedCells.max}`,
    `${scenario.engine.maximumResidentChunks}/${scenario.engine.maximumMeshQuads}`,
    formatDuration(scenario.timings.endToEndNs.median),
    Math.round(scenario.timings.changedCellsPerSecond.median).toLocaleString('en-US'),
    formatBytes(scenario.transfer.encodedTraceBytes),
    `${scenario.browser.rendererApplicationMs.median.toFixed(3)} / ${scenario.browser.browserPresentationMs.median.toFixed(3)} ms`,
  ]);
  return `# Rusty Engine CA scale baseline

Status: checked first baseline for \`${scale.source.benchmarkRepositoryCommit}\` against Rusty Engine \`${scale.source.engineCommit}\`.

## Reproduce and validate

\`\`\`bash
pnpm run engine:ca:scale
pnpm run engine:ca:scale:check
\`\`\`

The first command regenerates the release benchmark, runs its real Chromium
consumer, and rewrites the versioned matrix plus this report. The check command
recomputes deterministic summaries and source hashes without treating timings
as equality gates.

## Matrix

| Scenario | Resident cells | Peak CA density | Changed median/max | Resident chunks/max quads | Median measured step | Changed cells/s | Trace bytes | Browser apply/two-frame |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
${rows.map((row) => `| ${row.join(' | ')} |`).join('\n')}

Rust uses ${scale.source.benchmarkConfig.warmupRuns} warmup and
${scale.source.benchmarkConfig.recordedRuns} recorded runs on
${scale.source.benchmarkEnvironment.operatingSystem}/${
  scale.source.benchmarkEnvironment.architecture} with
\`${scale.source.benchmarkEnvironment.rustcVersion}\`. Structural hashes agree
across repeats. Browser values summarize three descriptive first-step
interaction samples per scenario on \`${scale.browser.chromiumVersion}\`; they
are not thresholds.

## Findings

- Slowest median measured step:
  \`${scale.findings.slowestMedianStep.scenarioId}\` at
  ${formatDuration(scale.findings.slowestMedianStep.medianNs)}; its largest
  measured phase was
  \`${scale.findings.slowestMedianStep.dominantMeasuredPhase}\`.
- Largest encoded trace:
  \`${scale.findings.largestEncodedTrace.scenarioId}\` at
  ${formatBytes(scale.findings.largestEncodedTrace.bytes)}.
- Largest resident scope:
  \`${scale.findings.largestResidentScope.scenarioId}\` at
  ${scale.findings.largestResidentScope.volume.toLocaleString('en-US')} cells
  and ${scale.findings.largestResidentScope.residentChunks} resident chunks.
- Highest published mesh surface:
  \`${scale.findings.highestPublishedMeshSurface.scenarioId}\` at
  ${scale.findings.highestPublishedMeshSurface.quads.toLocaleString('en-US')}
  quads.
- ${scale.findings.optimizationDisposition}

These are workload associations, not single-factor causal conclusions. The
matrix distinguishes latency, changed-cell throughput, encoded transfer size,
resident scope, update density, and mesh surface. Browser playback proves that
sparse and stress traces remain interactive, but visual smoothness is
**not measured** because the bounded 4–6-step traces do not include a
frame-pacing sampler.

## Nonclaims

${scale.nonClaims.map((claim) => `- ${claim}`).join('\n')}
`;
}

function distribution(values) {
  if (values.length === 0 || values.some((value) => !Number.isFinite(value))) {
    fail('cannot summarize an empty or non-finite distribution');
  }
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  const median = sorted.length % 2 === 0
    ? ((sorted[middle - 1] ?? 0) + (sorted[middle] ?? 0)) / 2
    : sorted[middle] ?? 0;
  return {
    samples: sorted.length,
    min: sorted[0],
    median,
    p95: sorted[Math.max(0, Math.ceil(sorted.length * 0.95) - 1)],
    max: sorted.at(-1),
  };
}

function dominantPhase(phases) {
  return Object.entries(phases)
    .sort((left, right) => right[1].median - left[1].median)[0]?.[0] ?? 'none';
}

function maximumBy(values, score) {
  return values.reduce((selected, candidate) =>
    score(candidate) > score(selected) ? candidate : selected);
}

function boundsVolume(bounds) {
  return (bounds.maxExclusive.x - bounds.min.x)
    * (bounds.maxExclusive.y - bounds.min.y)
    * (bounds.maxExclusive.z - bounds.min.z);
}

function ratio(numerator, denominator) {
  return denominator === 0 ? 0 : numerator / denominator;
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function gitHead() {
  return execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
}

function isAncestor(commit) {
  try {
    execFileSync('git', ['merge-base', '--is-ancestor', commit, 'HEAD']);
    return true;
  } catch {
    return false;
  }
}

function chromiumVersion() {
  for (const executable of ['chromium', 'chromium-browser', 'google-chrome']) {
    try {
      return execFileSync(executable, ['--version'], { encoding: 'utf8' }).trim();
    } catch {
      // Try the next public executable name.
    }
  }
  fail('Chromium is required to publish the browser scale observation');
}

function numericLeaves(value) {
  if (typeof value === 'number') {
    return [value];
  }
  if (Array.isArray(value)) {
    return value.flatMap(numericLeaves);
  }
  if (value !== null && typeof value === 'object') {
    return Object.values(value).flatMap(numericLeaves);
  }
  return [];
}

function percent(value) {
  return `${(value * 100).toFixed(2)}%`;
}

function formatDuration(nanoseconds) {
  return nanoseconds >= 1_000_000
    ? `${(nanoseconds / 1_000_000).toFixed(3)} ms`
    : `${(nanoseconds / 1_000).toFixed(3)} µs`;
}

function formatBytes(bytes) {
  return bytes >= 1_048_576
    ? `${(bytes / 1_048_576).toFixed(2)} MiB`
    : `${(bytes / 1_024).toFixed(2)} KiB`;
}

async function writeAtomic(path, value) {
  const staging = `${path}.staging`;
  await writeFile(staging, value, 'utf8');
  await rename(staging, path);
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
