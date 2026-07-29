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
   `migration/asha-disposition.json`.
5. Historical notes and predecessor documentation.

The repository is self-contained for agents without Den access. Den owns
current task and review state, not the committed product contract.

## Repository structure

```text
procgen-rs/     deterministic generator and validation implementation
src/            TypeScript publication and projection adapters
viewer/         local inspection workbench
fixtures/       authored inputs, policies, catalogs, and invalid cases
artifacts/      checked deterministic samples and evidence
config/         local workbench generation configuration
migration/      predecessor dispositions and identity-equivalence proof
scripts/        generators, checks, reports, smokes, and local host
docs/           artifact, algorithm, workflow, and boundary contracts
```

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
- Call Rusty Engine public named services directly. Do not recreate Asha-style
  RuntimeSession, command tunnels, hidden service location, or duplicate
  spatial/render authority.
- TypeScript may author, translate, project, and orchestrate product behavior;
  it must not silently become a second copy of reusable Engine authority.
- Renderer output is observational. It does not own placement, voxel, door,
  navigation, or gameplay truth.
- Durable artifacts use only the current `rusty_procgen.*` kinds. Predecessor
  artifact kinds are not accepted or decoded.

## Temporary predecessor adapters

Asha is historical donor evidence, not a compatibility target. The in-place
conversion temporarily retains a small number of explicit Asha adapters so
each working lane stays executable until its Rusty replacement lands.

`migration/asha-disposition.json` is the exact allowlist for every remaining
package, import, and integration script. `npm run check:migration-boundary`
fails on additions, missing ledger entries, hidden adapter names, or retired
artifact kinds. Do not add another Asha dependency or wrapper; route the need
to the owning conversion task.

The remaining lanes are:

- generated-content publication — Den #6397;
- voxel authority — Den #6398;
- retained inspection and viewer hosting — Den #6399;
- removal and clean closeout — Den #6400.

## Local commands

Run from the repository root.

```bash
npm install
npm run verify
npm run check:migration-boundary
npm run check:corpus-identity
npm run typecheck
npm run rust:check
npm run rust:test
```

Focused workflows:

```bash
npm run baseline
npm run batch:sample
npm run piece:smoke
npm run policy:smoke
npm run viewer:smoke
npm run catalog:coverage
npm run catalog-aware:coverage
```

Commands containing `legacy-asha` are temporary conversion evidence, not
approved architecture. Their exact removal tasks are recorded in the ledger.

## Change and verification posture

- Run the narrowest relevant check first, then `npm run verify`.
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
