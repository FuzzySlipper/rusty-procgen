# Rusty Procgen agent guidance

## Project role

Rusty Procgen is a downstream game-tooling product built around deterministic
dungeon generation. It owns dungeon vocabulary, intent grammar, graph
construction, scoring, repair, geometry, catalog matching, placement policy,
validation, generated artifacts, and its inspection workbench.

Rusty Engine owns reusable host-neutral mechanisms. Procgen may compose its
public crates and packages, but it does not own generic scene, asset, voxel,
spatial, mesh, render, component, or service authority. Missing generic
mechanisms should be reproduced for the `rusty-engine` project instead of being
copied or reimplemented here.

> Rusty Procgen owns dungeon meaning and orchestration. Rusty Engine owns
> reusable mechanisms exposed through direct named services.

## Source of truth

Use this order when sources disagree:

1. The user's current scope and any supplied Den task.
2. Current code, tests, fixtures, configuration, and generated evidence.
3. This file, the README, and the owning contract under `docs/`.
4. The machine-readable migration ledger in
   `migration/predecessor-disposition.json`.
5. Historical notes and predecessor documentation.

The repository is self-contained for agents without Den access. Den owns
current task and review state, not the committed product contract.

## Repository structure

```text
procgen-rs/     deterministic generator and validation implementation
integrations/   exact-pinned downstream Rusty Engine compositions
src/            TypeScript publication and projection adapters
viewer/         local inspection workbench
fixtures/       authored inputs, policies, catalogs, and invalid cases
artifacts/      checked deterministic samples and evidence
config/         local workbench generation configuration
migration/      predecessor dispositions and identity-equivalence proof
scripts/        generators, checks, reports, smokes, and local host
docs/           artifact, algorithm, workflow, and boundary contracts
```

The supported Rust host surface is
`rusty_procgen_preflight::core::ProcgenCore`. Read
[`docs/rust-library-api.md`](docs/rust-library-api.md) before changing domain
artifacts, deterministic behavior owners, CLI adapters, or their test routing.
The CLI must remain a filesystem/receipt consumer of the in-memory library
rather than a second implementation.

The terminal conversion proof and current product nonclaims live in
[`docs/conversion-closeout.md`](docs/conversion-closeout.md) and
[`docs/known-limitations.md`](docs/known-limitations.md).

The procedural CA workload API is owned by
`procgen-rs/crates/preflight/src/cellular_automata.rs`; its boundary and fixture
workflow are documented in
[`docs/cellular-automata-workloads.md`](docs/cellular-automata-workloads.md).
The direct exact-pinned Engine authority composition is isolated in
`integrations/rusty-engine-ca-benchmark/` and documented in
[`docs/rusty-engine-ca-benchmark.md`](docs/rusty-engine-ca-benchmark.md).

Catalog-aware semantic tracing and replay are owned by
`procgen-rs/crates/preflight/src/parts/catalog_generation_trace.rs` and
documented in
[`docs/catalog-generation-traces.md`](docs/catalog-generation-traces.md).
Strict TypeScript admission is owned by `src/catalog-generation-trace.ts`; the
observational SVG lifecycle and controls are owned by
`viewer/generation-trace-viewer.ts`. Neither may acquire search, selection, or
pathfinding authority.

`config/viewer-generation.json` may contain intentional local user
experiments. Preserve unrelated value changes and never stage them as
collateral.

## Architecture boundaries

- Keep generation deterministic from explicit inputs, policies, catalogs, and
  seeds.
- Make bounded search, rejection reasons, provenance, and validation evidence
  explicit.
- Keep dungeon semantics downstream. Do not promote a universal Procgen
  runtime, gameplay AST, scheduler, behavior graph, or ambient event bus.
- Call Rusty Engine public named services directly. Do not recreate universal
  runtime facades, command tunnels, hidden service location, or duplicate
  spatial/render authority.
- TypeScript may author, translate, project, and orchestrate product behavior;
  it must not silently become a second copy of reusable Engine authority.
- Renderer output is observational. It does not own placement, voxel, door,
  navigation, or gameplay truth.
- Durable artifacts use only the current `rusty_procgen.*` kinds. Predecessor
  artifact kinds are not accepted or decoded.

## Predecessor boundary

The retired donor is historical evidence, not a compatibility target. The
executable repository has no predecessor packages, imports, adapter scripts, or
sibling-checkout requirements.

`migration/predecessor-disposition.json` retains the exact historical
disposition.
`pnpm run check:migration-boundary` fails on any executable predecessor
dependency, import, adapter script, retired identity, or artifact kind. Route
any missing reusable mechanism to its Rusty Engine owner.

## Local commands

Run from the repository root.

```bash
pnpm install --frozen-lockfile
pnpm run verify
pnpm run check:migration-boundary
pnpm run check:corpus-identity
pnpm run typecheck
pnpm run rust:check
pnpm run rust:test
pnpm run engine:publication:test
pnpm run publish:rusty-engine-smoke
pnpm run engine:spatial:test
pnpm run voxel:rusty-engine-smoke
pnpm run engine:ca:test
pnpm run engine:ca:benchmark
```

Focused workflows:

```bash
pnpm run baseline
pnpm run batch:sample
pnpm run piece:smoke
pnpm run policy:smoke
pnpm run viewer:smoke
pnpm run catalog:coverage
pnpm run catalog-aware:coverage
pnpm run catalog-trace:fixtures:check
pnpm run catalog-trace:smoke
pnpm run catalog-trace:viewer:smoke
```

The publication, spatial, and CA benchmark adapters are isolated Rust workspaces under
`integrations/`. They select one exact public Engine commit through
`engine-source.json`. The root package manager resolves the renderer contract,
projection, host, and Three backend packages from that same exact public
revision. Use `./scripts/engine-revision check` before changing any public
Engine dependency.

## Change and verification posture

- Run the narrowest relevant check first, then `pnpm run verify`.
- Regenerate checked artifacts through their owning commands.
- Artifact/schema changes require deterministic regeneration, namespace checks,
  and corpus diff review.
- Algorithm changes require focused accepted/rejected topology and placement
  regressions; do not hide them in migration churn.
- Spatial authority claims require a real Rust host. Viewer claims require a
  real browser. Synthetic fixtures prove mechanisms, not full product behavior.
- Keep failure paths fail-closed and preserve structured diagnostics.
- Report exactly which commands ran and which live checks were skipped.

## Shared workspace and git

- Treat a dirty worktree as normal and preserve unrelated changes.
- Do not reset, restore, clean, delete, or reformat another agent's work.
- Inspect the task-scoped diff before staging.
- Commit and push completed task work directly to the current appropriate ref.
- Record exact SHAs and verification evidence in Den when work is Den-managed.

## Review checklist

- [ ] Dungeon meaning remains local and reusable authority remains upstream.
- [ ] No new predecessor package, import, wrapper, or old artifact kind exists.
- [ ] Deterministic artifacts were regenerated by owning commands.
- [ ] Corpus behavior changes, if any, are intentional and separately proved.
- [ ] Local configuration experiments were preserved.
- [ ] Focused checks and the owning gate passed.
