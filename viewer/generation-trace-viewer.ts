import {
  catalogGenerationStageFrames,
  decodeCatalogGenerationRun,
  replayCatalogGenerationAttempt,
  type CatalogGenerationAttempt,
  type CatalogGenerationRoomPlacement,
  type CatalogGenerationVisualState,
  type DecodedCatalogGenerationRun,
} from '../src/catalog-generation-trace.js';

export interface GenerationTraceViewerElements {
  readonly panel: HTMLElement;
  readonly svg: SVGSVGElement;
  readonly diagnostic: HTMLElement;
  readonly run: HTMLSelectElement;
  readonly attempt: HTMLSelectElement;
  readonly rate: HTMLSelectElement;
  readonly play: HTMLButtonElement;
  readonly back: HTMLButtonElement;
  readonly step: HTMLButtonElement;
  readonly reset: HTMLButtonElement;
  readonly previousStage: HTMLButtonElement;
  readonly nextStage: HTMLButtonElement;
  readonly seek: HTMLInputElement;
  readonly stepLabel: HTMLElement;
  readonly metrics: HTMLElement;
  readonly eventDetail: HTMLElement;
}

export interface GenerationTraceViewer {
  readonly activate: () => Promise<void>;
  readonly deactivate: () => void;
  readonly replaceRun: (
    label: string,
    trace: unknown,
    result: unknown,
  ) => Promise<void>;
  readonly dispose: () => void;
}

interface GenerationTraceBundle {
  readonly kind: 'rusty_procgen.catalog_generation_trace_bundle.v1';
  readonly schemaVersion: 1;
  readonly runs: readonly GenerationTraceBundleRun[];
}

interface GenerationTraceBundleRun {
  readonly id: string;
  readonly label: string;
  readonly trace: unknown;
  readonly result: unknown;
}

interface CompiledRun {
  readonly id: string;
  readonly label: string;
  readonly run: DecodedCatalogGenerationRun;
}

const EVIDENCE_URL = '/api/evidence/catalog-generation-runs';
const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';
const MAX_RUNS = 16;

export function createGenerationTraceViewer(
  elements: GenerationTraceViewerElements,
): GenerationTraceViewer {
  let runs: readonly CompiledRun[] = [];
  let selectedRun: CompiledRun | null = null;
  let selectedAttempt: CatalogGenerationAttempt | null = null;
  let active = false;
  let disposed = false;
  let loadRevision = 0;
  let frame = 0;
  let playbackTimer: number | null = null;
  let replacementCount = 0;

  setControlsEnabled(false);
  elements.run.addEventListener('change', () => {
    selectRun(elements.run.value);
  });
  elements.attempt.addEventListener('change', () => {
    selectAttempt(Number(elements.attempt.value));
  });
  elements.rate.addEventListener('change', restartPlayback);
  elements.play.addEventListener('click', togglePlayback);
  elements.back.addEventListener('click', () => {
    pause();
    seekTo(frame - 1);
  });
  elements.step.addEventListener('click', () => {
    pause();
    seekTo(frame + 1);
  });
  elements.reset.addEventListener('click', () => {
    pause();
    seekTo(0);
  });
  elements.previousStage.addEventListener('click', () => {
    pause();
    seekStage(-1);
  });
  elements.nextStage.addEventListener('click', () => {
    pause();
    seekStage(1);
  });
  elements.seek.addEventListener('input', () => {
    pause();
    seekTo(Number(elements.seek.value));
  });
  elements.panel.addEventListener('keydown', handleKeyboard);

  return {
    activate: async () => {
      if (disposed) {
        return;
      }
      active = true;
      if (runs.length === 0) {
        await load();
      } else {
        render();
      }
    },
    deactivate: () => {
      active = false;
      pause();
    },
    replaceRun: async (label, trace, result) => {
      if (disposed) {
        return;
      }
      const revision = ++loadRevision;
      const decoded = decodeCatalogGenerationRun(trace, result);
      const replacement: CompiledRun = {
        id: `live-${decoded.candidateId}-${decoded.outputHash}`,
        label,
        run: decoded,
      };
      await Promise.resolve();
      if (revision !== loadRevision || disposed) {
        return;
      }
      const nextRuns = [
        replacement,
        ...runs.filter((candidate) => !candidate.id.startsWith('live-')),
      ];
      publishRuns(nextRuns, replacement.id);
      replacementCount += 1;
      elements.panel.dataset.replacementCount = String(replacementCount);
    },
    dispose: () => {
      disposed = true;
      active = false;
      loadRevision += 1;
      pause();
      runs = [];
      selectedRun = null;
      selectedAttempt = null;
      elements.svg.replaceChildren();
      elements.panel.dataset.disposed = 'true';
    },
  };

  async function load(): Promise<void> {
    const revision = ++loadRevision;
    setDiagnostic('loading', 'Loading and verifying Rust-owned generation decisions…');
    try {
      const response = await fetch(EVIDENCE_URL);
      if (!response.ok) {
        throw new Error(`generation trace request failed with ${response.status}`);
      }
      const bundle = decodeBundle(await response.json());
      const compiled = bundle.runs.map((entry) => ({
        id: entry.id,
        label: entry.label,
        run: decodeCatalogGenerationRun(entry.trace, entry.result),
      }));
      if (revision !== loadRevision || disposed) {
        return;
      }
      publishRuns(compiled);
    } catch (error) {
      if (revision === loadRevision && !disposed) {
        setDiagnostic('error', `Generation trace unavailable: ${describeError(error)}`);
        setControlsEnabled(false);
      }
    }
  }

  function publishRuns(nextRuns: readonly CompiledRun[], preferredId?: string): void {
    if (nextRuns.length === 0) {
      throw new Error('generation trace bundle contains no runs');
    }
    const priorId = preferredId ?? selectedRun?.id;
    runs = nextRuns;
    elements.run.replaceChildren(...runs.map((candidate) => {
      const option = document.createElement('option');
      option.value = candidate.id;
      option.textContent = candidate.label;
      return option;
    }));
    const target = runs.find((candidate) => candidate.id === priorId) ?? runs[0];
    setControlsEnabled(true);
    selectRun(required(target, 'generation run'));
  }

  function selectRun(target: string | CompiledRun): void {
    const next = typeof target === 'string'
      ? runs.find((candidate) => candidate.id === target)
      : target;
    if (next === undefined) {
      setDiagnostic('error', `Generation run is unavailable: ${String(target)}`);
      return;
    }
    pause();
    selectedRun = next;
    elements.run.value = next.id;
    elements.attempt.replaceChildren(...next.run.attempts.map((attempt) => {
      const option = document.createElement('option');
      option.value = String(attempt.attempt);
      option.textContent = `Attempt ${attempt.attempt + 1} · ${displayName(
        attempt.evidence.classification,
      )}`;
      return option;
    }));
    const preferredAttempt = next.run.selectedAttempt
      ?? next.run.attempts.at(-1)?.attempt
      ?? 0;
    selectAttempt(preferredAttempt);
  }

  function selectAttempt(attemptNumber: number): void {
    const next = selectedRun?.run.attempts.find(
      (candidate) => candidate.attempt === attemptNumber,
    );
    if (next === undefined) {
      setDiagnostic('error', `Generation attempt is unavailable: ${attemptNumber}`);
      return;
    }
    pause();
    selectedAttempt = next;
    elements.attempt.value = String(next.attempt);
    elements.seek.min = '0';
    elements.seek.max = String(next.eventIndices.length);
    frame = 0;
    render();
  }

  function togglePlayback(): void {
    if (playbackTimer === null) {
      play();
    } else {
      pause();
    }
  }

  function play(): void {
    const attempt = selectedAttempt;
    if (attempt === null || playbackTimer !== null) {
      return;
    }
    if (frame >= attempt.eventIndices.length) {
      seekTo(0);
    }
    elements.play.textContent = 'Pause';
    elements.play.dataset.state = 'playing';
    schedule();
  }

  function schedule(): void {
    const rate = Number(elements.rate.value);
    const delay = Math.max(40, Math.round(1_000 / (Number.isFinite(rate) ? rate : 1)));
    playbackTimer = window.setTimeout(() => {
      playbackTimer = null;
      const attempt = selectedAttempt;
      if (!active || attempt === null || frame >= attempt.eventIndices.length) {
        pause();
        return;
      }
      seekTo(frame + 1);
      if (frame >= attempt.eventIndices.length) {
        pause();
      } else {
        schedule();
      }
    }, delay);
  }

  function restartPlayback(): void {
    if (playbackTimer !== null) {
      pause();
      play();
    }
  }

  function pause(): void {
    if (playbackTimer !== null) {
      window.clearTimeout(playbackTimer);
      playbackTimer = null;
    }
    elements.play.textContent = 'Play';
    elements.play.dataset.state = 'paused';
  }

  function seekTo(target: number): void {
    const attempt = selectedAttempt;
    if (attempt === null) {
      return;
    }
    frame = Math.max(0, Math.min(attempt.eventIndices.length, Math.trunc(target)));
    render();
  }

  function seekStage(direction: -1 | 1): void {
    const currentRun = selectedRun;
    const attempt = selectedAttempt;
    if (currentRun === null || attempt === null) {
      return;
    }
    const stages = catalogGenerationStageFrames(currentRun.run, attempt.attempt);
    const target = direction < 0
      ? [...stages].reverse().find((candidate) => candidate < frame)
      : stages.find((candidate) => candidate > frame);
    seekTo(target ?? (direction < 0 ? 0 : attempt.eventIndices.length));
  }

  function render(): void {
    const currentRun = selectedRun;
    const attempt = selectedAttempt;
    if (!active || currentRun === null || attempt === null) {
      return;
    }
    const visual = replayCatalogGenerationAttempt(
      currentRun.run,
      attempt.attempt,
      frame,
    );
    renderSvg(elements.svg, visual);
    renderMetrics(currentRun, attempt, visual);
    elements.seek.value = String(frame);
    elements.stepLabel.textContent = `Decision ${frame} of ${attempt.eventIndices.length}`;
    elements.back.disabled = frame === 0;
    elements.reset.disabled = frame === 0;
    elements.step.disabled = frame >= attempt.eventIndices.length;
    elements.previousStage.disabled = frame === 0;
    elements.nextStage.disabled = frame >= attempt.eventIndices.length;
    elements.panel.dataset.state = 'ready';
    elements.panel.dataset.runId = currentRun.id;
    elements.panel.dataset.candidateId = currentRun.run.candidateId;
    elements.panel.dataset.attempt = String(attempt.attempt);
    elements.panel.dataset.frame = String(frame);
    elements.panel.dataset.frameCount = String(attempt.eventIndices.length);
    elements.panel.dataset.eventType = visual.event?.body.type ?? 'initial';
    elements.panel.dataset.eventHash = visual.event?.eventHash
      ?? currentRun.run.trace.rootHash;
    elements.panel.dataset.finalOutputHash = currentRun.run.outputHash;
    elements.panel.dataset.selection = currentRun.run.selectedAttempt === null
      ? 'exhausted'
      : `attempt-${currentRun.run.selectedAttempt}`;
    elements.panel.dataset.roomCount = String(visual.rooms.length);
    elements.panel.dataset.routeCount = String(visual.routes.length);
    elements.panel.dataset.finalMatchesResult = String(
      currentRun.run.selectedAttempt === attempt.attempt
      && frame === attempt.eventIndices.length,
    );
    elements.panel.dataset.disposed = 'false';
    setDiagnostic(
      'ready',
      `${currentRun.label} · attempt ${attempt.attempt + 1} · ${displayName(
        attempt.evidence.classification,
      )}`,
    );
  }

  function renderMetrics(
    currentRun: CompiledRun,
    attempt: CatalogGenerationAttempt,
    visual: CatalogGenerationVisualState,
  ): void {
    const policy = currentRun.run.trace.generationPolicy;
    const outcome = visual.outcome;
    replaceMetrics(elements.metrics, [
      ['Stage', displayName(visual.event === null ? 'initial' : eventStage(visual.event.body.type))],
      ['Attempt result', displayName(attempt.evidence.classification)],
      ['Room compaction', String(attempt.evidence.roomCompactionCells)],
      ['Rooms placed', `${visual.rooms.length} / ${attempt.evidence.roomsPlaced}`],
      ['Sections routed', `${visual.routes.length} / ${attempt.evidence.sectionsRouted}`],
      ['Routing states', attempt.evidence.routingStates.toLocaleString()],
      ['Attempt budget', String(policy.maxGenerationAttempts)],
      ['Room candidate cap', String(policy.maxRoomCandidates)],
      ['Route state cap', policy.maxRoutingStatesPerSection.toLocaleString()],
      ['Route margin', String(policy.routeMarginCells)],
      ['Guide / turn weights', `${policy.guideDistanceWeight} / ${policy.turnPenalty}`],
      [
        'Hard placement limit',
        `${policy.outcomeConstraints.maxPlacementWidthCells.toLocaleString()} × ${
          policy.outcomeConstraints.maxPlacementHeightCells.toLocaleString()
        } / ${policy.outcomeConstraints.maxPlacementAreaCells.toLocaleString()} area`,
      ],
      [
        'Hard routed-cell limit',
        policy.outcomeConstraints.maxRoutedCatalogCells.toLocaleString(),
      ],
      [
        'Selection preference',
        `${displayName(policy.outcomePreferences.primaryMetric)} ≤ ${
          policy.outcomePreferences.preferredMaximum.toLocaleString()
        }`,
      ],
      [
        'Observed outcome',
        outcome === null
          ? 'Not evaluated at this decision'
          : `${outcome.metrics.placementWidthCells} × ${
            outcome.metrics.placementHeightCells
          }; ${outcome.metrics.routedCatalogCells.toLocaleString()} routed`,
      ],
      [
        'Hard-limit decision',
        outcome === null
          ? 'Pending'
          : outcome.admissible
            ? 'Admissible'
            : `Rejected · ${outcome.constraintMisses.map((miss) =>
              `${displayName(miss.metric)} ${miss.actual.toLocaleString()} > ${
                miss.limit.toLocaleString()
              }`).join('; ')}`,
      ],
      [
        'Comparison',
        outcome === null
          ? 'Pending'
          : `${displayName(outcome.comparison.ordering)} by ${
            displayName(outcome.comparison.decisiveMetric)
          }${outcome.preferenceSatisfied ? ' · preference met' : ''}`,
      ],
      ['Final selection', displayName(currentRun.run.trace.selection.reason)],
      ['Output hash', currentRun.run.outputHash],
    ]);
    const body = visual.event?.body;
    elements.eventDetail.textContent = body === undefined
      ? 'Input-bound root. Step forward to inspect the first generation attempt.'
      : JSON.stringify(body, null, 2);
  }

  function setControlsEnabled(enabled: boolean): void {
    for (const control of [
      elements.run,
      elements.attempt,
      elements.rate,
      elements.play,
      elements.back,
      elements.step,
      elements.reset,
      elements.previousStage,
      elements.nextStage,
      elements.seek,
    ]) {
      control.disabled = !enabled;
    }
  }

  function setDiagnostic(state: 'loading' | 'ready' | 'error', message: string): void {
    elements.diagnostic.dataset.state = state;
    elements.panel.dataset.state = state;
    elements.diagnostic.textContent = message;
  }

  function handleKeyboard(event: KeyboardEvent): void {
    if (!active || isEditableTarget(event.target)) {
      return;
    }
    if (event.key === ' ') {
      event.preventDefault();
      togglePlayback();
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();
      pause();
      seekTo(frame - 1);
    } else if (event.key === 'ArrowRight') {
      event.preventDefault();
      pause();
      seekTo(frame + 1);
    } else if (event.key === 'Home') {
      event.preventDefault();
      pause();
      seekTo(0);
    } else if (event.key === 'PageUp') {
      event.preventDefault();
      pause();
      seekStage(-1);
    } else if (event.key === 'PageDown') {
      event.preventDefault();
      pause();
      seekStage(1);
    }
  }
}

function decodeBundle(input: unknown): GenerationTraceBundle {
  if (typeof input !== 'object' || input === null || Array.isArray(input)) {
    throw new Error('generation trace bundle must be an object');
  }
  const value = input as Record<string, unknown>;
  const keys = Object.keys(value).sort();
  const expected = ['kind', 'runs', 'schemaVersion'];
  if (JSON.stringify(keys) !== JSON.stringify(expected)) {
    throw new Error('generation trace bundle has unexpected fields');
  }
  if (
    value.kind !== 'rusty_procgen.catalog_generation_trace_bundle.v1'
    || value.schemaVersion !== 1
    || !Array.isArray(value.runs)
    || value.runs.length === 0
    || value.runs.length > MAX_RUNS
  ) {
    throw new Error('generation trace bundle has invalid identity or run bounds');
  }
  const ids = new Set<string>();
  const runs = value.runs.map((inputRun, index): GenerationTraceBundleRun => {
    if (typeof inputRun !== 'object' || inputRun === null || Array.isArray(inputRun)) {
      throw new Error(`generation trace bundle run ${index} must be an object`);
    }
    const entry = inputRun as Record<string, unknown>;
    const entryKeys = Object.keys(entry).sort();
    if (JSON.stringify(entryKeys) !== JSON.stringify(['id', 'label', 'result', 'trace'])) {
      throw new Error(`generation trace bundle run ${index} has unexpected fields`);
    }
    if (
      typeof entry.id !== 'string'
      || entry.id.length === 0
      || entry.id.length > 128
      || ids.has(entry.id)
      || typeof entry.label !== 'string'
      || entry.label.length === 0
      || entry.label.length > 256
    ) {
      throw new Error(`generation trace bundle run ${index} has invalid identity`);
    }
    ids.add(entry.id);
    return {
      id: entry.id,
      label: entry.label,
      trace: entry.trace,
      result: entry.result,
    };
  });
  return {
    kind: 'rusty_procgen.catalog_generation_trace_bundle.v1',
    schemaVersion: 1,
    runs,
  };
}

function renderSvg(
  target: SVGSVGElement,
  visual: CatalogGenerationVisualState,
): void {
  const cells = collectVisualCells(visual);
  const minX = Math.min(...cells.map((cell) => cell.x), 0) - 2;
  const maxX = Math.max(...cells.map((cell) => cell.x), 1) + 2;
  const minY = Math.min(...cells.map((cell) => cell.y), 0) - 2;
  const maxY = Math.max(...cells.map((cell) => cell.y), 1) + 2;
  target.setAttribute('viewBox', `${minX} ${minY} ${maxX - minX + 1} ${maxY - minY + 1}`);
  target.setAttribute('preserveAspectRatio', 'xMidYMid meet');

  const children: SVGElement[] = [];
  children.push(svgElement('rect', {
    x: String(minX),
    y: String(minY),
    width: String(maxX - minX + 1),
    height: String(maxY - minY + 1),
    class: 'generation-trace-background',
  }));
  for (const room of visual.rooms) {
    children.push(...roomCells(room));
  }
  for (const route of visual.routes) {
    children.push(svgElement('polyline', {
      points: route.cells.map(center).join(' '),
      class: 'generation-trace-route',
      'data-section-id': route.sectionId,
    }));
  }
  if (visual.pendingRoute !== null) {
    children.push(svgElement('polyline', {
      points: visual.pendingRoute.guide.map(center).join(' '),
      class: 'generation-trace-guide',
    }));
    children.push(marker(visual.pendingRoute.start, 'generation-trace-route-start'));
    children.push(marker(visual.pendingRoute.goal, 'generation-trace-route-goal'));
  }
  if (visual.conflict !== null) {
    for (const cell of visual.conflict.cells) {
      children.push(cellRect(cell, 'generation-trace-conflict'));
    }
  }
  target.replaceChildren(...children);
}

function roomCells(room: CatalogGenerationRoomPlacement): readonly SVGElement[] {
  const cells: SVGElement[] = [];
  for (const cell of room.reservedCells) {
    cells.push(cellRect(cell, 'generation-trace-reserved', room.pieceId));
  }
  for (const cell of room.occupiedCells) {
    cells.push(cellRect(cell, 'generation-trace-room', room.pieceId));
  }
  return cells;
}

function collectVisualCells(
  visual: CatalogGenerationVisualState,
): readonly { readonly x: number; readonly y: number }[] {
  return [
    ...visual.rooms.flatMap((room) => [...room.occupiedCells, ...room.reservedCells]),
    ...visual.routes.flatMap((route) => route.cells),
    ...(visual.pendingRoute?.guide ?? []),
    ...(visual.pendingRoute === null
      ? []
      : [visual.pendingRoute.start, visual.pendingRoute.goal]),
    ...(visual.conflict?.cells ?? []),
  ];
}

function cellRect(
  cell: { readonly x: number; readonly y: number },
  className: string,
  pieceId?: string,
): SVGRectElement {
  return svgElement('rect', {
    x: String(cell.x),
    y: String(cell.y),
    width: '1',
    height: '1',
    class: className,
    ...(pieceId === undefined ? {} : { 'data-piece-id': pieceId }),
  });
}

function marker(
  cell: { readonly x: number; readonly y: number },
  className: string,
): SVGCircleElement {
  return svgElement('circle', {
    cx: String(cell.x + 0.5),
    cy: String(cell.y + 0.5),
    r: '1.25',
    class: className,
  });
}

function center(cell: { readonly x: number; readonly y: number }): string {
  return `${cell.x + 0.5},${cell.y + 0.5}`;
}

function svgElement<K extends keyof SVGElementTagNameMap>(
  name: K,
  attributes: Readonly<Record<string, string>>,
): SVGElementTagNameMap[K] {
  const element = document.createElementNS(SVG_NAMESPACE, name);
  for (const [key, value] of Object.entries(attributes)) {
    element.setAttribute(key, value);
  }
  return element;
}

function replaceMetrics(
  target: HTMLElement,
  entries: readonly (readonly [string, string])[],
): void {
  target.replaceChildren(...entries.map(([label, value]) => {
    const metric = document.createElement('div');
    metric.className = 'generation-trace-metric';
    const key = document.createElement('span');
    key.textContent = label;
    const fact = document.createElement('strong');
    fact.textContent = value;
    metric.append(key, fact);
    return metric;
  }));
}

function eventStage(type: string): string {
  if (type.startsWith('room_')) {
    return 'room placement';
  }
  if (type.startsWith('section_routing_')) {
    return 'section routing';
  }
  if (type === 'validation_completed') {
    return 'validation';
  }
  if (type === 'outcome_evaluated') {
    return 'outcome decision';
  }
  if (type === 'attempt_finished') {
    return 'attempt result';
  }
  return type;
}

function displayName(value: string): string {
  return value.replaceAll('_', ' ').replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function isEditableTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLInputElement
    || target instanceof HTMLSelectElement
    || target instanceof HTMLButtonElement
    || target instanceof HTMLTextAreaElement;
}

function required<T>(value: T | undefined, label: string): T {
  if (value === undefined) {
    throw new Error(`${label} is missing`);
  }
  return value;
}

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
