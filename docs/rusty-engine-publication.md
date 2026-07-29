# Rusty Engine generated-content publication

Rusty Procgen owns dungeon selection, authored mappings, provenance, output
layout, and publication timing. Reusable asset, prefab, scene, entity, and
atomic content validation belong to Rusty Engine.

The adapter is an isolated Rust workspace at
`integrations/rusty-engine-publication/`. The normal
`rusty-procgen-preflight` crate has no Engine dependency. The adapter selects
one exact public Engine commit through `engine-source.json`; no sibling Engine
checkout or Asha package participates.

## Mapping

The representative fixture combines:

- a local shape catalog;
- a deterministic shape-match report and piece placement;
- explicit stable prefab, part, role, node, and instance identities;
- pinned source-asset bodies; and
- the complete candidate, plan, match, and placement provenance chain.

The Rust host then calls the public owners directly:

1. `asset-catalog` validates the referenced source assets and generates a
   canonical lock;
2. `content-store` validates the prefab registry and builds a two-sided,
   compare-and-swap publication candidate;
3. `authored-scene` validates prefab instance nodes and prepares admission;
4. `entity-state` admits the scene entities at an exact state revision; and
5. `content-store` strictly reopens the complete manifest/body batch before
   evidence is published.

The scene uses `SceneNodeKind::EntityInstance` with
`SceneEntityReference::Prefab`. Procgen does not copy Engine DTOs or recreate
generic owner validation in TypeScript.

## Proof

Run:

```bash
./scripts/engine-revision check
npm run engine:publication:test
npm run publish:rusty-engine-smoke
```

The smoke regenerates
`artifacts/evidence/engine-authored-publication.json`. Focused tests cover
deterministic canonical readback, stale content-store authorization, missing
mapping and role, incompatible asset kind, duplicate identity, invalid
transform, stale source pin, selection quota, late owner validation, and
atomic output preservation.

The source bodies in this slice are opaque downstream resources used to prove
catalog and prefab references. This task does not claim voxel realization,
rendering, navigation, or collision. Den #6398 owns the real spatial/voxel
composition and Den #6399 owns retained browser inspection.

## Updating Engine

All selected Engine crates move together:

```bash
./scripts/engine-revision update <40-character-public-sha> --dry-run
./scripts/engine-revision update <40-character-public-sha>
```

The updater proves the exact public commit, works in a disposable checkout,
regenerates both isolated Cargo locks, runs the revision checker and focused
compiles, and copies only `engine-source.json` plus the two manifests and locks
after rechecking the caller. It never edits historical evidence or unrelated
local configuration.
