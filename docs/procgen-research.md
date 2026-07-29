# Dungeon Procgen Research Notes

Status: working research map for `rusty-procgen`.

This repo starts with traditional 2D-topology dungeon generation: rooms,
corridors, gates, keys, loops, encounters, and pacing connected as a graph. The
longer-term design should also support properly 3D dungeon structures, offline
batch generation, and validated layout libraries without assuming that runtime
generation is the only interesting target.

## Goals

- Find generation approaches that produce interesting structure, not just noise.
- Keep gameplay intent visible: why a room exists, what promise it creates, and
  how it changes player choice.
- Separate high-level dungeon flow from spatial embedding and local decoration.
- Start with 2D connectivity while keeping the framework dimension-agnostic.
- Treat generic ASHA runtime, collision, pathfinding, replay, and protocol work
  as upstream engine authority if an experiment proves broadly useful.

## Core Working Model

Split generation into layers:

```text
intent graph -> spatial embedding -> local realization -> validation -> scoring
```

- **Intent graph**: abstract player-flow structure: start, goal, lock, key,
  bypass, secret, hazard, reward, boss, shortcut, resource loop.
- **Spatial embedding**: maps abstract nodes and edges into 2D or 3D space.
- **Local realization**: turns nodes/edges into room shapes, corridors, voxel
  volumes, prefabs, dressing, and theme variants.
- **Validation**: proves reachability, lock/key order, encounter placement,
  loop function, and build constraints.
- **Scoring**: ranks candidates by interestingness, navigability, pacing, and
  novelty.

Noise, WFC, prefabs, graph rewriting, and constraint solving should be tools at
specific layers rather than the whole generator.

## Approaches Worth Exploring

### Cyclic / Mission-Graph-First Generation

The Unexplored approach still looks like the strongest baseline for interesting
dungeon flow. Its value is not "prefabs" by themselves; it is the use of cycles,
locks, keys, detours, shortcuts, and revealed alternate paths as first-class
design structures.

Useful cycle patterns to encode:

- lock/key loop
- secret bypass
- one-way return shortcut
- boss gate with preparation branch
- resource detour
- risk/reward side route
- nested obstacle chain
- alternate-solution loop

First experiment idea: generate only an abstract mission graph and validate that
the graph has useful loops before attempting room placement.

Reference:

- <https://www.boristhebrave.com/2021/04/10/dungeon-generation-in-unexplored/>

### Graph Grammars / Graph Rewriting

Graph rewriting is a good fit for this repo because it keeps generation
inspectable. A rule can say "replace a direct path with a lock/key branch" or
"insert a detour loop before the boss gate." That is easier to reason about than
tuning cellular automata, Perlin fields, or flat tile rules.

Possible rewrite phases:

1. Start from `start -> goal`.
2. Expand with mandatory progression structures.
3. Add optional loops and secrets.
4. Add pacing markers such as rest, pressure, combat, treasure, or traversal.
5. Mark edges with desired spatial character: short, long, vertical, hidden,
   dangerous, one-way, destructible, cramped, open.

The output should remain small enough to print, diff, and test. If a graph is
bad, we should be able to explain why.

Reference:

- <https://www.boristhebrave.com/2021/04/02/graph-rewriting/>

### Constraint-Based Placement

After an intent graph exists, placement becomes a constraint problem. This is
where we can ask for concrete guarantees:

- key appears before lock in reachability order
- treasure is off the critical path
- shortcut reconnects within a target path distance
- boss path has enough preparation opportunities
- rooms do not overlap
- 3D modules connect through compatible exits
- vertical travel stays within navigation/readability bounds
- destructible bypasses do not invalidate required progression unless intended

This layer can start with simple backtracking and grow into CSP/SMT-style
placement if needed. The important thing is that constraints are explicit and
diagnostics are useful.

References:

- <https://pvigier.github.io/2022/11/05/room-generation-using-constraint-satisfaction.html>
- <https://drops.dagstuhl.de/storage/00lipics/lipics-vol210-cp2021/LIPIcs.CP.2021.27/LIPIcs.CP.2021.27.pdf>

### Hierarchical Semantic WFC

Flat WFC is useful for local coherence but weak at global intent. The more
interesting direction is hierarchical/semantic WFC: collapse abstract categories
first, then resolve details later.

Dungeon use:

- abstract layer: crypt, cistern, barracks, cave, ritual wing, chasm, vault
- sublayer: room family, corridor family, vertical connector, landmark
- local layer: tiles, materials, clutter, decal affordances

This should be a local realization technique, not the main dungeon brain. The
intent graph should tell WFC what kind of space it is resolving and what
constraints it must preserve.

Reference:

- <https://publications.graphics.tudelft.nl/rails/active_storage/blobs/redirect/eyJfcmFpbHMiOnsibWVzc2FnZSI6IkJBaHBBcndDIiwiZXhwIjpudWxsLCJwdXIiOiJibG9iX2lkIn19--ef1bc5238a0f34702ec971cb7602f5ba5bf8903c/fdg2023-84.final.ACM.pdf>

### Generate-And-Score / Evolutionary Search

Rather than searching directly over finished maps, search over compact inputs:
graph grammar weights, cycle selections, module recipes, embedding parameters,
or WFC examples. Generate many candidates, validate them, score them, and keep
the best.

Possible scoring dimensions:

- loop count and loop usefulness
- critical path length
- optional-path value
- backtracking burden
- navigational sanity
- encounter/resource rhythm
- novelty against accepted layouts
- 3D vertical complexity
- destructible-route consequences

This pairs naturally with offline batch generation. It also keeps runtime cheap:
the game can load accepted artifacts rather than solve hard problems during play.

Reference:

- <https://arxiv.org/html/2607.02082v1>

### Example-Guided Grammar Tuning

Hand-authored graph grammar weights are hard to tune. A later experiment could
use a tiny set of "good" mission graphs and adjust rewrite probabilities toward
similar output. This is appealing for agent-assisted workflows: agents can
propose examples, validators can reject bad graphs, and the corpus becomes a
durable design artifact.

Reference:

- <https://ceur-ws.org/Vol-3217/paper8.pdf>

## Daggerfall-Inspired 3D Direction

Daggerfall is interesting because its dungeon layouts are truly 3D in a way many
modern 3D games are not. Many games have 3D art over fundamentally 2D graphs.
Daggerfall-like spaces can spiral, stack, fold, slope, and branch vertically,
which makes them feel alien, oppressive, and easy to get lost in.

The referenced "procedural recipes" article describes dungeons as connected 3D
modules with tagged exits. Modules can be rooms, corridors, or junctions; exits
carry position/orientation data and compatibility tags; connecting modules means
matching exit transforms. The article also calls out caveats that matter here:
naive module expansion produces trees, overlap checks are needed, loops require
extra work, and connectivity rules can become hard to maintain if they live only
on individual exits.

Research questions:

- Can graph grammar produce a Daggerfall-like 3D intent graph first, then embed
  it with module connectors?
- Can we preserve the oppressive "lost in a machine" feeling while bounding
  navigation pain?
- Can vertical complexity become a controlled score instead of an accident?
- Can landmarks, loop closures, automap affordances, sound cues, or authored
  "breadcrumb" rules make navigation sane without flattening the layout?
- Can voxel destructibility become a designed release valve: not free digging,
  but rule-bound breaches, collapses, shortcuts, or resource-expensive bypasses?

Potential 3D-specific graph annotations:

- `vertical_connector`: stairs, shaft, ramp, lift, collapsed drop
- `orientation_shift`: rotates the player's mental frame
- `stacked_loop`: loop reconnects above/below earlier space
- `overlook`: visible but unreachable space
- `destructible_bypass`: breachable wall/floor/ceiling with rule constraints
- `wayfinding_anchor`: landmark, sound source, light shaft, map marker
- `disorientation_budget`: allowed mental-load contribution

Reference:

- <https://www.gamedeveloper.com/design/bake-your-own-3d-dungeons-with-procedural-recipes>

## Offline / Compile-Time Generation

Runtime procgen is not the only target. Daggerfall's historical appeal includes
the idea that generation can happen before runtime, producing a huge fixed body
of content that the shipped game consumes cheaply.

Modern twist: use local agents and offline validators as a content factory.

Possible pipeline:

```text
agent/generator batch -> validator -> scorer -> artifact store -> runtime shuffle bag
```

- Run 8-64 local agents or generator workers in parallel.
- Let them propose thousands of layouts, graph recipes, or module assemblies.
- Validate all candidates offline for reachability, lock/key ordering,
  navigation sanity, collision, destructibility constraints, and ASHA boundary
  compatibility.
- Score and deduplicate accepted layouts.
- Store accepted layouts as permanent install-level artifacts.
- At runtime, draw from a shuffle bag so layouts repeat rarely.
- Skin, clutter, lighting, enemies, resource state, and material variants at
  runtime so repeated structure is harder to recognize.

This is especially attractive because validation can be expensive and exhaustive
offline. Runtime can stay deterministic, cheap, and fail-closed.

Important distinction:

- **Runtime generator**: must be fast, bounded, and robust every session.
- **Offline content factory**: may be slow, parallel, agent-assisted, and
  aggressively validated before artifacts are accepted.

### Agentic Procgen Workbench

The productive LLM shape is not "ask a model to make a level." The model should
operate a level-construction workbench made of typed deterministic tools. The
hard historical problem is often the central generator bus: coordinating many
good placement, validation, pacing, and repair routines without turning the
generator into a brittle pile of heuristics.

In this repo, LLMs should act as planners, routers, and repair strategists over
CLI tools:

```text
designer intent -> agent plan -> CLI tool calls -> receipts -> validators -> repair/discard
```

The tools should own concrete mutations and checks. The agent should choose
which tool to call next, inspect structured receipts, request validation, and
decide whether to repair, fork, or abandon a candidate. Verifier agents can run
behind the generator as separate judges. A low acceptance rate is acceptable in
offline batches if the accepted artifacts are strong.

See `docs/agentic-cli-workbench.md` for the proposed CLI contract.

## Framework Shape To Preserve

Avoid baking in "2D grid dungeon" as the central abstraction. The first
implementation can be 2D, but the code should separate concerns.

Candidate conceptual modules:

- `intent_graph`: mission nodes, progression edges, cycle metadata
- `grammar`: rewrite rules and weighted expansion recipes
- `constraints`: validation predicates and failure diagnostics
- `embedding_2d`: room/corridor placement on a plane
- `embedding_3d`: module/exits/transform placement in 3D
- `realization`: voxel, prefab, tile, or semantic-WFC realization
- `evaluation`: scores, novelty metrics, pacing metrics
- `artifact_store`: accepted layout records and manifests
- `batch_runner`: offline generation and validation orchestration
- `cli_tools`: stable command surface for external agent harnesses

Data should flow through serializable intermediate records. Each stage should be
testable without a renderer.

## Initial Experiment Plan

1. Define a small intent graph model for 2D dungeon flow.
2. Implement a few cycle rewrite rules:
   - direct path
   - lock/key loop
   - optional treasure detour
   - one-way shortcut
   - secret bypass
3. Write validators for reachability and progression order.
4. Write graph-level scoring and snapshot fixtures.
5. Add a simple 2D embedding pass only after graph output is interesting.
6. Keep a placeholder for 3D embedding interfaces so the core graph model does
   not assume planar grids.

The first win is not rendering a dungeon. The first win is generating a graph
that looks like a deliberate level designer made choices.

## Non-Goals For The First Slice

- No runtime ASHA authority changes.
- No renderer dependency.
- No full voxel dungeon output.
- No local replacement for ASHA pathfinding/collision authority.
- No LLM content factory until deterministic graph generation and validation
  exist.
- No attempt to solve Daggerfall-scale 3D layout before the 2D intent graph
  model is understandable.
