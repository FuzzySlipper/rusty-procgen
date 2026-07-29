# Rusty Procgen

Deterministic dungeon generation, validation, catalog placement, and inspection
workbench for Rusty Engine consumers.

Rusty Procgen owns dungeon meaning and orchestration: intent grammar, graph
construction, scoring, repair, geometry, bounded placement, artifact schemas,
content catalogs, and the interactive viewer. Reusable scene, asset, voxel,
spatial, mesh, render, component, and service mechanisms belong in
[`rusty-engine`](https://github.com/FuzzySlipper/rusty-engine).

The repository was converted in place from its historical Asha donor. No Asha
package, sibling checkout, runtime adapter, or predecessor artifact decoder is
part of the executable product. Historical dispositions remain recorded in
[`migration/asha-disposition.json`](migration/asha-disposition.json); the
default gate rejects any reintroduction. Predecessor artifact kinds are not
supported.

## Fresh Setup

The executable dependencies resolve from public exact revisions; no sibling
Engine checkout is required:

```bash
cd /home/dev
git clone git@github.com:FuzzySlipper/rusty-procgen.git rusty-procgen
cd rusty-procgen
pnpm install --frozen-lockfile
```

## Verification

```bash
pnpm run verify
```

Focused checks:

```bash
pnpm run check:migration-boundary
pnpm run check:corpus-identity
pnpm run engine:revision
pnpm run typecheck
pnpm run rust:check
pnpm run rust:test
pnpm run engine:publication:test
pnpm run publish:rusty-engine-smoke
pnpm run engine:spatial:test
pnpm run voxel:rusty-engine-smoke
pnpm run viewer:smoke
pnpm run catalog:coverage
```

## Rust library

`rusty-procgen-preflight` exposes the deterministic generator as a library as
well as the `rusty-procgen` binary. Downstream Rust hosts use the
filesystem-free `ProcgenCore` facade for typed graph, geometry, catalog,
placement, and validation operations; the CLI is a thin path/receipt adapter
over the same owners. See
[`docs/rust-library-api.md`](docs/rust-library-api.md) for the module map,
public boundary, and focused verification.

## Rusty Engine generated-content publication

The explicit downstream Rust adapter maps representative Procgen shape matches
and placements into the public Rusty Engine asset catalog, prefab registry,
authored scene, entity admission, and atomic content publication owners:

```bash
pnpm run publish:rusty-engine-smoke
```

The adapter is isolated from the normal Procgen core and selects one exact
public Engine commit through `engine-source.json`. It preserves the
candidate-to-placement provenance chain, reads the admitted scene back through
strict Engine codecs, and fails before publication on missing mappings or
roles, incompatible assets, duplicate identities, invalid transforms, stale
pins, quotas, and late owner validation. See
[`docs/rusty-engine-publication.md`](docs/rusty-engine-publication.md).

## Rusty Engine spatial extrusion

The isolated downstream Rust adapter at `integrations/rusty-engine-spatial/`
extrudes a validated 2D piece placement into a simple enclosed voxel volume.
Placement `x/y` maps to voxel `x/z`; Procgen adds a floor, three-voxel walls,
and a ceiling, then composes the exact-pinned
`engine-spatial::VoxelCollisionScene` and `VoxelEditService` public surfaces.
It contains no RuntimeSession-style facade, command tunnel, Asha package,
renderer, or sibling Engine checkout.

The source placement carries a versioned policy for minimum inter-piece
clearance, wall thickness, and doorway width (schema v1 supports width one).
Occupied cells retain their piece
owners through extrusion; walls surround the separated footprints and only
connection routes anchored to exact transformed glued exits become openings.
The compiler rejects unsafe policy combinations, wider unsupported openings,
and routes that would open a non-exit boundary or unrelated piece.

```bash
pnpm run engine:spatial:test
pnpm run voxel:rusty-engine-smoke
```

The smoke regenerates
`artifacts/evidence/engine-spatial-extrusion.json`. Large placements are applied
to an off-side Engine scene in deterministic transactions bounded by Engine's
public edit quota; the live authority is replaced only after every collision,
navigation, and mesh projection succeeds. Focused regressions prove
deterministic repetition, concrete-authority reopen, exact material and portal
provenance, and non-mutation on malformed placement, unknown material,
oversized plan, stale revision, and late Engine rejection. See
[`docs/rusty-engine-spatial.md`](docs/rusty-engine-spatial.md).

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
pnpm run procgen -- build validate-flow \
  --candidate artifacts/samples/batch-v2/candidate-000/accepted.json \
  --geometry artifacts/samples/batch-v2/candidate-000/geometry-2d.json \
  --piece-plan artifacts/samples/batch-v2/candidate-000/piece-plan.json \
  --piece-placement artifacts/samples/batch-v2/candidate-000/piece-placement.json \
  --out /tmp/built-flow.validation.json
```

Procgen owns this generation and validation evidence. Portals are not gameplay
doors: this work does not claim inventory, collision, navigation, persistence,
animation, or gameplay door authority.

Before geometry, batch generation now emits
`physical-connection-plan.json`. The plan normalizes compatible reciprocal open
edges into one physical corridor, assigns explicit room ports, and carries the
section id through piece placement and built-flow validation. Dense candidates
that exhaust the bounded compact-first search are kept as
`selection_geometry_search_exhausted` rejections rather than being rendered
with accidental junctions. This diagnostic means the configured search did not
find an embedding; it is not a proof that no single-floor embedding exists.

## Engine Voxel Inspection

The LAN viewer keeps the existing isometric `Voxel` evidence tab and adds a
separate `Voxel 3D` inspection tab. The 3D view compiles the same placement
extrusion, omits only its ceiling from the presentation frame, and mounts the
exact-revision public `@rusty-engine/renderer-host` inspection surface with its
procedural grid, mouse or arrow-key orbit, focused W/A/S/D movement, and
keyboard/wheel zoom. The frame crosses
`@rusty-engine/render-contracts` strict decoding before submission. It is
projection-only visual evidence, not collision, navigation, native-render, or
performance authority.

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

The viewer exposes renderer compatibility, retained-frame, viewport, grid, and
camera readouts without treating them as dungeon truth. The real-Chromium smoke
proves candidate/config replacement, rapid stale-work suppression, resizing,
controls, picking, and page-hide disposal. Malformed contract frames reject
before renderer mutation, and ordinary headless generation remains independent
of the renderer packages.

The Build, Voxel, and Voxel 3D tabs expose one generation configuration form
for geometry distribution, placement clearance, route wall buffer, catalog-aware
retry/routing policy, and corridor realization. Apply and rebuild submits every value together, runs geometry
through built-flow validation in a bounded Rust workspace, and atomically
persists the configuration only after every stage succeeds. Failed or invalid
builds leave both the active result and `config/viewer-generation.json`
unchanged. Each setting stores both `value` and `defaultValue`, so Reset to
defaults uses the same complete validated rebuild and persistence path.

The corridor-realization setting has three explicit whole-build modes.
`catalog` performs pure prefab assembly: it selects catalog rooms, aligns them
to the generated room zones, and then searches one-cell straight/bend chains
between their exact exits. Every room and corridor cell belongs to a selected
catalog shape, exits glue directly, and generated connection cells are
forbidden. The versioned policy controls bounded room alternatives, slack
escalation, route margin, guide/turn costs, and per-section state budgets.
Failures distinguish generation infeasibility, missing catalog vocabulary, and
search-budget exhaustion. `hybrid` preserves the
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

Pure-catalog corpus coverage is reproducible with:

```bash
pnpm run catalog:coverage
pnpm run catalog-aware:coverage
```

The command realizes every accepted batch candidate in `catalog` mode and
writes `artifacts/evidence/pure-catalog-coverage.json`. Successful outcomes
must have zero generated connection cells and pass placement plus built-flow
validation. Rejections retain their bounded structured evidence and are grouped
by endpoint, envelope, and exhausted-family signature. This distinguishes a
reusable catalog vocabulary gap from geometry whose room ports or corridor
anchors need a catalog-aware generation retry.

The catalog-aware command rebuilds every accepted candidate twice through the
unified viewer configuration path and writes
`artifacts/evidence/catalog-aware-generation-coverage.json`. The current sample
is 5/9 candidates and 4/6 topology fingerprints, while strict pure-catalog
assembly remains 0/9. Every recorded catalog-aware success has a stable
repeated build ID, zero generated connection cells, and successful geometry,
placement, and built-flow validation.

A separate geometry-layout panel controls the earlier room distribution pass:
initial outer/column/row spacing, per-tier growth, spacing-tier count, and room
ordering attempts. Apply reruns geometry, piece placement, and built-flow
validation together in a temporary workspace. The default policy starts
compact and escalates only after a tighter tier exhausts its route-order budget;
the route grid and exclusive corridor separation remain fixed safety
invariants. The current 10-layout corpus accepts 9 and rejects 1. The checked
geometry-recovery report pins seven representative family plans with
identity-neutral normalized hashes, compactness evidence, deterministic
realization-search receipts, and zero-fatal physical validation for accepted
placements.

## Rusty Engine boundary

Compose public Rusty Engine crates and packages through direct named services.
If Procgen needs a missing reusable mechanism, capture a minimal concrete
consumer reproduction and route the engine-owned work upstream. Do not recreate
generic scene, asset, voxel, spatial, renderer, component, or service authority
locally.

The local Rust lane in `procgen-rs/` is for downstream preflight tooling,
generation logic, validation evidence, and project-specific experiments. The
remaining Asha imports are temporary conversion exceptions owned exclusively by
the migration ledger and Den #6397–#6400; they are not patterns for new work.
