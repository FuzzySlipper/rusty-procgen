import { createHash } from 'node:crypto';
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const corpusRoot = join(repoRoot, 'artifacts');
const manifestPath = join(repoRoot, 'migration/corpus-identity-v1.json');
const proof = JSON.parse(readFileSync(manifestPath, 'utf8'));

if (
  proof.kind !== 'rusty_procgen.migration.corpus_identity.v1'
  || proof.schemaVersion !== 1
  || proof.corpus?.exactNormalizedFileEquivalence !== true
) {
  throw new Error('corpus identity manifest is missing its exact-equivalence contract');
}

const files = listCorpusFiles(corpusRoot);
const currentHash = hashComparableCorpus(corpusRoot, files);
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

function normalizeComparable(text) {
  return text.replace(
    /"([A-Za-z0-9_]*[Hh]ash)"\s*:\s*"[^"]+"/g,
    '"$1": "<identity-derived-hash>"',
  );
}

function listCorpusFiles(root) {
  const files = [];
  visit(root);
  return files.filter((path) => path.startsWith('samples/')).sort();

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
