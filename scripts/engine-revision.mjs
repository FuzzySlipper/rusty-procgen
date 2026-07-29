#!/usr/bin/env node

import {
  copyFileSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const publicRepository = 'https://github.com/FuzzySlipper/rusty-engine';
const fullCommit = /^[0-9a-f]{40}$/;
const scriptPath = fileURLToPath(import.meta.url);
const repoRoot = resolve(dirname(scriptPath), '..');
const manifestRelative = 'integrations/rusty-engine-publication/Cargo.toml';
const lockRelative = 'integrations/rusty-engine-publication/Cargo.lock';
const carrierPaths = ['engine-source.json', manifestRelative, lockRelative];
const command = process.argv[2];

if (command === 'check' && process.argv.length === 3) {
  check(repoRoot);
} else if (command === 'update') {
  const commit = process.argv[3] ?? '';
  const remaining = process.argv.slice(4);
  if (!fullCommit.test(commit) || remaining.some((argument) => argument !== '--dry-run')) {
    usage();
  }
  update(commit, remaining.includes('--dry-run'));
} else {
  usage();
}

function usage() {
  console.error(
    'usage: ./scripts/engine-revision check\n'
      + '       ./scripts/engine-revision update <40-character-public-sha> [--dry-run]',
  );
  process.exit(2);
}

function check(root) {
  const source = strictSource(root);
  const manifestPath = join(root, manifestRelative);
  const manifest = readFileSync(manifestPath, 'utf8');
  const dependencyPattern = /\{[^}\n]*git\s*=\s*"([^"]+)"[^}\n]*rev\s*=\s*"([^"]+)"[^}\n]*\}/g;
  const observed = [...manifest.matchAll(dependencyPattern)];
  if (observed.length === 0) {
    fail(`${manifestRelative} has no explicit Engine Git dependencies`);
  }
  for (const match of observed) {
    if (match[1] !== source.publicRepository || match[2] !== source.commit) {
      fail(
        `${manifestRelative} expected ${source.publicRepository}@${source.commit}, `
          + `observed ${match[1]}@${match[2]}; run ./scripts/engine-revision update ${source.commit}`,
      );
    }
  }
  if (/rusty-engine[^}\n]*path\s*=|path\s*=[^}\n]*rusty-engine/.test(manifest)) {
    fail(`${manifestRelative} contains a forbidden local Rusty Engine path dependency`);
  }

  const lock = readFileSync(join(root, lockRelative), 'utf8');
  const engineSources = [...lock.matchAll(/^source = "git\+([^"]*rusty-engine[^"]*)"$/gm)]
    .map((match) => match[1]);
  if (engineSources.length === 0) {
    fail(`${lockRelative} has no locked Rusty Engine packages`);
  }
  const expectedSource = `${source.publicRepository}?rev=${source.commit}#${source.commit}`;
  for (const observedSource of engineSources) {
    if (observedSource !== expectedSource) {
      fail(
        `${lockRelative} expected ${expectedSource}, observed ${observedSource}; `
          + `run ./scripts/engine-revision update ${source.commit}`,
      );
    }
  }
  console.log(
    `Rusty Engine revision check passed (${source.commit}, `
      + `${observed.length} manifest dependencies, ${engineSources.length} locked packages).`,
  );
}

function strictSource(root) {
  const path = join(root, 'engine-source.json');
  let source;
  try {
    source = JSON.parse(readFileSync(path, 'utf8'));
  } catch (error) {
    fail(`engine-source.json is not valid JSON: ${error.message}`);
  }
  if (
    source === null
    || typeof source !== 'object'
    || Array.isArray(source)
    || JSON.stringify(Object.keys(source).sort())
      !== JSON.stringify(['commit', 'publicRepository', 'schemaVersion'])
    || source.schemaVersion !== 1
    || source.publicRepository !== publicRepository
    || !fullCommit.test(source.commit)
  ) {
    fail(
      'engine-source.json must contain only schemaVersion=1, the canonical public repository, '
        + 'and one full lowercase commit',
    );
  }
  return source;
}

function update(commit, dryRun) {
  provePublicCommit(commit);
  refuseDirtyCarriers(repoRoot);
  const candidate = mkdtempSync(join(tmpdir(), 'rusty-procgen-engine-revision-'));
  let worktreeAdded = false;
  try {
    run('git', ['worktree', 'add', '--detach', candidate, 'HEAD'], repoRoot);
    worktreeAdded = true;
    rewriteCandidate(candidate, commit);
    run(
      'cargo',
      ['generate-lockfile', '--manifest-path', manifestRelative],
      candidate,
    );
    run('./scripts/engine-revision', ['check'], candidate);
    run(
      'cargo',
      ['check', '--manifest-path', manifestRelative, '--locked'],
      candidate,
    );
    const diff = capture('git', ['diff', '--', ...carrierPaths], candidate);
    if (diff.length === 0) {
      console.log(`Rusty Engine is already pinned to ${commit}; no changes.`);
      return;
    }
    process.stdout.write(diff);
    if (dryRun) {
      console.log('Dry run complete; caller was not modified.');
      return;
    }
    refuseDirtyCarriers(repoRoot);
    for (const path of carrierPaths) {
      copyFileSync(join(candidate, path), join(repoRoot, path));
    }
    check(repoRoot);
    console.log(`Updated Rusty Engine carriers to ${commit}; review and commit the displayed diff.`);
  } finally {
    if (worktreeAdded) {
      run('git', ['worktree', 'remove', '--force', candidate], repoRoot, false);
    }
    rmSync(candidate, { force: true, recursive: true });
  }
}

function provePublicCommit(commit) {
  const probe = mkdtempSync(join(tmpdir(), 'rusty-procgen-engine-fetch-'));
  try {
    run('git', ['init', '--quiet'], probe);
    run('git', ['fetch', '--quiet', '--depth=1', publicRepository, commit], probe);
    const observed = capture('git', ['rev-parse', 'FETCH_HEAD'], probe).trim();
    if (observed !== commit) {
      fail(`public fetch resolved ${commit} as unexpected commit ${observed}`);
    }
  } finally {
    rmSync(probe, { force: true, recursive: true });
  }
}

function refuseDirtyCarriers(root) {
  const status = capture('git', ['status', '--porcelain', '--', ...carrierPaths], root).trim();
  if (status.length > 0) {
    fail(`active Engine carriers are dirty:\n${status}`);
  }
}

function rewriteCandidate(root, commit) {
  const source = strictSource(root);
  source.commit = commit;
  writeFileSync(join(root, 'engine-source.json'), `${JSON.stringify(source, null, 2)}\n`);
  const manifestPath = join(root, manifestRelative);
  const before = readFileSync(manifestPath, 'utf8');
  const carrierPattern =
    /(\bgit\s*=\s*"https:\/\/github\.com\/FuzzySlipper\/rusty-engine"[^}\n]*\brev\s*=\s*")[0-9a-f]{40}(")/g;
  if (![...before.matchAll(carrierPattern)].length) {
    fail(`${manifestRelative} had no Engine revision carriers to update`);
  }
  const after = before.replace(
    carrierPattern,
    `$1${commit}$2`,
  );
  writeFileSync(manifestPath, after);
}

function run(program, args, cwd, fatal = true) {
  const result = spawnSync(program, args, { cwd, encoding: 'utf8', stdio: 'inherit' });
  if (result.status !== 0 && fatal) {
    fail(`${program} ${args.join(' ')} failed with status ${result.status}`);
  }
}

function capture(program, args, cwd) {
  const result = spawnSync(program, args, { cwd, encoding: 'utf8' });
  if (result.status !== 0) {
    fail(
      `${program} ${args.join(' ')} failed with status ${result.status}: `
        + `${result.stderr.trim()}`,
    );
  }
  return result.stdout;
}

function fail(message) {
  console.error(`Rusty Engine revision error: ${message}`);
  process.exit(1);
}
