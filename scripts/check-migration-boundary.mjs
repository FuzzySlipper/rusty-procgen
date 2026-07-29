import { readFileSync, readdirSync } from 'node:fs';
import { dirname, extname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const packageJson = readJson(join(repoRoot, 'package.json'));
const lockText = readFileSync(join(repoRoot, 'pnpm-lock.yaml'), 'utf8');
const ledger = readJson(join(repoRoot, 'migration/predecessor-disposition.json'));
const scannedExtensions = new Set([
  '.cjs',
  '.cts',
  '.html',
  '.js',
  '.json',
  '.jsx',
  '.md',
  '.mjs',
  '.mts',
  '.rs',
  '.toml',
  '.ts',
  '.tsx',
  '.yaml',
  '.yml',
]);
const importExtensions = new Set(['.cjs', '.cts', '.js', '.jsx', '.mjs', '.mts', '.ts', '.tsx']);
const ignoredDirectories = new Set(['.git', 'dist', 'node_modules', 'target']);
const historicalRecords = new Set([
  'migration/predecessor-disposition.json',
  'migration/corpus-identity-v1.json',
]);
const errors = [];
const sourceFiles = [];

validateLedger();
collectSourceFiles(repoRoot);
checkDependencies();
checkTransitivePackages();
checkImports();
checkRetiredIntegrationMentions();
checkArtifactNamespaces();

if (errors.length > 0) {
  console.error('rusty-procgen migration boundary check failed:');
  for (const error of errors) {
    console.error(`- ${error}`);
  }
  process.exit(1);
}

console.log(
  `rusty-procgen terminal migration boundary passed `
  + `(${ledger.schemaFamilies.length} converted schema families, `
  + `${ledger.newSchemaFamilies.length} repository-owned schema families, `
  + '0 predecessor dependencies/imports/scripts).',
);

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'));
  } catch (error) {
    throw new Error(`cannot read required JSON ${path}: ${error.message}`);
  }
}

function validateLedger() {
  if (packageJson.name !== 'rusty-procgen') {
    errors.push(`package.json name must be rusty-procgen, got ${JSON.stringify(packageJson.name)}`);
  }
  if (
    ledger.kind !== 'rusty_procgen.migration.predecessor_disposition.v2'
    || ledger.schemaVersion !== 2
    || ledger.status !== 'complete'
    || ledger.completedByTask !== 6400
  ) {
    errors.push('migration ledger must be the terminal task-6400 predecessor-disposition v2 record');
  }
  if (
    ledger.current?.project !== 'rusty-procgen'
    || ledger.current?.artifactNamespace !== 'rusty_procgen.'
    || ledger.current?.legacyArtifactDecoding !== 'forbidden'
  ) {
    errors.push('migration ledger must declare the one-way clean-break current identity');
  }
  for (const field of [
    'pendingPaths',
    'activePredecessorDependencies',
    'activePredecessorImports',
    'activePredecessorScripts',
  ]) {
    if (!Array.isArray(ledger[field]) || ledger[field].length !== 0) {
      errors.push(`migration ledger ${field} must be an empty terminal array`);
    }
  }
  if (
    typeof ledger.predecessor?.project !== 'string'
    || typeof ledger.predecessor?.displayName !== 'string'
    || typeof ledger.predecessor?.artifactNamespace !== 'string'
    || typeof ledger.predecessor?.packageScope !== 'string'
  ) {
    errors.push('migration ledger must retain complete predecessor identity evidence');
  }
}

function collectSourceFiles(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      if (!ignoredDirectories.has(entry.name)) {
        collectSourceFiles(join(directory, entry.name));
      }
      continue;
    }
    if (!entry.isFile() || !scannedExtensions.has(extname(entry.name))) {
      continue;
    }
    const path = join(directory, entry.name);
    sourceFiles.push({
      path,
      displayPath: relative(repoRoot, path).split(sep).join('/'),
      text: readFileSync(path, 'utf8'),
    });
  }
}

function checkDependencies() {
  const packageScope = ledger.predecessor.packageScope;
  const sections = ['dependencies', 'devDependencies', 'peerDependencies', 'optionalDependencies'];
  const actual = [];
  for (const section of sections) {
    for (const [packageName, specifier] of Object.entries(packageJson[section] ?? {})) {
      if (packageName.startsWith(packageScope)) {
        actual.push({ section, package: packageName, specifier });
      }
    }
  }
  compareExact(
    'predecessor dependencies',
    actual,
    ledger.activePredecessorDependencies,
  );
}

function checkTransitivePackages() {
  const packageScope = ledger.predecessor.packageScope;
  if (lockText.includes(packageScope)) {
    errors.push(`pnpm-lock.yaml retains predecessor package scope ${packageScope}`);
  }
}

function checkImports() {
  const packageScope = ledger.predecessor.packageScope;
  const grouped = new Map();
  const patterns = [
    /\bfrom\s*['"]([^'"]+)['"]/g,
    /\bimport\s*['"]([^'"]+)['"]/g,
    /\bimport\s*\(\s*['"]([^'"]+)['"]\s*\)/g,
  ];
  for (const file of sourceFiles) {
    if (!importExtensions.has(extname(file.path))) {
      continue;
    }
    for (const pattern of patterns) {
      for (const match of file.text.matchAll(pattern)) {
        if (!match[1].startsWith(packageScope)) {
          continue;
        }
        const key = `${file.displayPath}\0${match[1]}`;
        grouped.set(key, (grouped.get(key) ?? 0) + 1);
      }
    }
  }
  const actual = [...grouped.entries()].map(([key, occurrences]) => {
    const [file, specifier] = key.split('\0');
    return { file, specifier, occurrences };
  });
  compareExact('predecessor imports', actual, ledger.activePredecessorImports);
}

function checkRetiredIntegrationMentions() {
  const retiredIdentities = [
    ledger.predecessor.project,
    ledger.predecessor.displayName,
    ledger.predecessor.artifactNamespace,
    ledger.predecessor.packageScope,
  ];
  for (const file of sourceFiles) {
    if (historicalRecords.has(file.displayPath)) {
      continue;
    }
    for (const identity of retiredIdentities) {
      if (file.text.includes(identity)) {
        errors.push(`${file.displayPath} retains predecessor identity ${JSON.stringify(identity)}`);
      }
    }
  }
}

function checkArtifactNamespaces() {
  const currentNamespace = ledger.current.artifactNamespace;
  const declaredCurrent = new Set([
    ...ledger.schemaFamilies.map((family) => `${currentNamespace}${family}`),
    ...ledger.newSchemaFamilies.map((family) => `${currentNamespace}${family}`),
  ]);
  const observedCurrent = new Set();

  for (const file of sourceFiles) {
    if (
      file.displayPath === 'config/viewer-generation.json'
      && !file.text.includes('"kind": "rusty_procgen.viewer_generation_config.v1"')
    ) {
      errors.push('config/viewer-generation.json must use the current artifact kind');
    }
    for (const match of file.text.matchAll(/rusty_procgen\.[A-Za-z0-9_]+(?:\.[A-Za-z0-9_]+)*/g)) {
      observedCurrent.add(match[0].replace(/\.$/, ''));
    }
  }

  const undeclared = [...observedCurrent].filter((value) => !declaredCurrent.has(value)).sort();
  const missing = [...declaredCurrent].filter((value) => !observedCurrent.has(value)).sort();
  if (undeclared.length > 0) {
    errors.push(`unledgered current artifact/schema identities: ${undeclared.join(', ')}`);
  }
  if (missing.length > 0) {
    errors.push(`ledgered artifact/schema identities are absent: ${missing.join(', ')}`);
  }
}

function compareExact(label, actual, expected) {
  const normalize = (value) => JSON.stringify(value, Object.keys(value).sort());
  const actualRows = actual.map(normalize).sort();
  const expectedRows = expected.map(normalize).sort();
  if (JSON.stringify(actualRows) === JSON.stringify(expectedRows)) {
    return;
  }
  const unexpected = actualRows.filter((row) => !expectedRows.includes(row));
  const absent = expectedRows.filter((row) => !actualRows.includes(row));
  if (unexpected.length > 0) {
    errors.push(`unledgered ${label}: ${unexpected.join(', ')}`);
  }
  if (absent.length > 0) {
    errors.push(`ledgered ${label} are absent: ${absent.join(', ')}`);
  }
}
