# Rust Library API

Status: supported downstream API for deterministic in-memory generation.

The `rusty-procgen-preflight` package is both a Rust library and the owner of
the `rusty-procgen` CLI. Despite the historical package name, the library is
the canonical implementation of the current dungeon-generation pipeline.

## Ownership

The Rust modules under `procgen-rs/crates/preflight/src/parts/` have focused
behavior owners:

| Module | Owner |
|---|---|
| `model.rs` | Authored inputs, generated artifacts, diagnostics, and validation DTOs |
| `candidate_graph.rs` | Candidate construction, deterministic graph rules, and forks |
| `graph_reports.rs` | Analysis and compatible-rule reporting |
| `intermediate.rs` / `intermediate_validation.rs` | Spatial intent and intermediate breakdown |
| `geometry_preview.rs` | Physical connections, bounded 2D geometry, and geometry validation |
| `piece_plan.rs` | Explicit room/corridor piece requirements |
| `shape_matching.rs` | Deterministic catalog matching |
| `piece_placement.rs` / `pure_catalog_placement.rs` | Bounded assembly and placement validation |
| `built_flow.rs` | Item-aware logical-to-physical flow validation |
| `repair_validation.rs` / `scoring_embedding.rs` | Graph validation, repair, scoring, and inspection layout |
| `catalog_aware_generation.rs` | Bounded catalog-aware retry composition |
| `catalog_generation_trace.rs` | Bounded semantic event trace, hash closure, and replay validation |
| `batch_artifacts.rs` | Corpus orchestration and checked evidence |
| `cli.rs` / `dispatch.rs` / `io_utils.rs` | CLI parsing, shared file/receipt operations, dispatch, and process behavior |

`src/lib.rs` composes these owners. `src/main.rs` is intentionally only a
process wrapper around `run_cli()`.

Private `*_command` functions remain beside the behavior they adapt so a
command's orchestration is traceable without a second dispatch hierarchy.
Those functions may read and write explicit paths; they are not part of the
public library surface. Pure functions in the same owner are consumed by both
the command adapter and `ProcgenCore`.

## In-memory facade

Library consumers should use `rusty_procgen_preflight::core::ProcgenCore`.
Its methods accept and return typed values and never read or write files. The
facade covers candidate generation, graph rules, analysis, annotations,
intermediate layout, geometry, piece planning, catalog matching, bounded
placement, catalog-aware generation, built-flow validation, scoring, and stable
hashes.

Failed graph-rule application is fail-atomic: `RuleDisposition::Rejected`
returns a candidate with the same canonical hash as the input. Generated
artifacts retain `memory/...` references where the file-oriented artifact
contract requires a provenance label; those labels do not cause filesystem
access.

`ProcgenCore::realize_catalog_aware` owns the bounded retry composition used by
the `build realize-catalog-aware` command. Callers provide typed candidate,
geometry, catalog piece-plan, shape-catalog, policy, and provenance-label
values. The result records every attempted room compaction, measured outcome,
hard-limit decision, and selection comparison. It returns either the complete
accepted geometry/plan/match/placement/flow chain or the final typed
classification (`catalog_coverage_gap`, `generation_infeasibility`,
`search_budget_exhaustion`, or `outcome_constraint_miss`). The runner does not
mutate any caller value and does not interpret provenance labels as paths.

`ProcgenCore::validate_placement_with_catalog` accepts the immutable source
plan and materialized final plan so it can validate both ordinary
match/assemble chains and the catalog-aware runner's accepted chain. Ordinary
callers pass their plan in both positions. For catalog-aware chains it
revalidates the generated match against the source plan, exact final plan,
catalog-search decisions, transformed catalog exits and sockets before
recomputing every per-instance and aggregate placement surface. It does not
feed catalog-aware evidence back through the ordinary matcher because the
bounded runner materializes route pieces and room choices under a distinct,
recorded search policy.

`CatalogAwareOutcomeConstraints` owns hard maxima for final placement width,
height, area, and routed catalog cells. `CatalogAwareOutcomePreferences` owns
one bounded primary metric and preferred maximum. Rust evaluates the complete
attempt budget and publishes the deterministic best admissible outcome; the
preferred maximum records target satisfaction but never truncates the search.
These are dungeon-tooling decisions local to Procgen, not a generic Engine
optimization service.

`ProcgenCore::realize_catalog_aware_traced` returns that unchanged result beside
a bounded `rusty_procgen.catalog_generation_trace.v2` sibling artifact.
`replay_catalog_generation_trace` verifies the exact input/result binding and
reconstructs the committed room and route states. See
[`catalog-generation-traces.md`](catalog-generation-traces.md) for event,
quota, fixture, and nonclaim details.

The CLI uses the same deterministic behavior owners and adds only explicit
path reads/writes, receipt emission, transcripts, and exit status. A library
host does not spawn the CLI, depend on Node or a browser, or require Rusty
Engine or presentation dependencies.

## Verification

Focused public-consumer coverage lives in
`procgen-rs/crates/preflight/tests/core_api.rs` and
`procgen-rs/crates/preflight/tests/catalog_aware_core.rs`. Trace closure,
replay, tamper, exact-limit/one-over, and CLI non-publication coverage lives in
`procgen-rs/crates/preflight/tests/catalog_generation_trace.rs`. The
catalog-aware core test runs from an empty working directory, proves input
non-mutation and deterministic repetition, and compares the public result with
the actual CLI for success and each exhaustion class. Unit tests are grouped
by graph, intermediate, geometry, planning, matching, placement, built-flow,
catalog/batch, and scoring owners.

Run:

```bash
cargo test --manifest-path procgen-rs/Cargo.toml --locked
cargo clippy --manifest-path procgen-rs/Cargo.toml --all-targets --locked -- -D warnings
```

The repository `pnpm run verify` gate additionally proves fixed-corpus artifact
identity and the remaining explicitly ledgered integration lanes.
