# Cellular automata workloads

`cellular_automata` is a filesystem-free Rust module for deterministic,
bounded voxel workload generation. Its local state is procedural input for
later stress hosts; it is not Rusty Engine voxel authority and it has no
renderer, timing, gameplay, or storage policy.

## Contract

Each `CaScenario` declares:

- an explicit integer `min` / `maxExclusive` domain;
- a versioned rule identity, neighborhood, and fixed-empty or wrapping boundary;
- a workload class, seed, bounded step count, and ordered initial cells.

`CaAutomaton` validates the complete scenario before allocating working state.
Sparse frontier/trail rules evaluate only the prior changed region and its
neighbors. Dense parity churn intentionally evaluates the complete bounded
domain. Both publish lexicographically ordered cell deltas with previous/current
state, touched bounds, state counts, evaluated/active/changed counts, a delta
hash, a state hash, and a cumulative scenario hash.

Admission also checks the aggregate `volume * steps` cell-step capacity before
any trace is materialized. A scenario may declare at most 1,048,576 cell-steps,
and a complete suite at most 2,097,152. This deliberately conservative measure
bounds both full-domain evaluation work and the maximum retained delta count
without depending on rule-specific behavior. Products and checked sums reject
overflow, and a rejected suite cannot reach the fixture publication step.

The checked suite at `fixtures/ca/scenarios.v1.json` covers sparse propagation,
dense churn, wrapped cross-boundary activity, a large resident domain with a
small hot region, and high-surface-area churn. Its exact generated traces live
at `fixtures/ca/delta-traces.v1.json`.

Regenerate and validate them with:

```bash
pnpm run ca:fixtures
pnpm run ca:fixtures:check
cargo test --manifest-path procgen-rs/Cargo.toml --test cellular_automata --locked
```

## Adding a rule

Add a new explicit `CaRule` variant and stable `id()`, implement its transition
in `CaAutomaton::evaluate`, and decide whether it is sparse-frontier-safe or
must scan the complete domain. Add an authored scenario, regenerate the checked
trace set, and prove the optimized candidates against a straightforward
full-domain oracle when claiming sparse evaluation.

Do not introduce a callback registry, generic scheduler, ambient randomness,
unbounded growth, Engine mutation, renderer dependency, or timing into this
module. Later hosts translate its accepted ordered deltas into their own named
authority calls.
