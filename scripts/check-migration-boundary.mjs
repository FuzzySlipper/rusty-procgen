import { readFileSync, readdirSync } from 'node:fs';
import { dirname, extname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const packageJson = readJson(join(repoRoot, 'package.json'));
const packageLockText = readFileSync(join(repoRoot, 'pnpm-lock.yaml'), 'utf8');
const ledgerPath = join(repoRoot, packageJson.legacyAshaMigration?.ledger ?? '');
const ledger = readJson(ledgerPath);
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
]);
const importExtensions = new Set(['.cjs', '.cts', '.js', '.jsx', '.mjs', '.mts', '.ts', '.tsx']);
const ignoredDirectories = new Set(['.git', 'dist', 'node_modules', 'target']);
const ignoredSchemaFiles = new Set([
  'migration/asha-disposition.json',
  'migration/corpus-identity-v1.json',
]);
const legacyNamespaceOwnerFiles = new Set([
  'scripts/check-corpus-identity.mjs',
  'scripts/check-migration-boundary.mjs',
]);
const errors = [];
const sourceFiles = [];

validateLedger();
collectSourceFiles(repoRoot);
checkDependencies();
checkTransitivePackages();
checkImports();
checkIntegrationScripts();
checkArtifactNamespaces();

if (errors.length > 0) {
  console.error('rusty-procgen migration boundary check failed:');
  for (const error of errors) {
    console.error(`- ${error}`);
  }
  process.exit(1);
}

console.log(
  `rusty-procgen migration boundary check passed `
  + `(${ledger.activeAshaDependencies.length} temporary packages, `
  + `${ledger.transitiveAshaPackages.length} transitive packages, `
  + `${ledger.activeAshaImports.length} exact import allowances, `
  + `${ledger.schemaFamilies.length} migrated schema families).`,
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
    ledger.kind !== 'rusty_procgen.migration.asha_disposition.v1'
    || ledger.schemaVersion !== 1
  ) {
    errors.push('migration ledger must use rusty_procgen.migration.asha_disposition.v1 schema 1');
  }
  if (
    ledger.predecessor?.artifactNamespace !== 'asha_procgen.'
    || ledger.current?.artifactNamespace !== 'rusty_procgen.'
    || ledger.current?.legacyArtifactDecoding !== 'forbidden'
  ) {
    errors.push('migration ledger must declare the one-way clean-break artifact namespace');
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
  const sections = ['dependencies', 'devDependencies', 'peerDependencies', 'optionalDependencies'];
  const actual = [];
  for (const section of sections) {
    for (const [packageName, specifier] of Object.entries(packageJson[section] ?? {})) {
      if (packageName.startsWith('@asha/')) {
        actual.push({ section, package: packageName, specifier });
      }
    }
  }
  const expected = ledger.activeAshaDependencies.map(({ section, package: packageName, specifier }) => ({
    section,
    package: packageName,
    specifier,
  }));
  compareExact('temporary Asha dependencies', actual, expected);
}

function checkTransitivePackages() {
  const lockedAshaPackages = [...new Set(
    [...packageLockText.matchAll(/@asha\/[A-Za-z0-9._-]+/g)].map((match) => match[0]),
  )].sort();
  const expected = ledger.transitiveAshaPackages.map((entry) => entry.package).sort();
  compareExact('transitive Asha packages', lockedAshaPackages, expected);
}

function checkImports() {
  const grouped = new Map();
  const patterns = [
    /\bfrom\s*['"](@asha\/[^'"]+)['"]/g,
    /\bimport\s*['"](@asha\/[^'"]+)['"]/g,
    /\bimport\s*\(\s*['"](@asha\/[^'"]+)['"]\s*\)/g,
  ];
  for (const file of sourceFiles) {
    if (!importExtensions.has(extname(file.path))) {
      continue;
    }
    for (const pattern of patterns) {
      for (const match of file.text.matchAll(pattern)) {
        const key = `${file.displayPath}\0${match[1]}`;
        grouped.set(key, (grouped.get(key) ?? 0) + 1);
      }
    }
  }
  const actual = [...grouped.entries()].map(([key, occurrences]) => {
    const [file, specifier] = key.split('\0');
    return { file, specifier, occurrences };
  });
  const expected = ledger.activeAshaImports.map(({ file, specifier, occurrences }) => ({
    file,
    specifier,
    occurrences,
  }));
  compareExact('temporary Asha imports', actual, expected);
}

function checkIntegrationScripts() {
  for (const entry of ledger.integrationScripts) {
    if (!entry.path.startsWith('scripts/legacy-asha-')) {
      errors.push(`legacy integration script is not explicitly named: ${entry.path}`);
    }
    if (!sourceFiles.some((file) => file.displayPath === entry.path)) {
      errors.push(`ledgered integration script is missing: ${entry.path}`);
    }
    if (!Number.isInteger(entry.removalTask)) {
      errors.push(`ledgered integration script lacks a removal task: ${entry.path}`);
    }
  }
}

function checkArtifactNamespaces() {
  const oldNamespace = ledger.predecessor.artifactNamespace;
  const currentNamespace = ledger.current.artifactNamespace;
  const declaredCurrent = new Set([
    ...ledger.schemaFamilies.map((family) => `${currentNamespace}${family}`),
    ...ledger.newSchemaFamilies.map((family) => `${currentNamespace}${family}`),
  ]);
  const observedCurrent = new Set();

  for (const file of sourceFiles) {
    if (file.displayPath === 'config/viewer-generation.json') {
      if (!file.text.includes('"kind": "rusty_procgen.viewer_generation_config.v1"')) {
        errors.push('config/viewer-generation.json must use the Rusty artifact kind');
      }
    }
    if (ignoredSchemaFiles.has(file.displayPath)) {
      continue;
    }
    if (file.text.includes(oldNamespace) && !legacyNamespaceOwnerFiles.has(file.displayPath)) {
      errors.push(`${file.displayPath} still contains retired artifact namespace ${oldNamespace}`);
    }
    for (const match of file.text.matchAll(/rusty_procgen\.[A-Za-z0-9_]+(?:\.[A-Za-z0-9_]+)*/g)) {
      observedCurrent.add(match[0].replace(/\.$/, ''));
    }
  }

  const undeclared = [...observedCurrent].filter((value) => !declaredCurrent.has(value)).sort();
  const missing = [...declaredCurrent].filter((value) => !observedCurrent.has(value)).sort();
  if (undeclared.length > 0) {
    errors.push(`unledgered Rusty artifact/schema identities: ${undeclared.join(', ')}`);
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
