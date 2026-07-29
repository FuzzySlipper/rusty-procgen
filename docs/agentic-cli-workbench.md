# Agentic Procgen CLI Workbench

Status: proposed tool contract for external agent harnesses.

The repo should expose ordinary CLI tools and markdown documentation rather than
a custom orchestration service. External scalable harnesses can run many agents
in parallel by invoking commands, reading/writing JSON artifacts, and letting
deterministic validators decide what survives.

## Principle

Do not ask an LLM to emit a complete final level in one shot. Ask it to operate
a constrained workbench:

```text
state file + typed command -> deterministic tool -> new state file + receipt
```

The LLM chooses the next command. The CLI owns the mutation, validation, scoring,
and artifact writing. This keeps the fuzzy part in planning and repair, not in
authority or validation.

## File-Oriented Contract

All tools should accept input paths and write output paths. Avoid hidden global
state.

Recommended conventions:

- `--state <path>` reads a candidate state.
- `--out <path>` writes a new candidate state.
- `--receipt <path>` writes a structured receipt for the tool call.
- `--seed <u64>` makes stochastic behavior reproducible.
- `--format json` is the default machine-readable mode.
- Exit code `0` means the command completed, not necessarily that the candidate
  is valid.
- Validation failures should be data in receipts, not process crashes.
- Process crashes are reserved for malformed input, IO failure, or tool bugs.

Artifacts should be small and serializable:

- `*.candidate.json`: mutable candidate layout state.
- `*.receipt.json`: one tool-call result.
- `*.validation.json`: validator report.
- `*.score.json`: score report.
- `*.transcript.jsonl`: append-only command/receipt trace.
- `*.accepted.json`: accepted immutable artifact.

## Candidate Lifecycle

```text
init -> graph edits -> validation -> embedding -> validation -> scoring -> accept/reject
```

At any point an agent may fork a candidate, repair it, or discard it.

Minimum lifecycle commands:

```bash
rusty-procgen init \
  --intent docs/seeds/lock-key-loop.json \
  --seed 4103 \
  --out runs/4103/base.candidate.json \
  --receipt runs/4103/init.receipt.json

rusty-procgen graph apply-rule \
  --state runs/4103/base.candidate.json \
  --rule lock_key_loop \
  --seed 4104 \
  --out runs/4103/graph-001.candidate.json \
  --receipt runs/4103/graph-001.receipt.json

rusty-procgen validate graph \
  --state runs/4103/graph-001.candidate.json \
  --out runs/4103/graph-001.validation.json

rusty-procgen score graph \
  --state runs/4103/graph-001.candidate.json \
  --out runs/4103/graph-001.score.json
```

Names can change when implemented, but the input/output/receipt pattern should
hold.

## Tool Families

### Graph Tools

Operate on the abstract intent graph.

Candidate commands:

- `init`: create a minimal candidate from a seed intent.
- `graph apply-rule`: apply a graph rewrite rule.
- `graph add-cycle`: add a named cycle pattern.
- `graph annotate-edge`: mark path character such as hidden, one-way,
  dangerous, vertical, destructible, or cramped.
- `graph fork`: copy a candidate with provenance.
- `graph summarize`: print a compact human/agent-readable summary.

Graph tools should never need a renderer.

### Validation Tools

Prove constraints and explain failures.

Candidate commands:

- `validate graph`: reachability, progression order, loop utility, key/lock
  sanity.
- `validate embedding-2d`: overlap, corridor connectivity, path lengths.
- `validate embedding-3d`: exit compatibility, transform consistency, vertical
  navigation, overlap volumes.
- `validate destructibility`: breakable-route consequences and bypass rules.

Validators should return diagnostics with stable codes, for example:

```json
{
  "ok": false,
  "diagnostics": [
    {
      "code": "lock_reachable_before_key",
      "severity": "fatal",
      "node": "gate_crypt_01",
      "detail": "Gate can be reached before its required key branch is reachable.",
      "repairHint": "Move the key branch before the gate or add a safe alternate route."
    }
  ]
}
```

### Scoring Tools

Rank candidates without deciding authority.

Candidate commands:

- `score graph`: cycle usefulness, optional-path value, pacing, novelty.
- `batch generate`: produce multiple deterministic candidates and a sorted
  selection report.
- `score navigation`: backtracking burden, orientation anchors, path length.
- `score three-d`: vertical complexity and disorientation budget.
- `score novelty`: similarity to accepted artifacts.

Scores should be explicit enough for agents to repair against.

### Embedding Tools

Turn abstract graph state into spatial candidates.

Candidate commands:

- `embed 2d`: place rooms/corridors on a plane.
- `embed 3d`: place modules/exits/transforms in 3D.
- `embed repair`: attempt local spatial repair without changing core intent.

Embedding should preserve graph node/edge identities so diagnostics can map back
to design intent.

### Artifact Tools

Manage accepted outputs.

Candidate commands:

- `accept`: copy a candidate into an immutable artifact with validation and
  score references.
- `catalog add`: add accepted artifact metadata to a local catalog.
- `catalog compare`: compute similarity against accepted artifacts.
- `catalog shuffle-bag`: build an install-level shuffle-bag manifest.

Accepted artifacts should include provenance: seed, commands, tool versions,
validation report paths, score report paths, and source candidate hash.

## Agent Roles

The external harness can keep orchestration simple by assigning roles:

- **Planner**: decides which graph/embedding tools to call.
- **Repairer**: receives diagnostics and tries bounded repairs.
- **Structural verifier**: checks hard graph and reachability rules.
- **Design verifier**: judges loop usefulness, pacing, and branch value.
- **Navigation verifier**: judges wayfinding, landmarks, and confusion budget.
- **Novelty verifier**: rejects near-duplicates.
- **Archivist**: accepts artifacts and updates catalogs.

Agents should be disposable. The durable truth is the files they produce and the
deterministic validators they pass.

## Transcript Shape

Each candidate run should leave an append-only transcript:

```json
{"kind":"tool_call","step":1,"command":"graph apply-rule","input":"base.candidate.json","args":{"rule":"lock_key_loop","seed":4104}}
{"kind":"tool_receipt","step":1,"receipt":"graph-001.receipt.json","state":"graph-001.candidate.json"}
{"kind":"validation","step":2,"report":"graph-001.validation.json","ok":true}
{"kind":"score","step":3,"report":"graph-001.score.json","overall":0.74}
```

This gives agents context without requiring special runtime harness APIs.

## First Slice

Build the smallest useful CLI workbench:

1. `init` creates a direct `start -> goal` graph candidate.
2. `graph apply-rule --rule lock_key_loop` adds a key branch and locked gate.
3. `graph apply-rule --rule optional_treasure_detour` adds a side loop.
4. `validate graph` proves reachability and progression order.
5. `score graph` reports path length, loop count, branch value, and dead-end
   count.
6. `graph summarize` prints a compact summary for agent context.

No LLM integration is required for the first slice. A scripted baseline should
be able to drive the same CLI so agent behavior can be compared against boring
automation.

## Current V2 Slice

The current richer grammar slice adds:

- `graph apply-rule --rule hub_spoke_cluster`
- `graph apply-rule --rule nested_lock_key_chain`
- `graph apply-rule --rule hazard_resource_tradeoff`
- `graph apply-rule --rule boss_preparation_loop`
- `graph apply-rule --rule gated_treasure_branch`
- `graph apply-rule --rule branch_merge_shortcut`
- `batch generate --out-dir <dir> --seed <u64> --count <n>`

`batch generate` writes a `selection-report.json` with sorted accepted entries
and rejected entries carrying validator diagnostics. This is the first intended
surface for external agent harnesses to compare candidate variety without
requiring a custom service.

## Non-Goals

- No custom agent service or socket protocol.
- No renderer dependency.
- No hidden mutable workspace state.
- No trusting LLMs for validation.
- No runtime game authority in this repo.
- No direct ASHA engine internals.
