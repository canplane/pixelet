# geo-bake

The world the client draws is a set of files made ahead of time. geo-bake makes
them. Every tile it writes is meant to be a function of two things only: what
the geo database held and the code that baked it, so that the same inputs give
the same bytes. That is what lets a published version be cached for good and a
dev tile server answer as the bucket would.

```text
 sources ─ingest─▶ db-geo ─bake─▶ a set ─publish─▶ the bucket ─▶ the Worker
 Overture,          one shard      leaves,          versions,
 GEDTM30,           at a time      folded up        and the one
 OSM land                                           declaration
 polygons                                           of what is served
```

The database holds each provider's data with the release each shard came from.
A bake reads it only where every shard it needs is in, and names the ingest to
run where one is missing (`infra/geo/README.md`).

## A shard is the unit of work

Every step names its work by shards, the coarsest cubes of the world, each by
its Morton key. A shard holds everything its leaves read, so a leaf can be baked
from its shard alone, and baking shards one at a time gives the same result as
baking them together. What a bake reads past a leaf stays within a fixed
distance of it (pxl-geo-spec `BAKE_READ_DIST`); that distance is the shard's
apron, the margin it holds past its edges. Which shards are published is the
coverage, `infra/geo/coverage.json`.

## A leaf

A terrain leaf is baked from its shard's rows (`terrain/leaf.rs` `bake_leaf`).
The rows become features by the OpenStreetMap tag each Overture class comes
from, and they are painted in the order OpenStreetMap Carto stacks its layers,
ties broken so that no row order decides a cell (`terrain/pipeline/feature.rs`).
A cell takes what covers its center as seen from above, so a tunnel paints
nothing. A leaf is baked only where land, a building or a way reaches it; the
sea is wherever OSM's land polygons do not reach.

Bridges and tunnels are solved from the tile and as much of its neighbors as
their structures reach, never further, so two neighbors solve the same height
where they meet (`separation/`). The elevation is sampled at the leaf's
resolution with a pad of samples past each edge, which the client's interpolant
needs, and a bridge's footing is the elevation of the cell it stands in as the
client draws it, so a bridge's end meets the ground on screen. The tile is
written in the form the client reads (pxl-spec `bake/tile/terrain.rs`). Place
and division leaves are baked from their own tables the same way.

## The pyramid

Every tile above the leaves is folded from the four under it, with no database
in the loop (`pyramid.rs`, pxl-spec `bake/fold.rs`). A terrain parent also reads
the ring around its children, for its elevation's pad. The fold stops at the
coarsest zoom the client reads each layer at.

A tile with nothing drawn is no file at all. The client draws a missing tile
from the one over it, so open sea costs nothing. When a parent is folded, a
missing child stands for what it means (`terrain/absent.rs` `Absence`): sea
inside the shards baked, and unknown past them, which has no surface, so the
picture of the planet shows under it.

A bake folds again only what its shards changed: at each zoom, the parents
whose folds read a changed tile, each staged only where its content differs,
until a zoom changes nothing.

## A set

A bake keeps one set for each kind under `out/<kind>/`: every zoom of the
pyramid side by side under the tiles' names, with a manifest of what it was made
from. It changes the set only through a stage beside it, one shard at a time
(`tile/shard.rs` `Stage`). The leaf step stages the named shards' leaves and
marks each shard whole; the fold step folds again what those shards change,
reading the set as it will be; the commit step, which refuses to run before the
fold, moves the staged tiles in and removes what is gone. Every step can stop
and run again. A change of a provider's release, of the code or of the coverage
means a purge: the set is deleted and every shard baked again.

A terrain tile is refused when it is encoded if its packet is longer than a
client takes one, 2 MiB (pxl-spec `bake/tile/terrain.rs` `MAX_PKT_BYTES`); the
leaf or fold step that made it fails and nothing is committed. A tile's ways
stay lines the client draws, so a change in how they are drawn takes no rebake,
and what the client holds of them is its own to refuse: the bake knows nothing
of the client's octree, pool or memory.

## Publishing a version

A version is a copy of a set uploaded under a new name made of its format and
the day (`script/publish.py`). The format changes only when the way the client
reads a tile changes. Kinds whose parents say a child holds nothing are pruned
of those children on the way up.

Which version of each kind is served is said in one place, the bucket's
declaration, which may also serve single shards from another version
(`source.rs`). Only the publish script writes it, and it refuses anything a
client would refuse (`bin/declaration.rs`). A version never changes after it
is uploaded, so a tile is cached as immutable, and going back means serving an
older version again.

In development the dev tile server (`bin/server.rs`) bakes a tile the
first time it is asked for, by the same functions, and answers as the bucket
does.
