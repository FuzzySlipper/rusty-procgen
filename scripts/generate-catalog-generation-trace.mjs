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
const catalog = 'fixtures/shape-catalogs/2d-basic.json';
const scenarios = [
  {
    id: 'candidate-000',
    candidate:
      'artifacts/samples/batch-v2/candidate-000/candidate-003-branch_merge_shortcut.json',
    intermediate:
      'artifacts/samples/batch-v2/candidate-000/intermediate-breakdown.json',
    geometry: 'artifacts/samples/batch-v2/candidate-000/geometry-2d.json',
    policy: 'fixtures/policies/catalog-aware-generation-default.json',
    seed: '14334',
  },
  {
    id: 'candidate-000-exhausted',
    candidate:
      'artifacts/samples/batch-v2/candidate-000/candidate-003-branch_merge_shortcut.json',
    intermediate:
      'artifacts/samples/batch-v2/candidate-000/intermediate-breakdown.json',
    geometry: 'artifacts/samples/batch-v2/candidate-000/geometry-2d.json',
    policy: 'fixtures/policies/catalog-aware-generation-trace-exhausted.json',
    seed: '14334',
  },
  {
    id: 'candidate-000-selection',
    candidate:
      'artifacts/samples/batch-v2/candidate-000/candidate-003-branch_merge_shortcut.json',
    intermediate:
      'artifacts/samples/batch-v2/candidate-000/intermediate-breakdown.json',
    geometry: 'artifacts/samples/batch-v2/candidate-000/geometry-2d.json',
    policy: 'fixtures/policies/catalog-aware-generation-selection-probe.json',
    seed: '14334',
  },
].map((scenario) => ({
  ...scenario,
  plan: `fixtures/catalog-generation/${scenario.id}-piece-plan.v1.json`,
  result: `fixtures/catalog-generation/${scenario.id}-result.v1.json`,
  trace: `fixtures/catalog-generation/${scenario.id}-trace.v1.json`,
}));
const inputs = [
  catalog,
  ...scenarios.flatMap((scenario) => [
    scenario.candidate,
    scenario.intermediate,
    scenario.geometry,
    scenario.policy,
  ]),
];
const outputs = scenarios.flatMap((scenario) => [scenario.result, scenario.trace]);

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
  for (const input of new Set(inputs)) {
    const target = join(scratch, input);
    mkdirSync(dirname(target), { recursive: true });
    copyFileSync(join(repoRoot, input), target);
  }
  for (const scenario of scenarios) {
    run(binary, [
      'build',
      'emit-piece-plan',
      '--candidate',
      scenario.candidate,
      '--intermediate',
      scenario.intermediate,
      '--geometry',
      scenario.geometry,
      '--corridor-realization',
      'catalog',
      '--out',
      scenario.plan,
    ], scratch);
    run(binary, [
      'build',
      'realize-catalog-aware',
      '--candidate',
      scenario.candidate,
      '--geometry',
      scenario.geometry,
      '--piece-plan',
      scenario.plan,
      '--catalog',
      catalog,
      '--policy',
      scenario.policy,
      '--seed',
      scenario.seed,
      '--out',
      scenario.result,
      '--trace-out',
      scenario.trace,
    ], scratch);
  }

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
