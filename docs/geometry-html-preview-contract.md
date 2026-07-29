# Geometry And HTML Preview Contract

Status: first contract for generated 2D dungeon floor-plan previews.

This contract starts the layer after `intermediate_breakdown`: concrete enough
to draw rooms and corridors, still separate from tile maps, game runtime state,
meshes, voxels, or renderer integration.

## Pipeline

```text
candidate + intermediate breakdown
  -> physical_connection_plan
  -> connection-aware room ports + exclusive geometry_2d corridors
  -> geometry validation
  -> html_preview metadata + standalone HTML/SVG file
  -> optional smoke screenshot evidence
```

## Geometry Artifact

Kind: `rusty_procgen.geometry_2d.v1`

Commands:

```bash
pnpm run procgen -- geometry plan-connections \
  --candidate <candidate.json> \
  --intermediate <intermediate-breakdown.json> \
  --out <physical-connection-plan.json>

pnpm run procgen -- geometry emit-2d \
  --candidate <candidate.json> \
  --intermediate <intermediate-breakdown.json> \
  --connection-plan <physical-connection-plan.json> \
  --layout-policy fixtures/geometry-layout-policies/compact-first-v1.json \
  --seed <u64> \
  --out <geometry.json>
```

The versioned physical plan is authoritative for physical corridor identity.
Compatible reciprocal open edges between the same regions normalize into one
`corridor_2` section while retaining both directed traversal refs. Incompatible
locked, hidden, one-way, or item-bearing edges remain separate sections.

Important fields:

- `geometryId`: stable generated id.
- `candidateId`: source candidate id.
- `seed`: deterministic layout seed.
- `sourceCandidateRef`: source candidate path.
- `sourceIntermediateRef`: source intermediate breakdown path.
- `sourceConnectionPlanRef` and `connectionPlanId`: exact physical plan input.
- `layoutPolicy`: versioned compact-first spacing and bounded-search policy.
- `layoutSearch`: successful spacing tier, room/route order attempts, cumulative
  route attempts, and effective outer/column/row spacing.
- `bounds`: total drawing bounds and grid size.
- `rooms`: variable room rectangles with role, footprint class, geometry role,
  source region, source node refs, and one boundary port per physical section.
- `corridors`: routed corridor polylines with width, traversal hint, source
  connectors/edges, exact traversal refs, physical section id, terminal port
  ids, and semantic tags.
- `contents`: lightweight room annotations for preview labels. Each annotation
  has an `id`, `roomId`, `sourceRef`, `kind`, `label`, and style/function tags.
  Current kinds include `start_marker`, `goal_marker`, `key_pickup`,
  `locked_gate`, `boss_threshold`, `reward_cache`, `hazard`,
  `resource_clue`, `shortcut_marker`, and `secret_route_marker`.
- `skippedConnectors`: explicit skipped connector records when the emitter
  cannot or should not draw a connector.

The geometry artifact is allowed to use coordinates and rectangles. It is not a
tile map and does not imply collision, runtime navigation, mesh, voxel, or final
renderer authority.

The emitter sizes and distributes rooms from their planned connection demands,
then routes each physical section through its declared ports. Routes reserve
separation space and retry deterministic room and route orders at the smallest
spacing tier first. It increases only outer margin and room gaps between tiers;
route grid, corridor separation, and port safety stay invariant. Exhaustion
reports the complete configured spacing range, route-attempt count, and last
blocked section instead of claiming proven non-embeddability. Emission still
fails closed rather than accepting corridor overlap, corridor contact, room
intrusion, or an expanded doorway.

## Geometry Validation

Kind: `rusty_procgen.validation.geometry_2d.v1`

Command:

```bash
pnpm run procgen -- geometry validate-2d \
  --state <geometry.json> \
  --out <geometry.validation.json>
```

The validator checks valid room bounds, non-overlapping room rectangles,
corridor room/port refs, exact physical-section ownership, connector/edge
coverage, unrelated corridor separation, room intrusion, directed
start-to-goal reachability, semantic refs for locked/hidden/shortcut corridors,
and content room anchors.

## HTML Preview Artifact

Kind: `rusty_procgen.html_preview.v1`

Command:

```bash
pnpm run procgen -- preview html \
  --geometry <geometry.json> \
  --validation <geometry.validation.json> \
  --out <preview.html>
```

By default the command refuses invalid geometry. Pass `--allow-invalid` to render
a diagnostic preview for debugging.

This metadata records the relationship between:

- `geometryRef`;
- `validationRef`;
- `htmlRef`;
- optional `screenshotHint`.

The HTML file itself should be standalone: dark CSS, inline SVG, labels,
legend/metadata, and no dev server requirement.

## Compatibility

Existing `rusty_procgen.layout_2d.v1` remains the simple graph embedding used by
the preview site. It is not replaced by `geometry_2d`.

`rusty_procgen.intermediate_breakdown.v1` remains the semantic pre-geometry
handoff. `geometry_2d` consumes it and may fail validation if the geometry
artifact loses required source refs, connectors, or content anchors.

## Non-Goals

- No tile-perfect roguelike map.
- No final renderer or gameplay runtime integration.
- No mesh generation.
- No voxel output.
- No hand-authored art assets.
