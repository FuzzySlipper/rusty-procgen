import { execFile } from 'node:child_process';
import { mkdir, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';
import { spawn } from 'node:child_process';

const execFileAsync = promisify(execFile);
const host = '127.0.0.1';
const port = Number(process.env.VIEWER_SMOKE_PORT ?? 5194);
const baseUrl = `http://${host}:${port}`;
const outDir = process.env.VIEWER_SMOKE_OUT ?? join(tmpdir(), 'rusty-procgen-viewer-smoke');

await mkdir(outDir, { recursive: true });
const generationConfigPath = join(outDir, 'viewer-generation-config.json');
const generationConfig = JSON.parse(
  await readFile('config/viewer-generation.json', 'utf8'),
);
for (const settings of [
  generationConfig.geometryLayoutPolicy,
  generationConfig.placementPolicy,
]) {
  for (const setting of Object.values(settings)) {
    setting.value = setting.defaultValue;
  }
}
generationConfig.corridorRealization.value =
  generationConfig.corridorRealization.defaultValue;
await writeFile(
  generationConfigPath,
  `${JSON.stringify(generationConfig, null, 2)}\n`,
  'utf8',
);

const server = spawn(process.execPath, ['scripts/serve-viewer.mjs', '--host', host, '--port', String(port)], {
  cwd: process.cwd(),
  env: {
    ...process.env,
    RUSTY_PROCGEN_GENERATION_CONFIG_PATH: generationConfigPath,
  },
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
  if (!Array.isArray(batch.accepted) || batch.accepted.length === 0) {
    throw new Error('sample batch has no accepted candidates');
  }
  const top = batch.accepted[0];
  if (typeof top.intermediateBreakdownRef !== 'string') {
    throw new Error('top selection is missing intermediateBreakdownRef');
  }
  if (typeof top.physicalConnectionPlanRef !== 'string') {
    throw new Error('top selection is missing physicalConnectionPlanRef');
  }
  if (typeof top.htmlRef !== 'string') {
    throw new Error('top selection is missing htmlRef');
  }
  if (typeof top.piecePlacementRef !== 'string') {
    throw new Error('top selection is missing piecePlacementRef');
  }
  if (typeof top.piecePlacementValidationRef !== 'string') {
    throw new Error('top selection is missing piecePlacementValidationRef');
  }
  if (typeof top.builtFlowValidationRef !== 'string') {
    throw new Error('top selection is missing builtFlowValidationRef');
  }
  if (typeof top.shapeCatalogRef !== 'string') {
    throw new Error('top selection is missing shapeCatalogRef');
  }
  const breakdown = await fetchArtifact(top.intermediateBreakdownRef);
  if (breakdown.kind !== 'rusty_procgen.intermediate_breakdown.v1') {
    throw new Error(`unexpected intermediate kind: ${breakdown.kind}`);
  }
  if (!Array.isArray(breakdown.regions) || breakdown.regions.length === 0) {
    throw new Error('intermediate breakdown has no regions');
  }
  if (!Array.isArray(breakdown.connectors) || breakdown.connectors.length === 0) {
    throw new Error('intermediate breakdown has no connectors');
  }
  const connectionPlan = await fetchArtifact(top.physicalConnectionPlanRef);
  if (
    connectionPlan.kind !== 'rusty_procgen.physical_connection_plan.v1'
    || !Array.isArray(connectionPlan.sections)
    || connectionPlan.sections.length === 0
    || !Array.isArray(connectionPlan.edgeMappings)
    || connectionPlan.edgeMappings.length !== breakdown.connectors.length
  ) {
    throw new Error('physical connection plan does not cover the selected intermediate connectors');
  }
  const placement = await fetchArtifact(top.piecePlacementRef);
  if (placement.kind !== 'rusty_procgen.piece_placement.v1') {
    throw new Error(`unexpected placement kind: ${placement.kind}`);
  }
  if (!Array.isArray(placement.instances) || placement.instances.length < 10) {
    throw new Error('piece placement has too few instances');
  }
  if (!Array.isArray(placement.gluedExits) || placement.gluedExits.length < 10) {
    throw new Error('piece placement has too few glued exits');
  }
  if (placement.gridConnectivity !== 'four_way') {
    throw new Error(`unexpected piece placement connectivity: ${placement.gridConnectivity}`);
  }
  if (!Array.isArray(placement.connectionCells) || placement.connectionCells.length < 10) {
    throw new Error('piece placement has too few connection cells');
  }
  const placementValidation = await fetchArtifact(top.piecePlacementValidationRef);
  if (placementValidation.kind !== 'rusty_procgen.validation.piece_placement.v1' || !placementValidation.ok) {
    throw new Error('piece placement validation is not ok');
  }
  const builtFlowValidation = await fetchArtifact(top.builtFlowValidationRef);
  if (
    builtFlowValidation.kind !== 'rusty_procgen.validation.built_flow.v1'
    || !builtFlowValidation.ok
    || builtFlowValidation.placementId !== placement.placementId
    || builtFlowValidation.portalCount !== placement.gatePortals.length
  ) {
    throw new Error('built flow validation does not verify the selected placement portals');
  }
  const catalog = await fetchArtifact(top.shapeCatalogRef);
  if (catalog.kind !== 'rusty_procgen.shape_catalog.v1') {
    throw new Error(`unexpected shape catalog kind: ${catalog.kind}`);
  }
  if (!Array.isArray(catalog.shapes) || catalog.shapes.length < 10) {
    throw new Error('shape catalog has too few shapes');
  }
  const directCatalog = await fetchJson('/fixtures/shape-catalogs/2d-basic.json');
  if (directCatalog.catalogId !== catalog.catalogId) {
    throw new Error('direct fixture catalog route did not match artifact catalog route');
  }
  const voxelEvidence = await fetchJson('/api/evidence/engine-spatial-extrusion');
  const caEvidence = await fetchJson('/api/evidence/engine-ca-benchmark');
  if (
    voxelEvidence.kind !== 'rusty_procgen.evidence.engine_spatial_extrusion.v2'
    || voxelEvidence.schemaVersion !== 2
    || !/^sha256:[0-9a-f]{64}$/.test(voxelEvidence.planSha256)
  ) {
    throw new Error('Rusty Engine spatial evidence has no canonical plan binding');
  }
  const voxelEntry = batch.accepted.find((entry) => entry.piecePlacementRef === voxelEvidence.sourcePlacement);
  if (
    voxelEntry === undefined
    || voxelEvidence.authority?.deterministic !== true
    || voxelEvidence.authority?.readout?.projectionRevisionsCoherent !== true
  ) {
    throw new Error('Rusty Engine spatial evidence has no matching coherent batch placement');
  }
  const alternateVoxelEntries = await Promise.all(batch.accepted
    .filter((entry) => (
      entry.candidateId !== voxelEntry.candidateId
      && entry.topologyFingerprint !== voxelEntry.topologyFingerprint
      && typeof entry.piecePlacementRef === 'string'
    ))
    .map(async (entry) => {
      const candidatePlacement = await fetchArtifact(entry.piecePlacementRef);
      return {
        entry,
        projectedCellCount:
          candidatePlacement.occupiedCells.length + candidatePlacement.connectionCells.length,
      };
    }));
  alternateVoxelEntries.sort((left, right) => (
    left.projectedCellCount - right.projectedCellCount
    || left.entry.candidateId.localeCompare(right.entry.candidateId)
  ));
  const alternateVoxelEntry = alternateVoxelEntries[0]?.entry;
  const css = await fetchText('/viewer/styles.css');
  if (!css.includes('color-scheme: dark') || !css.includes('#11161d')) {
    throw new Error('viewer dark theme CSS was not found');
  }
  const previewHtml = await fetchText(`/api/artifacts/by-path?path=${encodeURIComponent(top.htmlRef)}`);
  const previewRoomCount = countOccurrences(previewHtml, '<rect ');
  const previewCorridorCount = countOccurrences(previewHtml, '<polyline ');
  if (!previewHtml.includes('background: #0b0d10')) {
    throw new Error('standalone preview dark background was not found');
  }
  if (previewRoomCount < 2 || previewCorridorCount < 1) {
    throw new Error(`standalone preview SVG looks sparse: rooms=${previewRoomCount}, corridors=${previewCorridorCount}`);
  }
  const requiredPreviewLabels = ['Key Pickup'];
  if (top.tags?.includes('boss')) {
    requiredPreviewLabels.push('Boss Threshold');
  }
  for (const label of requiredPreviewLabels) {
    if (!previewHtml.includes(label)) {
      throw new Error(`standalone preview missing content label: ${label}`);
    }
  }

  const chromium = await findChromium();
  const previewUrl = `${baseUrl}/api/artifacts/by-path?path=${encodeURIComponent(top.htmlRef)}`;
  const buildDom = await dumpDom(chromium, `${baseUrl}/#build`);
  const buildCellCount = countOccurrences(buildDom, 'class="build-cell');
  const buildMarkerCount = countOccurrences(buildDom, 'class="build-marker');
  const glueLinkCount = countOccurrences(buildDom, 'class="build-glue-link');
  const connectionCellCount = countOccurrences(buildDom, 'class="build-cell connection');
  if (!buildDom.includes('Piece Placement Grid')) {
    throw new Error('build tab did not render the piece placement grid');
  }
  if (buildCellCount < 20 || buildMarkerCount < 2) {
    throw new Error(`build tab rendered too little grid detail: cells=${buildCellCount}, markers=${buildMarkerCount}`);
  }
  if (glueLinkCount < 10) {
    throw new Error(`build tab rendered too few glued exits: ${glueLinkCount}`);
  }
  if (connectionCellCount < 10) {
    throw new Error(`build tab rendered too few connection cells: ${connectionCellCount}`);
  }
  const catalogDom = await dumpDom(chromium, `${baseUrl}/#catalog`);
  const catalogCardCount = countOccurrences(catalogDom, 'class="catalog-shape-card');
  const catalogCellCount = countOccurrences(catalogDom, 'class="catalog-cell');
  if (!catalogDom.includes('Build Piece Catalog')) {
    throw new Error('catalog tab did not render the build piece catalog');
  }
  if (catalogCardCount < 10 || catalogCellCount < 20) {
    throw new Error(`catalog tab rendered too little detail: cards=${catalogCardCount}, cells=${catalogCellCount}`);
  }
  const voxelUrl = `${baseUrl}/?candidate=${encodeURIComponent(voxelEntry.candidateId)}#voxel`;
  const voxelDom = await dumpDom(chromium, voxelUrl);
  const voxelFaceCount = countOccurrences(voxelDom, 'class="voxel-face');
  if (!voxelDom.includes('Rusty Engine Voxel Extrusion Cutaway')) {
    throw new Error('voxel tab did not render the extrusion cutaway');
  }
  if (!voxelDom.includes(voxelEvidence.authority.readout.authorityHash)) {
    throw new Error('voxel tab did not show matching Engine spatial authority evidence');
  }
  if (!voxelDom.includes(voxelEvidence.planSha256.slice(0, 19))) {
    throw new Error('voxel tab did not show its Rust-owned plan binding');
  }
  if (voxelFaceCount < 500) {
    throw new Error(`voxel tab rendered too few exposed faces: ${voxelFaceCount}`);
  }
  const voxel3dUrl = `${baseUrl}/?inspection=once&candidate=${encodeURIComponent(voxelEntry.candidateId)}#voxel3d`;
  const voxel3dDom = await dumpEngineDom(chromium, voxel3dUrl);
  if (!voxel3dDom.includes('Engine Voxel Inspection')) {
    throw new Error('Voxel 3D tab was not found');
  }
  if (!voxel3dDom.includes('data-renderer-host="rusty_renderer_inspection_surface.v1"')) {
    throw new Error('Voxel 3D tab did not mount the engine inspection surface');
  }
  if (!voxel3dDom.includes('data-renderer-role="projection_only_inspection"')) {
    throw new Error('Voxel 3D tab did not expose its projection-only renderer role');
  }
  if (!voxel3dDom.includes('data-state="ready"')) {
    throw new Error(`Voxel 3D engine mount was not ready: ${attributeValue(voxel3dDom, 'data-state')}`);
  }
  if (!voxel3dDom.includes('Arrow keys to orbit') || !voxel3dDom.includes('wheel to zoom')) {
    throw new Error('Voxel 3D tab did not expose keyboard orbit and zoom controls');
  }
  const projectedVoxelCount = Number(attributeValue(voxel3dDom, 'data-projected-voxel-count'));
  const omittedCeilingVoxelCount = Number(attributeValue(voxel3dDom, 'data-omitted-ceiling-voxel-count'));
  const voxel3dFrameHash = attributeValue(voxel3dDom, 'data-frame-hash');
  const voxel3dPlacementId = attributeValue(voxel3dDom, 'data-placement-id');
  const voxel3dPickHitCount = Number(attributeValue(voxel3dDom, 'data-pick-hit-count'));
  const voxel3dGridLineCount = Number(attributeValue(voxel3dDom, 'data-grid-line-count'));
  const voxel3dGridRevision = Number(attributeValue(voxel3dDom, 'data-grid-revision'));
  const voxel3dDoorNodeCount = Number(attributeValue(voxel3dDom, 'data-door-node-count'));
  const voxel3dLockedDoorCount = Number(attributeValue(voxel3dDom, 'data-locked-door-count'));
  const voxel3dUnlockedDoorCount = Number(attributeValue(voxel3dDom, 'data-unlocked-door-count'));
  const voxel3dNativeAuthority = attributeValue(voxel3dDom, 'data-native-authority');
  const voxel3dPlanSha256 = attributeValue(voxel3dDom, 'data-plan-sha256');
  if (
    projectedVoxelCount < 500
    || omittedCeilingVoxelCount <= 0
    || voxel3dFrameHash.length === 0
    || voxel3dPickHitCount <= 0
    || voxel3dGridLineCount <= 0
    || voxel3dGridRevision < 1
    || voxel3dDoorNodeCount <= 0
    || voxel3dLockedDoorCount <= 0
    || voxel3dUnlockedDoorCount <= 0
    || voxel3dNativeAuthority !== 'verified'
    || voxel3dPlanSha256 !== voxelEvidence.planSha256
  ) {
    throw new Error(
      `Voxel 3D projection evidence is incomplete: projected=${projectedVoxelCount}, omitted=${omittedCeilingVoxelCount}, picks=${voxel3dPickHitCount}, grid=${voxel3dGridLineCount}`,
    );
  }
  let alternatePlacementId = null;
  let alternateFrameHash = null;
  if (alternateVoxelEntry !== undefined) {
    const alternateVoxel3dUrl = `${baseUrl}/?inspection=once&candidate=${encodeURIComponent(alternateVoxelEntry.candidateId)}#voxel3d`;
    const alternateVoxel3dDom = await dumpEngineDom(chromium, alternateVoxel3dUrl);
    if (
      !alternateVoxel3dDom.includes('data-state="error"')
      || !alternateVoxel3dDom.includes('data-native-authority="rejected"')
      || !alternateVoxel3dDom.includes('does not match native')
      || alternateVoxel3dDom.includes('data-renderer-host=')
    ) {
      throw new Error(
        'Voxel 3D admitted a committed candidate with no matching Rust-owned plan',
      );
    }
  }
  const voxel3dInteraction = await exerciseEngineInspection(
    chromium,
    `${baseUrl}/?candidate=${encodeURIComponent(voxelEntry.candidateId)}#voxel3d`,
    voxelEntry.candidateId,
    alternateVoxelEntry?.candidateId,
  );
  const nativeAuthorityTamper = await exerciseNativeAuthorityTamper(
    chromium,
    voxelEntry.candidateId,
    voxelEvidence,
    await fetchArtifact(voxelEvidence.sourcePlacement),
  );
  const caTraceInteraction = await exerciseCaTrace(chromium, caEvidence);
  const screenshots = [
    {
      name: 'layout-desktop.png',
      url: `${baseUrl}/#layout`,
      size: '1000,760',
    },
    {
      name: 'intermediate-desktop.png',
      url: `${baseUrl}/#intermediate`,
      size: '1000,760',
    },
    {
      name: 'intermediate-mobile.png',
      url: `${baseUrl}/#intermediate`,
      size: '390,800',
    },
    {
      name: 'build-desktop.png',
      url: `${baseUrl}/#build`,
      size: '1100,780',
    },
    {
      name: 'catalog-desktop.png',
      url: `${baseUrl}/#catalog`,
      size: '1100,780',
    },
    {
      name: 'voxel-desktop.png',
      url: voxelUrl,
      size: '1200,820',
    },
    {
      name: 'voxel-3d-desktop.png',
      url: `${baseUrl}/?candidate=${encodeURIComponent(voxelEntry.candidateId)}#voxel3d`,
      size: '1200,820',
      capturedByInteractionProbe: true,
    },
    {
      name: 'ca-trace-desktop.png',
      url: `${baseUrl}/#ca`,
      size: '1280,860',
      capturedByInteractionProbe: true,
    },
    {
      name: 'standalone-preview-desktop.png',
      url: previewUrl,
      size: '1100,780',
    },
    {
      name: 'standalone-preview-mobile.png',
      url: previewUrl,
      size: '390,820',
    },
  ];
  for (const screenshot of screenshots) {
    const out = join(outDir, screenshot.name);
    if (screenshot.capturedByInteractionProbe) {
      const file = await stat(out);
      if (file.size < 5_000) {
        throw new Error(`${screenshot.name} looks too small to be a useful screenshot`);
      }
      continue;
    }
    await execFileAsync(chromium, [
      '--headless',
      '--no-sandbox',
      '--disable-gpu',
      '--run-all-compositor-stages-before-draw',
      '--virtual-time-budget=3000',
      `--window-size=${screenshot.size}`,
      `--screenshot=${out}`,
      screenshot.url,
    ]);
    const file = await stat(out);
    if (file.size < 5_000) {
      throw new Error(`${screenshot.name} looks too small to be a useful screenshot`);
    }
  }

  const report = {
    ok: true,
    baseUrl,
    batchId: batch.batchId,
    candidateId: top.candidateId,
    regions: breakdown.regions.length,
    connectors: breakdown.connectors.length,
    standalonePreview: {
      htmlRef: top.htmlRef,
      rooms: previewRoomCount,
      corridors: previewCorridorCount,
      hasDarkBackground: true,
      requiredLabels: requiredPreviewLabels,
    },
    buildTab: {
      cells: buildCellCount,
      connectionCells: connectionCellCount,
      markers: buildMarkerCount,
      gluedExits: glueLinkCount,
      placementRef: top.piecePlacementRef,
    },
    catalogTab: {
      catalogRef: top.shapeCatalogRef,
      shapes: catalog.shapes.length,
      cards: catalogCardCount,
      cells: catalogCellCount,
    },
    voxel3dTab: {
      placementId: voxel3dPlacementId,
      projectedVoxels: projectedVoxelCount,
      omittedCeilingVoxels: omittedCeilingVoxelCount,
      frameHash: voxel3dFrameHash,
      pickHits: voxel3dPickHitCount,
      gridLines: voxel3dGridLineCount,
      gridRevision: voxel3dGridRevision,
      doors: {
        nodes: voxel3dDoorNodeCount,
        locked: voxel3dLockedDoorCount,
        unlocked: voxel3dUnlockedDoorCount,
      },
      alternatePlacementId,
      alternateFrameHash,
      rendererRole: 'projection_only_inspection',
      interaction: voxel3dInteraction,
      nativeAuthorityTamper,
    },
    caTraceTab: caTraceInteraction,
    screenshots: screenshots.map((screenshot) => join(outDir, screenshot.name)),
  };
  await writeFile(join(outDir, 'viewer-smoke-report.json'), `${JSON.stringify(report, null, 2)}\n`);
  console.log(`viewer smoke passed; evidence written to ${outDir}`);
} finally {
  server.kill('SIGTERM');
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
  throw new Error(`viewer server did not start:\n${serverLog}`);
}

async function fetchJson(path) {
  const response = await fetch(`${baseUrl}${path}`);
  if (!response.ok) {
    throw new Error(`failed to fetch ${path}: ${response.status}`);
  }
  return await response.json();
}

async function fetchText(path) {
  const response = await fetch(`${baseUrl}${path}`);
  if (!response.ok) {
    throw new Error(`failed to fetch ${path}: ${response.status}`);
  }
  return await response.text();
}

async function fetchArtifact(path) {
  return await fetchJson(`/api/artifacts/by-path?path=${encodeURIComponent(path)}`);
}

function countOccurrences(text, pattern) {
  return text.split(pattern).length - 1;
}

async function dumpDom(chromium, url) {
  const { stdout } = await execFileAsync(chromium, [
    '--headless',
    '--no-sandbox',
    '--disable-gpu',
    '--run-all-compositor-stages-before-draw',
    '--virtual-time-budget=3000',
    '--dump-dom',
    url,
  ], { maxBuffer: 16 * 1024 * 1024 });
  return stdout;
}

async function dumpEngineDom(chromium, url) {
  const { stdout } = await execFileAsync(chromium, [
    '--headless',
    '--no-sandbox',
    '--enable-unsafe-swiftshader',
    '--run-all-compositor-stages-before-draw',
    '--virtual-time-budget=5000',
    '--dump-dom',
    url,
  ], { maxBuffer: 16 * 1024 * 1024 });
  return stdout;
}

async function exerciseNativeAuthorityTamper(
  chromium,
  candidateId,
  evidence,
  placement,
) {
  const profileDir = join(outDir, 'chromium-native-authority-tamper-profile');
  const cdpPort = Number(process.env.VIEWER_SMOKE_TAMPER_CDP_PORT ?? port + 1002);
  await rm(profileDir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  const tamperedPlacement = structuredClone(placement);
  const furthest = tamperedPlacement.occupiedCells.reduce(
    (maximum, cell) => ({
      x: Math.max(maximum.x, cell.x),
      y: Math.max(maximum.y, cell.y),
    }),
    { x: 0, y: 0 },
  );
  tamperedPlacement.occupiedCells.push({
    instanceId: tamperedPlacement.occupiedCells[0].instanceId,
    x: furthest.x + 100,
    y: furthest.y + 100,
  });
  const responseBody = Buffer.from(JSON.stringify(tamperedPlacement)).toString('base64');
  const sourceToken = encodeURIComponent(evidence.sourcePlacement);
  const url = `${baseUrl}/?candidate=${encodeURIComponent(candidateId)}#voxel`;
  const browser = spawn(chromium, [
    '--headless',
    '--no-sandbox',
    '--enable-unsafe-swiftshader',
    `--remote-debugging-port=${cdpPort}`,
    `--user-data-dir=${profileDir}`,
    '--window-size=1200,820',
    'about:blank',
  ], { stdio: ['ignore', 'ignore', 'pipe'] });
  let browserLog = '';
  browser.stderr.on('data', (chunk) => {
    browserLog += chunk.toString();
  });
  let cdp;
  let interceptionError;
  let tamperedResponses = 0;
  try {
    const page = await waitForCdpPage(cdpPort, 'about:blank');
    cdp = await connectCdp(page.webSocketDebuggerUrl);
    const unsubscribe = cdp.on('Fetch.requestPaused', (event) => {
      void (async () => {
        if (
          event.responseStatusCode !== undefined
          && event.request.url.includes(sourceToken)
        ) {
          tamperedResponses += 1;
          const headers = (event.responseHeaders ?? [])
            .filter((header) => !['content-length', 'content-encoding']
              .includes(header.name.toLowerCase()));
          await cdp.send('Fetch.fulfillRequest', {
            requestId: event.requestId,
            responseCode: event.responseStatusCode,
            responseHeaders: headers,
            body: responseBody,
          });
          return;
        }
        await cdp.send('Fetch.continueRequest', { requestId: event.requestId });
      })().catch((error) => {
        interceptionError = error;
      });
    });
    await cdp.send('Page.enable');
    await cdp.send('Fetch.enable', {
      patterns: [{
        urlPattern: `*${sourceToken}*`,
        requestStage: 'Response',
      }],
    });
    await cdp.send('Page.navigate', { url });
    await waitForCdpValue(
      cdp,
      `document.querySelector('#layout')?.textContent.includes('native authority mismatch')`,
      true,
      30_000,
    );
    if (interceptionError !== undefined) {
      throw interceptionError;
    }
    const voxel = await evaluateCdp(cdp, `(() => ({
      text: document.querySelector('#layout')?.textContent,
      leakedAuthority: document.querySelector('#layout')?.textContent
        .includes(${JSON.stringify(evidence.authority.readout.authorityHash)}),
    }))()`);
    if (
      tamperedResponses !== 1
      || voxel.leakedAuthority
      || !String(voxel.text).includes('plan SHA')
    ) {
      throw new Error(
        `Voxel same-ID tamper did not fail closed: ${JSON.stringify({ tamperedResponses, voxel })}`,
      );
    }

    const openedVoxel3d = await evaluateCdp(cdp, `(() => {
      const button = document.querySelector('[data-view="voxel3d"]');
      button?.click();
      return button !== null;
    })()`);
    if (!openedVoxel3d) {
      throw new Error('Voxel 3D tab was unavailable during same-ID tamper probe');
    }
    await waitForCdpValue(
      cdp,
      `document.querySelector('#voxel-3d-diagnostic')?.dataset.state`,
      'error',
      30_000,
    );
    const voxel3d = await evaluateCdp(cdp, `(() => {
      const panel = document.querySelector('#voxel-3d-panel');
      const diagnostic = document.querySelector('#voxel-3d-diagnostic');
      return {
        nativeAuthority: panel?.dataset.nativeAuthority,
        rendererHost: panel?.dataset.rendererHost ?? '',
        diagnostic: diagnostic?.textContent,
      };
    })()`);
    if (
      voxel3d.nativeAuthority !== 'rejected'
      || voxel3d.rendererHost !== ''
      || !String(voxel3d.diagnostic).includes('plan SHA')
      || !String(voxel3d.diagnostic).includes('does not match native')
    ) {
      throw new Error(`Voxel 3D same-ID tamper did not fail visibly: ${JSON.stringify(voxel3d)}`);
    }
    unsubscribe();
    return {
      placementId: evidence.placementId,
      interceptedResponses: tamperedResponses,
      voxelRejected: true,
      voxel3dRejected: true,
      authorityHashWithheld: true,
    };
  } catch (error) {
    throw new Error(`${error.message}\nChromium native-authority tamper log:\n${browserLog}`);
  } finally {
    cdp?.close();
    browser.kill('SIGTERM');
    await waitForChildExit(browser);
    await rm(profileDir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  }
}

async function exerciseEngineInspection(chromium, url, primaryCandidateId, alternateCandidateId) {
  const profileDir = join(outDir, 'chromium-cdp-profile');
  const cdpPort = Number(process.env.VIEWER_SMOKE_CDP_PORT ?? port + 1000);
  await rm(profileDir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  const browser = spawn(chromium, [
    '--headless',
    '--no-sandbox',
    '--enable-unsafe-swiftshader',
    `--remote-debugging-port=${cdpPort}`,
    `--user-data-dir=${profileDir}`,
    '--window-size=1200,820',
    url,
  ], { stdio: ['ignore', 'ignore', 'pipe'] });
  let browserLog = '';
  browser.stderr.on('data', (chunk) => {
    browserLog += chunk.toString();
  });
  let cdp;
  try {
    const page = await waitForCdpPage(cdpPort, url);
    cdp = await connectCdp(page.webSocketDebuggerUrl);
    await waitForCdpValue(cdp, `document.querySelector('#voxel-3d-diagnostic')?.dataset.state`, 'ready');
    const initial = await inspectionDataset(cdp);
    if (
      initial.rendererHost !== 'rusty_renderer_inspection_surface.v1'
      || initial.rendererRole !== 'projection_only_inspection'
      || initial.rendererCompatibilityVersion !== 'inspection-surface.v1'
      || initial.rendererStatus !== 'running'
      || initial.retainedOpCount <= 0
      || initial.nativeAuthority !== 'verified'
      || !/^sha256:[0-9a-f]{64}$/.test(initial.planSha256)
    ) {
      throw new Error(`Rusty Engine renderer identity/readout is incomplete: ${JSON.stringify(initial)}`);
    }
    if (initial.gridLineCount <= 0 || initial.gridRevision < 1) {
      throw new Error(`engine grid was not realized: lines=${initial.gridLineCount}, revision=${initial.gridRevision}`);
    }
    if (initial.doorNodeCount <= 0 || initial.lockedDoorCount <= 0 || initial.unlockedDoorCount <= 0) {
      throw new Error(`verified initial doors were not rendered: ${JSON.stringify(initial)}`);
    }
    await cdp.send('Emulation.setDeviceMetricsOverride', {
      width: 960,
      height: 700,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await waitForCdpValue(
      cdp,
      `document.querySelector('#voxel-3d-panel')?.dataset.viewportHash !== ${JSON.stringify(initial.viewportHash)}`,
      true,
    );
    const resized = await inspectionDataset(cdp);
    await cdp.send('Emulation.setDeviceMetricsOverride', {
      width: 1200,
      height: 820,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await waitForCdpValue(
      cdp,
      `document.querySelector('#voxel-3d-panel')?.dataset.viewportHash !== ${JSON.stringify(resized.viewportHash)}`,
      true,
    );

    if (alternateCandidateId !== undefined) {
      const rapidSwitchStarted = await evaluateCdp(cdp, `(() => {
        const buttons = [...document.querySelectorAll('.candidate-button')];
        const alternate = buttons.find((candidate) =>
          candidate.dataset.candidateId === ${JSON.stringify(alternateCandidateId)});
        const primary = buttons.find((candidate) =>
          candidate.dataset.candidateId === ${JSON.stringify(primaryCandidateId)});
        alternate?.click();
        primary?.click();
        return alternate !== undefined && primary !== undefined;
      })()`);
      if (!rapidSwitchStarted) {
        throw new Error('rapid stale-candidate replacement controls were unavailable');
      }
      await waitForCdpValue(
        cdp,
        `document.querySelector('#voxel-3d-diagnostic')?.dataset.state`,
        'ready',
      );
      await waitForCdpValue(
        cdp,
        `document.querySelector('#voxel-3d-panel')?.dataset.placementId`,
        initial.placementId,
      );
      const afterRapidSwitch = await inspectionDataset(cdp);
      if (afterRapidSwitch.frameHash !== initial.frameHash) {
        throw new Error(
          `stale candidate work replaced the latest projection: ${initial.frameHash} -> ${afterRapidSwitch.frameHash}`,
        );
      }
    }
    await evaluateCdp(cdp, `(() => {
      const select = document.querySelector('#voxel-3d-door-state');
      if (!(select instanceof HTMLSelectElement)) return false;
      select.value = 'all';
      select.dispatchEvent(new Event('change', { bubbles: true }));
      return true;
    })()`);
    await waitForCdpValue(cdp, `Number(document.querySelector('#voxel-3d-panel')?.dataset.lockedDoorCount)`, 0);
    const allUnlocked = await inspectionDataset(cdp);
    if (allUnlocked.unlockedDoorCount !== initial.doorNodeCount || allUnlocked.frameHash === initial.frameHash) {
      throw new Error(`all-unlocked door state did not rebuild the engine frame: ${JSON.stringify(allUnlocked)}`);
    }
    await evaluateCdp(cdp, `(() => {
      const select = document.querySelector('#voxel-3d-door-state');
      select.value = 'initial';
      select.dispatchEvent(new Event('change', { bubbles: true }));
    })()`);
    await waitForCdpValue(cdp, `document.querySelector('#voxel-3d-panel')?.dataset.frameHash`, initial.frameHash);
    await evaluateCdp(cdp, `document.querySelector('#voxel-3d-canvas')?.scrollIntoView({ block: 'center' })`);
    await delay(100);
    const rect = await evaluateCdp(cdp, `(() => {
      const rect = document.querySelector('#voxel-3d-canvas').getBoundingClientRect();
      return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
    })()`);
    const x = rect.x + rect.width * 0.5;
    const y = rect.y + rect.height * 0.5;
    await cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 });
    await cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 });

    await cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'ArrowRight', code: 'ArrowRight' });
    await delay(180);
    await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'ArrowRight', code: 'ArrowRight' });
    await waitForCdpValue(cdp, `document.querySelector('#voxel-3d-panel')?.dataset.lastCameraChange`, 'keyboard_orbit');
    const keyboardOrbit = await inspectionDataset(cdp);

    await cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'w', code: 'KeyW' });
    await delay(180);
    await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'w', code: 'KeyW' });
    await waitForCdpValue(cdp, `document.querySelector('#voxel-3d-panel')?.dataset.lastCameraChange`, 'keyboard_movement');
    const keyboardMovement = await inspectionDataset(cdp);

    await cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', key: '+', code: 'NumpadAdd' });
    await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: '+', code: 'NumpadAdd' });
    await waitForCdpValue(cdp, `document.querySelector('#voxel-3d-panel')?.dataset.lastCameraChange`, 'keyboard_zoom');
    const keyboardZoom = await inspectionDataset(cdp);

    await cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 });
    await cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: x + 80, y: y + 30, button: 'left', buttons: 1 });
    await cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: x + 80, y: y + 30, button: 'left', clickCount: 1 });
    await waitForCdpValue(cdp, `document.querySelector('#voxel-3d-panel')?.dataset.lastCameraChange`, 'pointer_orbit');
    const pointerOrbit = await inspectionDataset(cdp);

    await cdp.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x, y, deltaX: 0, deltaY: -120 });
    await waitForCdpValue(cdp, `document.querySelector('#voxel-3d-panel')?.dataset.lastCameraChange`, 'wheel_zoom');
    const wheelZoom = await inspectionDataset(cdp);

    const invalidGenerationConfigReadout = await evaluateCdp(cdp, `(() => {
      const gap = document.querySelector('#generation-config-initial-column-gap');
      const validation = document.querySelector('#generation-config-validation');
      const apply = document.querySelector('#generation-config-apply');
      if (!(gap instanceof HTMLInputElement) || !(apply instanceof HTMLButtonElement)) {
        return null;
      }
      gap.value = '145';
      gap.dispatchEvent(new Event('input', { bubbles: true }));
      return {
        valid: gap.validity.valid,
        state: validation?.dataset.state,
        message: validation?.textContent,
        applyDisabled: apply.disabled,
      };
    })()`);
    if (
      invalidGenerationConfigReadout?.valid !== false
      || invalidGenerationConfigReadout?.state !== 'invalid'
      || !String(invalidGenerationConfigReadout?.message).includes('8-unit route grid')
      || invalidGenerationConfigReadout?.applyDisabled !== true
    ) {
      throw new Error(`invalid generation config was not explained inline: ${JSON.stringify(invalidGenerationConfigReadout)}`);
    }

    if (alternateCandidateId === undefined) {
      throw new Error('pure catalog rejection smoke requires an alternate candidate');
    }
    const switchedToRejectingCandidate = await evaluateCdp(cdp, `(() => {
      const button = [...document.querySelectorAll('.candidate-button')]
        .find((candidate) => candidate.dataset.candidateId === ${JSON.stringify(alternateCandidateId)});
      button?.click();
      return button !== undefined;
    })()`);
    if (!switchedToRejectingCandidate) {
      throw new Error(`pure catalog rejection candidate was not found: ${alternateCandidateId}`);
    }
    await waitForCdpValue(cdp, `document.querySelector('#voxel-3d-diagnostic')?.dataset.state`, 'error');
    await waitForCdpValue(
      cdp,
      `document.querySelector('#voxel-3d-panel')?.dataset.nativeAuthority`,
      'rejected',
    );
    const pureCatalogBaseline = initial;

    const submittedPureCatalogConfig = await evaluateCdp(cdp, `(() => {
      const form = document.querySelector('#generation-config-form');
      const gap = document.querySelector('#generation-config-initial-column-gap');
      const corridor = document.querySelector('#generation-config-corridor-realization');
      if (
        !(form instanceof HTMLFormElement)
        || !(gap instanceof HTMLInputElement)
        || !(corridor instanceof HTMLSelectElement)
      ) {
        return false;
      }
      gap.value = '144';
      corridor.value = 'catalog';
      for (const input of [gap, corridor]) {
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
      }
      form.requestSubmit();
      return true;
    })()`);
    if (!submittedPureCatalogConfig) {
      throw new Error('pure catalog generation config controls were not available in Voxel 3D');
    }
    await waitForCdpValue(
      cdp,
      `document.querySelector('#generation-config-status')?.dataset.state`,
      'ready',
      120_000,
    );
    await waitForCdpValue(
      cdp,
      `document.querySelector('#generation-config-panel')?.dataset.configState`,
      'persisted',
    );
    const pureCatalogBuild = await evaluateCdp(cdp, `(() => ({
      configState: document.querySelector('#generation-config-panel')?.dataset.configState,
      status: document.querySelector('#generation-config-status')?.textContent,
      frameHash: document.querySelector('#voxel-3d-panel')?.dataset.frameHash,
      corridorRealization: document.querySelector('#generation-config-corridor-realization')?.value,
      impact: document.querySelector('#generation-config-impact')?.textContent,
    }))()`);
    if (
      pureCatalogBuild.configState !== 'persisted'
      || pureCatalogBuild.frameHash === pureCatalogBaseline.frameHash
      || pureCatalogBuild.corridorRealization !== 'catalog'
      || !String(pureCatalogBuild.status).includes('Persisted configuration build')
      || !String(pureCatalogBuild.impact).includes('0 routed cells')
    ) {
      throw new Error(`catalog-aware build was not retained and explained: ${JSON.stringify(pureCatalogBuild)}`);
    }
    const switchedBackToPrimaryCandidate = await evaluateCdp(cdp, `(() => {
      const button = [...document.querySelectorAll('.candidate-button')]
        .find((candidate) => candidate.dataset.candidateId === ${JSON.stringify(primaryCandidateId)});
      button?.click();
      return button !== undefined;
    })()`);
    if (!switchedBackToPrimaryCandidate) {
      throw new Error(`primary candidate button was not found: ${primaryCandidateId}`);
    }
    await waitForCdpValue(cdp, `document.querySelector('#voxel-3d-diagnostic')?.dataset.state`, 'ready');
    await waitForCdpValue(
      cdp,
      `document.querySelector('#voxel-3d-panel')?.dataset.placementId === ${JSON.stringify(initial.placementId)}`,
      true,
    );

    const submittedGenerationConfig = await evaluateCdp(cdp, `(() => {
      const form = document.querySelector('#generation-config-form');
      const gap = document.querySelector('#generation-config-initial-column-gap');
      const clearance = document.querySelector('#generation-config-clearance');
      const walls = document.querySelector('#generation-config-wall-thickness');
      const corridor = document.querySelector('#generation-config-corridor-realization');
      if (
        !(form instanceof HTMLFormElement)
        || !(gap instanceof HTMLInputElement)
        || !(clearance instanceof HTMLInputElement)
        || !(walls instanceof HTMLInputElement)
        || !(corridor instanceof HTMLSelectElement)
      ) {
        return false;
      }
      gap.value = '160';
      clearance.value = '5';
      walls.value = '1';
      corridor.value = 'procedural';
      for (const input of [gap, clearance, walls, corridor]) {
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
      }
      form.requestSubmit();
      return true;
    })()`);
    if (!submittedGenerationConfig) {
      throw new Error('combined generation config controls were not available in Voxel 3D');
    }
    await waitForCdpValue(
      cdp,
      `document.querySelector('#generation-config-status')?.dataset.state`,
      'ready',
      30_000,
    );
    await waitForCdpValue(cdp, `document.querySelector('#generation-config-panel')?.dataset.configState`, 'persisted');
    await waitForCdpValue(cdp, `document.querySelector('#voxel-3d-panel')?.dataset.frameHash !== ${JSON.stringify(initial.frameHash)}`, true);
    const generationConfigReadout = await evaluateCdp(cdp, `(() => {
      const panel = document.querySelector('#generation-config-panel');
      const status = document.querySelector('#generation-config-status');
      const impact = document.querySelector('#generation-config-impact');
      return {
        configState: panel?.dataset.configState,
        buildId: panel?.dataset.buildId,
        columnGap: Number(document.querySelector('#generation-config-initial-column-gap')?.value),
        clearance: Number(document.querySelector('#generation-config-clearance')?.value),
        wallThickness: Number(document.querySelector('#generation-config-wall-thickness')?.value),
        corridorRealization: document.querySelector('#generation-config-corridor-realization')?.value,
        status: status?.textContent,
        impact: impact?.textContent,
        legacyPanelsHidden: [
          '#geometry-policy-panel',
          '#placement-policy-panel',
          '#corridor-realization-panel',
        ].every((selector) => document.querySelector(selector)?.hidden === true),
      };
    })()`);
    if (
      generationConfigReadout.configState !== 'persisted'
      || typeof generationConfigReadout.buildId !== 'string'
      || generationConfigReadout.buildId.length === 0
      || generationConfigReadout.columnGap !== 160
      || generationConfigReadout.clearance !== 5
      || generationConfigReadout.wallThickness !== 1
      || generationConfigReadout.corridorRealization !== 'procedural'
      || !String(generationConfigReadout.status).includes('Persisted configuration build')
      || !String(generationConfigReadout.impact).includes('Configured build:')
      || generationConfigReadout.legacyPanelsHidden !== true
    ) {
      throw new Error(`combined generation config readout was incomplete: ${JSON.stringify(generationConfigReadout)}`);
    }
    await evaluateCdp(cdp, `document.querySelector('[data-view="voxel"]')?.click()`);
    await waitForCdpValue(cdp, `document.querySelector('#layout')?.textContent.includes('no native authority receipt')`, true);
    await evaluateCdp(cdp, `document.querySelector('[data-view="voxel3d"]')?.click()`);
    await waitForCdpValue(cdp, `document.querySelector('#voxel-3d-diagnostic')?.dataset.state`, 'ready');
    const configured = await inspectionDataset(cdp);
    const configuredBuildId = generationConfigReadout.buildId;
    await evaluateCdp(cdp, `document.querySelector('#generation-config-reset')?.click()`);
    await waitForCdpValue(
      cdp,
      `document.querySelector('#generation-config-status')?.dataset.state`,
      'ready',
      30_000,
    );
    await waitForCdpValue(cdp, `document.querySelector('#generation-config-corridor-realization')?.value`, 'hybrid');
    await waitForCdpValue(cdp, `document.querySelector('#generation-config-panel')?.dataset.buildId !== ${JSON.stringify(configuredBuildId)}`, true);
    const resetReadout = await evaluateCdp(cdp, `(() => {
      const panel = document.querySelector('#generation-config-panel');
      return {
        configState: panel?.dataset.configState,
        buildId: panel?.dataset.buildId,
        columnGap: Number(document.querySelector('#generation-config-initial-column-gap')?.value),
        clearance: Number(document.querySelector('#generation-config-clearance')?.value),
        wallThickness: Number(document.querySelector('#generation-config-wall-thickness')?.value),
        corridorRealization: document.querySelector('#generation-config-corridor-realization')?.value,
      };
    })()`);
    if (
      resetReadout.configState !== 'persisted'
      || resetReadout.columnGap !== 144
      || resetReadout.clearance !== 3
      || resetReadout.wallThickness !== 1
      || resetReadout.corridorRealization !== 'hybrid'
    ) {
      throw new Error(`generation config defaults did not rebuild and persist: ${JSON.stringify(resetReadout)}`);
    }
    const resetBuild = await inspectionDataset(cdp);

    const screenshot = await cdp.send('Page.captureScreenshot', { format: 'png', fromSurface: true });
    await writeFile(join(outDir, 'voxel-3d-desktop.png'), screenshot.data, 'base64');

    let replacement = resetBuild;
    if (alternateCandidateId !== undefined) {
      const switched = await evaluateCdp(cdp, `(() => {
        const button = [...document.querySelectorAll('.candidate-button')]
          .find((candidate) => candidate.dataset.candidateId === ${JSON.stringify(alternateCandidateId)});
        button?.click();
        return button !== undefined;
      })()`);
      if (!switched) {
        throw new Error(`alternate candidate button was not found: ${alternateCandidateId}`);
      }
      await waitForCdpValue(cdp, `document.querySelector('#voxel-3d-diagnostic')?.dataset.state`, 'error');
      await waitForCdpValue(
        cdp,
        `document.querySelector('#voxel-3d-panel')?.dataset.nativeAuthority`,
        'rejected',
      );
      const rejectedReplacement = await inspectionDataset(cdp);
      if (
        rejectedReplacement.frameHash !== undefined
        || rejectedReplacement.rendererHost !== undefined
        || Number.isFinite(rejectedReplacement.gridRevision)
      ) {
        throw new Error(
          `unverified candidate changed the retained Engine frame: ${JSON.stringify({
            resetBuild,
            rejectedReplacement,
          })}`,
        );
      }
    }
    const revisions = [
      initial.cameraRevision,
      keyboardOrbit.cameraRevision,
      keyboardMovement.cameraRevision,
      keyboardZoom.cameraRevision,
      pointerOrbit.cameraRevision,
      wheelZoom.cameraRevision,
    ];
    if (revisions.some((revision, index) => index > 0 && revision <= revisions[index - 1])) {
      throw new Error(`engine camera revisions did not advance for every control path: ${revisions.join(',')}`);
    }
    const disposed = await evaluateCdp(cdp, `(() => {
      window.dispatchEvent(new Event('pagehide'));
      return document.querySelector('#voxel-3d-panel')?.dataset.disposed;
    })()`);
    if (disposed !== 'true') {
      throw new Error(`renderer disposal was not observable at pagehide: ${JSON.stringify(disposed)}`);
    }
    return {
      cameraRevisions: revisions,
      initialDistance: initial.cameraDistance,
      finalDistance: wheelZoom.cameraDistance,
      controlPaths: ['keyboard_orbit', 'keyboard_movement', 'keyboard_zoom', 'pointer_orbit', 'wheel_zoom'],
      generationConfig: {
        buildId: configuredBuildId,
        clearance: generationConfigReadout.clearance,
        wallThickness: generationConfigReadout.wallThickness,
        corridorRealization: generationConfigReadout.corridorRealization,
        configuredFrameHash: configured.frameHash,
        resetBuildId: resetReadout.buildId,
        resetFrameHash: resetBuild.frameHash,
        persisted: true,
      },
      gridLines: replacement.gridLineCount,
      initialGridRevision: initial.gridRevision,
      replacementGridRevision: replacement.gridRevision,
      replacementPlacementId: replacement.placementId,
      candidateReplacementExercised: alternateCandidateId !== undefined,
      unverifiedCandidateRejected: alternateCandidateId !== undefined,
      resizeHashes: [initial.viewportHash, resized.viewportHash],
      staleReplacementPreservedLatest: alternateCandidateId !== undefined,
      disposedOnPagehide: true,
    };
  } catch (error) {
    throw new Error(`${error.message}\nChromium log:\n${browserLog}`);
  } finally {
    cdp?.close();
    browser.kill('SIGTERM');
    await waitForChildExit(browser);
    await rm(profileDir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  }
}

async function exerciseCaTrace(chromium, evidence) {
  if (
    evidence.kind !== 'rusty_procgen.evidence.engine_ca_benchmark.v1'
    || !Array.isArray(evidence.scenarios)
  ) {
    throw new Error('CA benchmark evidence route did not return the accepted contract');
  }
  const expected = new Map(evidence.scenarios.map((scenario) => [scenario.scenarioId, scenario]));
  for (const scenarioId of [
    'sparse-propagation',
    'dense-churn',
    'cross-boundary',
    'large-resident-small-hot-region',
    'high-surface-area',
  ]) {
    if (!expected.has(scenarioId)) {
      throw new Error(`CA browser smoke is missing scenario ${scenarioId}`);
    }
  }

  const profileDir = join(outDir, 'chromium-ca-trace-profile');
  const cdpPort = Number(process.env.VIEWER_SMOKE_CA_CDP_PORT ?? port + 1001);
  await rm(profileDir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  const url = `${baseUrl}/#ca`;
  const browser = spawn(chromium, [
    '--headless',
    '--no-sandbox',
    '--enable-unsafe-swiftshader',
    `--remote-debugging-port=${cdpPort}`,
    `--user-data-dir=${profileDir}`,
    '--window-size=1280,860',
    url,
  ], { stdio: ['ignore', 'ignore', 'pipe'] });
  let browserLog = '';
  browser.stderr.on('data', (chunk) => {
    browserLog += chunk.toString();
  });
  let cdp;
  try {
    const page = await waitForCdpPage(cdpPort, url);
    cdp = await connectCdp(page.webSocketDebuggerUrl);
    await waitForCdpValue(
      cdp,
      `document.querySelector('#ca-trace-panel')?.dataset.state`,
      'ready',
      30_000,
    );
    const sparseEvidence = expected.get('sparse-propagation');
    const initial = await caTraceDataset(cdp);
    if (
      initial.scenarioId !== 'sparse-propagation'
      || initial.step !== 0
      || initial.authoritySource !== 'captured_engine_trace'
      || initial.timingRole !== 'observational_non_gating'
      || initial.rendererHost !== 'rusty_renderer_inspection_surface.v1'
      || initial.rendererRole !== 'projection_only_inspection'
      || initial.rendererStatus !== 'running'
      || initial.gridLineCount <= 0
      || initial.retainedOpCount <= 0
      || initial.traceHash !== sparseEvidence.trace.initial.traceHash
      || initial.projectionStateHash !== sparseEvidence.trace.initial.projectionStateHash
      || initial.meshChunkCount !== sparseEvidence.trace.initial.readout.meshChunkCount
    ) {
      throw new Error(`initial CA trace projection is incomplete: ${JSON.stringify(initial)}`);
    }

    await evaluateCdp(cdp, `document.querySelector('#ca-trace-step')?.focus()`);
    await cdp.send('Input.dispatchKeyEvent', {
      type: 'keyDown',
      key: 'Enter',
      code: 'Enter',
      text: '\r',
      unmodifiedText: '\r',
      windowsVirtualKeyCode: 13,
    });
    await cdp.send('Input.dispatchKeyEvent', {
      type: 'keyUp',
      key: 'Enter',
      code: 'Enter',
      windowsVirtualKeyCode: 13,
    });
    await waitForCdpValue(cdp, `Number(document.querySelector('#ca-trace-panel')?.dataset.step)`, 1);
    await waitForCdpValue(
      cdp,
      `Number(document.querySelector('#ca-trace-panel')?.dataset.presentationSample) > ${initial.presentationSample}`,
      true,
    );
    const stepped = await caTraceDataset(cdp);
    if (
      stepped.traceHash !== sparseEvidence.trace.steps[0].traceHash
      || stepped.projectionStateHash !== sparseEvidence.trace.steps[0].projectionStateHash
      || stepped.meshChunkCount !== sparseEvidence.trace.steps[0].readout.meshChunkCount
    ) {
      throw new Error(`CA step readout diverged from captured evidence: ${JSON.stringify(stepped)}`);
    }
    const sparseTimingSamples = await collectCaTimingSamples(
      cdp,
      sparseEvidence,
      initial,
      stepped,
    );

    await evaluateCdp(cdp, `document.querySelector('#ca-trace-reset')?.focus()`);
    await cdp.send('Input.dispatchKeyEvent', {
      type: 'keyDown',
      key: 'Enter',
      code: 'Enter',
      text: '\r',
      unmodifiedText: '\r',
      windowsVirtualKeyCode: 13,
    });
    await cdp.send('Input.dispatchKeyEvent', {
      type: 'keyUp',
      key: 'Enter',
      code: 'Enter',
      windowsVirtualKeyCode: 13,
    });
    await waitForCdpValue(cdp, `Number(document.querySelector('#ca-trace-panel')?.dataset.step)`, 0);
    await waitForCdpValue(cdp, `document.querySelector('#ca-trace-panel')?.dataset.frameHash`, initial.frameHash);
    const reset = await caTraceDataset(cdp);
    if (reset.traceHash !== initial.traceHash || reset.replacementCount <= initial.replacementCount) {
      throw new Error(`CA reset did not restore exact initial authority: ${JSON.stringify(reset)}`);
    }

    const runChanged = await evaluateCdp(cdp, `(() => {
      const select = document.querySelector('#ca-trace-run');
      if (!(select instanceof HTMLSelectElement) || select.options.length < 2) return false;
      select.value = select.options[1].value;
      select.dispatchEvent(new Event('change', { bubbles: true }));
      return true;
    })()`);
    if (runChanged) {
      await waitForCdpValue(cdp, `Number(document.querySelector('#ca-trace-panel')?.dataset.run)`, 2);
    }

    await evaluateCdp(cdp, `(() => {
      const rate = document.querySelector('#ca-trace-rate');
      if (!(rate instanceof HTMLSelectElement)) return false;
      rate.value = '4';
      rate.dispatchEvent(new Event('change', { bubbles: true }));
      document.querySelector('#ca-trace-play')?.click();
      return true;
    })()`);
    await waitForCdpValue(
      cdp,
      `Number(document.querySelector('#ca-trace-panel')?.dataset.step) >= 2`,
      true,
    );
    await evaluateCdp(cdp, `document.querySelector('#ca-trace-play')?.click()`);

    const scenarioReadouts = {};
    for (const scenarioId of ['dense-churn', 'cross-boundary', 'high-surface-area']) {
      const scenarioEvidence = expected.get(scenarioId);
      await evaluateCdp(cdp, `(() => {
        const select = document.querySelector('#ca-trace-scenario');
        if (!(select instanceof HTMLSelectElement)) return false;
        select.value = ${JSON.stringify(scenarioId)};
        select.dispatchEvent(new Event('change', { bubbles: true }));
        return true;
      })()`);
      await waitForCdpValue(cdp, `document.querySelector('#ca-trace-panel')?.dataset.state`, 'ready');
      await waitForCdpValue(cdp, `document.querySelector('#ca-trace-panel')?.dataset.scenarioId`, scenarioId);
      const scenarioInitial = await caTraceDataset(cdp);
      if (
        scenarioInitial.traceHash !== scenarioEvidence.trace.initial.traceHash
        || scenarioInitial.projectionStateHash !== scenarioEvidence.trace.initial.projectionStateHash
        || scenarioInitial.meshChunkCount !== scenarioEvidence.trace.initial.readout.meshChunkCount
      ) {
        throw new Error(`${scenarioId} initial browser projection diverged from evidence`);
      }
      await evaluateCdp(cdp, `document.querySelector('#ca-trace-step')?.click()`);
      await waitForCdpValue(cdp, `Number(document.querySelector('#ca-trace-panel')?.dataset.step)`, 1);
      await waitForCdpValue(
        cdp,
        `Number(document.querySelector('#ca-trace-panel')?.dataset.presentationSample) > ${scenarioInitial.presentationSample}`,
        true,
      );
      const scenarioStep = await caTraceDataset(cdp);
      if (
        scenarioStep.traceHash !== scenarioEvidence.trace.steps[0].traceHash
        || scenarioStep.projectionStateHash !== scenarioEvidence.trace.steps[0].projectionStateHash
      ) {
        throw new Error(`${scenarioId} step browser projection diverged from evidence`);
      }
      scenarioReadouts[scenarioId] = {
        initialTraceHash: scenarioInitial.traceHash,
        stepTraceHash: scenarioStep.traceHash,
        chunks: scenarioStep.meshChunkCount,
        timingSamples: await collectCaTimingSamples(
          cdp,
          scenarioEvidence,
          scenarioInitial,
          scenarioStep,
        ),
      };
    }

    const largeEvidence = expected.get('large-resident-small-hot-region');
    await evaluateCdp(cdp, `(() => {
      const select = document.querySelector('#ca-trace-scenario');
      select.value = 'large-resident-small-hot-region';
      select.dispatchEvent(new Event('change', { bubbles: true }));
    })()`);
    await waitForCdpValue(cdp, `document.querySelector('#ca-trace-panel')?.dataset.state`, 'ready');
    await waitForCdpValue(
      cdp,
      `document.querySelector('#ca-trace-panel')?.dataset.scenarioId`,
      'large-resident-small-hot-region',
    );
    const largeInitial = await caTraceDataset(cdp);
    await evaluateCdp(cdp, `document.querySelector('#ca-trace-step')?.click()`);
    await waitForCdpValue(cdp, `Number(document.querySelector('#ca-trace-panel')?.dataset.step)`, 1);
    await waitForCdpValue(
      cdp,
      `Number(document.querySelector('#ca-trace-panel')?.dataset.presentationSample) > ${largeInitial.presentationSample}`,
      true,
    );
    const largeStep = await caTraceDataset(cdp);
    if (largeStep.traceHash !== largeEvidence.trace.steps[0].traceHash) {
      throw new Error('large-resident browser step diverged from captured evidence');
    }
    scenarioReadouts['large-resident-small-hot-region'] = {
      initialTraceHash: largeInitial.traceHash,
      stepTraceHash: largeStep.traceHash,
      chunks: largeStep.meshChunkCount,
      timingSamples: await collectCaTimingSamples(
        cdp,
        largeEvidence,
        largeInitial,
        largeStep,
      ),
    };
    await evaluateCdp(cdp, `document.querySelector('#ca-trace-reset')?.click()`);
    await waitForCdpValue(cdp, `Number(document.querySelector('#ca-trace-panel')?.dataset.step)`, 0);
    await waitForCdpValue(cdp, `document.querySelector('#ca-trace-panel')?.dataset.frameHash`, largeInitial.frameHash);
    const large = await caTraceDataset(cdp);
    const expectedLargeRetainedOps = largeEvidence.trace.initial.readout.meshChunkCount + 7;
    if (large.retainedOpCount !== expectedLargeRetainedOps) {
      throw new Error(`large retained projection has ${large.retainedOpCount} ops, expected ${expectedLargeRetainedOps}`);
    }
    await evaluateCdp(cdp, `(() => {
      const select = document.querySelector('#ca-trace-scenario');
      select.value = 'dense-churn';
      select.dispatchEvent(new Event('change', { bubbles: true }));
    })()`);
    await waitForCdpValue(cdp, `document.querySelector('#ca-trace-panel')?.dataset.state`, 'ready');
    await waitForCdpValue(cdp, `document.querySelector('#ca-trace-panel')?.dataset.scenarioId`, 'dense-churn');
    const afterRelease = await caTraceDataset(cdp);
    const expectedDenseRetainedOps =
      expected.get('dense-churn').trace.initial.readout.meshChunkCount + 7;
    if (
      afterRelease.retainedOpCount !== expectedDenseRetainedOps
      || afterRelease.retainedOpCount >= large.retainedOpCount
    ) {
      throw new Error(`scenario replacement retained obsolete resources: ${large.retainedOpCount} -> ${afterRelease.retainedOpCount}`);
    }
    const crossEvidence = expected.get('cross-boundary');
    await evaluateCdp(cdp, `(() => {
      const scenario = document.querySelector('#ca-trace-scenario');
      const seek = document.querySelector('#ca-trace-seek');
      scenario.value = 'cross-boundary';
      scenario.dispatchEvent(new Event('change', { bubbles: true }));
      return seek instanceof HTMLInputElement;
    })()`);
    await waitForCdpValue(cdp, `document.querySelector('#ca-trace-panel')?.dataset.state`, 'ready');
    await waitForCdpValue(cdp, `document.querySelector('#ca-trace-panel')?.dataset.scenarioId`, 'cross-boundary');
    await evaluateCdp(cdp, `document.querySelector('#ca-trace-seek')?.focus()`);
    await cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'End', code: 'End' });
    await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'End', code: 'End' });
    await waitForCdpValue(
      cdp,
      `Number(document.querySelector('#ca-trace-panel')?.dataset.step)`,
      crossEvidence.trace.steps.length,
    );
    const selectedFinalStep = await caTraceDataset(cdp);
    if (selectedFinalStep.traceHash !== crossEvidence.trace.steps.at(-1).traceHash) {
      throw new Error('bounded step selection did not reach the captured cross-boundary final state');
    }

    await evaluateCdp(cdp, `document.querySelector('#ca-trace-canvas')?.scrollIntoView({ block: 'center' })`);
    const rect = await evaluateCdp(cdp, `(() => {
      const rect = document.querySelector('#ca-trace-canvas').getBoundingClientRect();
      return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
    })()`);
    const x = rect.x + rect.width * 0.5;
    const y = rect.y + rect.height * 0.5;
    await cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 });
    await cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 });
    await cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'ArrowRight', code: 'ArrowRight' });
    await delay(180);
    await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'ArrowRight', code: 'ArrowRight' });
    await waitForCdpValue(cdp, `document.querySelector('#ca-trace-panel')?.dataset.lastCameraChange`, 'keyboard_orbit');
    const camera = await caTraceDataset(cdp);

    await cdp.send('Emulation.setDeviceMetricsOverride', {
      width: 980,
      height: 720,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await waitForCdpValue(
      cdp,
      `document.querySelector('#ca-trace-panel')?.dataset.viewportHash !== ${JSON.stringify(camera.viewportHash)}`,
      true,
    );
    const resized = await caTraceDataset(cdp);
    await cdp.send('Emulation.setDeviceMetricsOverride', {
      width: 1600,
      height: 860,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await waitForCdpValue(
      cdp,
      `document.querySelector('#ca-trace-panel')?.dataset.viewportHash !== ${JSON.stringify(resized.viewportHash)}`,
      true,
    );
    await evaluateCdp(cdp, `document.querySelector('#ca-trace-panel')?.scrollIntoView({ block: 'start' })`);
    await delay(100);
    const screenshot = await cdp.send('Page.captureScreenshot', { format: 'png', fromSurface: true });
    await writeFile(join(outDir, 'ca-trace-desktop.png'), screenshot.data, 'base64');
    const screenshotFile = await stat(join(outDir, 'ca-trace-desktop.png'));
    if (screenshotFile.size < 5_000) {
      throw new Error('CA trace screenshot is too small to prove a rendered inspection view');
    }

    const disposed = await evaluateCdp(cdp, `(() => {
      window.dispatchEvent(new Event('pagehide'));
      return document.querySelector('#ca-trace-panel')?.dataset.disposed;
    })()`);
    if (disposed !== 'true') {
      throw new Error(`CA trace renderer disposal was not observable: ${JSON.stringify(disposed)}`);
    }
    return {
      authority: initial.authoritySource,
      timingRole: initial.timingRole,
      sparse: {
        initialTraceHash: initial.traceHash,
        stepTraceHash: stepped.traceHash,
        resetFrameHash: reset.frameHash,
        timingSamples: sparseTimingSamples,
      },
      scenarios: scenarioReadouts,
      cameraRevision: camera.cameraRevision,
      cameraControl: camera.lastCameraChange,
      resizeHashes: [camera.viewportHash, resized.viewportHash],
      deterministicReset: true,
      retainedOpsAfterLargeToDense: [large.retainedOpCount, afterRelease.retainedOpCount],
      obsoleteResourcesReleased: true,
      boundedStepSelection: {
        scenario: 'cross-boundary',
        step: selectedFinalStep.step,
        traceHash: selectedFinalStep.traceHash,
      },
      disposedOnPagehide: true,
    };
  } catch (error) {
    throw new Error(`${error.message}\nChromium CA trace log:\n${browserLog}`);
  } finally {
    cdp?.close();
    browser.kill('SIGTERM');
    await waitForChildExit(browser);
    await rm(profileDir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  }
}

async function caTraceDataset(cdp) {
  return await evaluateCdp(cdp, `(() => {
    const data = document.querySelector('#ca-trace-panel').dataset;
    return {
      authoritySource: data.authoritySource,
      timingRole: data.timingRole,
      scenarioId: data.scenarioId,
      step: Number(data.step),
      run: Number(data.run),
      traceHash: data.traceHash,
      projectionStateHash: data.projectionStateHash,
      meshChunkCount: Number(data.meshChunkCount),
      sourceRevision: Number(data.sourceRevision),
      replacementCount: Number(data.replacementCount),
      rendererHost: data.rendererHost,
      rendererRole: data.rendererRole,
      rendererStatus: data.rendererStatus,
      frameHash: data.frameHash,
      retainedOpCount: Number(data.retainedOpCount),
      gridLineCount: Number(data.gridLineCount),
      cameraRevision: Number(data.cameraRevision),
      lastCameraChange: data.lastCameraChange,
      viewportHash: data.viewportHash,
      rendererApplicationMs: Number(data.rendererApplicationMs),
      renderSubmissionMs: Number(data.renderSubmissionMs),
      browserPresentationMs: Number(data.browserPresentationMs),
      presentationSample: Number(data.presentationSample),
    };
  })()`);
}

async function collectCaTimingSamples(
  cdp,
  scenarioEvidence,
  initial,
  firstStep,
) {
  const samples = [caBrowserTiming(firstStep)];
  while (samples.length < 3) {
    await evaluateCdp(cdp, `document.querySelector('#ca-trace-reset')?.click()`);
    await waitForCdpValue(
      cdp,
      `Number(document.querySelector('#ca-trace-panel')?.dataset.step)`,
      0,
    );
    await waitForCdpValue(
      cdp,
      `document.querySelector('#ca-trace-panel')?.dataset.frameHash`,
      initial.frameHash,
    );
    const before = await caTraceDataset(cdp);
    await evaluateCdp(cdp, `document.querySelector('#ca-trace-step')?.click()`);
    await waitForCdpValue(
      cdp,
      `Number(document.querySelector('#ca-trace-panel')?.dataset.step)`,
      1,
    );
    await waitForCdpValue(
      cdp,
      `Number(document.querySelector('#ca-trace-panel')?.dataset.presentationSample) > ${before.presentationSample}`,
      true,
    );
    const stepped = await caTraceDataset(cdp);
    if (
      stepped.traceHash !== scenarioEvidence.trace.steps[0].traceHash
      || stepped.projectionStateHash !== scenarioEvidence.trace.steps[0].projectionStateHash
      || stepped.meshChunkCount !== scenarioEvidence.trace.steps[0].readout.meshChunkCount
    ) {
      throw new Error(
        `${scenarioEvidence.scenarioId} repeated timing step diverged from captured evidence`,
      );
    }
    samples.push(caBrowserTiming(stepped));
  }
  return samples;
}

function caBrowserTiming(readout) {
  return {
    rendererApplicationMs: readout.rendererApplicationMs,
    renderSubmissionMs: readout.renderSubmissionMs,
    browserPresentationMs: readout.browserPresentationMs,
  };
}

async function inspectionDataset(cdp) {
  return await evaluateCdp(cdp, `(() => {
    const data = document.querySelector('#voxel-3d-panel').dataset;
    return {
      cameraRevision: Number(data.cameraRevision),
      cameraDistance: Number(data.cameraDistance),
      gridRevision: Number(data.gridRevision),
      gridLineCount: Number(data.gridLineCount),
      lastCameraChange: data.lastCameraChange,
      placementId: data.placementId,
      frameHash: data.frameHash,
      policyMode: data.policyMode,
      policyExperimentId: data.policyExperimentId,
      nativeAuthority: data.nativeAuthority,
      planSha256: data.planSha256,
      doorNodeCount: Number(data.doorNodeCount),
      lockedDoorCount: Number(data.lockedDoorCount),
      unlockedDoorCount: Number(data.unlockedDoorCount),
      rendererHost: data.rendererHost,
      rendererRole: data.rendererRole,
      rendererCompatibilityVersion: data.rendererCompatibilityVersion,
      rendererStatus: data.rendererStatus,
      retainedOpCount: Number(data.retainedOpCount),
      viewportHash: data.viewportHash,
    };
  })()`);
}

async function waitForCdpPage(cdpPort, url) {
  const started = Date.now();
  while (Date.now() - started < 10_000) {
    try {
      const targets = await fetch(`http://127.0.0.1:${cdpPort}/json/list`).then((response) => response.json());
      const page = targets.find((target) => target.type === 'page' && target.url.startsWith(url.split('#')[0]));
      if (page?.webSocketDebuggerUrl) return page;
    } catch {
      // Chromium is still starting.
    }
    await delay(50);
  }
  throw new Error(`Chromium CDP page did not start on port ${cdpPort}`);
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
      if (message.error) request.reject(new Error(message.error.message));
      else request.resolve(message.result);
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
    throw new Error(response.exceptionDetails.exception?.description ?? response.exceptionDetails.text);
  }
  return response.result.value;
}

async function waitForCdpValue(cdp, expression, expected, timeoutMs = 10_000) {
  const started = Date.now();
  let actual;
  while (Date.now() - started < timeoutMs) {
    actual = await evaluateCdp(cdp, expression);
    if (actual === expected) return;
    await delay(50);
  }
  throw new Error(`timed out waiting for ${expression}; expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function waitForChildExit(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  await new Promise((resolve) => child.once('exit', resolve));
}

function attributeValue(dom, name) {
  return dom.match(new RegExp(`${name}="([^"]*)"`))?.[1] ?? '';
}

async function findChromium() {
  for (const command of ['chromium', 'chromium-browser', 'google-chrome']) {
    try {
      const { stdout } = await execFileAsync('sh', ['-lc', `command -v ${command}`]);
      const resolved = stdout.trim();
      if (resolved.length > 0) {
        return resolved;
      }
    } catch {
      // Try next candidate.
    }
  }
  const hint = await readFile('/etc/os-release', 'utf8').catch(() => '');
  throw new Error(`chromium executable not found; install chromium to run viewer smoke\n${hint}`);
}
