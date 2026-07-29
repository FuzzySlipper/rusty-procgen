# Rusty Engine spatial composition

Rusty Procgen owns dungeon placement meaning: placement-cell coordinates,
piece ownership, glued-exit routes, enclosure dimensions, material assignment,
portal provenance, and when a generated result is admitted. Rusty Engine owns
the canonical material voxels and the collision, navigation, and mesh
projections derived from them.

The isolated downstream Rust workspace at
`integrations/rusty-engine-spatial/` depends on
`rusty-procgen-preflight` plus the public `engine-spatial` crate at the exact
commit in `engine-source.json`. The normal Procgen core remains Engine
independent. No Asha package, Node runtime, native addon, browser, renderer,
sibling Engine checkout, RuntimeSession, or general command facade participates
in this proof.

## Composition

`compile_placement_extrusion` validates the public `PiecePlacement`, preserves
the established `x/y` to voxel `x/z` mapping, verifies cell ownership and
glued-exit opening routes, builds the floor/wall/ceiling enclosure, retains
door source-edge and item provenance, and bounds the concrete voxel proposal by
Engine's public authority limit.

`SpatialExtrusionHost` then:

1. checks the exact observed `VoxelSourceRevision`;
2. rebuilds the observed Engine authority off to the side;
3. applies coordinate-ordered changes through `VoxelEditService` transactions
   capped at `MAX_VOXEL_EDITS_PER_TRANSACTION`;
4. lets Engine rebuild collision, navigation, and mesh projections after every
   batch; and
5. replaces the live scene only after all transactions succeed.

No partial batch is published. Rejection leaves authority bytes, revision,
material counts, and all projection hashes unchanged. Concrete material voxels
plus the accepted revision can reopen through
`VoxelCollisionScene::from_material_voxels_at_revision`; the downstream game or
tool retains storage location and persistence policy.

## Proof

Run:

```bash
./scripts/engine-revision check
pnpm run engine:spatial:test
pnpm run engine:spatial:clippy
pnpm run voxel:rusty-engine-smoke
```

The smoke regenerates
`artifacts/evidence/engine-spatial-extrusion.json`. Tests cover the complete
checked accepted-placement corpus, exact representative parity with the former
extrusion policy, deterministic repetition, coherent collision/navigation/mesh
readout, exact reopen, bounded multi-transaction application, and fail-atomic
malformed, unknown-material, oversized, stale, and late Engine failures.

This slice does not claim renderer behavior, browser interaction, gameplay,
navigation quality, persistence policy, or performance. The TypeScript viewer
may still preview temporary policy experiments, but those previews are
explicitly non-authoritative; Den #6399 owns the remaining renderer migration.
