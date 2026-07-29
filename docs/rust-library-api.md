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
placement, built-flow validation, scoring, and stable hashes.

Failed graph-rule application is fail-atomic: `RuleDisposition::Rejected`
returns a candidate with the same canonical hash as the input. Generated
artifacts retain `memory/...` references where the file-oriented artifact
contract requires a provenance label; those labels do not cause filesystem
access.

The CLI uses the same deterministic behavior owners and adds only explicit
path reads/writes, receipt emission, transcripts, and exit status. A library
host does not spawn the CLI, depend on Node or a browser, or require Rusty
Engine/Asha dependencies.

## Verification

Focused public-consumer coverage lives in
`procgen-rs/crates/preflight/tests/core_api.rs`. Unit tests are grouped by
graph, intermediate, geometry, planning, matching, placement, built-flow,
catalog/batch, and scoring owners.

Run:

```bash
cargo test --manifest-path procgen-rs/Cargo.toml --locked
cargo clippy --manifest-path procgen-rs/Cargo.toml --all-targets --locked -- -D warnings
```

The repository `pnpm run verify` gate additionally proves fixed-corpus artifact
identity and the remaining explicitly ledgered integration lanes.
