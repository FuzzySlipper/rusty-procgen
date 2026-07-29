# Build Piece Library Structure

Status: proposed directory-first structure for switchable build-piece and
layout packs.

## Recommendation

Use directory-backed packs as the authored source of truth. Keep SQLite as a
later generated index/cache for large generated corpora, search, and shuffle
bags.

Directory packs fit the current goals better because they are:

- diffable in Git;
- easy for agents and humans to inspect;
- friendly to hand-authored prefabs and fixtures;
- simple to copy, fork, disable, or mix;
- compatible with future generated indexes.

SQLite becomes useful when the project needs fast queries across thousands of
layouts or build pieces, but it should not be the only authored storage format.

## Pack Root

A pack root should have a manifest plus typed subdirectories:

```text
fixtures/packs/
  2d-basic/
    procgen-pack.json
    build-pieces/
      shape-catalog.json
    layouts/
      curated/
      generated/
    themes/
    indexes/
```

The manifest is the switchable module boundary. CLI tools should eventually
accept `--pack fixtures/packs/2d-basic` and resolve piece catalogs, layout catalogs, and
default generator settings from that manifest.

Example manifest shape:

```json
{
  "kind": "rusty_procgen.pack.v1",
  "schemaVersion": 1,
  "packId": "pack.2d_basic",
  "label": "2D Basic",
  "dimensionModel": "2d_grid",
  "defaults": {
    "gridConnectivity": "four_way",
    "shapeCatalog": "build-pieces/shape-catalog.json"
  },
  "exports": {
    "buildPieces": ["build-pieces/shape-catalog.json"],
    "layoutSets": ["layouts/curated", "layouts/generated"]
  },
  "tags": ["fixture", "2d", "grid"]
}
```

## Build Pieces

Build pieces should remain explicit catalog data. A shape catalog can stay as a
single JSON file while small, then split into multiple files when useful:

```text
build-pieces/
  shape-catalog.json
  rooms/
  corridors/
  thresholds/
```

If split, the pack manifest should own the file list or glob policy so tools do
not have to guess. Each build piece needs stable ids, piece kinds, footprint
cells, exits, feature sockets, allowed transforms, tags, and provenance.

## Layouts

Layouts should be separate from build pieces. They can reference pack ids and
piece catalog ids, but should not embed the entire piece library.

Recommended subgroups:

- `layouts/curated`: hand-authored or promoted layouts.
- `layouts/generated`: offline generated candidates that passed validation.
- `layouts/rejected`: optional debugging corpus for verifier work.
- `indexes`: generated summaries, shuffle bags, fingerprints, or SQLite files.

## SQLite Role

SQLite is a good fit for generated indexes, not the first authored format.

Use it later for:

- fast search by tags, exits, dimensions, sockets, or fingerprints;
- install-level shuffle bags;
- dedupe and similarity lookup;
- verifier run summaries;
- large offline LLM generation batches.

Keep it rebuildable from directory pack sources whenever possible.

## Near-Term CLI Shape

Useful next CLI options:

```text
--pack fixtures/packs/2d-basic
--shape-catalog <path>       # override pack default
--layout-set curated         # select a layout group
--connectivity four-way      # override pack default
```

The viewer should expose the active pack, shape catalog, and layout set so a
generated dungeon is inspectable as "which modular ingredients produced this?"
