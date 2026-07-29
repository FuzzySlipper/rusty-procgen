# Temporary legacy Asha adapters

This document describes bounded predecessor evidence that remains executable
during the in-place conversion. It is not the target Rusty architecture and
does not create a compatibility promise. The exact package/import/script
allowance is machine-owned by
[`../migration/asha-disposition.json`](../migration/asha-disposition.json).
Den #6397, #6398, and #6399 replace the three lanes; #6400 removes the residual
coupling.

## Ownership

| Surface | Owner | Procgen responsibility |
| --- | --- | --- |
| Shape catalog and placement | `rusty-procgen` | Footprints, reserved cells, exits, sockets, matching constraints, transforms, seeded selection, and placement provenance. |
| Prefab registry | ASHA ProjectBundle contracts | Stable `PrefabId`, part identity, stable part roles, source asset references, variants, and overrides. Procgen supplies an explicit mapping into these generated types. |
| Scene and ProjectBundle inventory | ASHA ProjectBundle contracts | Procgen emits a typed manifest plus a durable scene-side prefab-instance artifact and source-asset references. Rust ProjectBundle load remains the acceptance authority. |
| Voxel geometry and accepted mutation | ASHA voxel/runtime authority | Procgen may reference voxel-object assets. The separate direct-command extrusion is only a bounded native authority smoke lane. |

The adapter in `src/prefab-publishing.ts` consumes the local shape catalog,
shape-match artifact, piece placement, and an explicit mapping fixture. It uses
generated `@asha/contracts` types and validates the constructed registry with
the public `@asha/game-workspace` source validator. It does not copy the ASHA
prefab schema.

## Reproducible proof

Run:

```bash
npm run publish:legacy-asha-smoke
```

The proof reads the representative mapping at
`fixtures/prefab-mappings/first-slice.json` and regenerates
`artifacts/evidence/prefab-project-bundle-publication.json`. It publishes two
placed pieces as stable prefab definitions and instance records, contributes a
generated `FlatSceneDocument` projection, preserves the
candidate -> shape match -> placement -> published-artifact chain, and records
the prefab registry, scene, asset lock, and voxel-object source artifacts in a
generated `ProjectBundleManifest`.

The smoke also proves fail-closed handling for:

- a selected shape without a prefab mapping;
- a missing or malformed stable part role;
- a source whose asset kind is incompatible with the ASHA prefab part;
- duplicate or missing stable prefab instance identities;
- invalid or unsupported placement transforms.

The output is authoring evidence. It does not claim live prefab instantiation,
Rust ProjectBundle load acceptance, rendering, navigation, or collision.

## Consumer role and distribution

Sibling-checkout development currently declares four direct Asha packages with
`file:` dependencies from `../asha-engine`; their linked metadata identifies
four additional transitive packages. That is a temporary conversion
convenience, not a distribution contract. The repository-local ledger lists
each direct dependency, transitive package, and importing file exactly; the
boundary check fails closed on additions or drift. `@asha/runtime-session` was
unused directly and remains only in the temporary runtime-bridge closure.

## Voxel command lane

`src/voxel-extrusion.ts` and `npm run voxel:legacy-asha-smoke` remain a focused proof
that bounded `VoxelCommand` batches are accepted by native Rust authority,
repeat deterministically, reject an unknown material with the exact generated
tagged DTO, and do not mutate state on rejection. They are not the canonical
generated-level publication format. The maintained 2D-extrusion and
non-mesh-fidelity limits remain documented in Den's `known-limitations` entry.

## Crate and code disposition

No current Procgen crate should move upstream wholesale:

- `rusty-procgen-preflight` remains downstream authoring/generation tooling;
- graph grammar, scoring, repair, embedding, shape matching, placement, and
  HTML/viewer generation remain Procgen policy;
- `src/voxel-extrusion.ts` remains a dungeon-specific realization adapter.

Only a concrete missing reusable mechanism proven at this publishing border
should become an upstream Rusty Engine task. Determinism or potential reuse
alone is not a reason to promote a local algorithm into engine authority.
