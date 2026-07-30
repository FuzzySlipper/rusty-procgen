# Generation Control Characterization

Status: checked evidence; no generation behavior change.

This report uses the Rust-owned catalog trace to separate the stages that make
a layout sprawling from settings that are merely present in configuration. It
is the input to the next behavior-changing task, not a compactness claim by
itself.

The tracked owner is
[`fixtures/generation-control/characterization-v1.json`](../fixtures/generation-control/characterization-v1.json).
It never reads `config/viewer-generation.json`. Its base configuration is the
tracked catalog-coverage policy, and its explicit `32 / 32 / 32` initial
spacing overrides copy the current tight-style experiment without depending on
that mutable workbench.

Regenerate or check the complete evidence with:

```bash
pnpm run generation-control:report
pnpm run generation-control:report:check
```

The owner writes
[`artifacts/evidence/generation-control-characterization.v1.json`](../artifacts/evidence/generation-control-characterization.v1.json)
and two named result/trace pairs. Every generated named pair is run twice and
must be byte-identical. The report probes each of the 20 editable configuration
values exactly once from the catalog-default baseline.

## Named outcomes

| Outcome | Candidate | Final placement | Room envelope | Routed cells / bends | Fill | Routing states |
|---|---|---:|---:|---:|---:|---:|
| Catalog defaults, accepted | `5201` | 178 × 111 | 1,264,512 | 941 / 82 | 8.07% | 42,442 |
| Tight initial spacing, rejected | `5201` | none | none | no complete route | none | 469,590 across four attempts |
| Tight initial spacing, accepted | `5801` | 81 × 36 | 186,624 | 125 / 22 | 18.03% | 4,569 |

The compact and sprawling accepted examples have different graph sizes:
`5201` places nine rooms and routes thirteen sections, while `5801` places four
rooms and routes four sections. They are useful visual representatives, not a
claim that spacing alone caused the full difference.

The same-candidate comparison is more important: changing `5201` from catalog
defaults to the current tight initial margin/column/row values does not produce
a compact accepted result. It places all nine rooms, then all four bounded
catalog attempts fail after routing only three to five sections. Tight spacing
therefore cannot be treated as an unconditional solution.

All three traces are available in the viewer's `Generation Trace` outcome
selector. The rejected tight run exposes each failed attempt rather than
collapsing it into a generic error.

## Observed cause classification

- **Embedding and origins:** the selected `5201` embedding is the first valid
  spacing-tier-zero result. Reducing its initial room margin moves the first
  placed room and changes routes, yet leaves final catalog placement width and
  height unchanged. The tested column/row gap changes are unused by this
  embedding.
- **Slack and shape choice:** one cell of initial catalog room slack changes
  the attempt-start decision but not the accepted layout. Lowering the
  room-candidate cap changes a room domain but retains the same selected
  shapes. Slack growth is unused after attempt zero succeeds.
- **Route bounds and costs:** a smaller route margin changes search bounds but
  not the chosen routes. Lower guide-distance weight and turn penalty reduce
  search work while preserving route cells and bend count.
- **First-success selection:** spacing growth, attempt growth, and adjacent
  generation/routing caps remain unobserved because the first admissible
  attempt is selected. They are not proven irrelevant outside this fixture.
- **Validation constraints:** minimum clearance is semantically active and its
  adjacent increase exhausts catalog routing. Independently changing coupled
  search-budget fields or wall thickness is rejected before generation.

The characterized sprawl is therefore not owned by one universal "room
spacing" value. The accepted embedding, the room/shape feasibility domain,
route search, validation, and first-success policy contribute separate
constraints, and the final catalog placement can remain the same size even
when the upstream geometry moves.

## Configuration influence

For the default accepted `5201` fixture:

- Reducing `initialRoomMargin` by eight changes the first room placement and
  the routes. It removes eight geometry units in each dimension, eight routed
  cells, eight bends, and 1,198 visited routing states, but does not change the
  178 × 111 final placement span.
- Adjacent changes to initial column/row gap, all three growth values, and
  `maxSearchAttempts` have no semantic effect on this first-tier selected
  embedding. The values remain hash-bound, but no room, route, validation
  outcome, or measured projection changes.
- Reducing `maxSpacingTiers` or `roomOrderAttemptsPerTier` alone is rejected by
  the coupled `maxSearchAttempts` bound. Increasing wall thickness alone is
  likewise rejected by the clearance invariant.
- Increasing minimum clearance from three to four admits the configuration but
  makes catalog routing exhaust, so clearance is a real feasibility control.
- Initial room slack changes the attempt-start decision but not the first
  successful projection. Slack growth is unused because attempt zero succeeds.
- Reducing the room-candidate cap changes an admitted room domain but not the
  selected rooms. Reducing the route margin changes the routed bounds but not
  the selected routes.
- Adjacent guide-weight and turn-penalty changes retain identical route cells
  and bends while reducing visited states by 181 and 250 respectively.
- Adjacent generation-attempt and route-state caps have no semantic effect
  because the first accepted attempt remains well within those limits.
- Switching to hybrid corridors changes the owner and the projection
  substantially: its room envelope and centerline are smaller, but its
  procedural shell spans 505 × 321 cells versus the catalog placement's
  178 × 111. “Smaller geometry” and “smaller final placement” are not the same
  metric.

The report records the owning stage, first changed semantic trace event,
changed stage hashes, and numeric metric deltas for every probe. Input and
output hashes alone do not count as semantic influence.

## Constraints for the behavior task

The evidence supports a small opinionated control model:

1. Validation and route feasibility remain hard constraints. Raw spacing
   targets are inputs, not promises.
2. Evaluate more than the first valid geometry/catalog composition when the
   configured compactness envelope is not met.
3. Rank accepted candidates by final placement span/area and routed cells,
   then bends and routing work; use fill ratio as a visible density diagnostic.
4. Keep geometry envelope, catalog placement span, and procedural shell
   separate. A constraint must name the stage it governs.
5. Report coupled-budget rejection before generation instead of making a
   setting appear inert.
6. If no candidate meets the requested preferences, return the deterministic
   best admissible result with explicit shortfall evidence rather than either
   publishing an invalid layout or treating tighter spacing as sufficient.

Task #6414 owns that behavior change. This task changes only fixtures, report
projection, viewer evidence selection, and documentation.
