# Rusty Procgen

Deterministic dungeon generation, validation, catalog placement, and inspection
workbench for Rusty Engine consumers.

Rusty Procgen owns dungeon meaning and orchestration: intent grammar, graph
construction, scoring, repair, geometry, bounded placement, artifact schemas,
content catalogs, and the interactive viewer. Reusable scene, asset, voxel,
spatial, mesh, render, component, and service mechanisms belong in
[`rusty-engine`](https://github.com/FuzzySlipper/rusty-engine).

The repository is being converted in place from its historical Asha donor.
Three explicitly named legacy adapter lanes remain executable while their
Rusty Engine replacements land. Their exact packages, imports, scripts, and
removal tasks are recorded in
[`migration/asha-disposition.json`](migration/asha-disposition.json); the
default gate rejects any unledgered addition. Predecessor artifact kinds are
not supported.

## Fresh Setup

Clone beside `rusty-engine`:

```bash
cd /home/dev
git clone git@github.com:FuzzySlipper/rusty-engine.git rusty-engine
git clone git@github.com:FuzzySlipper/rusty-procgen.git rusty-procgen
cd rusty-procgen
npm install
```

Until the bounded adapter migration closes in Den #6400, the legacy
publication, voxel-authority, and retained-renderer smokes also require the
historical `asha-engine` checkout at `/home/dev/asha-engine`. The generator
algorithms and Rust tests do not depend on its Rust crates.

## Verification

```bash
npm run verify
```

Focused checks:

```bash
npm run check:migration-boundary
npm run check:corpus-identity
npm run typecheck
npm run rust:check
npm run rust:test
npm run publish:legacy-asha-smoke
npm run viewer:smoke
npm run catalog:coverage
```

## Rust library

`rusty-procgen-preflight` exposes the deterministic generator as a library as
well as the `rusty-procgen` binary. Downstream Rust hosts use the
filesystem-free `ProcgenCore` facade for typed graph, geometry, catalog,
placement, and validation operations; the CLI is a thin path/receipt adapter
over the same owners. See
[`docs/rust-library-api.md`](docs/rust-library-api.md) for the module map,
public boundary, and focused verification.

## Temporary legacy publication proof

The temporary downstream adapter maps representative Procgen shape matches and
placements to the predecessor prefab and project-bundle contracts:

```bash
npm run publish:legacy-asha-smoke
```

This is migration evidence scheduled for replacement by Rusty Engine
authored-scene, content, and asset owners in Den #6397. It preserves generation
provenance and fails closed on missing mappings/roles, incompatible assets,
duplicate identities, and invalid transforms. See
[`docs/legacy-asha-adapters.md`](docs/legacy-asha-adapters.md) for its explicit
non-claims and removal boundary.

## Temporary legacy voxel-authority proof

The separate engine-backed authority smoke extrudes a validated 2D piece placement into
a simple enclosed voxel volume. Placement `x/y` maps to voxel `x/z`; the proof
adds a floor, three-voxel walls, and a ceiling, then submits bounded command
batches through the predecessor Rust-backed authority. Den #6398 replaces this
lane with direct Rusty Engine spatial composition; no RuntimeSession-style
facade is part of the target design.

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
animation, or gameplay door authority.

Before geometry, batch generation now emits
`physical-connection-plan.json`. The plan normalizes compatible reciprocal open
edges into one physical corridor, assigns explicit room ports, and carries the
section id through piece placement and built-flow validation. Dense candidates
that exhaust the bounded compact-first search are kept as
`selection_geometry_search_exhausted` rejections rather than being rendered
with accidental junctions. This diagnostic means the configured search did not
find an embedding; it is not a proof that no single-floor embedding exists.

```bash
npm run voxel:legacy-asha-smoke
```

The command regenerates
`artifacts/evidence/native-voxel-extrusion.json` with deterministic authority
voxel-state hashes, command-phase receipts, and bounded comparison readbacks.

This temporary smoke requires the sibling `asha-engine` checkout and its built
native addon. It proves native command acceptance, deterministic authority
voxel-state hashes, and fail-closed unknown-material rejection while the Rusty
host replaces it. A separate voxel-conversion comparison preserves bounded
model/material readback coverage, but it is not the mutation path under test.
The proof does not claim 3D piece placement, exit-socket alignment, rendering,
navigation, or performance evidence.

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
npm run catalog:coverage
npm run catalog-aware:coverage
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
