# Asha donor boundary

The in-place conversion retains no executable Asha package, transitive package,
import, adapter script, sibling checkout, or predecessor artifact decoder. The
historical disposition is machine-owned by
[`../migration/asha-disposition.json`](../migration/asha-disposition.json).
The migration boundary gate requires its active arrays to remain empty.

Generated-content publication and voxel authority use exact-pinned public
Rusty Engine Rust owners. Their contracts and evidence are documented in
[`rusty-engine-publication.md`](rusty-engine-publication.md) and
[`rusty-engine-spatial.md`](rusty-engine-spatial.md).

## Inspection lane

`src/voxel-inspection-projection.ts`,
`scripts/smoke-rusty-renderer-inspection.mjs`, and `viewer/app.ts` are a bounded
presentation-only consumer of exact-revision public
`@rusty-engine/render-contracts` and `@rusty-engine/renderer-host`. Their public
peer closure is pinned to the same Engine commit in `engine-source.json`.
Procgen imports neither Three nor a backend implementation directly.

The lane does not own placement, accepted voxels, collision, navigation, mesh
truth, or publication. The TypeScript extrusion projection is an explicitly
unverified convenience for viewer experiments; committed spatial evidence
comes from the Rust host. Strict frame decoding and the Engine host reject
malformed input without replacing retained content. Real-Chromium evidence
covers controls, candidate/config replacement, stale-work suppression, resize,
picking, structural readouts, and disposal.

## Crate and code disposition

No Procgen crate moves upstream wholesale:

- `rusty-procgen-preflight` remains downstream generation tooling;
- graph grammar, scoring, repair, embedding, shape matching, placement, and
  viewer orchestration remain Procgen policy; and
- `rusty-procgen-engine-spatial` remains the downstream composition that maps
  dungeon cells and doorway provenance into generic Engine spatial authority.

Only a concrete missing reusable mechanism proven at an Engine border should
become an upstream Rusty Engine task.
