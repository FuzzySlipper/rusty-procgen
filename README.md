# Asha Procgen

Dungeon procgen incubator for richer ASHA generated-level experiments.

This repository is an ASHA downstream project. It should consume `asha-engine`
through public package roots, generated contracts, documented runtime/session
surfaces, and local prototype evidence. It should not import engine internals or
silently patch a sibling engine checkout.

## Fresh Setup

Clone beside `asha-engine`:

```bash
cd /home/dev
git clone git@github.com:FuzzySlipper/asha-engine.git asha-engine
git clone git@github.com:FuzzySlipper/asha-procgen.git asha-procgen
cd asha-procgen
npm install
```

## Verification

```bash
npm run verify
```

Focused checks:

```bash
npm run check:asha-boundary
npm run typecheck
npm run rust:check
npm run rust:test
npm run publish:asha-smoke
npm run viewer:smoke
```

## ASHA Prefab and ProjectBundle Publishing Proof

The downstream publishing adapter maps representative Procgen shape matches
and placements to generated ASHA prefab identities and a ProjectBundle-shaped
durable artifact inventory:

```bash
npm run publish:asha-smoke
```

It uses public `@asha/contracts` and `@asha/game-workspace` roots, preserves
generation provenance, and fails closed on missing mappings/roles, incompatible
assets, duplicate identities, and invalid transforms. See
[`docs/asha-publishing-boundary.md`](docs/asha-publishing-boundary.md) for the
ownership, distribution, non-claims, and crate-disposition contract.

## Native ASHA Voxel Extrusion Proof

The separate engine-backed authority smoke extrudes a validated 2D piece placement into
a simple enclosed voxel volume. Placement `x/y` maps to ASHA voxel `x/z`; the
proof adds a floor, three-voxel walls, and a ceiling, then submits bounded
`generateChunk`, `fillRegion`, and `setVoxel` batches through a Rust-backed
public `RuntimeSession`.

The source placement carries a versioned policy for minimum inter-piece
clearance, wall thickness, and doorway width (schema v1 supports width one).
Occupied cells retain their piece
owners through extrusion; walls surround the separated footprints and only
connection routes anchored to exact transformed glued exits become openings.
The compiler rejects unsafe policy combinations, wider unsupported openings,
and routes that would open a non-exit boundary or unrelated piece.

## Built Flow Validation

Piece plans retain link-specific exit ids, physical-section ids, all mapped
source edge/corridor ids, traversal refs, and item requirements as structured
fields. Shape matching consumes every required exit exactly once. Assembly
emits one stable gate portal per physical section, including the exact cell,
orientation, width, mapped logical edges, controlling pieces, and provenance
chain. Compatible reciprocal open edges therefore share one corridor and one
portal instead of creating overlapping physical routes.

Every accepted batch entry includes `built-flow.validation.json`. The report
checks the candidate → geometry → ordered piece links → glued exits → routed
cell chain, then runs an item-aware directional flood over the presentation
walkable projection. A route is activated only after its authored source node
is physically reached and its required item is available, so a reverse-facing
edge or crossing cannot silently bypass a gate. The report is reproducible with:

```bash
npm run procgen -- build validate-flow \
  --candidate artifacts/samples/batch-v2/candidate-000/accepted.json \
  --geometry artifacts/samples/batch-v2/candidate-000/geometry-2d.json \
  --piece-plan artifacts/samples/batch-v2/candidate-000/piece-plan.json \
  --piece-placement artifacts/samples/batch-v2/candidate-000/piece-placement.json \
  --out /tmp/built-flow.validation.json
```

Procgen owns this generation and validation evidence. Portals are not gameplay
doors: this work does not claim inventory, collision, navigation, persistence,
animation, or RuntimeSession door authority.

Before geometry, batch generation now emits
`physical-connection-plan.json`. The plan normalizes compatible reciprocal open
edges into one physical corridor, assigns explicit room ports, and carries the
section id through piece placement and built-flow validation. Dense candidates
that exhaust the bounded compact-first search are kept as
`selection_geometry_search_exhausted` rejections rather than being rendered
with accidental junctions. This diagnostic means the configured search did not
find an embedding; it is not a proof that no single-floor embedding exists.

```bash
npm run voxel:asha-smoke
```

The command regenerates
`artifacts/evidence/native-voxel-extrusion.json` with deterministic authority
voxel-state hashes, command-phase receipts, and bounded comparison readbacks.

The smoke test requires the sibling `asha-engine` checkout and its built native
addon at `ts/packages/native-bridge/dist/native-bridge.node`. It proves native
command acceptance, deterministic authority voxel-state hashes, and fail-closed
unknown-material rejection. A separate voxel-conversion comparison preserves
bounded model/material readback coverage, but it is not the mutation path under
test. The proof does not claim 3D piece placement, exit-socket alignment,
rendering, navigation, or performance evidence.

## Engine Voxel Inspection

The LAN viewer keeps the existing isometric `Voxel` evidence tab and adds a
separate `Voxel 3D` inspection tab. The 3D view compiles the same placement
extrusion, omits only its ceiling from the presentation frame, and mounts the
public `@asha/renderer-host` inspection surface with its procedural grid, mouse
or arrow-key orbit, focused W/A/S/D movement, and keyboard/wheel zoom. It is
projection-only visual evidence, not RuntimeSession,
collision, navigation, native-render, or performance authority.

Before submission, the presentation projection deterministically partitions
same-material voxels into lossless maximal cuboids. The projection smoke
reconstructs every cuboid cell-by-cell and checks the complete accepted sample
corpus against the renderer host's public per-frame operation limit. This
compaction changes only renderer node count; it does not alter placement,
extrusion, or authoritative voxel commands.

When the selected placement has a matching successful built-flow report, the
3D tab adds one public renderer-host cuboid per verified portal cell. Locked
doors are translucent red and unlocked doors are translucent blue. The Door
state selector can show the initial state, each verified item-collection step,
or an all-unlocked presentation. Door nodes participate in the same frame-op
budget and include source-edge, portal, and required-item identity in their
pick label.

The Build, Voxel, and Voxel 3D tabs expose one generation configuration form
for geometry distribution, placement clearance, route wall buffer, and corridor
realization. Apply and rebuild submits every value together, runs geometry
through built-flow validation in a bounded Rust workspace, and atomically
persists the configuration only after every stage succeeds. Failed or invalid
builds leave both the active result and `config/viewer-generation.json`
unchanged. Each setting stores both `value` and `defaultValue`, so Reset to
defaults uses the same complete validated rebuild and persistence path.

The corridor-realization setting has three explicit whole-build modes.
`catalog` performs pure prefab assembly: every room and corridor cell belongs
to a selected catalog shape, exits glue directly, and generated connection
cells are forbidden. Its bounded deterministic search jointly considers shape,
90-degree rotation, and exit-anchored origin; geometry facing is an ordering
hint rather than a hard room-orientation constraint. `hybrid` preserves the
earlier behavior, covering planned route segments with bounded
short/medium/long straight families and sized bend prefabs while route cells
fill uncovered gaps. `procedural` keeps catalog-backed room and feature pieces but
replaces every corridor prefab set with one direct physical-section route constrained
to the planned geometry polyline's bounded lane envelope. All three modes preserve
section provenance, exclusive routing, portals, placement validation, and
built-flow validation; modes are never mixed automatically. Dedicated
`planned_junction` catalog shapes remain ineligible until a physical-section
plan explicitly requests the `junction` kind. The panel reports
corridor-prefab, routed-cell, and footprint counts; a successful pure catalog
build always reports zero routed cells. The configured result is
used by Build, Voxel, and Voxel 3D, including verified door progression. A
configured build is persisted downstream evidence, but does not inherit a
matching native-authority receipt.

A separate geometry-layout panel controls the earlier room distribution pass:
initial outer/column/row spacing, per-tier growth, spacing-tier count, and room
ordering attempts. Apply reruns geometry, piece placement, and built-flow
validation together in a temporary workspace. The default policy starts
compact and escalates only after a tighter tier exhausts its route-order budget;
the route grid and exclusive corridor separation remain fixed safety
invariants. The current 10-layout corpus remains 3 accepted / 7 exhausted, but
accepted frames shrink from 2160×720 to 1392×480 for the nested-boss layouts
and from 1328×688 to 848×480 for the lock-key baseline. The unchanged rejection
count shows that those seven cases need broader topology-aware room/port
embedding alternatives, not merely larger fixed gaps. The three accepted
placements include deterministic realization-search evidence; both
non-baseline nested-boss layouts require route-order backtracking and retain
zero-fatal built-flow reports.

## ASHA Boundary

Use public ASHA package roots and documented subpaths only. If a prototype needs
a missing public ASHA capability, capture a minimal reproduction here and move
the engine-owned work upstream to `asha-engine`.

The local Rust lane in `procgen-rs/` is for downstream preflight tooling,
prototype generation evidence, and project-specific experiments. Generic
runtime/session, collision, pathfinding, render projection, protocol/codegen,
and replay authority belong upstream.
