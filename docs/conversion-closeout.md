# Conversion closeout

Rusty Procgen is a standalone downstream consumer of public Rusty Engine
mechanisms. The executable repository has no predecessor dependency, import,
adapter script, sibling-checkout requirement, or retired artifact decoder.
Historical identity evidence exists only in the migration records.

## Exact dependency boundary

`engine-source.json` pins the public Rusty Engine repository at
`db5641fc4e9d033112bc2b374a35933c3838e39c`. The Rust publication and spatial
workspaces, renderer packages, package-manager allowlist, and lockfiles must all
resolve that same revision. `pnpm run engine:revision` enforces the pin and
rejects local carriers.

The exact Rusty Procgen revision containing this record is captured in Den task
#6400 and its review packet. A source file cannot embed its own commit hash
without changing that hash.

## Reproducible evidence

Run from a fresh clone at the recorded revision:

```bash
pnpm install --frozen-lockfile
pnpm run verify
pnpm run viewer:smoke
```

The provider gate checks the terminal migration boundary, checked corpus,
Engine revision, TypeScript contracts, Rust core, exact-pinned Engine
publication and spatial hosts, strict Clippy, deterministic publication
receipts, spatial readback, and failure atomicity. The viewer gate builds the
product and exercises strict renderer boundaries plus real Chromium interaction.

The complete checked evidence corpus is regenerated only through its owning
commands. Catalog-aware coverage deliberately consumes
`fixtures/policies/catalog-aware-coverage-config.v1.json`, never the mutable
workbench configuration:

```bash
pnpm run baseline
pnpm run batch:sample
pnpm run geometry:recovery:report
pnpm run catalog:coverage
pnpm run catalog-aware:coverage
pnpm run catalog-trace:fixtures:check
pnpm run generation-control:report:check
pnpm run publish:rusty-engine-smoke
pnpm run voxel:rusty-engine-smoke
```

The legacy-named catalog-aware coverage fixture is a tracked input and is
explicitly migrated through the current policy boundary. Controlled-generation
trace fixtures and characterization instead use
`fixtures/policies/viewer-generation-default.v2.json`. Neither evidence owner
reads `config/viewer-generation.json`.

The post-conversion controlled-generation surface remains downstream:
Procgen Rust owns room compaction, final-placement constraints, deterministic
preference selection, and their semantic trace. Rusty Engine remains limited to
the exact public publication, spatial, and renderer mechanisms composed by the
named integrations.

## Scope

This closeout proves the in-place conversion and the named Rusty Engine
compositions. It does not claim that Procgen owns a universal generation
framework, gameplay runtime, generic content authority, native renderer, or
complete downstream game. Current nonclaims and deferred product work are
listed in [`known-limitations.md`](known-limitations.md).
