import { spawn } from 'node:child_process';
import { execFile } from 'node:child_process';
import { mkdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);
const host = '127.0.0.1';
const port = Number(process.env.GENERATION_TRACE_SMOKE_PORT ?? 5195);
const baseUrl = `http://${host}:${port}`;
const outDir = process.env.GENERATION_TRACE_SMOKE_OUT
  ?? join(tmpdir(), 'rusty-procgen-generation-trace-smoke');
const configPath = join(outDir, 'viewer-generation-config.json');

await mkdir(outDir, { recursive: true });
const generationConfig = JSON.parse(await readFile(
  'fixtures/policies/viewer-generation-default.v2.json',
  'utf8',
));
for (const settings of [
  generationConfig.geometryLayoutPolicy,
  generationConfig.placementPolicy,
  generationConfig.catalogAwareGenerationPolicy,
]) {
  for (const setting of Object.values(settings)) {
    setting.value = setting.defaultValue;
  }
}
generationConfig.corridorRealization.value =
  generationConfig.corridorRealization.defaultValue;
await writeFile(configPath, `${JSON.stringify(generationConfig, null, 2)}\n`);

const server = spawn(
  process.execPath,
  ['scripts/serve-viewer.mjs', '--host', host, '--port', String(port)],
  {
    cwd: process.cwd(),
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
  const fixtureBundle = await fetch(`${baseUrl}/api/evidence/catalog-generation-runs`)
    .then((response) => response.json());
  const accepted = fixtureBundle.runs.find((run) => run.id === 'accepted-default');
  const exhausted = fixtureBundle.runs.find((run) => run.id === 'exhausted-route-budget');
  const bestAdmissible = fixtureBundle.runs.find(
    (run) => run.id === 'best-admissible-selection',
  );
  const controlRejected = fixtureBundle.runs.find(
    (run) => run.id === 'control-tight-5201-rejected',
  );
  const controlCompact = fixtureBundle.runs.find(
    (run) => run.id === 'control-tight-5801-accepted',
  );
  if (
    accepted?.result?.ok !== true
    || accepted.trace?.events?.length !== 188
    || accepted.result.selectedAttempt !== 2
    || accepted.result.attempts?.length !== 4
    || exhausted?.result?.ok !== false
    || exhausted.trace?.events?.length !== 90
    || bestAdmissible?.result?.ok !== true
    || bestAdmissible.result.selectedAttempt !== 2
    || bestAdmissible.result.attempts?.length !== 4
    || controlRejected?.result?.ok !== false
    || controlRejected.result.candidateId !== 'candidate.first_slice.5201'
    || controlCompact?.result?.ok !== true
    || controlCompact.result.candidateId !== 'candidate.first_slice.5801'
  ) {
    throw new Error('generation trace evidence endpoint did not return all bounded outcomes');
  }
  const batch = await fetch(`${baseUrl}/api/batches/v2`).then((response) => response.json());
  const candidateId = batch.accepted?.find(
    (entry) => entry.profileSequence === 'lock-key-baseline',
  )?.candidateId ?? batch.accepted?.[0]?.candidateId;
  if (typeof candidateId !== 'string') {
    throw new Error('generation trace browser smoke requires one accepted candidate');
  }

  const chromium = await findChromium();
  const result = await exerciseViewer(
    chromium,
    accepted,
    exhausted,
    bestAdmissible,
    controlRejected,
    controlCompact,
    fixtureBundle,
    candidateId,
  );
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} catch (error) {
  throw new Error(`${error.message}\nViewer server log:\n${serverLog}`);
} finally {
  server.kill('SIGTERM');
  await waitForChildExit(server);
  await rm(configPath, { force: true });
}

async function exerciseViewer(
  chromium,
  accepted,
  exhausted,
  bestAdmissible,
  controlRejected,
  controlCompact,
  fixtureBundle,
  candidateId,
) {
  const profileDir = join(outDir, 'chromium-profile');
  await rm(profileDir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  const cdpPort = port + 1000;
  const url = `${baseUrl}/?candidate=${encodeURIComponent(candidateId)}#generation-trace`;
  const browser = spawn(chromium, [
    '--headless',
    '--no-sandbox',
    '--disable-gpu',
    '--disable-dev-shm-usage',
    '--remote-debugging-address=127.0.0.1',
    `--remote-debugging-port=${cdpPort}`,
    `--user-data-dir=${profileDir}`,
    '--window-size=1440,900',
    url,
  ], { stdio: ['ignore', 'pipe', 'pipe'] });
  let browserLog = '';
  browser.stdout.on('data', (chunk) => {
    browserLog += chunk.toString();
  });
  browser.stderr.on('data', (chunk) => {
    browserLog += chunk.toString();
  });
  let browserTermination = null;
  browser.once('error', (error) => {
    browserTermination = `spawn error: ${error.message}`;
  });
  browser.once('exit', (code, signal) => {
    browserTermination =
      `exit code ${code === null ? 'null' : code}, signal ${signal ?? 'none'}`;
  });
  let cdp;
  try {
    const page = await waitForCdpPage(
      cdpPort,
      url,
      chromium,
      () => browserTermination,
    );
    cdp = await connectCdp(page.webSocketDebuggerUrl);
    await cdp.send('Runtime.enable');
    await cdp.send('Page.enable');
    await waitForExpression(
      cdp,
      `document.querySelector('#generation-trace-panel')?.dataset.state`,
      'ready',
      20_000,
    );

    const initial = await dataset(cdp);
    if (
      initial.runId !== 'accepted-default'
      || initial.outputHash !== accepted.trace.finalOutputHash
      || initial.frame !== 0
      || initial.roomCount !== 0
      || initial.routeCount !== 0
    ) {
      throw new Error(`accepted initial trace state diverged: ${JSON.stringify(initial)}`);
    }
    const svgInitial = await svgReadout(cdp);
    if (svgInitial.childCount !== 1 || svgInitial.viewBox.length === 0) {
      throw new Error(`initial SVG did not mount an empty bounded frame: ${JSON.stringify(svgInitial)}`);
    }

    await click(cdp, '#generation-trace-step');
    await waitForDatasetNumber(cdp, 'frame', 1);
    const stepped = await dataset(cdp);
    if (stepped.eventType !== 'attempt_started') {
      throw new Error(`first visible decision was not attempt_started: ${stepped.eventType}`);
    }
    await click(cdp, '#generation-trace-back');
    await waitForDatasetNumber(cdp, 'frame', 0);

    await focus(cdp, '#generation-trace-svg');
    await key(cdp, 'ArrowRight');
    await waitForDatasetNumber(cdp, 'frame', 1);
    await key(cdp, 'PageDown');
    await waitForExpression(
      cdp,
      `Number(document.querySelector('#generation-trace-panel')?.dataset.frame) > 1`,
      true,
    );
    const stageFrame = (await dataset(cdp)).frame;
    await key(cdp, 'Home');
    await waitForDatasetNumber(cdp, 'frame', 0);

    await evaluateCdp(cdp, `(() => {
      const rate = document.querySelector('#generation-trace-rate');
      if (!(rate instanceof HTMLSelectElement)) return false;
      rate.value = '8';
      rate.dispatchEvent(new Event('change', { bubbles: true }));
      document.querySelector('#generation-trace-play')?.click();
      return true;
    })()`);
    await waitForExpression(
      cdp,
      `Number(document.querySelector('#generation-trace-panel')?.dataset.frame) >= 2`,
      true,
    );
    await click(cdp, '#generation-trace-play');

    await select(cdp, '#generation-trace-run', 'exhausted-route-budget');
    await waitForDatasetValue(cdp, 'runId', 'exhausted-route-budget');
    const exhaustedInitial = await dataset(cdp);
    const exhaustedAttemptOptions = await evaluateCdp(
      cdp,
      `document.querySelector('#generation-trace-attempt')?.options.length`,
    );
    if (
      exhaustedInitial.selection !== 'exhausted'
      || exhaustedInitial.attempt !== 3
      || exhaustedAttemptOptions !== 4
    ) {
      throw new Error(`exhausted attempt switching is incomplete: ${JSON.stringify({
        exhaustedInitial,
        exhaustedAttemptOptions,
      })}`);
    }
    await select(cdp, '#generation-trace-attempt', '0');
    await waitForDatasetNumber(cdp, 'attempt', 0);
    await seekToEnd(cdp);
    const exhaustedFinal = await dataset(cdp);
    if (
      exhaustedFinal.outputHash !== exhausted.trace.finalOutputHash
      || exhaustedFinal.routeCount !== 0
      || exhaustedFinal.eventType !== 'attempt_finished'
    ) {
      throw new Error(`exhausted final trace state diverged: ${JSON.stringify(exhaustedFinal)}`);
    }

    await select(cdp, '#generation-trace-run', 'best-admissible-selection');
    await waitForDatasetValue(cdp, 'runId', 'best-admissible-selection');
    await seekToEnd(cdp);
    const bestFinal = await dataset(cdp);
    const bestMetrics = await evaluateCdp(
      cdp,
      `document.querySelector('#generation-trace-metrics')?.textContent ?? ''`,
    );
    if (
      bestFinal.selection !== 'attempt-2'
      || bestFinal.attempt !== 2
      || bestFinal.outputHash !== bestAdmissible.trace.finalOutputHash
      || bestFinal.finalMatchesResult !== true
      || !bestMetrics.includes('Hard placement limit')
      || !bestMetrics.includes('Selection preference')
      || !bestMetrics.includes('Best Admissible Placement Span Cells')
    ) {
      throw new Error(`best-admissible comparison was not visible: ${JSON.stringify({
        bestFinal,
        bestMetrics,
      })}`);
    }

    await select(cdp, '#generation-trace-run', 'control-tight-5201-rejected');
    await waitForDatasetValue(cdp, 'runId', 'control-tight-5201-rejected');
    const tightRejected = await dataset(cdp);
    const tightRejectedAttempts = await evaluateCdp(
      cdp,
      `document.querySelector('#generation-trace-attempt')?.options.length`,
    );
    if (
      tightRejected.candidateId !== 'candidate.first_slice.5201'
      || tightRejected.selection !== 'exhausted'
      || tightRejectedAttempts !== 4
      || tightRejected.outputHash !== controlRejected.trace.finalOutputHash
    ) {
      throw new Error(`tight 5201 rejection trace diverged: ${JSON.stringify({
        tightRejected,
        tightRejectedAttempts,
      })}`);
    }
    const mostAdvancedRejected = controlRejected.result.attempts.reduce(
      (selected, attempt) =>
        attempt.sectionsRouted > selected.sectionsRouted ? attempt : selected,
    );
    await select(
      cdp,
      '#generation-trace-attempt',
      String(mostAdvancedRejected.attempt),
    );
    await waitForDatasetNumber(cdp, 'attempt', mostAdvancedRejected.attempt);
    await seekToEnd(cdp);
    const tightRejectedFinal = await dataset(cdp);
    if (
      tightRejectedFinal.roomCount !== mostAdvancedRejected.roomsPlaced
      || tightRejectedFinal.routeCount !== mostAdvancedRejected.sectionsRouted
      || tightRejectedFinal.eventType !== 'attempt_finished'
    ) {
      throw new Error(
        `tight 5201 failed attempt projection diverged: ${JSON.stringify(tightRejectedFinal)}`,
      );
    }
    await captureScreenshot(cdp, 'generation-trace-tight-rejected.png');

    await select(cdp, '#generation-trace-run', 'control-tight-5801-accepted');
    await waitForDatasetValue(cdp, 'runId', 'control-tight-5801-accepted');
    await seekToEnd(cdp);
    const tightCompact = await dataset(cdp);
    if (
      tightCompact.candidateId !== 'candidate.first_slice.5801'
      || tightCompact.selection !== 'attempt-2'
      || tightCompact.roomCount !== 4
      || tightCompact.routeCount !== 4
      || tightCompact.outputHash !== controlCompact.trace.finalOutputHash
      || tightCompact.finalMatchesResult !== true
    ) {
      throw new Error(`tight 5801 compact trace diverged: ${JSON.stringify(tightCompact)}`);
    }
    await captureScreenshot(cdp, 'generation-trace-tight-compact.png');

    await evaluateCdp(cdp, `(() => {
      const run = document.querySelector('#generation-trace-run');
      if (!(run instanceof HTMLSelectElement)) return false;
      for (const value of ['accepted-default', 'exhausted-route-budget', 'accepted-default']) {
        run.value = value;
        run.dispatchEvent(new Event('change', { bubbles: true }));
      }
      return true;
    })()`);
    await waitForDatasetValue(cdp, 'runId', 'accepted-default');
    await seekToEnd(cdp);
    const acceptedFinal = await dataset(cdp);
    if (
      acceptedFinal.outputHash !== accepted.trace.finalOutputHash
      || acceptedFinal.roomCount !== 9
      || acceptedFinal.routeCount !== 13
      || acceptedFinal.finalMatchesResult !== true
      || acceptedFinal.eventType !== 'attempt_finished'
    ) {
      throw new Error(`accepted final trace state diverged: ${JSON.stringify(acceptedFinal)}`);
    }

    await cdp.send('Emulation.setDeviceMetricsOverride', {
      width: 390,
      height: 820,
      deviceScaleFactor: 1,
      mobile: true,
    });
    const mobile = await svgReadout(cdp);
    if (
      mobile.width <= 0
      || mobile.height < 300
      || mobile.documentWidth > mobile.viewportWidth
      || mobile.left < 0
      || mobile.right > mobile.viewportWidth
    ) {
      throw new Error(`generation trace mobile layout overflowed: ${JSON.stringify(mobile)}`);
    }
    await cdp.send('Emulation.setDeviceMetricsOverride', {
      width: 1440,
      height: 900,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await captureScreenshot(cdp, 'generation-trace-desktop.png');

    const tampered = structuredClone(fixtureBundle);
    tampered.runs[0].trace.events[1].body.roomCompactionCells += 1;
    const removeIntercept = cdp.on('Fetch.requestPaused', (params) => {
      if (params.request.url.includes('/api/evidence/catalog-generation-runs')) {
        void cdp.send('Fetch.fulfillRequest', {
          requestId: params.requestId,
          responseCode: 200,
          responseHeaders: [{ name: 'Content-Type', value: 'application/json' }],
          body: Buffer.from(JSON.stringify(tampered)).toString('base64'),
        });
      } else {
        void cdp.send('Fetch.continueRequest', { requestId: params.requestId });
      }
    });
    await cdp.send('Fetch.enable', {
      patterns: [{ urlPattern: '*api/evidence/catalog-generation-runs*' }],
    });
    await cdp.send('Page.reload', { ignoreCache: true });
    await waitForDatasetValue(cdp, 'state', 'error', 20_000);
    const rejected = await evaluateCdp(cdp, `(() => {
      const panel = document.querySelector('#generation-trace-panel');
      const svg = document.querySelector('#generation-trace-svg');
      return {
        childCount: svg?.childElementCount,
        outputHash: panel?.dataset.finalOutputHash ?? '',
        diagnostic: document.querySelector('#generation-trace-diagnostic')?.textContent ?? '',
      };
    })()`);
    if (
      rejected.childCount !== 0
      || rejected.outputHash !== ''
      || !rejected.diagnostic.includes('eventHash')
    ) {
      throw new Error(`tampered trace mutated the mounted view: ${JSON.stringify(rejected)}`);
    }
    await cdp.send('Fetch.disable');
    removeIntercept();
    await cdp.send('Page.reload', { ignoreCache: true });
    await waitForDatasetValue(cdp, 'state', 'ready', 20_000);

    await submitCatalogRebuild(cdp, 4, 200_000);
    await waitForExpression(
      cdp,
      `document.querySelector('#generation-config-status')?.dataset.state`,
      'ready',
      120_000,
    );
    await waitForExpression(
      cdp,
      `document.querySelector('#generation-trace-panel')?.dataset.runId?.startsWith('live-')`,
      true,
    );
    const liveAccepted = await dataset(cdp);
    if (
      liveAccepted.candidateId !== candidateId
      || liveAccepted.selection !== 'attempt-2'
    ) {
      throw new Error(`live accepted rebuild published the wrong trace: ${JSON.stringify(liveAccepted)}`);
    }

    await submitCatalogRebuild(cdp, 1, 100);
    await waitForExpression(
      cdp,
      `document.querySelector('#generation-config-status')?.dataset.state`,
      'error',
      120_000,
    );
    await waitForDatasetValue(cdp, 'selection', 'exhausted');
    const liveExhausted = await dataset(cdp);
    if (
      liveExhausted.candidateId !== candidateId
      || liveExhausted.runId === liveAccepted.runId
    ) {
      throw new Error(`live exhausted rebuild did not replace the accepted trace: ${JSON.stringify({
        liveAccepted,
        liveExhausted,
      })}`);
    }

    const disposed = await evaluateCdp(cdp, `(() => {
      window.dispatchEvent(new Event('pagehide'));
      return document.querySelector('#generation-trace-panel')?.dataset.disposed;
    })()`);
    if (disposed !== 'true') {
      throw new Error(`generation trace disposal was not observable: ${disposed}`);
    }
    return {
      accepted: {
        outputHash: acceptedFinal.outputHash,
        rooms: acceptedFinal.roomCount,
        routes: acceptedFinal.routeCount,
      },
      exhausted: {
        outputHash: exhaustedFinal.outputHash,
        attempts: exhaustedAttemptOptions,
      },
      bestAdmissible: {
        outputHash: bestFinal.outputHash,
        selectedAttempt: bestFinal.attempt,
      },
      characterized: {
        rejectedCandidate: tightRejected.candidateId,
        rejectedAttempts: tightRejectedAttempts,
        compactCandidate: tightCompact.candidateId,
        compactRooms: tightCompact.roomCount,
        compactRoutes: tightCompact.routeCount,
      },
      keyboardStageFrame: stageFrame,
      tamperedTraceRejectedBeforeMount: true,
      liveRebuilds: {
        candidateId,
        acceptedOutputHash: liveAccepted.outputHash,
        exhaustedOutputHash: liveExhausted.outputHash,
      },
      mobileWidth: mobile.width,
      disposedOnPagehide: true,
    };
  } catch (error) {
    throw new Error(`${error.message}\nChromium generation trace log:\n${browserLog}`);
  } finally {
    cdp?.close();
    browser.kill('SIGTERM');
    await waitForChildExit(browser);
    await rm(profileDir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  }
}

async function dataset(cdp) {
  return await evaluateCdp(cdp, `(() => {
    const data = document.querySelector('#generation-trace-panel').dataset;
    return {
      state: data.state,
      runId: data.runId,
      candidateId: data.candidateId,
      attempt: Number(data.attempt),
      frame: Number(data.frame),
      frameCount: Number(data.frameCount),
      eventType: data.eventType,
      outputHash: data.finalOutputHash,
      selection: data.selection,
      roomCount: Number(data.roomCount),
      routeCount: Number(data.routeCount),
      finalMatchesResult: data.finalMatchesResult === 'true',
    };
  })()`);
}

async function svgReadout(cdp) {
  return await evaluateCdp(cdp, `(() => {
    const svg = document.querySelector('#generation-trace-svg');
    const rect = svg.getBoundingClientRect();
    return {
      childCount: svg.childElementCount,
      viewBox: svg.getAttribute('viewBox') ?? '',
      width: rect.width,
      height: rect.height,
      left: rect.left,
      right: rect.right,
      viewportWidth: document.documentElement.clientWidth,
      documentWidth: document.documentElement.scrollWidth,
    };
  })()`);
}

async function captureScreenshot(cdp, fileName) {
  await evaluateCdp(
    cdp,
    `document.querySelector('#generation-trace-panel')?.scrollIntoView({ block: 'start' })`,
  );
  await delay(100);
  const screenshot = await cdp.send('Page.captureScreenshot', {
    format: 'png',
    fromSurface: true,
  });
  const screenshotPath = join(outDir, fileName);
  await writeFile(screenshotPath, screenshot.data, 'base64');
  if ((await stat(screenshotPath)).size < 5_000) {
    throw new Error(`${fileName} is too small to prove visible playback`);
  }
}

async function submitCatalogRebuild(cdp, attempts, routeStates) {
  const submitted = await evaluateCdp(cdp, `(() => {
    const corridor = document.querySelector('#generation-config-corridor-realization');
    const attempts = document.querySelector('#generation-config-catalog-attempts');
    const routeStates = document.querySelector('#generation-config-catalog-route-states');
    const form = document.querySelector('#generation-config-form');
    if (
      !(corridor instanceof HTMLSelectElement)
      || !(attempts instanceof HTMLInputElement)
      || !(routeStates instanceof HTMLInputElement)
      || !(form instanceof HTMLFormElement)
    ) {
      return false;
    }
    corridor.value = 'catalog';
    attempts.value = ${JSON.stringify(String(attempts))};
    routeStates.value = ${JSON.stringify(String(routeStates))};
    for (const control of [corridor, attempts, routeStates]) {
      control.dispatchEvent(new Event('input', { bubbles: true }));
      control.dispatchEvent(new Event('change', { bubbles: true }));
    }
    form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    return true;
  })()`);
  if (submitted !== true) {
    throw new Error('generation configuration controls were unavailable');
  }
  await waitForExpression(
    cdp,
    `document.querySelector('#generation-config-status')?.dataset.state`,
    'loading',
  );
  const busy = await evaluateCdp(cdp, `(() => ({
    applyDisabled: document.querySelector('#generation-config-apply')?.disabled,
    controlsDisabled: document.querySelector('#generation-config-catalog-route-states')?.disabled,
    selectedCandidate: document.querySelector('.candidate-button[data-selected="true"]')?.dataset.candidateId,
    alternateCandidate: [...document.querySelectorAll('.candidate-button')]
      .find((candidate) => candidate.dataset.selected !== 'true')?.dataset.candidateId,
  }))()`);
  if (busy.applyDisabled !== true || busy.controlsDisabled !== true) {
    throw new Error(`in-flight config switching was not disabled: ${JSON.stringify(busy)}`);
  }
  if (busy.alternateCandidate !== undefined) {
    await evaluateCdp(cdp, `(() => {
      const candidate = [...document.querySelectorAll('.candidate-button')]
        .find((entry) => entry.dataset.candidateId === ${JSON.stringify(busy.alternateCandidate)});
      candidate?.click();
    })()`);
    const retainedCandidate = await evaluateCdp(
      cdp,
      `document.querySelector('.candidate-button[data-selected="true"]')?.dataset.candidateId`,
    );
    if (retainedCandidate !== busy.selectedCandidate) {
      throw new Error(`in-flight candidate switching published stale selection state: ${JSON.stringify({
        before: busy.selectedCandidate,
        attempted: busy.alternateCandidate,
        after: retainedCandidate,
      })}`);
    }
  }
}

async function seekToEnd(cdp) {
  await evaluateCdp(cdp, `(() => {
    const seek = document.querySelector('#generation-trace-seek');
    if (!(seek instanceof HTMLInputElement)) return false;
    seek.value = seek.max;
    seek.dispatchEvent(new Event('input', { bubbles: true }));
    return true;
  })()`);
  await waitForExpression(
    cdp,
    `Number(document.querySelector('#generation-trace-panel')?.dataset.frame) === Number(document.querySelector('#generation-trace-panel')?.dataset.frameCount)`,
    true,
  );
}

async function select(cdp, selector, value) {
  await evaluateCdp(cdp, `(() => {
    const select = document.querySelector(${JSON.stringify(selector)});
    if (!(select instanceof HTMLSelectElement)) return false;
    select.value = ${JSON.stringify(value)};
    select.dispatchEvent(new Event('change', { bubbles: true }));
    return true;
  })()`);
}

async function click(cdp, selector) {
  await evaluateCdp(cdp, `document.querySelector(${JSON.stringify(selector)})?.click()`);
}

async function focus(cdp, selector) {
  await evaluateCdp(cdp, `document.querySelector(${JSON.stringify(selector)})?.focus()`);
}

async function key(cdp, keyValue) {
  await cdp.send('Input.dispatchKeyEvent', {
    type: 'keyDown',
    key: keyValue,
    code: keyValue,
  });
  await cdp.send('Input.dispatchKeyEvent', {
    type: 'keyUp',
    key: keyValue,
    code: keyValue,
  });
}

async function waitForDatasetNumber(cdp, name, expected) {
  await waitForExpression(
    cdp,
    `Number(document.querySelector('#generation-trace-panel')?.dataset.${name})`,
    expected,
  );
}

async function waitForDatasetValue(cdp, name, expected, timeoutMs = 10_000) {
  await waitForExpression(
    cdp,
    `document.querySelector('#generation-trace-panel')?.dataset.${name}`,
    expected,
    timeoutMs,
  );
}

async function waitForExpression(cdp, expression, expected, timeoutMs = 10_000) {
  const started = Date.now();
  let actual;
  while (Date.now() - started < timeoutMs) {
    actual = await evaluateCdp(cdp, expression);
    if (actual === expected) {
      return;
    }
    await delay(50);
  }
  throw new Error(
    `timed out waiting for ${expression}; expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`,
  );
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
    await delay(50);
  }
  throw new Error('viewer server did not become healthy');
}

async function waitForCdpPage(cdpPort, url, chromium, termination) {
  const started = Date.now();
  while (Date.now() - started < 10_000) {
    const terminal = termination();
    if (terminal !== null) {
      throw new Error(`Chromium ${chromium} terminated before CDP startup: ${terminal}`);
    }
    try {
      const targets = await fetch(`http://127.0.0.1:${cdpPort}/json/list`)
        .then((response) => response.json());
      const page = targets.find((target) =>
        target.type === 'page' && target.url.startsWith(url.split('#')[0]));
      if (page?.webSocketDebuggerUrl) {
        return page;
      }
    } catch {
      // Chromium is still starting.
    }
    await delay(50);
  }
  throw new Error(
    `Chromium ${chromium} did not expose its CDP page on port ${cdpPort} within 10000ms`,
  );
}

async function connectCdp(url) {
  const socket = new WebSocket(url);
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true });
    socket.addEventListener('error', reject, { once: true });
  });
  let nextId = 0;
  const pending = new Map();
  const listeners = new Map();
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data);
    const request = pending.get(message.id);
    if (request !== undefined) {
      pending.delete(message.id);
      if (message.error) {
        request.reject(new Error(message.error.message));
      } else {
        request.resolve(message.result);
      }
      return;
    }
    if (typeof message.method === 'string') {
      for (const listener of listeners.get(message.method) ?? []) {
        listener(message.params);
      }
    }
  });
  return {
    send(method, params = {}) {
      const id = ++nextId;
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        socket.send(JSON.stringify({ id, method, params }));
      });
    },
    on(method, listener) {
      const methodListeners = listeners.get(method) ?? new Set();
      methodListeners.add(listener);
      listeners.set(method, methodListeners);
      return () => {
        methodListeners.delete(listener);
      };
    },
    close() {
      socket.close();
    },
  };
}

async function evaluateCdp(cdp, expression) {
  const response = await cdp.send('Runtime.evaluate', {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails) {
    throw new Error(
      response.exceptionDetails.exception?.description ?? response.exceptionDetails.text,
    );
  }
  return response.result.value;
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function waitForChildExit(child) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return;
  }
  await new Promise((resolve) => child.once('exit', resolve));
}

async function findChromium() {
  const candidates = [];
  if (typeof process.env.CHROME_PATH === 'string' && process.env.CHROME_PATH.trim() !== '') {
    candidates.push(process.env.CHROME_PATH.trim());
  }
  for (const command of ['chromium', 'chromium-browser', 'google-chrome']) {
    try {
      const { stdout } = await execFileAsync('sh', ['-lc', `command -v ${command}`]);
      const resolved = stdout.trim();
      if (resolved.length > 0) {
        candidates.push(resolved);
      }
    } catch {
      // Try the next browser.
    }
  }
  const checked = new Set();
  const failures = [];
  for (const candidate of candidates) {
    if (checked.has(candidate)) {
      continue;
    }
    checked.add(candidate);
    try {
      const { stdout, stderr } = await execFileAsync(candidate, ['--version']);
      const version = `${stdout}${stderr}`.trim();
      if (version !== '') {
        return candidate;
      }
      failures.push(`${candidate}: empty --version output`);
    } catch (error) {
      failures.push(`${candidate}: ${error.message}`);
    }
  }
  throw new Error(
    `chromium executable not found or unusable${
      failures.length === 0 ? '' : `: ${failures.join('; ')}`
    }`,
  );
}
