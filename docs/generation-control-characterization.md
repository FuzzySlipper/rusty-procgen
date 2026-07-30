# Generation Control Characterization

Status: checked controlled-generation behavior and evidence.

This report uses the Rust-owned catalog trace to show where a layout becomes
sprawling, which settings influence the result, and how the version-2 policy
selects a bounded outcome. It is evidence for the current generator, not a
claim that every seed can meet a compactness preference.

The tracked owner is
[`fixtures/generation-control/characterization-v1.json`](../fixtures/generation-control/characterization-v1.json).
It never reads `config/viewer-generation.json`. Its base configuration is
[`fixtures/policies/viewer-generation-default.v2.json`](../fixtures/policies/viewer-generation-default.v2.json),
and named tight-spacing cases carry their overrides directly.

Regenerate or check the complete evidence with:

```bash
pnpm run generation-control:report
pnpm run generation-control:report:check
```

The owner writes
[`artifacts/evidence/generation-control-characterization.v1.json`](../artifacts/evidence/generation-control-characterization.v1.json)
and two named result/trace pairs. Every generated named pair is run twice and
must be byte-identical. The report probes each of the 26 editable configuration
values exactly once from the tracked version-2 baseline.

## Named outcomes

| Outcome | Candidate | Final placement | Room envelope | Routed cells / bends | Fill | Routing states |
|---|---|---:|---:|---:|---:|---:|
| Current defaults, best admissible | `5201` | 174 × 110 | 1,191,552 | 911 / 71 | 8.17% | 34,401 |
| Tight initial spacing, rejected | `5201` | none | none | no complete route | none | 506,551 across four attempts |
| Tight initial spacing, best admissible | `5801` | 77 × 32 | 157,696 | 115 / 16 | 20.94% | 2,982 |

The compact and sprawling accepted examples have different graph sizes:
`5201` places nine rooms and routes thirteen sections, while `5801` places four
rooms and routes four sections. They are useful visual representatives, not a
claim that spacing alone caused the full difference.

The same-candidate comparison remains important: changing `5201` from defaults
to the tracked tight initial margin/column/row values does not produce a compact
accepted result. It places all nine rooms, then all four bounded catalog
attempts fail after routing only three to five sections. Tight spacing is
therefore a feasibility input, not an unconditional size control.

All three traces are available in the viewer's `Generation Trace` outcome
selector. The rejected tight run exposes each failed attempt rather than
collapsing it into a generic error.

## Controlled-generation result

The behavior-changing baseline before the version-2 policy selected the first
valid catalog attempt for `5201`. The current policy evaluates the complete
four-attempt budget, records that attempt 1 meets the span target, and still
selects the strictly better attempt 2:

| Measure | First-success baseline | Current version 2 | Delta |
|---|---:|---:|---:|
| Selected attempt | 0 | 2 | +2 attempts |
| Placement | 178 × 111 | 174 × 110 | −4 × −1 |
| Placement span | 289 | 284 | −5 |
| Placement area | 19,758 | 19,140 | −618 |
| Routed catalog cells | 941 | 911 | −30 |
| Route bends | 82 | 71 | −11 |
| Routing states | 42,442 | 34,401 | −8,041 |
| Room envelope | 1,424 × 888 | 1,392 × 856 | −72,960 area |
| Geometry | 1,672 × 1,112 | 1,656 × 1,104 | −16 × −8 |

The compact `5801` fixture also improves: 81 × 36 becomes 77 × 32, routed cells
drop from 125 to 115, bends from 22 to 16, and routing states from 4,569 to
2,982. It selects attempt 2. This is not an early-stop optimization: all four
bounded attempts are evaluated for both accepted fixtures.

## Outcome-control semantics

The version-2 catalog-aware policy distinguishes two kinds of control:

- Hard constraints cap final placement width, height, area, and routed catalog
  cells. A miss is typed `outcome_constraint_miss`, cannot be selected, and
  cannot publish an accepted artifact.
- A preference chooses placement span, placement area, or routed catalog cells
  as the primary metric and supplies a preferred maximum. The primary metric
  controls deterministic ordering. The maximum records target satisfaction but
  never stops the complete bounded search early.

Tie-breaking is explicit and stable. The selected primary metric comes first,
then the other size metrics, routed cells where not already primary, route
bends, routing states, and attempt order. This is an opinionated bounded
selection policy, not a general optimizer.

`initialRoomCompactionCells` and `roomCompactionGrowthCells` move selected room
origins toward the global geometry center before catalog routing. Compaction
does not bypass room-domain, route, placement, or built-flow validation. A
later attempt may improve the outcome, remain unchanged, or become infeasible.

## Configuration influence

The 26-probe matrix records the owning stage, first changed semantic trace
event, changed stage hashes, and numeric metric deltas. Input and output hashes
alone do not count as semantic influence.

For the current default `5201` fixture:

- reducing `initialRoomMargin` changes geometry, room placement, routing, and
  the final projection;
- adjacent column/row gap and geometry-growth changes remain bound but unused
  by the selected embedding;
- coupled search and wall/clearance invariants reject incoherent values before
  generation;
- increasing minimum clearance is admitted but exhausts catalog routing;
- room compaction changes room origins and can improve the final measured
  outcome, while a larger growth value changes later attempts rather than
  silently altering attempt zero;
- room-candidate, route-margin, guide-weight, turn-penalty, generation-attempt,
  and route-state controls expose their bounded domain or search effects in the
  trace;
- each hard outcome maximum has an exact measured boundary and rejects
  one-under values without publishing;
- changing the primary metric changes comparison semantics, while changing the
  preferred maximum changes target evidence without allowing a dominated
  admissible outcome to win;
- switching to hybrid corridors changes the owner and projection
  substantially, so geometry envelope, catalog placement, and procedural shell
  remain separate metrics.

The checked artifact records `behaviorChanges: true` because the default
`5201` result intentionally changed. Regeneration from the tracked suite is
byte-exact and does not consume the mutable viewer configuration.

## Nonclaims

The current policy does not prove a globally minimal layout, guarantee that a
preference is attainable, or certify generation performance. It does not add
callbacks, pausable generation, a scheduler, an event bus, a generic optimizer,
or browser selection authority. The viewer only explains and replays admitted
Rust decisions.
