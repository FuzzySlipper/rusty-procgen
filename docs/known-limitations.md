# Known limitations

Rusty Procgen is a deterministic dungeon-generation and inspection product,
not a complete game runtime.

Prefab scene sockets are symbolic and explicitly authored. Procgen places and
validates symbolic props and bounded point lights, but it does not resolve
content ids, load meshes, create renderer resources, assess lighting density,
or add automatic fill lights. Those remain downstream product/presentation
concerns and future work.

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
- Catalog-aware outcome control has four explicit hard metrics and one bounded
  preference. It is not an arbitrary constraint language or general optimizer.
  Room compaction moves selected room origins inward and may make later attempts
  infeasible; neither compaction nor best-admissible selection proves a globally
  minimal layout.
- Generation traces retain semantic decisions, not pathfinder frontier states.
  They support exact bounded replay and diagnosis but are not pausable generator
  sessions, performance profiles, or evidence that a preference is attainable
  for every seed.
- The Tight, Normal, and Spread Build profiles are complete corpus-backed
  configurations, but their names express search intent rather than guaranteed
  monotonic dimensions. Large merge topologies can converge on similar spans,
  and the current complex `5501` hub/hazard/boss layout rejects under all three
  profiles instead of bypassing bounded catalog validation.
- Downstream games remain responsible for full gameplay execution, persistence,
  encounter meaning, and presentation policy.
- Cellular-automata workloads are a separate post-conversion campaign. The
  current proof covers deterministic bounded deltas plus exact-pinned Engine
  spatial execution, coherent collision/navigation/mesh readback, public mesh
  authority traces, retained-trace browser playback, and one bounded same-host
  scale matrix. It does not prove performance thresholds, single-factor
  causality, memory/GPU behavior, visual smoothness, or gameplay/simulation
  semantics.

These limits should narrow claims, not create compatibility shims or duplicate
Rusty Engine authority.
