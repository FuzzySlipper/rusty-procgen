# Catalog Generation Traces

Status: supported Rust-owned semantic trace and replay contract.

Catalog-aware generation remains one deterministic operation owned by
`rusty-procgen-preflight`. A trace is a sibling diagnostic artifact describing
that operation; it is not a second generator, a mutable generation session, or
an authority that may change the selected result.

## Contract

`rusty_procgen.catalog_generation_trace.v1` binds:

- canonical hashes of the candidate, source geometry, catalog piece plan,
  catalog, generation policy, and inert provenance labels;
- the effective generation policy, seed, and trace limits;
- monotonically indexed events with exact previous/event hash links;
- the exact generation-result hash and an explicit first-success or exhausted
  selection reason.

The structural hashes use the repository's canonical compact-JSON FNV-1a
contract. They detect drift and ordinary tampering and make deterministic
fixtures easy to compare; they are not a security signature.

Events retain semantic decisions rather than search implementation churn:

- input binding and attempt start;
- bounded room domains, selected shape/transform/origin, committed occupied and
  reserved cells, and overlap conflicts;
- section start/goal/guide/bounds and the committed route or typed failed
  result;
- geometry, placement, and built-flow validation outcomes;
- attempt classification and final selection/exhaustion.

Pathfinder frontier states are deliberately excluded. Routing work remains
bounded by `maxRoutingStatesPerSection`; the trace records the visited-state
count and final route cells.

## Bounds

The default trace limits are:

| Measure | Default | Hard maximum |
|---|---:|---:|
| events | 1,024 | 4,096 |
| compact encoded event-body bytes | 1,048,576 | 4,194,304 |
| retained visual cells | 131,072 | 1,048,576 |

Each event is admitted before publication. Checked arithmetic rejects an event,
byte, or visual-cell overflow with a typed `CatalogGenerationTraceError`.
Event count and fixed-size event headers bound the non-body artifact overhead.
A traced CLI rejection writes neither the result nor trace output.

## Public Rust API

Use `ProcgenCore::realize_catalog_aware_traced` with a
`CatalogGenerationTraceRequest`. It returns the unchanged
`CatalogAwareGenerationResult` beside its trace.

Use `replay_catalog_generation_trace` to validate exact inputs, policy, limits,
root, event order, every chain link and event body, result hash, and selection
evidence. Replay applies room and route deltas to a bounded in-memory state,
emits a state hash/metric frame after every event, and retains one reconstructed
room/route state per completed attempt.

The existing `ProcgenCore::realize_catalog_aware` remains the result-only
surface and produces byte-identical results.

## CLI and fixtures

The existing command accepts an explicit trace output:

```bash
pnpm run procgen -- build realize-catalog-aware \
  --candidate candidate.json \
  --geometry geometry.json \
  --piece-plan catalog-piece-plan.json \
  --catalog catalog.json \
  --policy catalog-policy.json \
  --seed 14334 \
  --out result.json \
  --trace-out trace.json
```

Optional `--trace-max-events`, `--trace-max-event-body-bytes`, and
`--trace-max-visual-cells` values select stricter admitted limits. The CLI only
loads typed inputs, supplies inert labels, invokes the shared runner, and writes
the already validated values.

The checked candidate-000 trace is owned by:

```bash
pnpm run catalog-trace:fixtures
pnpm run catalog-trace:fixtures:check
```

The generator creates its large intermediate plan and result in an isolated
temporary directory and commits only the replay artifact. The current fixture
contains 51 events, 55,432 compact event-body bytes, and 2,353 visual cells.

## Nonclaims

This contract does not add pausing/resuming generation, callbacks, a scheduler,
an event bus, a generic procgen framework, browser authority, or per-frontier
pathfinding playback. Viewer controls consume and replay the checked artifact
in the later presentation task.
