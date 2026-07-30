import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

import {
  catalogGenerationStageFrames,
  decodeCatalogGenerationRun,
  replayCatalogGenerationAttempt,
} from '../dist/ts/src/catalog-generation-trace.js';

const fixtures = new URL('../fixtures/catalog-generation/', import.meta.url);

const accepted = await readRun('candidate-000');
const exhausted = await readRun('candidate-000-exhausted');

assert.equal(accepted.run.attempts.length, 1);
assert.equal(accepted.run.selectedAttempt, 0);
assert.equal(accepted.run.attempts[0]?.evidence.classification, 'success');
const acceptedFinal = replayCatalogGenerationAttempt(
  accepted.run,
  0,
  accepted.run.attempts[0].eventIndices.length,
);
assert.equal(acceptedFinal.rooms.length, 9);
assert.equal(acceptedFinal.routes.length, 13);
assert.equal(acceptedFinal.frame, acceptedFinal.frameCount);
assert.ok(catalogGenerationStageFrames(accepted.run, 0).length >= 5);

assert.equal(exhausted.run.attempts.length, 4);
assert.equal(exhausted.run.selectedAttempt, null);
assert.ok(exhausted.run.attempts.every((attempt) =>
  attempt.evidence.classification === 'search_budget_exhaustion'));
for (const attempt of exhausted.run.attempts) {
  const initial = replayCatalogGenerationAttempt(exhausted.run, attempt.attempt, 0);
  const final = replayCatalogGenerationAttempt(
    exhausted.run,
    attempt.attempt,
    attempt.eventIndices.length,
  );
  assert.equal(initial.rooms.length, 0);
  assert.equal(final.frame, final.frameCount);
  assert.equal(final.routes.length, 0);
}

rejects(
  mutate(accepted.trace, (trace) => {
    trace.unexpected = true;
  }),
  accepted.result,
  'unexpected key',
);
rejects(
  mutate(accepted.trace, (trace) => {
    trace.limits.maxEvents = 4_097;
  }),
  accepted.result,
  'hard event quota',
);
rejects(
  mutate(accepted.trace, (trace) => {
    trace.events[1].previousHash = 'fnv1a64:0000000000000000';
  }),
  accepted.result,
  'broken chain link',
);
rejects(
  mutate(accepted.trace, (trace) => {
    trace.events[1].body.roomSlackCells += 1;
  }),
  accepted.result,
  'event body without matching hash',
);
rejects(
  accepted.trace,
  mutate(accepted.result, (result) => {
    result.candidateId = 'candidate-tampered';
  }),
  'mismatched result output hash',
);
rejects(
  mutate(accepted.trace, (trace) => {
    trace.selection.selectedAttempt = null;
  }),
  accepted.result,
  'mismatched selection',
);
rejects(
  mutate(accepted.trace, (trace) => {
    trace.events.pop();
  }),
  accepted.result,
  'truncated trace',
);

console.log(JSON.stringify({
  schema: accepted.trace.kind,
  accepted: {
    events: accepted.trace.events.length,
    attempts: accepted.run.attempts.length,
    outputHash: accepted.run.outputHash,
    rooms: acceptedFinal.rooms.length,
    routes: acceptedFinal.routes.length,
  },
  exhausted: {
    events: exhausted.trace.events.length,
    attempts: exhausted.run.attempts.length,
    outputHash: exhausted.run.outputHash,
  },
  tamperCases: 7,
}));

async function readRun(name) {
  const result = JSON.parse(await readFile(new URL(`${name}-result.v1.json`, fixtures), 'utf8'));
  const trace = JSON.parse(await readFile(new URL(`${name}-trace.v1.json`, fixtures), 'utf8'));
  return {
    result,
    trace,
    run: decodeCatalogGenerationRun(trace, result),
  };
}

function rejects(trace, result, label) {
  assert.throws(
    () => decodeCatalogGenerationRun(trace, result),
    Error,
    label,
  );
}

function mutate(value, change) {
  const copy = structuredClone(value);
  change(copy);
  return copy;
}
