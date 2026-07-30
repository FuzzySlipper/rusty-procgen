# Procgen Artifact Contract

Status: graph grammar, batch selection, and intermediate layout intent contract.

The CLI workbench is file-oriented. Every command reads explicit inputs, writes
explicit outputs, and produces structured JSON a human or agent can inspect.

## Command Pattern

```bash
pnpm run procgen -- <command> --state <candidate.json> --out <output.json> --receipt <receipt.json> --seed <u64>
```

Use `--transcript <path>` on mutating commands when building an auditable run.

Exit code `0` means the command completed successfully. Validation failures are
written as JSON diagnostics; malformed input, IO failure, and rejected mutating
operations return non-zero.

## Candidate

Kind: `rusty_procgen.candidate.v1`

The candidate is dimension-agnostic at the graph layer. The first implementation
uses `dimensionModel: "topology_graph"` and later commands may add 2D or 3D
layout artifacts without changing the graph contract.

Important fields:

- `candidateId`: stable generated id.
- `seed`: source seed.
- `sourceIntent`: seed-intent id.
- `provenance`: ordered command history.
- `graph.nodes`: intent nodes.
- `graph.edges`: directed intent edges.

Node kinds:

- `start`
- `goal`
- `gate`
- `key`
- `treasure`
- `shortcut`
- `secret`
- `hazard`
- `resource`
- `junction`

Edge kinds:

- `critical_path`
- `key_branch`
- `optional_branch`
- `shortcut`
- `secret_bypass`

Traversal kinds:

- `open`
- `locked`
- `one_way_return`
- `hidden`

Locked edges use `requiredItem`. Key nodes use `grantsItem`.

## Rule Catalog

Kind: `rusty_procgen.rule_catalog.v1`

The v2 graph grammar catalog lives at:

```text
fixtures/rule-catalog/v2-graph-patterns.json
```

The companion design document is:

```text
docs/v2-graph-grammar-catalog.md
```

Pattern ids in this catalog should match future `graph apply-rule --rule <id>`
values. The catalog records required node/edge kinds, tags, validator
invariants, scoring hints, repair hints, and 2D/3D embedding notes.

Implemented v2 rule ids:

- `hub_spoke_cluster`
- `nested_lock_key_chain`
- `hazard_resource_tradeoff`
- `boss_preparation_loop`
- `gated_treasure_branch`
- `branch_merge_shortcut`

## Receipt

Kind: `rusty_procgen.receipt.v1`

Receipts record command status, seed, input/output hashes, output file refs, and
diagnostics. Receipts are the primary tool-call evidence for agent transcripts.

## Validation Report

Kind: `rusty_procgen.validation.graph.v1`

Validation reports contain:

- `ok`
- `fatalCount`
- `stateHash`
- `diagnostics`

Diagnostics may include `repairHint`. Agents should treat it as a suggested
next edit, not as proof that the edit is the only valid repair.

Stable diagnostic codes currently emitted by graph validation/rule rejection:

- `start_count_invalid`
- `goal_count_invalid`
- `edge_from_missing`
- `edge_to_missing`
- `required_item_unavailable`
- `goal_unreachable`
- `locked_edge_never_traversed`
- `non_goal_dead_end`
- `orphan_node`
- `hub_incident_edges_low`
- `hub_missing_wayfinding_anchor`
- `hub_missing_return_or_rejoin`
- `boss_missing_preparation`
- `boss_preparation_missing_return`
- `hazard_missing_rejoin`
- `merge_upstream_routes_low`
- `rule_already_applied`
- `missing_required_pattern`

Fatal diagnostics block acceptance. Warnings are advisory.

## Graph Analysis Report

Kind: `rusty_procgen.graph_analysis.v1`

Graph analysis reports contain:

- `criticalPath`
- `dominators`
- `optionalBranches`
- `lockKeyOrder`
- `loopSignals`
- `shortcutBypassRisks`

They are intended as agent planning context, not as validation authority.

## Rule Compatibility Report

Kind: `rusty_procgen.rule_compatibility.v1`

Compatibility reports list every known graph rule with one of:

- `applicable`
- `blocked`
- `duplicate`
- `risky`

Each entry may include reasons and recommended actions.

## Spatial Intent Report

Kind: `rusty_procgen.spatial_intent.v1`

Spatial intent reports annotate graph nodes and edges with pre-geometry hints
such as `landmark_hub`, `visible_before_reachable`, `pressure_path`,
`shortcut_connector`, `one_way_drop`, and `hidden_route`.

## Intermediate Breakdown

Kind: `rusty_procgen.intermediate_breakdown.v1`

Intermediate breakdowns contain:

- `regions`: graph-derived region roles and optional anchor nodes
- `connectors`: graph-edge-derived connector intents
- `constraints`: named constraints that later geometry passes should preserve

Validation uses kind `rusty_procgen.validation.intermediate.v1`. This schema is
intentionally not a 2D room layout, 3D prefab graph, mesh, voxel grid, or tile
map. See `docs/intermediate-layout-contract.md`.

## Score Report

Kind: `rusty_procgen.score.graph.v1`

First-slice metrics:

- `nodeCount`
- `edgeCount`
- `criticalPathLength`
- `loopCount`
- `optionalBranchCount`
- `lockedEdgeCount`
- `shortcutCount`
- `deadEndCount`
- `hubCount`
- `wayfindingAnchorCount`
- `preparationCount`
- `hazardCount`
- `bossCount`
- `mergeCount`
- `pressureEdgeCount`
- `rejoinEdgeCount`

`overall` is a deterministic heuristic score, not a final design verdict.

## Selection Report

Kind: `rusty_procgen.selection_report.v1`

Batch generation writes:

```text
artifacts/samples/batch-v2/selection-report.json
```

The report contains:

- `batchId`
- `seed`
- `requestedCount`
- `generatedCount`
- `accepted`: sorted accepted candidates with artifact, validation, score, and
  layout refs
- `rejected`: rejected candidate refs plus diagnostics

Accepted entries include:

- `topologyFingerprint`
- `duplicateOf`
- `budgetChecks`
- `budgetPenalty`
- `selectionScore`
- `analysisRef`
- `compatibleRulesRef`
- `spatialIntentRef`
- `intermediateBreakdownRef`
- `intermediateValidationRef`

Accepted entries are sorted by descending `selectionScore`, then candidate id
for stable tie-breaking.

## Layout Artifact

Kind: `rusty_procgen.layout_2d.v1`

The first layout artifact is an inspectable 2D embedding. It preserves graph
node and edge IDs so diagnostics and viewer labels map back to intent. It is
not a renderer or final tile map.

## Geometry 2D Artifact

Kind: `rusty_procgen.geometry_2d.v1`

Geometry artifacts are the next layer after intermediate breakdowns. They hold
variable room rectangles, routed corridor polylines, bounds, source refs,
semantic style tags, and lightweight contents annotations for generated dungeon
previews. They do not replace `layout_2d`; the older artifact remains the simple
graph embedding.

`layoutPolicy` records the versioned compact-first spacing/search inputs.
`layoutSearch` records the successful spacing tier, room-order and route-order
attempts, cumulative bounded route attempts, and effective spacing. A failed
search emits no geometry artifact and is reported as
`selection_geometry_search_exhausted`; that outcome means this finite policy
found no route, not that single-floor topology has been proven impossible.

`contents` entries are room-scoped labels with source refs back to graph and
intermediate structure. Current kinds include `start_marker`, `goal_marker`,
`key_pickup`, `locked_gate`, `boss_threshold`, `reward_cache`, `hazard`,
`resource_clue`, `shortcut_marker`, and `secret_route_marker`.

See `docs/geometry-html-preview-contract.md`.

## Physical Connection Plan

Kind: `rusty_procgen.physical_connection_plan.v1`

This versioned artifact sits between `intermediate_breakdown` and
`geometry_2d`. Each section declares an explicit physical topology, terminal
regions, width, source connectors/edges, and directional traversal refs.
Compatible reciprocal open edges normalize to one physical section; traversal
semantics are retained through `edgeMappings` rather than duplicated geometry.
Geometry, piece links, glued exits, gate portals, and built-flow validation all
carry the section id so unrelated corridors cannot silently overlap or become
an undeclared junction.

## HTML Preview Artifact

Kind: `rusty_procgen.html_preview.v1`

Preview metadata records geometry, validation, and standalone HTML refs. The
HTML file itself should open from disk and render the generated 2D dungeon as
dark-mode SVG with labels and annotations.

## Shape Catalog Artifact

Kind: `rusty_procgen.shape_catalog.v1`

Shape catalogs describe reusable prefab metadata: occupied cells, reserved
cells, exits, allowed transforms, tags, and feature sockets. Catalog shapes are
JSON metadata in this repo; they are not final art assets, meshes, voxels, or
runtime authority.

The top-level `placementPolicy` is copied into generated placement artifacts.
Schema v1 exposes minimum piece clearance, glued-exits-only contact, wall
thickness, doorway width, and mandatory boundary preservation. It rejects
clearance smaller than `2 * wallThicknessCells + 1` so downstream extrusion can
preserve walls and open only declared routed connections. Schema v1 supports
only `doorwayWidthCells: 1`; wider values fail closed until the placement
artifact can carry an authoritative oriented opening footprint.

Catalog inspection uses kind `rusty_procgen.catalog_inspection.v1` and reports
shape counts, piece kinds, feature socket kinds, exit directions, transforms,
per-shape summaries, and catalog diagnostics.

For modular pack structure, see `docs/build-piece-library-structure.md`.

## Piece Build Plan Artifact

Kind: `rusty_procgen.piece_build_plan.v1`

Piece build plans expand geometry/intermediate intent into explicit piece
requirements before catalog matching. Rooms, corridors, bends, thresholds,
landings, reward pockets, hazards, boss spaces, shortcuts, secrets, and resource
rooms are all first-class pieces. Corridors are not hidden runtime negotiation.
The top-level artifact records `planId`, `candidateId`, `geometryId`,
`sourceCandidateRef`, `sourceIntermediateRef`, `sourceGeometryRef`,
`requirements`, `links`, and `contentRequirements`.
Requirements may carry `requiredShapeTags` when a route planner needs an exact
compatible catalog family rather than a soft tag score.

## Piece Shape Match Artifact

Kind: `rusty_procgen.piece_shape_match.v1`

Piece shape match reports select catalog shape ids and transforms for each
piece-plan requirement before occupancy placement. Each match records the
source `pieceId`, selected `shapeId`, `transform`, mapped exits, mapped feature
sockets, and deterministic score. Rejections preserve agent-readable reasons
for incompatible shapes.

## Piece Placement Artifact

Kind: `rusty_procgen.piece_placement.v1`

Piece placements record selected catalog shapes, transforms, occupied cells,
reserved cells, glued exits, generated connection cells, dangling exits, and
feature/socket placements. They carry the catalog `placementPolicy` and also
declare `gridConnectivity` (`four_way` by
default, optionally `eight_way`) so validators and previews agree on whether
diagonal contact counts as reachable. They are the first artifact layer that
owns occupancy, while still stopping before mesh, voxel, renderer, collision,
or gameplay runtime integration.

Validation rejects configured-clearance violations between unrelated piece
instances. Catalog pieces with the same physical-section provenance may
approach without overlap, and glued-exit-owned route cells fill the gaps
between their occupied footprints in `hybrid` mode. Pure `catalog` placements
instead require exact adjacent opposing prefab ports, zero generated
`connectionCells`, and versioned `catalogSearch` budget/decision evidence.
Unrelated routes may not cross
occupied/reserved cells, enter a non-exit wall, or omit their exact transformed
catalog exit endpoints.

Validation uses kind `rusty_procgen.validation.piece_placement.v1`.

See `docs/piece-assembly-contract.md`.

## Catalog-Aware Generation Policy

Kind: `rusty_procgen.catalog_aware_generation_policy.v2`

The policy bounds attempts, room candidates, per-section routing states, route
margin, guide and turn costs, and the room-compaction sequence. It also owns:

- hard `outcomeConstraints` for final placement width, height, area, and routed
  catalog cells;
- one `outcomePreferences` primary metric (`placement_span`,
  `placement_area`, or `routed_catalog_cells`) and preferred maximum.

Hard constraints are admission rules, not score penalties. Preferences select
among otherwise valid outcomes and do not bypass geometry, placement, or
built-flow validation.

## Catalog-Aware Generation Result

Result kind: `rusty_procgen.catalog_aware_generation.v2`

Exhaustion kind: `rusty_procgen.catalog_aware_generation_exhaustion.v2`

Each attempt records its compaction input, typed stage/classification, bounded
work metrics, and optional final outcome. A final outcome records placement
width, height, span, area, routed catalog cells, bends, routing states, every
hard-limit miss, preference satisfaction, and the comparison against the
current incumbent.

The first admissible outcome meeting the preference is selected. If none meets
it, the deterministic best admissible outcome is selected after the attempt
budget is exhausted. If no attempt is admissible, the result is a typed
exhaustion and no accepted artifact is published.

## Catalog Generation Trace

Kind: `rusty_procgen.catalog_generation_trace.v2`

The trace is a bounded sibling diagnostic artifact for the exact catalog-aware
operation. It binds canonical input hashes, the complete included policy,
semantic room/route/validation/outcome/comparison events, the exact result
hash, and hash-linked event order. It is replay evidence, not mutable
generation authority or a resumable session.

Rust replay reruns the exact typed generation request and requires complete
event equality. The strict TypeScript boundary recomputes the included policy,
event chain, quotas, visible semantic metrics, comparisons, and result
agreement before mounting. See
[`catalog-generation-traces.md`](catalog-generation-traces.md).

## Accepted Artifact

Kind: `rusty_procgen.accepted_artifact.v1`

Accepted artifacts bundle the candidate, layout, score summary, hashes, and
validation/score refs. They are suitable for later catalog and shuffle-bag work.

## Transcript

Transcript files are JSONL. Each line is a `tool_event` with command, output
state, receipt, seed, and args.

Example:

```json
{"kind":"tool_event","command":"graph apply-rule","state":"artifacts/samples/first-run/candidate-001-lock_key_loop.json","receipt":"artifacts/samples/first-run/receipt-001-lock_key_loop.json","seed":4104,"args":{"rule":"lock_key_loop"}}
```
