# Catalog Generation Traces

Status: supported Rust-owned semantic trace and replay contract.

Catalog-aware generation remains one deterministic operation owned by
`rusty-procgen-preflight`. A trace is a sibling diagnostic artifact describing
that operation; it is not a second generator, a mutable generation session, or
an authority that may change the selected result.

## Contract

`rusty_procgen.catalog_generation_trace.v2` binds:

- canonical hashes of the candidate, source geometry, catalog piece plan,
  catalog, generation policy, and inert provenance labels;
- the effective generation policy, seed, and trace limits;
- monotonically indexed events with exact previous/event hash links;
- the exact generation-result hash, per-attempt outcome evaluation, and an
  explicit preference-satisfied, best-admissible, or exhausted selection
  reason.

The structural hashes use the repository's canonical compact-JSON FNV-1a
contract. They detect drift and ordinary tampering and make deterministic
fixtures easy to compare; they are not a security signature.

The browser boundary independently recomputes `generationPolicyHash` because
the complete typed generation policy is included in the trace. The candidate,
source geometry, source piece plan, catalog, and provenance source tuple are
not included: only their Rust-authored hashes are retained. Their
`candidateHash`, `sourceGeometryHash`, `sourcePlanHash`, `catalogHash`, and
`provenanceHash` values therefore bind the Rust event chain but cannot be
independently recomputed by the browser. In particular, the selected result
geometry and piece plan are generated outputs, not substitutes for the source
geometry and source plan.

Events retain semantic decisions rather than search implementation churn:

- input binding and attempt start;
- bounded room domains, selected shape/transform/origin, committed occupied and
  reserved cells, and overlap conflicts;
- section start/goal/guide/bounds and the committed route or typed failed
  result;
- geometry, placement, and built-flow validation outcomes;
- measured final-placement outcomes, hard-limit misses, deterministic
  comparison decisions, and final selection/exhaustion.

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
room/route state per completed attempt. Because an internally re-chained
artifact cannot authenticate its own changed semantic facts, replay also
performs a deterministic authoritative rerun from the supplied typed inputs and
requires the complete event sequence to match. This closes selected and failed
attempts alike, including room domains/conflicts, routing witnesses and ordered
routes, validation facts, and attempt settings.

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
the already validated values. Result and trace bytes are encoded and staged
together; distinct non-aliasing targets are required, and a staging, backup, or
commit failure restores both prior destinations rather than publishing a
mixed-generation pair.

The checked playback corpus is owned by:

```bash
pnpm run catalog-trace:fixtures
pnpm run catalog-trace:fixtures:check
```

The generator creates intermediate plans in an isolated temporary directory and
publishes exact result/trace pairs for:

- a preference-satisfied default-policy run with 102 events, 110,900 compact
  event-body bytes, and 4,686 visual cells;
- a four-attempt route-budget exhaustion with 90 events, 93,593 compact
  event-body bytes, and 3,080 visual cells;
- a four-attempt run that selects the deterministic best admissible outcome
  with 188 events, 210,330 compact event-body bytes, and 8,870 visual cells.

Keeping the result beside each trace lets a host independently recompute the
final output hash and prove that a selected attempt's rooms and routed sections
match the ordinary generator output. Fixture regeneration never reads
`config/viewer-generation.json`.

The browser outcome selector also includes two tracked characterization pairs:
the rejected tight-spacing `5201` run and the accepted compact `5801` run.
Their owner, reproducibility contract, and stage metrics are documented in
[Generation Control Characterization](generation-control-characterization.md).

## Outcome policy and selection

The version-2 policy separates validity from preference:

- `outcomeConstraints` owns hard maxima for placement width, placement height,
  placement area, and routed catalog cells;
- `outcomePreferences` chooses one primary metric—placement span, placement
  area, or routed catalog cells—and a preferred maximum.

Each otherwise successful attempt is measured after geometry, placement, and
built-flow validation. A hard-limit miss produces typed
`outcome_constraint_miss` evidence and cannot become the selected result.
Admissible outcomes are compared lexicographically:

- span preference: span, area, routed cells, bends, routing states, attempt;
- area preference: area, span, routed cells, bends, routing states, attempt;
- routed-cell preference: routed cells, span, area, bends, routing states,
  attempt.

The first admissible attempt meeting the preference ends the bounded search.
If none meets it, the runner exhausts the configured attempt budget and
publishes the deterministic best admissible outcome. If no attempt is
admissible, generation rejects with its typed failure evidence and publishes
no accepted artifact.

`roomCompactionCells` is an attempt input, not a score or validity rule. It
moves selected room origins inward toward the geometry center before catalog
routing. Later attempts may therefore improve the measured outcome, remain
unchanged, or become infeasible; the trace shows which occurred.

## Browser playback

The viewer's `Generation Trace` tab consumes the checked result/trace pairs
through `src/catalog-generation-trace.ts`. The decoder admits the complete
pair before changing the visible SVG:

- exact schema fields, authored limits, and hard limits are checked;
- the included generation policy is recomputed against its declared input
  hash;
- the root and every previous/event hash link are recomputed;
- event-body bytes and visual cells are independently recounted;
- attempt order, effective compaction, room-domain/placement membership, route
  endpoints and cardinal continuity, validation stages, measured outcomes,
  hard-limit decisions, comparison ordering, and final selection are checked;
- the result hash is recomputed and a successful selected attempt is compared
  with the ordinary placement and piece-plan sections.

The UI replays only admitted semantic events. It supports
preference-satisfied, best-admissible, and exhausted run selection,
failed-attempt switching, play/pause, single-decision forward and back, reset,
bounded seek, and previous/next stage navigation. Room
occupied/reserved cells, current conflicts, route guides/endpoints, and
committed routes are SVG observations of the retained trace. Policy values,
hard limits, preference, classification, metrics, comparison, event identity,
and output hash remain visible beside the projection.

Catalog-mode configuration rebuilds return their paired Rust result and trace
through the existing request revision guard. A successful run and a typed
attempt-budget exhaustion are both inspectable; the latter does not persist its
rejected configuration. Strict decode completes before trace replacement, and
an older request cannot publish after a newer generation-config revision.
Non-catalog rebuilds do not synthesize a catalog trace.

The workbench configuration schema is
`rusty_procgen.viewer_generation_config.v2`. Reading a version-1 workbench
config performs an explicit in-memory migration with compaction defaults
`0 + attempt * 1`, hard maxima `4096 / 4096 / 16777216 / 1048576`, and a
placement-span preference of `286`. A successful rebuild persists version 2
with no migration marker. A rejected rebuild preserves the original bytes.
Checked fixtures use tracked policy files and never read the mutable workbench
configuration.

Before invoking the traced runner, the viewer host replaces scratch-directory
provenance in its generated geometry and piece plan with the checked candidate
and configuration labels. Those labels are inert inputs to the Rust runner, so
identical rebuilds publish byte-identical result/trace pairs regardless of the
temporary working directory.

Focused proof:

```bash
pnpm run catalog-trace:smoke
pnpm run catalog-trace:viewer:smoke
```

The first command checks strict cross-language decode, preference-satisfied,
best-admissible and exhausted replay, plus malformed/tampered/mismatched
rejection. The second uses real
Chromium for keyboard and pointer controls, back/seek/reset/stage navigation,
attempt and outcome switching, mobile sizing, final result agreement,
tamper-before-mount behavior, live accepted/exhausted rebuild replacement, and
pagehide disposal. The live proof also attempts a different candidate while a
rebuild is in flight and requires the original selection and trace publication
to remain guarded.

## Nonclaims

This contract does not add pausing/resuming generation, callbacks, a scheduler,
an event bus, a generic procgen framework, browser authority, or per-frontier
pathfinding playback. The browser cannot change the generation result or resume
an attempt; controls only replay already admitted Rust decisions. The bounded
preference is not a proof of a globally minimal layout or a performance
optimization claim.
