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
rejects(
  mutate(accepted.trace, (trace) => {
    const event = trace.events.find(
      (candidate) =>
        candidate.body.type === 'section_routing_finished'
        && candidate.body.status === 'found'
        && alternateCorner(candidate.body.cells) !== null,
    );
    assert.ok(event, 'accepted trace has one route corner');
    const alternate = alternateCorner(event.body.cells);
    assert.ok(alternate, 'accepted route corner has an alternate cell');
    event.body.cells[alternate.index] = alternate.cell;
    rechainTrace(trace);
  }),
  accepted.result,
  'fully re-chained route projection mismatch',
);
const slackMismatchResult = mutate(accepted.result, (result) => {
  result.attempts[0].roomSlackCells += 1;
});
rejects(
  mutate(accepted.trace, (trace) => {
    const outputHash = fnv1a64Json(slackMismatchResult);
    trace.finalOutputHash = outputHash;
    const finished = trace.events.find((event) => event.body.type === 'run_finished');
    assert.ok(finished, 'accepted trace has run_finished');
    finished.body.outputHash = outputHash;
    rechainTrace(trace);
  }),
  slackMismatchResult,
  'fully re-chained attempt slack mismatch',
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
  tamperCases: 9,
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

function alternateCorner(cells) {
  for (let index = 1; index < cells.length - 1; index += 1) {
    const prior = cells[index - 1];
    const current = cells[index];
    const next = cells[index + 1];
    if (prior.x === next.x || prior.y === next.y) {
      continue;
    }
    const alternate = {
      x: prior.x + next.x - current.x,
      y: prior.y + next.y - current.y,
    };
    if (!cells.some((cell) => cell.x === alternate.x && cell.y === alternate.y)) {
      return { index, cell: alternate };
    }
  }
  return null;
}

function rechainTrace(trace) {
  let previousHash = trace.rootHash;
  let bodyBytes = 0;
  for (const event of trace.events) {
    event.previousHash = previousHash;
    event.eventHash = fnv1a64Json({
      index: event.index,
      attempt: event.attempt,
      previousHash,
      body: event.body,
    });
    previousHash = event.eventHash;
    bodyBytes += new TextEncoder().encode(JSON.stringify(event.body)).length;
  }
  trace.eventBodyBytes = bodyBytes;
  trace.finalEventHash = previousHash;
}

function fnv1a64Json(value) {
  const bytes = new TextEncoder().encode(JSON.stringify(value));
  let hash = 0xcbf29ce484222325n;
  for (const byte of bytes) {
    hash ^= BigInt(byte);
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return `fnv1a64:${hash.toString(16).padStart(16, '0')}`;
}
