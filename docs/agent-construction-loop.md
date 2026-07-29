# Agent Construction Loop

Status: executable contract through graph analysis and intermediate layout intent.

This repo is intentionally a file-oriented procgen workbench, not an agent
service. External harnesses should be able to steer level construction by
calling deterministic CLI tools, reading JSON artifacts, and discarding bad
candidates without trusting an LLM as validator or authority.

## Loop Shape

The intended agent loop is:

```text
inspect rules -> analyze candidate -> fork -> apply rule -> validate -> repair or score -> annotate intent -> breakdown -> embed -> accept or discard
```

The important boundary is that the agent chooses actions, but the CLI owns the
state mutation and the validators own pass/fail evidence.

## Current Commands

Already available:

```bash
pnpm run procgen -- init --intent <intent.json> --seed <u64> --out <candidate.json> --receipt <receipt.json>
pnpm run procgen -- graph apply-rule --state <candidate.json> --rule <rule_id> --seed <u64> --out <candidate.json> --receipt <receipt.json>
pnpm run procgen -- graph compatible-rules --state <candidate.json> --out <compatible-rules.json>
pnpm run procgen -- graph fork --state <candidate.json> --label <name> --seed <u64> --out <candidate.json> --receipt <receipt.json>
pnpm run procgen -- graph summarize --state <candidate.json>
pnpm run procgen -- analyze graph --state <candidate.json> --out <analysis.json>
pnpm run procgen -- validate graph --state <candidate.json> --out <validation.json>
pnpm run procgen -- repair apply --state <candidate.json> --action <action_id> --target <node_id> --seed <u64> --out <candidate.json> --receipt <receipt.json>
pnpm run procgen -- score graph --state <candidate.json> --out <score.json>
pnpm run procgen -- annotate spatial-intent --state <candidate.json> --analysis <analysis.json> --out <spatial-intent.json>
pnpm run procgen -- breakdown emit --state <candidate.json> --annotations <spatial-intent.json> --out <intermediate-breakdown.json>
pnpm run procgen -- breakdown validate --state <intermediate-breakdown.json> --out <intermediate.validation.json>
pnpm run procgen -- embed 2d --state <candidate.json> --seed <u64> --out <layout.json> --receipt <receipt.json>
pnpm run procgen -- accept --candidate <candidate.json> --layout <layout.json> --validation <validation.json> --score <score.json> --out <accepted.json> --receipt <receipt.json>
pnpm run procgen -- batch generate --out-dir <dir> --seed <u64> --count <n>
pnpm run procgen -- batch generate --profile fixtures/batch-profiles/v2-sample.json --out-dir <dir> --seed <u64> --count <n>
```

Implemented graph rules:

```text
lock_key_loop
optional_treasure_detour
one_way_shortcut
secret_bypass
hub_spoke_cluster
nested_lock_key_chain
hazard_resource_tradeoff
boss_preparation_loop
gated_treasure_branch
branch_merge_shortcut
```

## Agent Surfaces

The core command/artifact surfaces are implemented as deterministic files.

### Rule Metadata

Implemented command:

```bash
pnpm run procgen -- graph rules --out artifacts/manual/rules.json
```

Artifact kind:

```text
rusty_procgen.rule_metadata.v1
```

The artifact includes rule id, intent, emitted node/edge tags, required existing
patterns, duplicate marker ids, compatibility hints, and repair hints. This lets
an agent choose a plausible next rule before mutating state.

### Candidate Summary

Implemented command:

```bash
pnpm run procgen -- graph summarize --state <candidate.json> --json --out <summary.json>
```

Artifact kind:

```text
rusty_procgen.graph_summary.v1
```

The summary includes node/edge counts, tags, locked items, dead ends, score
metrics, validation status, node/edge summaries, and a short provenance tail. It
is small enough to paste into an agent context window for most current
candidates.

### Candidate Fork

Implemented command:

```bash
pnpm run procgen -- graph fork --state <candidate.json> --label <name> --seed <u64> --out <candidate.json> --receipt <receipt.json>
```

Forking should preserve the graph and provenance, add a fork provenance step,
and produce a receipt/transcript event. Agents should use this instead of shell
copying candidates when trying alternate plans.

### Repair Report

Implemented command:

```bash
pnpm run procgen -- repair suggest --state <candidate.json> --out <repair.json>
```

Artifact kind:

```text
rusty_procgen.repair_report.v1
```

The report sorts validation diagnostics by severity, preserves `repairHint`,
and includes known next tool actions where the workbench can suggest one.
Suggestions are advisory: they do not replace validation.

### Bounded Repair Actions

Implemented command:

```bash
pnpm run procgen -- repair apply --state <candidate.json> --action add_rejoin_edge --target <node_id> --seed <u64> --out <candidate.json> --receipt <receipt.json>
```

Supported actions are `add_rejoin_edge` for simple terminal branch nodes and
`remove_orphan_node` for non-start/non-goal nodes with no incoming route.
Rejected repairs still write receipts with diagnostics.

### Graph Analysis And Intermediate Intent

Implemented commands:

```bash
pnpm run procgen -- analyze graph --state <candidate.json> --out <analysis.json>
pnpm run procgen -- graph compatible-rules --state <candidate.json> --out <compatible-rules.json>
pnpm run procgen -- annotate spatial-intent --state <candidate.json> --analysis <analysis.json> --out <spatial-intent.json>
pnpm run procgen -- breakdown emit --state <candidate.json> --annotations <spatial-intent.json> --out <intermediate-breakdown.json>
pnpm run procgen -- breakdown validate --state <intermediate-breakdown.json> --out <intermediate.validation.json>
```

See `docs/intermediate-layout-contract.md` for artifact kinds and the
intentional non-geometry boundary. These commands are for planning and
pre-geometry validation; they do not produce rooms, meshes, voxels, or vertical
dungeon layouts.

### Batch Profile Fixture

Implemented command:

```bash
pnpm run procgen -- batch generate --profile fixtures/batch-profiles/v2-sample.json --out-dir <dir> --seed <u64> --count <n>
```

Artifact kind:

```text
rusty_procgen.batch_profile.v1
```

Profiles move rule sequences and selection labels into JSON so external agents
can propose generation strategies without editing Rust. Selection reports record
the profile id/ref and each accepted/rejected candidate's profile sequence.
Accepted entries also include topology fingerprints, duplicate markers, budget
checks, budget penalties, and `selectionScore`.

### Viewer Context Panes

The static viewer should remain LAN-first through `den-serve`. The next viewer
layer now shows, for the selected batch candidate:

- provenance steps;
- validation diagnostics and repair hints;
- score metrics;
- artifact refs;
- rule/tag summary.

## Agent Guidance

Prefer short, reversible steps:

- inspect rule metadata before choosing a rule;
- inspect compatible rules and graph analysis before mutating mature candidates;
- fork before applying a risky or incompatible rule;
- validate immediately after structural changes;
- use repair reports to pick the next bounded action;
- score only valid candidates;
- use spatial intent and intermediate breakdown validation before any future
  geometry pass;
- accept only candidates with validation and score refs.

Batch generation should be used as a cheap search tool, not as proof that all
accepted candidates are good game levels. Selection scores are deterministic
heuristics for triage.

## Boundaries

This series does not add:

- an in-repo LLM harness;
- a socket/server orchestration API;
- hidden mutable workspace state;
- 3D embedding;
- voxel output;
- ASHA runtime/renderer integration;
- imports from ASHA internals.

All durable state should remain in explicit files under the caller-chosen output
directory.
