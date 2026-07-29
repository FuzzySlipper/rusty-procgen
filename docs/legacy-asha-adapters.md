# Temporary legacy Asha adapter

Only retained inspection and viewer hosting still use predecessor packages
during the in-place conversion. This is not the target Rusty architecture and
does not create a compatibility promise. The exact package, transitive-package,
import, and script allowances are machine-owned by
[`../migration/asha-disposition.json`](../migration/asha-disposition.json).
Den #6399 replaces the renderer lane; #6400 proves clean closeout.

Generated-content publication and voxel authority already use exact-pinned
public Rusty Engine Rust owners. Their contracts and evidence are documented in
[`rusty-engine-publication.md`](rusty-engine-publication.md) and
[`rusty-engine-spatial.md`](rusty-engine-spatial.md).

## Retained inspection lane

`src/voxel-inspection-projection.ts`,
`scripts/legacy-asha-voxel-inspection-frame-smoke.mjs`, and `viewer/app.ts`
remain a bounded presentation-only consumer of the predecessor renderer host.
They do not own placement, accepted voxels, collision, navigation, mesh truth,
or publication. The TypeScript extrusion projection remains an explicitly
unverified convenience for temporary viewer experiments; committed spatial
evidence comes from the Rust host.

The remaining Asha dependencies are sibling-checkout conversion conveniences,
not a distribution contract. The migration boundary check fails closed on any
new package, import, adapter script, or predecessor artifact identity.

## Crate and code disposition

No Procgen crate moves upstream wholesale:

- `rusty-procgen-preflight` remains downstream generation tooling;
- graph grammar, scoring, repair, embedding, shape matching, placement, and
  viewer orchestration remain Procgen policy; and
- `rusty-procgen-engine-spatial` remains the downstream composition that maps
  dungeon cells and doorway provenance into generic Engine spatial authority.

Only a concrete missing reusable mechanism proven at an Engine border should
become an upstream Rusty Engine task.
