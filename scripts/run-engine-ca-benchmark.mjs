#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const allowedDirtyPaths = new Set([
  'config/viewer-generation.json',
  'artifacts/evidence/engine-ca-benchmark.json',
]);
const status = capture('git', ['status', '--porcelain=v1'], repoRoot)
  .trimEnd()
  .split('\n')
  .filter(Boolean);
const unexpected = status.filter((line) => !allowedDirtyPaths.has(line.slice(3)));
if (unexpected.length > 0) {
  fail(
    'commit benchmark source before generating exact-provenance evidence; '
      + `unexpected worktree entries: ${unexpected.join(', ')}`,
  );
}

const repositoryCommit = capture('git', ['rev-parse', 'HEAD'], repoRoot).trim();
if (!/^[0-9a-f]{40}$/.test(repositoryCommit)) {
  fail(`git returned invalid repository commit ${JSON.stringify(repositoryCommit)}`);
}
run(
  'cargo',
  [
    'run',
    '--quiet',
    '--release',
    '--manifest-path',
    'integrations/rusty-engine-ca-benchmark/Cargo.toml',
    '--locked',
    '--bin',
    'rusty-procgen-ca-benchmark',
    '--',
    repoRoot,
    repositoryCommit,
  ],
  repoRoot,
);

function capture(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: 'utf8' });
  if (result.status !== 0) {
    fail(`${command} ${args.join(' ')} failed:\n${result.stdout}${result.stderr}`);
  }
  return result.stdout;
}

function run(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, stdio: 'inherit' });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
