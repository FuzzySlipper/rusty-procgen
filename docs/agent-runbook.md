# Agent Runbook

Status: v2 sample runbook for the CLI workbench and viewer.

## Install

```bash
pnpm install --frozen-lockfile
```

## Build A Sample Run

```bash
pnpm run baseline
```

This writes a deterministic example under:

```text
artifacts/samples/first-run/
```

Important files:

- `candidate-000-base.json`
- `candidate-001-lock_key_loop.json`
- `candidate-002-optional_treasure_detour.json`
- `candidate-003-one_way_shortcut.json`
- `candidate-004-secret_bypass.json`
- `validation.graph.json`
- `score.graph.json`
- `layout-2d.json`
- `accepted.json`
- `transcript.jsonl`

## Build A Batch Run

```bash
pnpm run batch:sample
```

This writes a deterministic v2 batch under:

```text
artifacts/samples/batch-v2/
```

Important files:

- `selection-report.json`
- `candidate-000/accepted.json`
- `candidate-000/validation.graph.json`
- `candidate-000/analysis.graph.json`
- `candidate-000/compatible-rules.json`
- `candidate-000/spatial-intent.json`
- `candidate-000/intermediate-breakdown.json`
- `candidate-000/intermediate.validation.json`
- `candidate-000/score.graph.json`
- `candidate-000/transcript.jsonl`

The sample command generates 10 candidates from:

```text
fixtures/batch-profiles/v2-sample.json
```

### Geometry recovery corpus

Run the versioned topology-embedding and exclusive-routing recovery corpus with:

```bash
pnpm run geometry:recovery
```

This regenerates the unchanged ten-candidate sample in release mode and writes
`artifacts/evidence/geometry-recovery-v1.json`. The corpus manifest at
`fixtures/geometry-recovery/v1.json` pins the seven unique physical topology
families (the original five rejected and two accepted families) by profile,
candidate seed, and normalized physical-plan SHA-256. The report records each
pipeline stage, rotation witness or bounded rejection evidence, configured
realization result, routed-section count, blocking owners, alternatives,
decisions, repairs, backtracks, and exhausted budget. It also compares every
accepted layout with the exact `75ed65f` baseline using room-envelope area,
corridor centerline length, routed-shell/connection-cell count, and occupied
piece cells. Regeneration fails if the median compactness target, per-layout
regression limit, 9/10 acceptance floor, or physical validators regress.

Geometry search evaluates every valid alternative that fits within the current
bounded spacing tier, rejects portal distributions the generic catalog cannot
realize, and chooses the lowest envelope/routed-shell score before escalating.
Planar room scaling uses room-center separation only; the exclusive router
independently proves corridor-to-room and corridor-to-corridor clearance. This
avoids allowing a near edge in the abstract planar witness to magnify the whole
level.

In the viewer, outer margin affects both embedding kinds. Column/row gap and
growth values tune depth-column layouts; planar-rotation layouts derive their
safe room separation from room envelopes and compare bounded compact
alternatives automatically. Maximum tiers and route attempts remain hard
escalation bounds for both.

Geometry compactness and physical realization scale are separate. Procedural
and hybrid assembly first probe the geometry at physical scale 1 with a small,
deterministic route budget, then scale 2, and only use the full route-search
budget at scales 3 and 4. A dense layout may therefore retain more spacing
after lower scales fail clearance-safe exclusive routing, while a simple layout
such as seed 5801 stays at scale 1. The viewer rebuild summary reports the
selected physical scale and number of attempted scales.

After regeneration, verify that the committed report is byte-identical with:

```bash
pnpm run geometry:recovery:check
```

The selection report records the profile id/ref, the profile sequence used for
each candidate, topology fingerprints, budget checks, and sorts accepted entries
by deterministic selection score. Accepted entries also carry refs to graph
analysis, compatible rules, spatial intent, intermediate breakdown, and
intermediate validation artifacts.

`pnpm run batch:sample` also emits the full generated dungeon preview stack for
each accepted candidate:

```text
artifacts/samples/batch-v2/<candidate>/geometry-2d.json
artifacts/samples/batch-v2/<candidate>/geometry-2d.validation.json
artifacts/samples/batch-v2/<candidate>/geometry-2d.preview.html
artifacts/samples/batch-v2/<candidate>/html-preview.json
artifacts/samples/batch-v2/<candidate>/shape-catalog.report.json
artifacts/samples/batch-v2/<candidate>/piece-plan.json
artifacts/samples/batch-v2/<candidate>/piece-shape-match.json
artifacts/samples/batch-v2/<candidate>/piece-placement.json
artifacts/samples/batch-v2/<candidate>/piece-placement.validation.json
```

Each `accepted` entry in `selection-report.json` carries `geometryRef`,
`geometryValidationRef`, `htmlPreviewRef`, `htmlRef`, `shapeCatalogRef`,
`catalogInspectionRef`, `piecePlanRef`, `shapeMatchRef`, `piecePlacementRef`,
and `piecePlacementValidationRef`. The viewer Build tab prefers the catalog
piece placement grid and falls back to the older geometry-rasterized grid when
piece artifacts are absent.

## Manual CLI Sequence

```bash
pnpm run procgen -- init \
  --intent fixtures/intents/first-slice.intent.json \
  --seed 4103 \
  --out artifacts/manual/candidate-000-base.json \
  --receipt artifacts/manual/receipt-000-init.json \
  --transcript artifacts/manual/transcript.jsonl

pnpm run procgen -- graph apply-rule \
  --state artifacts/manual/candidate-000-base.json \
  --rule lock_key_loop \
  --seed 4104 \
  --out artifacts/manual/candidate-001-lock_key_loop.json \
  --receipt artifacts/manual/receipt-001-lock_key_loop.json \
  --transcript artifacts/manual/transcript.jsonl

pnpm run procgen -- validate graph \
  --state artifacts/manual/candidate-001-lock_key_loop.json \
  --out artifacts/manual/validation.graph.json

pnpm run procgen -- score graph \
  --state artifacts/manual/candidate-001-lock_key_loop.json \
  --out artifacts/manual/score.graph.json
```

Use `pnpm run procgen -- graph summarize --state <candidate>` to print a compact
agent-readable graph summary.

Fork before trying alternate plans:

```bash
pnpm run procgen -- graph fork \
  --state artifacts/manual/candidate-001-lock_key_loop.json \
  --label boss-prep-attempt \
  --seed 4201 \
  --out artifacts/manual/candidate-001a-boss-prep-fork.json \
  --receipt artifacts/manual/receipt-001a-fork.json \
  --transcript artifacts/manual/transcript.jsonl
```

For machine-readable planning context:

```bash
pnpm run procgen -- graph rules --out artifacts/manual/rules.json

pnpm run procgen -- graph summarize \
  --state artifacts/samples/batch-v2/candidate-005/candidate-007-branch_merge_shortcut.json \
  --json \
  --out artifacts/manual/summary.json

pnpm run procgen -- analyze graph \
  --state artifacts/samples/batch-v2/candidate-005/candidate-007-branch_merge_shortcut.json \
  --out artifacts/manual/analysis.json

pnpm run procgen -- graph compatible-rules \
  --state artifacts/samples/batch-v2/candidate-005/candidate-007-branch_merge_shortcut.json \
  --out artifacts/manual/compatible-rules.json
```

Implemented richer graph rules:

```text
hub_spoke_cluster
nested_lock_key_chain
hazard_resource_tradeoff
boss_preparation_loop
gated_treasure_branch
branch_merge_shortcut
```

Duplicate or incompatible rule applications are rejected with receipt
diagnostics and `repairHint` text where the tool can suggest a next edit.

## Intermediate Layout Intent

The pre-geometry graph analysis and breakdown contract is documented in:

```text
docs/intermediate-layout-contract.md
```

A typical manual chain:

```bash
pnpm run procgen -- annotate spatial-intent \
  --state artifacts/samples/batch-v2/candidate-005/candidate-007-branch_merge_shortcut.json \
  --analysis artifacts/manual/analysis.json \
  --out artifacts/manual/spatial-intent.json

pnpm run procgen -- breakdown emit \
  --state artifacts/samples/batch-v2/candidate-005/candidate-007-branch_merge_shortcut.json \
  --annotations artifacts/manual/spatial-intent.json \
  --out artifacts/manual/intermediate-breakdown.json

pnpm run procgen -- breakdown validate \
  --state artifacts/manual/intermediate-breakdown.json \
  --out artifacts/manual/intermediate.validation.json
```

This layer names regions, connectors, and constraints for later geometry passes.
It does not emit rooms, meshes, voxels, or 3D placement.

## Geometry HTML Preview

The generated 2D dungeon preview target is documented in:

```text
docs/geometry-html-preview-contract.md
```

This is the planned path from intermediate breakdowns to standalone HTML/SVG
floor-plan previews with variable rooms, corridors, labels, and contents. It is
separate from the existing simple `layout-2d.json` graph embedding.

Plan physical connections, then emit geometry from that exact plan:

```bash
pnpm run procgen -- geometry plan-connections \
  --candidate artifacts/samples/batch-v2/candidate-005/candidate-007-branch_merge_shortcut.json \
  --intermediate artifacts/samples/batch-v2/candidate-005/intermediate-breakdown.json \
  --out artifacts/manual/physical-connection-plan.json

pnpm run procgen -- geometry emit-2d \
  --candidate artifacts/samples/batch-v2/candidate-005/candidate-007-branch_merge_shortcut.json \
  --intermediate artifacts/samples/batch-v2/candidate-005/intermediate-breakdown.json \
  --connection-plan artifacts/manual/physical-connection-plan.json \
  --layout-policy fixtures/geometry-layout-policies/compact-first-v1.json \
  --seed 6101 \
  --out artifacts/manual/geometry-2d.json
```

Validate the emitted geometry before using it as preview evidence:

```bash
pnpm run procgen -- geometry validate-2d \
  --state artifacts/manual/geometry-2d.json \
  --out artifacts/manual/geometry-2d.validation.json
```

Render the standalone HTML/SVG preview:

```bash
pnpm run procgen -- preview html \
  --geometry artifacts/manual/geometry-2d.json \
  --validation artifacts/manual/geometry-2d.validation.json \
  --out artifacts/manual/geometry-2d.preview.html
```

## Piece Assembly Preview

The catalog-driven piece assembly target is documented in:

```text
docs/piece-assembly-contract.md
docs/build-piece-library-structure.md
```

This is the path from geometry rectangles/corridors to prefab or voxel-ready
build data. It treats rooms, corridors, bends, thresholds, landings,
reward pockets, hazards, boss spaces, shortcuts, secrets, and resource rooms as
explicit pieces with exits, feature sockets, catalog matches, transformed
occupancy cells, reservations, and glued-exit validation.

Current piece assembly commands:

```bash
pnpm run procgen -- build catalog inspect \
  --catalog fixtures/shape-catalogs/2d-basic.json \
  --out artifacts/manual/shape-catalog.report.json

pnpm run procgen -- build emit-piece-plan \
  --candidate artifacts/samples/batch-v2/candidate-005/candidate-007-branch_merge_shortcut.json \
  --geometry artifacts/manual/geometry-2d.json \
  --intermediate artifacts/manual/intermediate-breakdown.json \
  --out artifacts/manual/piece-plan.json

pnpm run procgen -- build match-shapes \
  --catalog fixtures/shape-catalogs/2d-basic.json \
  --piece-plan artifacts/manual/piece-plan.json \
  --seed 7101 \
  --out artifacts/manual/piece-shape-match.json

pnpm run procgen -- build assemble \
  --catalog fixtures/shape-catalogs/2d-basic.json \
  --piece-plan artifacts/manual/piece-plan.json \
  --shape-match artifacts/manual/piece-shape-match.json \
  --connectivity four-way \
  --out artifacts/manual/piece-placement.json

pnpm run procgen -- build validate-placement \
  --state artifacts/manual/piece-placement.json \
  --out artifacts/manual/piece-placement.validation.json
```

Focused smoke:

```bash
pnpm run piece:smoke
```

Do not treat the current viewer Build tab's geometry-rasterized cells as final
piece-placement authority. The `piece-plan.json` artifact is the requirement
graph, and `piece-shape-match.json` records selected catalog shape ids,
transforms, exit maps, socket maps, and rejected alternatives. The
`piece-placement.json` artifact owns the first catalog-driven occupancy cells,
generated physical connection cells, reservations, glued exits, and
dangling-exit diagnostics. Assembly defaults to four-way grid connectivity;
use `--connectivity eight-way` only for games where diagonal contact is meant
to count as reachable.

The initial metadata-only fixture catalog is:

```text
fixtures/shape-catalogs/2d-basic.json
fixtures/packs/2d-basic/procgen-pack.json
```

The viewer Catalog tab renders the active shape catalog from
`shapeCatalogRef`, including each build piece's footprint, reserved cells,
exits, sockets, transforms, and tags. Treat this as the visible contract for
whether build pieces are first-class inputs instead of hidden placement
side-effects.

## Pattern Catalog

The next graph grammar vocabulary is documented in:

```text
docs/v2-graph-grammar-catalog.md
fixtures/rule-catalog/v2-graph-patterns.json
```

Implemented `graph apply-rule --rule <id>` values should stay aligned with the
catalog ids and preserve the documented invariants, scoring hints, and repair
hints.

## Agent Construction Loop

The next workbench layer is tracked in:

```text
docs/agent-construction-loop.md
```

That document defines the intended external-agent loop and the planned command
surfaces for rule metadata, JSON graph summaries, candidate forking, repair
reports, data-driven batch profiles, and viewer context panes.

## Broken Fixture Check

This intentionally fails with stable diagnostics:

```bash
pnpm run procgen -- validate graph \
  --state fixtures/candidates/invalid-missing-key.candidate.json \
  --out artifacts/manual/invalid.validation.json
```

Expected fatal diagnostic code:

```text
required_item_unavailable
```

To turn diagnostics into an advisory repair artifact:

```bash
pnpm run procgen -- repair suggest \
  --state fixtures/candidates/invalid-missing-key.candidate.json \
  --out artifacts/manual/invalid.repair.json
```

Repair reports preserve validator diagnostics and add `suggestedActions`.
Suggestions are planning aids only; validate repaired candidates before scoring
or accepting them.

Some diagnostics can now be handled with bounded repair actions:

```bash
pnpm run procgen -- repair apply \
  --state <candidate.json> \
  --action add_rejoin_edge \
  --target <terminal-node-id> \
  --seed <u64> \
  --out <candidate.json> \
  --receipt <receipt.json>
```

## LAN Viewer

Use `den-serve` so the viewer is reachable from another machine on the LAN:

```bash
den-serve up rusty-procgen -repo /home/dev/rusty-procgen
```

The LAN URL printed by `den-serve` is the URL to give the human.

Useful commands:

```bash
den-serve status rusty-procgen -repo /home/dev/rusty-procgen
den-serve logs rusty-procgen -repo /home/dev/rusty-procgen
den-serve stop rusty-procgen -repo /home/dev/rusty-procgen
```

Serving semantics come from Den document `den-services/den-serve-agent-usage`.
Do not replace this with localhost-only instructions.

Viewer API routes:

- `/api/artifacts/first-run`
- `/api/batches/v2`
- `/api/artifacts/by-path?path=<artifact-ref-from-selection-report>`
- `/api/evidence/engine-spatial-extrusion`

The batch viewer shows candidate scores, profile sequence, artifact refs,
validation status, provenance steps, and any diagnostics/repair hints for the
selected artifact. Its Build tab renders catalog piece placements when
`piecePlacementRef` is present: occupied cells, connection cells, reserved
cells, glued exits, piece labels, and socket/content markers.
Its Catalog tab renders the active build-piece shape catalog when
`shapeCatalogRef` is present.
Its Voxel tab uses the same downstream placement policy as
`pnpm run voxel:rusty-engine-smoke` to render an isometric floor/wall/ceiling cutaway.
The compiler consumes the placement's versioned clearance/wall/doorway policy,
keeps piece ownership through validation, and turns only glued-exit-owned
connection routes into openings.
When the selected placement matches the committed Engine spatial evidence, the
tab also shows the canonical authority hash and Engine pin. Other candidates are
clearly labelled as unverified voxel proposals.
Its separate Voxel 3D tab sends a strictly decoded ceiling-free floor/wall
projection through the exact-revision public Engine inspection renderer and
procedural grid. Drag with the
primary mouse button or use Arrow keys to orbit; focus the canvas and use
W/A/S/D to move, and use +/− or the wheel to zoom. Candidate changes atomically
replace the retained frame and grid, and rapid selection changes discard stale
async work. The downstream projection compacts
same-material cells into deterministic maximal cuboids, and its smoke expands
those cuboids cell-by-cell to prove exact, non-overlapping material coverage for
every accepted sample before checking the public renderer frame-op limit. This
tab is projection-only inspection and does not claim runtime, collision,
navigation, native-render, or performance authority.

Build, Voxel, and Voxel 3D show one generation configuration form backed by
`config/viewer-generation.json`. Every editable field has a `value` and
`defaultValue`. The form edits compact-first room margin, column/row gaps,
per-tier growth, bounded search tiers and room orders, placement clearance,
route wall buffer, catalog-aware retry/routing policy, and
`catalog`/`hybrid`/`procedural` corridor realization together. `catalog` is
strict prefab-only assembly: catalog rooms are aligned first, then
deterministic one-cell straight/bend chains are searched between exact room
exits. It may reject with an explicit generation-infeasibility,
catalog-coverage-gap, or search-budget-exhaustion classification; `hybrid` is
the former prefab-plus-routed-gap behavior.
Schema v1 keeps the 8-unit route grid, `contactPolicy=glued_exits_only`,
`doorwayWidthCells=1`, `preservePieceBoundaries=true`, physical-section
exclusivity, and built-flow validation fixed.

Apply and rebuild posts the selected candidate ID and exact versioned config to
the local viewer server. The server derives all artifact refs and deterministic
seeds from the committed batch report, then runs geometry emission and
validation, piece planning, shape matching, assembly, placement validation, and
built-flow validation in one temporary workspace. It atomically replaces the
config file only after every stage passes. Any validation, geometry search,
assembly, or flow failure returns JSON and preserves the prior config and active
result. The endpoint accepts no browser filesystem paths and does not mutate
fixtures, samples, Engine spatial authority, or checked voxel evidence.

Reset to defaults copies every `defaultValue` into `value` and submits the same
complete rebuild. Candidate switching reloads the persisted configuration; it
does not silently discard or substitute per-panel temporary state. For isolated
automation, set `RUSTY_PROCGEN_GENERATION_CONFIG_PATH` to a temporary config
copy. The endpoint and browser smokes do this so checks never rewrite the
tracked configuration.

Catalog corridor mode uses the versioned `catalogAwareGenerationPolicy` from
the same configuration file. Its bounded attempts can vary room candidates and
room-zone slack; its guide-biased route search composes only catalog pieces and
never emits procedural connection cells. Procedural mode keeps room/feature
prefabs, omits connector/corridor/bend instances, and constrains each direct
physical-section route to its planned geometry-lane envelope. All modes produce
matching placement and built-flow proof, so Voxel 3D door progression remains
available. Configured builds remain downstream, non-native-authority evidence.

Reproduce the catalog-aware accepted-corpus result with:

```bash
pnpm run catalog-aware:coverage
```

This uses an isolated configuration copy, repeats successful builds to prove a
stable build ID, and writes
`artifacts/evidence/catalog-aware-generation-coverage.json`.

## Verification

```bash
pnpm run verify
```

The default gate checks the empty predecessor boundary, exact public Engine
revision across Rust and renderer packages, corpus identity, TypeScript, Rust
compile, Rust tests, publication, and a focused two-room voxel-boundary smoke.
Browser smoke is not part of the default gate.

For optional preview-site evidence:

```bash
pnpm run viewer:smoke
```

`pnpm run viewer:serve` builds the browser bundle and serves the already
verified committed artifacts so managed health checks are not blocked by the
bounded-but-expensive batch search. Use `pnpm run viewer:serve:regenerate` only
when intentionally rebuilding baseline and batch evidence before serving.

The transactional generation-config contract and the legacy focused endpoint
contracts can be checked separately:

```bash
pnpm run policy:smoke
```

The standalone HTML preview smoke alias is:

```bash
pnpm run preview:smoke
```

This builds the viewer, starts the local preview server on `127.0.0.1`, checks
the sample batch and intermediate artifact API, verifies the dark theme CSS, and
checks the top generated standalone HTML preview for dark styling, SVG room and
corridor elements, and required content labels. It also checks the viewer Build
tab for the catalog piece placement grid, rendered cells, socket markers, and
glued-exit links, checks the Voxel tab for exposed isometric faces plus a
matching native authority receipt, and exercises the engine-owned Voxel 3D
mount, ceiling omission, public picking, candidate/config frame replacement,
rapid stale-work suppression, resize, controls, structural readouts, and
page-hide disposal. It
uses Chromium to write layout/intermediate/build/voxel/voxel-3d/standalone-preview
screenshots plus a report under:

```text
/tmp/rusty-procgen-viewer-smoke/
```

## Current Non-Goals

- No in-repo LLM harness.
- No custom agent service.
- No runtime-backed 3D authority; Voxel 3D is an engine-rendered, projection-only
  inspection of deterministic downstream build data.
- No large accepted-layout corpus yet.
- No doorway widths above one in placement-policy schema v1. Wider values fail
  closed until Rust routing validates the complete oriented opening footprint.
