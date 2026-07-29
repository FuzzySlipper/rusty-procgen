# Intermediate Layout Contract

Status: executable contract for graph analysis and pre-geometry layout intent.

This layer sits between topology graphs and any future 2D, 3D, mesh, or voxel
embedding. It gives agents more structure to reason about without baking in
room shapes, coordinates, verticality, tile grids, mesh chunks, or destructible
voxel rules.

## Command Sequence

Analyze the graph:

```bash
npm run procgen -- analyze graph \
  --state <candidate.json> \
  --out <analysis.json>
```

Inspect rule compatibility:

```bash
npm run procgen -- graph compatible-rules \
  --state <candidate.json> \
  --out <compatible-rules.json>
```

Apply a bounded repair action:

```bash
npm run procgen -- repair apply \
  --state <candidate.json> \
  --action add_rejoin_edge \
  --target <node_id> \
  --seed <u64> \
  --out <candidate.json> \
  --receipt <receipt.json>
```

Current repair actions:

- `add_rejoin_edge`: adds an open repair/rejoin edge from a simple terminal
  branch node to `goal`.
- `remove_orphan_node`: removes a non-start/non-goal node with no incoming
  route.

Annotate spatial intent:

```bash
npm run procgen -- annotate spatial-intent \
  --state <candidate.json> \
  --analysis <analysis.json> \
  --out <spatial-intent.json>
```

Emit an intermediate breakdown:

```bash
npm run procgen -- breakdown emit \
  --state <candidate.json> \
  --annotations <spatial-intent.json> \
  --out <intermediate-breakdown.json>
```

Validate the breakdown:

```bash
npm run procgen -- breakdown validate \
  --state <intermediate-breakdown.json> \
  --out <intermediate.validation.json>
```

## Artifact Kinds

- `rusty_procgen.graph_analysis.v1`: critical path, dominators, optional branch
  classifications, lock/key order checks, loop signals, and shortcut risk
  hints.
- `rusty_procgen.rule_compatibility.v1`: per-rule `applicable`, `blocked`,
  `duplicate`, or `risky` status with reasons and recommended actions.
- `rusty_procgen.spatial_intent.v1`: node/edge annotations such as
  `landmark_hub`, `visible_before_reachable`, `pressure_path`,
  `shortcut_connector`, `one_way_drop`, and `hidden_route`.
- `rusty_procgen.intermediate_breakdown.v1`: graph-derived regions,
  connectors, constraints, and geometry-prep hints for a later geometry pass.
- `rusty_procgen.validation.intermediate.v1`: fatal diagnostics for invalid
  intermediate breakdowns.

Stable intermediate validation diagnostic codes:

- `intermediate_start_missing`
- `intermediate_goal_missing`
- `intermediate_region_geometry_prep_missing`
- `intermediate_region_scale_invalid`
- `intermediate_anchor_missing`
- `intermediate_landmark_geometry_role_missing`
- `intermediate_connector_unbound`
- `intermediate_connector_endpoint_missing`
- `intermediate_connector_affordance_missing`
- `intermediate_connector_traversal_hint_missing`
- `intermediate_gated_constraint_missing`
- `intermediate_hidden_affordance_missing`
- `intermediate_shortcut_affordance_missing`
- `intermediate_one_way_affordance_missing`
- `intermediate_vertical_candidate_unsupported`
- `intermediate_3d_claim_unsupported`

## Intermediate Schema Versioning

The intermediate breakdown keeps kind `rusty_procgen.intermediate_breakdown.v1`
while the pre-geometry handoff is still evolving additively. The enriched
geometry-prep shape uses `schemaVersion: 2`.

Version 2 adds abstract planning hints only:

- region `geometryRole`;
- region `footprintClass`;
- region `scaleBand`;
- region `anchorQuality`;
- region `entranceExpectations`;
- connector `affordances`;
- connector `traversalHint`;
- connector `constraintRefs`;
- constraint `targetType`;
- constraint `sourceIntents`;
- constraint `graphRefs`.

These fields are intentionally not coordinates, room bounds, tile footprints,
mesh handles, prefab ids, voxel volumes, or 3D transforms. Older v1 breakdowns
should still deserialize with empty/default values for the added fields.

## Intentional Non-Geometry Boundary

The intermediate breakdown is not a room plan. It is not a tile map. It is not
a 3D prefab graph. It is a structured handoff record that names roles and
constraints so later passes can choose a geometric realization.

This matters because the project will eventually explore both 2D-style topology
generation and properly 3D dungeon structures. This contract preserves the
shared intent vocabulary while leaving vertical connectors, mesh placement,
voxel volumes, and destructive-layout rules to future schemas.

The validator currently rejects `vertical_candidate` connector intent because
that requires a geometry-aware schema. This is deliberate: agents can talk about
future verticality in research docs, but accepted artifacts should not imply
3D support until the downstream pass can validate it.

## Batch Selection Fields

Batch accepted entries now include:

- `topologyFingerprint`: deterministic shape fingerprint that ignores specific
  candidate ids and seed-derived names.
- `duplicateOf`: first accepted candidate id with the same topology
  fingerprint, when present.
- `budgetChecks`: pass/fail checks from the batch profile budget stanza.
- `budgetPenalty`: deterministic penalty for failed budget checks.
- `selectionScore`: `overall - budgetPenalty`, used for sorting accepted
  entries.

Sample profile budgets live in:

```text
fixtures/batch-profiles/v2-sample.json
```
