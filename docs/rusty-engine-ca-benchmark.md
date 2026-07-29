# Rusty Engine CA benchmark

The isolated workspace at `integrations/rusty-engine-ca-benchmark/` composes
Rusty Procgen's deterministic cellular-automata deltas with the public,
exact-pinned `engine-spatial` authority. It is a downstream benchmark and trace
producer, not a second voxel store or a generic Engine runtime.

## Ownership and transaction

Rusty Procgen owns scenario selection, CA state evolution, delta translation,
benchmark orchestration, timing policy, and checked trace artifacts. Rusty
Engine owns canonical material voxels and the collision, navigation, and mesh
projections derived from them.

One step follows an explicit prepare/commit boundary:

1. Clone and evolve the CA state off-side.
2. Translate its ordered deltas into `VoxelEdit` values.
3. Reject the complete request if it exceeds the configured or Engine public
   edit quota, or its conservative affected-chunk mesh budget.
4. Ask `VoxelEditService::preview` to build a complete candidate Engine
   authority and all projections without mutation.
5. Commit the guarded Engine candidate, then publish the corresponding CA
   state and trace step.

Dropping a preparation publishes neither state. Stale, malformed, duplicate,
oversized, projection-build, and superseded-preparation failures preserve the
original CA and Engine revisions, hashes, counts, and projections.

The large-resident/small-hot-region workload intentionally materializes its
entire 64×16×64 domain with an explicit resident-empty material. Its initial
65,536 voxels and chunks are therefore real Engine authority, while later
requests contain only changed CA cells. Other workloads omit empty cells.

## Trace and bounded readback

`rusty_procgen.engine_ca_authority_trace.v1` records:

- the complete authored scenario, rule, seed, bounds, boundary, and material
  posture;
- initial Engine authority and full public mesh chunks;
- every CA delta, Engine revision/fact, coherent collision/navigation/mesh
  readout, changed public mesh chunk upsert/delete, and cumulative trace hash;
- complete public vertex/index/group buffers for changed chunks so a later
  renderer can consume actual Engine projection facts.

Initial admission captures the complete mesh once. Each step derives the
affected chunk set from changed voxel coordinates, uses binary lookup over the
Engine's canonical public chunk slice, and records only those changed chunks.
Compact chunk summaries, counts, authority hashes, and projection hashes avoid
rescanning material voxels or copying unchanged mesh buffers.

The default limits are one Engine transaction (4,096 edits), two million mesh
scalar values for initial or prospectively affected chunks, eight recorded
runs, and the scenario quotas owned by the filesystem-free CA API.

## Timing and correctness

`rusty_procgen.evidence.engine_ca_benchmark.v1` separates:

- initial state materialization, Engine build, evidence readback, and encoding;
- CA evolution, edit construction, Engine preview, authority commit, evidence
  readback, and encoding per step.

The clock is injected. Tests use scripted ticks; checked baseline generation
uses monotonic `std::time::Instant` and records OS, architecture, Rust version,
build profile, repository code SHA, Engine SHA, warmups, repeats, and limits.
Recorded timing is observational and never gates correctness. Structural
hashes, exact repeated traces, bounded work, coherent projections, and
fail-atomic behavior do.

## Commands

```bash
pnpm run engine:ca:test
pnpm run engine:ca:clippy
pnpm run engine:ca:benchmark
```

The benchmark owner refuses uncommitted source changes, while allowing the
user-owned workbench configuration and its own output artifact. Commit source
first, run the release benchmark, then commit the generated
`artifacts/evidence/engine-ca-benchmark.json`. The embedded repository SHA is
the exact code revision exercised, not the later evidence-only commit.

This proof does not certify browser playback, renderer performance, gameplay
semantics, scheduling, persistence, or a maximum Engine scale.
