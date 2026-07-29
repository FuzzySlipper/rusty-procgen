import { createHash } from 'node:crypto';
import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const currentRoot = join(repoRoot, 'artifacts');
const manifestPath = join(repoRoot, 'migration/corpus-identity-v1.json');
const beforeArgument = process.argv.indexOf('--before');

if (beforeArgument >= 0) {
  const beforeRoot = resolve(process.argv[beforeArgument + 1] ?? '');
  recordIdentityProof(beforeRoot);
} else {
  checkRecordedIdentity();
}

function recordIdentityProof(beforeRoot) {
  if (!statSync(beforeRoot).isDirectory()) {
    throw new Error(`--before must name an artifact directory: ${beforeRoot}`);
  }
  const beforeFiles = listCorpusFiles(beforeRoot);
  const currentFiles = listCorpusFiles(currentRoot);
  compareFileSets(beforeFiles, currentFiles);

  const mismatches = [];
  for (const relativePath of currentFiles) {
    const before = normalizeComparable(
      normalizePredecessor(readFileSync(join(beforeRoot, relativePath), 'utf8')),
    );
    const current = normalizeComparable(readFileSync(join(currentRoot, relativePath), 'utf8'));
    if (before !== current) {
      mismatches.push(relativePath);
    }
  }
  if (mismatches.length > 0) {
    throw new Error(
      `identity cutover changed ${mismatches.length} corpus file(s) beyond normalized identity: `
      + mismatches.slice(0, 12).join(', '),
    );
  }

  const selection = JSON.parse(
    readFileSync(join(currentRoot, 'samples/batch-v2/selection-report.json'), 'utf8'),
  );
  const accepted = selection.accepted ?? [];
  const rejected = selection.rejected ?? [];
  const placementFiles = currentFiles.filter((path) => path.endsWith('/piece-placement.json'));
  const proof = {
    kind: 'rusty_procgen.migration.corpus_identity.v1',
    schemaVersion: 1,
    predecessorNormalization: {
      artifactNamespace: ['asha_procgen.', 'rusty_procgen.'],
      repositoryIdentity: ['asha-procgen', 'rusty-procgen'],
      displayIdentity: ['Asha Procgen|ASHA Procgen', 'Rusty Procgen'],
      identityDerivedJsonHashValues: 'normalized_to_hash_sentinel',
    },
    corpus: {
      root: 'artifacts',
      includedPaths: ['samples/**'],
      fileCount: currentFiles.length,
      normalizedSha256: hashComparableCorpus(currentRoot, currentFiles),
      exactNormalizedFileEquivalence: true,
    },
    behavior: {
      acceptedCount: accepted.length,
      rejectedCount: rejected.length,
      acceptedTopologyFingerprints: accepted
        .map((entry) => entry.topologyFingerprint)
        .filter((value) => typeof value === 'string')
        .sort(),
      rejectedReasons: rejected
        .map((entry) => entry.reason)
        .filter((value) => typeof value === 'string')
        .sort(),
      placementArtifactCount: placementFiles.length,
    },
  };
  writeFileSync(manifestPath, `${JSON.stringify(proof, null, 2)}\n`);
  console.log(
    `recorded exact normalized identity equivalence for ${currentFiles.length} files `
    + `(${accepted.length} accepted, ${rejected.length} rejected, `
    + `${placementFiles.length} placement artifacts)`,
  );
}

function checkRecordedIdentity() {
  const proof = JSON.parse(readFileSync(manifestPath, 'utf8'));
  if (
    proof.kind !== 'rusty_procgen.migration.corpus_identity.v1'
    || proof.schemaVersion !== 1
    || proof.corpus?.exactNormalizedFileEquivalence !== true
  ) {
    throw new Error('corpus identity manifest is missing its exact-equivalence contract');
  }
  const files = listCorpusFiles(currentRoot);
  const currentHash = hashComparableCorpus(currentRoot, files);
  if (files.length !== proof.corpus.fileCount || currentHash !== proof.corpus.normalizedSha256) {
    throw new Error(
      `checked corpus drifted from identity proof: expected ${proof.corpus.fileCount} files / `
      + `${proof.corpus.normalizedSha256}, got ${files.length} files / ${currentHash}`,
    );
  }
  console.log(
    `rusty-procgen corpus identity check passed `
    + `(${files.length} files, ${proof.behavior.acceptedCount} accepted, `
    + `${proof.behavior.rejectedCount} rejected, `
    + `${proof.behavior.placementArtifactCount} placement artifacts)`,
  );
}

function normalizePredecessor(text) {
  return text
    .replaceAll('asha_procgen.', 'rusty_procgen.')
    .replaceAll('asha-procgen', 'rusty-procgen')
    .replaceAll('ASHA Procgen', 'Rusty Procgen')
    .replaceAll('Asha Procgen', 'Rusty Procgen');
}

function normalizeComparable(text) {
  return text.replace(
    /"([A-Za-z0-9_]*[Hh]ash)"\s*:\s*"[^"]+"/g,
    '"$1": "<identity-derived-hash>"',
  );
}

function listFiles(root) {
  const files = [];
  visit(root);
  return files.sort();

  function visit(directory) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const absolute = join(directory, entry.name);
      if (entry.isDirectory()) {
        visit(absolute);
      } else if (entry.isFile()) {
        files.push(relative(root, absolute).split(sep).join('/'));
      }
    }
  }
}

function listCorpusFiles(root) {
  return listFiles(root).filter((path) => path.startsWith('samples/'));
}

function compareFileSets(before, current) {
  if (JSON.stringify(before) === JSON.stringify(current)) {
    return;
  }
  const missing = before.filter((path) => !current.includes(path));
  const added = current.filter((path) => !before.includes(path));
  throw new Error(
    `identity cutover changed corpus file set; missing=${missing.join(', ')}, added=${added.join(', ')}`,
  );
}

function hashComparableCorpus(root, files) {
  const hash = createHash('sha256');
  for (const relativePath of files) {
    hash.update(relativePath);
    hash.update('\0');
    hash.update(normalizeComparable(readFileSync(join(root, relativePath), 'utf8')));
    hash.update('\0');
  }
  return `sha256:${hash.digest('hex')}`;
}
