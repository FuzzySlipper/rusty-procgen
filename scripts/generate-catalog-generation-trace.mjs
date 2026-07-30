import { spawnSync } from 'node:child_process';
import {
  copyFileSync,
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const check = process.argv.includes('--check');
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const scratch = mkdtempSync(join(tmpdir(), 'rusty-procgen-catalog-trace-'));
const binary = resolve(repoRoot, 'procgen-rs/target/debug/rusty-procgen');
const inputs = [
  'artifacts/samples/batch-v2/candidate-000/candidate-003-branch_merge_shortcut.json',
  'artifacts/samples/batch-v2/candidate-000/intermediate-breakdown.json',
  'artifacts/samples/batch-v2/candidate-000/geometry-2d.json',
  'fixtures/shape-catalogs/2d-basic.json',
  'fixtures/policies/catalog-aware-generation-default.json',
];
const planPath = 'fixtures/catalog-generation/candidate-000-piece-plan.v1.json';
const resultPath = 'fixtures/catalog-generation/candidate-000-result.v1.json';
const tracePath = 'fixtures/catalog-generation/candidate-000-trace.v1.json';
const outputs = [tracePath];

try {
  run('cargo', [
    'build',
    '--quiet',
    '--manifest-path',
    'procgen-rs/Cargo.toml',
    '--bin',
    'rusty-procgen',
    '--locked',
  ], repoRoot);
  for (const input of inputs) {
    const target = join(scratch, input);
    mkdirSync(dirname(target), { recursive: true });
    copyFileSync(join(repoRoot, input), target);
  }
  run(binary, [
    'build',
    'emit-piece-plan',
    '--candidate',
    inputs[0],
    '--intermediate',
    inputs[1],
    '--geometry',
    inputs[2],
    '--corridor-realization',
    'catalog',
    '--out',
    planPath,
  ], scratch);
  run(binary, [
    'build',
    'realize-catalog-aware',
    '--candidate',
    inputs[0],
    '--geometry',
    inputs[2],
    '--piece-plan',
    planPath,
    '--catalog',
    inputs[3],
    '--policy',
    inputs[4],
    '--seed',
    '14334',
    '--out',
    resultPath,
    '--trace-out',
    tracePath,
  ], scratch);

  if (check) {
    for (const output of outputs) {
      const expected = readFileSync(join(repoRoot, output));
      const actual = readFileSync(join(scratch, output));
      if (!expected.equals(actual)) {
        throw new Error(`${output} is stale; run pnpm run catalog-trace:fixtures`);
      }
    }
    console.log('catalog generation trace fixtures match the Rust owner');
  } else {
    for (const output of outputs) {
      const target = join(repoRoot, output);
      mkdirSync(dirname(target), { recursive: true });
      cpSync(join(scratch, output), target);
    }
    console.log(`wrote ${outputs.join(', ')}`);
  }
} finally {
  rmSync(scratch, { recursive: true, force: true });
}

function run(command, args, cwd) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: 'utf8',
    stdio: 'pipe',
  });
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(' ')} failed (${result.status})\n`
      + `${result.stdout}${result.stderr}`,
    );
  }
}
