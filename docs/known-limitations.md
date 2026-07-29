# Known limitations

Rusty Procgen is a deterministic dungeon-generation and inspection product,
not a complete game runtime.

- The checked corpus and interactive workbench currently emphasize bounded
  single-floor 2D dungeon topology, catalog placement, and simple voxel
  extrusion.
- The voxel adapter proves exact-pinned Rusty Engine spatial admission,
  collision/navigation/mesh readback, atomic replacement, and persistence. It
  does not certify gameplay movement, AI, streaming, destructibility, or
  product save policy.
- The renderer lane is inspection-only. It proves strict frame admission,
  replacement, controls, picking, resize, stale-work suppression, and disposal
  in Chromium; it does not own dungeon or gameplay truth and is not a
  performance certification.
- Downstream games remain responsible for full gameplay execution, persistence,
  encounter meaning, and presentation policy.
- Cellular-automata generation is a later feature campaign. It is not a missing
  conversion requirement and no current artifact or API promises it.

These limits should narrow claims, not create compatibility shims or duplicate
Rusty Engine authority.
