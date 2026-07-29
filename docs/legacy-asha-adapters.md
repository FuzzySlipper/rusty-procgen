# Temporary legacy Asha adapters

This document describes the two bounded predecessor lanes that remain
executable during the in-place conversion. It is not the target Rusty
architecture and does not create a compatibility promise. The exact
package/import/script allowance is machine-owned by
[`../migration/asha-disposition.json`](../migration/asha-disposition.json).
Den #6398 and #6399 replace those lanes; #6400 removes the residual coupling.
Generated-content publication already uses Rusty Engine public authored owners;
its contract and proof are in
[`rusty-engine-publication.md`](rusty-engine-publication.md).

## Consumer role and distribution

Sibling-checkout development currently declares three direct Asha packages with
`file:` dependencies from `../asha-engine`; their linked metadata identifies
the exact remaining transitive closure. That is a temporary conversion
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
