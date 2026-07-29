import {
  mountRendererInspectionSurface,
  type RendererInspectionSurface,
} from '@rusty-engine/renderer-host';

import {
  compileCaScenario,
  decodeCaBenchmarkEvidence,
  type CaBenchmarkEvidence,
  type CaRecordedRun,
  type CaScenarioEvidence,
  type CaSpatialStep,
  type CompiledCaScenario,
} from '../src/ca-authority-trace.js';

export interface CaTraceViewerElements {
  readonly panel: HTMLElement;
  readonly canvas: HTMLCanvasElement;
  readonly diagnostic: HTMLElement;
  readonly scenario: HTMLSelectElement;
  readonly run: HTMLSelectElement;
  readonly rate: HTMLSelectElement;
  readonly play: HTMLButtonElement;
  readonly step: HTMLButtonElement;
  readonly reset: HTMLButtonElement;
  readonly seek: HTMLInputElement;
  readonly stepLabel: HTMLElement;
  readonly metrics: HTMLElement;
  readonly timings: HTMLElement;
}

export interface CaTraceViewer {
  readonly activate: () => Promise<void>;
  readonly deactivate: () => void;
  readonly dispose: () => void;
}

interface CaTraceViewerOptions {
  readonly renderOnce?: boolean;
}

const EVIDENCE_URL = '/api/evidence/engine-ca-benchmark';

export function createCaTraceViewer(
  elements: CaTraceViewerElements,
  options: CaTraceViewerOptions = {},
): CaTraceViewer {
  let evidence: CaBenchmarkEvidence | null = null;
  let compiledScenarios: readonly CompiledCaScenario[] = [];
  let selected: CompiledCaScenario | null = null;
  let surface: RendererInspectionSurface | null = null;
  let mount: Promise<RendererInspectionSurface> | null = null;
  let active = false;
  let disposed = false;
  let loadRevision = 0;
  let stepIndex = 0;
  let playbackTimer: number | null = null;
  let readoutFrame: number | null = null;
  let replacementCount = 0;

  setControlsEnabled(false);
  elements.canvas.addEventListener('pointerdown', () => elements.canvas.focus());
  elements.scenario.addEventListener('change', () => {
    void selectScenario(elements.scenario.value);
  });
  elements.run.addEventListener('change', renderEvidence);
  elements.rate.addEventListener('change', () => {
    if (playbackTimer !== null) {
      pause();
      play();
    }
  });
  elements.play.addEventListener('click', () => {
    if (playbackTimer === null) {
      play();
    } else {
      pause();
    }
  });
  elements.step.addEventListener('click', () => {
    pause();
    advance();
  });
  elements.reset.addEventListener('click', () => {
    pause();
    void seekTo(0);
  });
  elements.seek.addEventListener('input', () => {
    pause();
    void seekTo(Number(elements.seek.value));
  });

  return {
    activate: async () => {
      if (disposed) {
        return;
      }
      active = true;
      if (evidence === null) {
        await load();
      } else if (selected !== null) {
        await mountSelected();
      }
    },
    deactivate: () => {
      active = false;
      pause();
      stopReadoutSync();
      surface?.stop();
    },
    dispose: () => {
      disposed = true;
      active = false;
      loadRevision += 1;
      pause();
      stopReadoutSync();
      surface?.dispose();
      elements.panel.dataset.disposed = String(
        surface?.readout().status === 'disposed',
      );
      surface = null;
      mount = null;
    },
  };

  async function load(): Promise<void> {
    const revision = ++loadRevision;
    setDiagnostic('loading', 'Loading and verifying captured Engine authority traces…');
    try {
      const response = await fetch(EVIDENCE_URL);
      if (!response.ok) {
        throw new Error(`evidence request failed with ${response.status}`);
      }
      const decoded = decodeCaBenchmarkEvidence(await response.json());
      const compiled = decoded.scenarios.map(compileCaScenario);
      if (revision !== loadRevision || disposed) {
        return;
      }
      evidence = decoded;
      compiledScenarios = compiled;
      populateScenarioOptions(decoded.scenarios);
      elements.panel.dataset.repositoryCommit = decoded.repositoryCommit;
      elements.panel.dataset.engineCommit = decoded.engineCommit;
      elements.panel.dataset.authoritySource = 'captured_engine_trace';
      elements.panel.dataset.timingRole = 'observational_non_gating';
      setControlsEnabled(true);
      await selectScenario(compiled[0]?.evidence.scenarioId ?? '');
    } catch (error) {
      if (revision === loadRevision) {
        setDiagnostic('error', `CA trace unavailable: ${describeError(error)}`);
        setControlsEnabled(false);
      }
    }
  }

  async function selectScenario(scenarioId: string): Promise<void> {
    const next = compiledScenarios.find(
      (candidate) => candidate.evidence.scenarioId === scenarioId,
    );
    if (next === undefined) {
      setDiagnostic('error', `CA trace scenario is unavailable: ${scenarioId}`);
      return;
    }
    pause();
    selected = next;
    elements.scenario.value = scenarioId;
    populateRunOptions(next.evidence.recordedRuns);
    elements.seek.min = '0';
    elements.seek.max = String(next.stepFrames.length);
    elements.seek.value = '0';
    stepIndex = 0;
    await mountSelected();
  }

  async function mountSelected(): Promise<void> {
    const scenario = selected;
    if (!active || scenario === null) {
      return;
    }
    const revision = ++loadRevision;
    setDiagnostic('loading', `Mounting ${scenario.evidence.scenarioId} captured projection…`);
    try {
      if (surface === null) {
        mount ??= mountRendererInspectionSurface(elements.canvas, {
          autoStart: false,
          clearColor: 0x0c1219,
          frame: scenario.initialFrame,
          initialGrid: scenario.grid,
          controls: {
            initialPosition: scenario.camera.position,
            initialTarget: scenario.camera.target,
            moveSpeed: scenario.camera.moveSpeed,
            orbitDegreesPerPixel: 0.22,
          },
        });
        surface = await mount;
      } else {
        requireApplied(
          surface.replaceFrame(scenario.initialFrame),
          'captured initial projection',
        );
        requireApplied(surface.setGrid(scenario.grid), 'captured grid');
        surface.focusTarget(scenario.camera.target);
        replacementCount += 1;
      }
      if (
        revision !== loadRevision
        || !active
        || scenario !== selected
        || surface === null
      ) {
        surface?.stop();
        return;
      }
      surface.resizeToCanvas();
      surface.renderOnce();
      if (!options.renderOnce) {
        surface.start();
      }
      stepIndex = 0;
      syncReadout(surface);
      startReadoutSync(surface);
      renderEvidence();
      setDiagnostic(
        'ready',
        `${scenario.evidence.scenarioId} · captured initial authority at revision ${scenario.evidence.trace.initial.readout.sourceRevision}`,
      );
    } catch (error) {
      if (revision === loadRevision) {
        setDiagnostic('error', `CA trace renderer rejected the projection: ${describeError(error)}`);
      }
    }
  }

  function play(): void {
    if (selected === null || playbackTimer !== null) {
      return;
    }
    if (stepIndex >= selected.stepFrames.length) {
      void seekTo(0).then(schedule);
      return;
    }
    schedule();
  }

  function schedule(): void {
    const rate = Number(elements.rate.value);
    const delay = Math.max(50, Math.round(1_000 / (Number.isFinite(rate) ? rate : 1)));
    elements.play.textContent = 'Pause';
    elements.play.dataset.state = 'playing';
    playbackTimer = window.setTimeout(() => {
      playbackTimer = null;
      if (!advance()) {
        pause();
        return;
      }
      schedule();
    }, delay);
  }

  function pause(): void {
    if (playbackTimer !== null) {
      window.clearTimeout(playbackTimer);
      playbackTimer = null;
    }
    elements.play.textContent = 'Play';
    elements.play.dataset.state = 'paused';
  }

  function advance(): boolean {
    const scenario = selected;
    if (scenario === null || surface === null || stepIndex >= scenario.stepFrames.length) {
      return false;
    }
    try {
      const frame = scenario.stepFrames[stepIndex];
      requireApplied(
        surface.applyAuthoredFrame(frame),
        `captured step ${stepIndex + 1}`,
      );
      stepIndex += 1;
      elements.seek.value = String(stepIndex);
      surface.renderOnce();
      syncReadout(surface);
      renderEvidence();
      return true;
    } catch (error) {
      pause();
      setDiagnostic('error', `Captured CA step rejected: ${describeError(error)}`);
      return false;
    }
  }

  async function seekTo(target: number): Promise<void> {
    const scenario = selected;
    if (scenario === null || surface === null) {
      return;
    }
    const bounded = Math.max(0, Math.min(scenario.stepFrames.length, Math.trunc(target)));
    try {
      if (bounded < stepIndex) {
        requireApplied(
          surface.replaceFrame(scenario.initialFrame),
          'captured reset projection',
        );
        requireApplied(surface.setGrid(scenario.grid), 'captured reset grid');
        replacementCount += 1;
        stepIndex = 0;
      }
      while (stepIndex < bounded) {
        const frame = scenario.stepFrames[stepIndex];
        requireApplied(surface.applyAuthoredFrame(frame), `captured step ${stepIndex + 1}`);
        stepIndex += 1;
      }
      elements.seek.value = String(stepIndex);
      surface.renderOnce();
      syncReadout(surface);
      renderEvidence();
      setDiagnostic(
        'ready',
        stepIndex === 0
          ? `${scenario.evidence.scenarioId} · reset to captured initial authority`
          : `${scenario.evidence.scenarioId} · captured authority step ${stepIndex}/${scenario.stepFrames.length}`,
      );
    } catch (error) {
      setDiagnostic('error', `Captured CA seek rejected: ${describeError(error)}`);
    }
  }

  function renderEvidence(): void {
    const scenario = selected;
    const documentEvidence = evidence;
    if (scenario === null || documentEvidence === null) {
      return;
    }
    const step = stepIndex === 0 ? null : scenario.evidence.trace.steps[stepIndex - 1] ?? null;
    const readout = step?.readout ?? scenario.evidence.trace.initial.readout;
    const run = selectedRun(scenario.evidence);
    const timing = stepIndex === 0
      ? run.admissionTiming
      : run.stepTimings[stepIndex - 1] ?? null;
    elements.stepLabel.textContent = `Step ${stepIndex} of ${scenario.stepFrames.length}`;
    elements.step.disabled = stepIndex >= scenario.stepFrames.length;
    elements.reset.disabled = stepIndex === 0;
    elements.panel.dataset.scenarioId = scenario.evidence.scenarioId;
    elements.panel.dataset.step = String(stepIndex);
    elements.panel.dataset.run = String(run.run);
    elements.panel.dataset.traceHash = step?.traceHash
      ?? scenario.evidence.trace.initial.traceHash;
    elements.panel.dataset.projectionStateHash = step?.projectionStateHash
      ?? scenario.evidence.trace.initial.projectionStateHash;
    elements.panel.dataset.projectionOpCount = String(step?.projectionOps.length ?? 0);
    elements.panel.dataset.meshChunkCount = String(readout.meshChunkCount);
    elements.panel.dataset.sourceRevision = String(readout.sourceRevision);
    elements.panel.dataset.replacementCount = String(replacementCount);

    const structural: readonly [string, string][] = [
      ['Workload', scenario.evidence.trace.workload],
      ['Rule', scenario.evidence.trace.ruleId],
      ['Boundary', `${scenario.evidence.trace.neighborhood} · ${scenario.evidence.trace.boundary}`],
      ['Authority revision', String(readout.sourceRevision)],
      ['Active / changed cells', step === null
        ? `${readout.solidVoxelCount} / initial`
        : `${step.ca.activeCellCount} / ${step.ca.changedCellCount}`],
      ['Requests / batches', step === null ? 'initial admission / 1' : '1 / 1'],
      ['Accepted / rejected', '1 / 0'],
      ['Canonical / Engine edits', step === null
        ? 'initial admission'
        : `${step.canonicalEditCount} / ${step.engineDeltaCount}`],
      ['Projection ops / chunks', `${step?.projectionOps.length ?? 0} / ${readout.meshChunkCount}`],
      ['Resident bounds', formatBounds(scenario.evidence.trace.bounds)],
      ['Touched bounds', formatTouchedBounds(step)],
      ['Vertices / quads', `${readout.meshVertexCount} / ${readout.meshQuadCount}`],
      ['Authority hash', readout.authorityHash],
      ['Projection hash', readout.meshProjectionHash],
      ['Trace hash', step?.traceHash ?? scenario.evidence.trace.initial.traceHash],
      ['Structural run hash', run.structuralHash],
    ];
    replaceMetrics(elements.metrics, structural);

    const timingEntries: readonly (readonly [string, string])[] = timing === null
      ? [['Timing', 'No timing sample for this step']]
      : Object.entries(timing).map(([name, nanoseconds]) => [
        displayName(name),
        `${formatNanoseconds(nanoseconds)} · observational`,
      ] as const);
    replaceMetrics(elements.timings, [
      ['Recorded run', `${run.run} of ${scenario.evidence.recordedRuns.length}`],
      ['Environment', `${documentEvidence.environment.operatingSystem} · ${documentEvidence.environment.architecture}`],
      ['Rust toolchain', documentEvidence.environment.rustcVersion],
      ...timingEntries,
    ]);
  }

  function syncReadout(target: RendererInspectionSurface): void {
    const readout = target.readout();
    elements.panel.dataset.rendererHost = target.kind;
    elements.panel.dataset.rendererRole = target.role;
    elements.panel.dataset.rendererStatus = readout.status;
    elements.panel.dataset.frameHash = readout.retainedFrameHash;
    elements.panel.dataset.retainedOpCount = String(readout.retainedOpCount);
    elements.panel.dataset.cameraRevision = String(readout.cameraRevision);
    elements.panel.dataset.cameraDistance = readout.cameraDistance.toFixed(3);
    elements.panel.dataset.lastCameraChange = readout.lastCameraChange;
    elements.panel.dataset.gridRevision = String(readout.gridRevision);
    elements.panel.dataset.gridLineCount = String(readout.grid?.renderedLineCount ?? 0);
    elements.panel.dataset.viewportHash = readout.viewportHash;
  }

  function startReadoutSync(target: RendererInspectionSurface): void {
    stopReadoutSync();
    const sync = (): void => {
      if (!active || target !== surface) {
        readoutFrame = null;
        return;
      }
      syncReadout(target);
      readoutFrame = requestAnimationFrame(sync);
    };
    readoutFrame = requestAnimationFrame(sync);
  }

  function stopReadoutSync(): void {
    if (readoutFrame !== null) {
      cancelAnimationFrame(readoutFrame);
      readoutFrame = null;
    }
  }

  function setControlsEnabled(enabled: boolean): void {
    elements.scenario.disabled = !enabled;
    elements.run.disabled = !enabled;
    elements.rate.disabled = !enabled;
    elements.play.disabled = !enabled;
    elements.step.disabled = !enabled;
    elements.reset.disabled = !enabled;
    elements.seek.disabled = !enabled;
  }

  function setDiagnostic(state: 'loading' | 'ready' | 'error', message: string): void {
    elements.diagnostic.dataset.state = state;
    elements.panel.dataset.state = state;
    elements.diagnostic.textContent = message;
  }

  function populateScenarioOptions(scenarios: readonly CaScenarioEvidence[]): void {
    elements.scenario.replaceChildren(...scenarios.map((scenario) => {
      const option = document.createElement('option');
      option.value = scenario.scenarioId;
      option.textContent = `${displayName(scenario.trace.workload)} · ${scenario.scenarioId}`;
      return option;
    }));
  }

  function populateRunOptions(runs: readonly CaRecordedRun[]): void {
    const prior = elements.run.value;
    elements.run.replaceChildren(...runs.map((run) => {
      const option = document.createElement('option');
      option.value = String(run.run);
      option.textContent = `Recorded run ${run.run}`;
      return option;
    }));
    elements.run.value = runs.some((run) => String(run.run) === prior)
      ? prior
      : String(runs[0]?.run ?? 1);
  }

  function selectedRun(scenario: CaScenarioEvidence): CaRecordedRun {
    return scenario.recordedRuns.find(
      (run) => String(run.run) === elements.run.value,
    ) ?? requiredRun(scenario.recordedRuns);
  }
}

function replaceMetrics(target: HTMLElement, entries: readonly (readonly [string, string])[]): void {
  target.replaceChildren(...entries.map(([label, value]) => {
    const metric = document.createElement('div');
    metric.className = 'ca-trace-metric';
    const key = document.createElement('span');
    key.textContent = label;
    const fact = document.createElement('strong');
    fact.textContent = value;
    metric.append(key, fact);
    return metric;
  }));
}

function requireApplied(
  receipt: { readonly applied: boolean; readonly diagnostics: readonly { readonly message: string }[] },
  owner: string,
): void {
  if (!receipt.applied) {
    const detail = receipt.diagnostics.map((diagnostic) => diagnostic.message).join('; ');
    throw new Error(`${owner} was rejected${detail.length > 0 ? `: ${detail}` : ''}`);
  }
}

function requiredRun(runs: readonly CaRecordedRun[]): CaRecordedRun {
  const run = runs[0];
  if (run === undefined) {
    throw new Error('captured scenario has no recorded run');
  }
  return run;
}

function displayName(value: string): string {
  return value
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .replaceAll('_', ' ')
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function formatNanoseconds(value: number): string {
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(3)} ms`;
  }
  if (value >= 1_000) {
    return `${(value / 1_000).toFixed(3)} µs`;
  }
  return `${value} ns`;
}

function formatBounds(bounds: CaScenarioEvidence['trace']['bounds']): string {
  const min = bounds.min;
  const max = bounds.maxExclusive;
  return `[${min.x},${min.y},${min.z})–[${max.x},${max.y},${max.z})`;
}

function formatTouchedBounds(step: CaSpatialStep | null): string {
  const bounds = step?.ca.touchedBounds;
  if (bounds === null || bounds === undefined) {
    return 'none';
  }
  return `[${bounds.min.x},${bounds.min.y},${bounds.min.z}]–[${bounds.maxInclusive.x},${bounds.maxInclusive.y},${bounds.maxInclusive.z}]`;
}

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
