# Ways that are not on the ground

**Status: its solve is withdrawn** by
[20261006-bridges-and-tunnels-as-arnis-solves-them.md](./20261006-bridges-and-tunnels-as-arnis-solves-them.md)
— heights are Arnis's, taken whole, not reinterpreted, and the clearances
and constants below are not the bake's. What stands is how a deck is
stored: a line in the terrain page.

**2026-10-01** — a design conversation and the first half of its code.

The ground is a surface, and a page paints only what lies on it. Until
this day it painted every road and track it was given, so the map showed
tunnels drawn across rivers and ferry routes drawn across the sea, while a
bridge lay flat on whatever it crossed. Two things came out of it. A way
that cannot be seen from above is not painted. A way carried above the
ground is a **deck**: a line with a height along it. It is stored as
vectors in the terrain page, drawn into the chunk the way the ground is,
and its heights are solved offline, structure by structure, after Arnis.

## What the map showed

A segment's tunnel and bridge flags never reached the bake. The ingest read
a `road` property that Overture had since split into `road_flags`,
`rail_flags` and `level_rules`, so the column was NULL on all 3,223,677
rows, and the policy hardcoded `is_tunnel: false` until the field path was
confirmed. A sample from Gangnam confirmed it: flags and levels hold over a
`between` range of a segment's length, so one segment of a subway line
runs over a bridge, along the surface and into a tunnel.

Re-ingested on release `2026-09-23.1`, the table has 3,237,671 rows: 243,960
carry road flags, 5,030 rail flags and 175,300 level rules. 152,994 carry
`is_bridge`.

On the Han between Yeouinaru and Mapo, the page before the change showed the
tunnels of the Shin-Ansan line under construction and of a main line as
rails across open water, and the cruise routes as roads.

## What is not painted

One rule replaced the tunnel flag: a piece of a way is not painted where
nothing of it shows from above (`UNSEEN_FLAGS`, geo-bake
`terrain/pipeline/overture/policy.rs`):

| Not painted                | Why                                   |
| -------------------------- | ------------------------------------- |
| `is_tunnel`                | under the ground                      |
| `is_indoor`                | inside a building                     |
| `is_under_construction`    | not built yet                         |
| `is_abandoned`             | its track is gone                     |
| a `water` segment          | a ferry's route, painted over the sea |
| a base water line `strait` | where a strait's name is written      |

`is_disused` stays, since its rails are still there, and so does
`is_covered`, which is under a roof the buildings paint over where they
have it. A segment is cut where a flag starts or stops, so only the range
it holds on goes. The decor's trains now run on subway track too: what the
ground shows of a subway is at the surface.

The ingest now keeps its extract. An extract is every column of every
feature in the box, and an ingest whose extract is already the latest
release over the same bbox reads it rather than downloading it again
(`is_current`, `infra/geo/overture/script/overture.rs`), so a change to what
a table keeps is a re-run and not a 3 GB pull.

## Where a deck's height comes from

The height has to be solved offline, over the whole network. Three facts a
deck's height depends on — where the structure comes down to the ground,
its layer, and what passes under it — are in the vectors, and the client
holds only rasters and the pages it has resident. A rule the client could
run is a local one: the deck a clearance above the ground under it, ramped
toward its ends. That passes for an overpass or a river bridge and fails
for a viaduct across a valley, which has to hold a straight line for
hundreds of meters, far past the 32 cells a page's elevation reaches beyond
it. Run per page, it also disagrees at every page's edge, and it would cost
the device on every chunk build what the bake pays once. What is procedural
is the deck's shape — its thickness and its piers — drawn when the chunk is.

OSM's `layer` is an order, not a height, and is meaningful only with
`bridge` or `tunnel` beside it. A deck's height is therefore what clears
what lies under it, not a layer times a step.

The prior art agreed:

- **Arnis** (Apache-2.0, OSM to Minecraft, Rust) solves decks in
  `src/element_processing/bridges.rs`. Bridge ways sharing an end at one
  layer are one structure, and parallel decks merge. Each deck clears what
  passes under it: 6 m over a road, 5 m over a path, 8 m over a railway,
  4 m over a river, 2 m over a stream, 6 m over a lower deck. Its free ends
  come down to the terrain, a clearance climbs no steeper than a cap, ramps
  are found at the ends, and lower structures are solved first. It works
  structure by structure and not as one system.
- **OSM2World** has a `BridgeTunnelElevationCalculator` and a global
  constraint solver, documented as fragile and off by default; a diffusion
  solver (PR #176) closed unmerged. Solving a whole network at once is the
  road not to take.
- **Mapbox Standard** draws elevated roads, and the modes that lift a line
  need the elevation precomputed in the tileset.

Arnis is reinterpreted, not imported. It is a binary with no library
target; its bridges read the Minecraft world being built
(`WorldEditor::get_ground_level`, `highest_block_between`) and OSM tag maps,
in one-metre blocks over a region held in memory, and half of its bridge
code places blocks for arches and pylons. What carries over is the solve
and its constants, rescaled: a unit is about 2.39 m at the equator and
about 1.9 m in Seoul, so a road's 6 m is about 3 units there.

## How a deck is stored

Three shapes were weighed.

A **lane in the terrain page**, a byte a cell, held a deck's rise and kind.
At the ground's resolution it costs half the ground lane again across the
120 records, about 30 MiB; at two cells a side, about 7.5 MiB, which was
chosen and then dropped. A lane holds one deck a cell. A bridge over a road
fits, the road in the ground's transport channel and the bridge in the
lane, but an interchange does not: where ramps cross, the lower one is cut
by the upper one's width. A lane of K decks a cell only moves that limit,
and nothing in the data bounds K.

A **voxel layer**, baked objects beside the user's, stacks anything. It is
a layer of its own: one more request a page against the Worker's free
100,000 a day, and an index of its own beside the terrain's, which every
ray and every chunk's bounds would have to read and reconcile.

A **line in the terrain page** is what was chosen: a polyline in the page's
cells, a top height at each point, a width and a kind. A chunk draws the
decks that cross it into its columns, as it draws the ground. Decks stack
without a limit, their edges fall at the ground's resolution, and a page
carries a few kilobytes where it has any.

The rule this follows decides every layer:

| Layer                   | Nature                                                     | Shape         |
| ----------------------- | ---------------------------------------------------------- | ------------- |
| elevation               | a continuous field                                         | raster        |
| land, water, structure  | areas covering every cell                                  | raster        |
| transport on the ground | lines, dense in a city (18% of the Han page's cells), flat | raster        |
| POI                     | points summed to cells                                     | raster        |
| user objects            | sparse 3D                                                  | sparse octree |
| decks                   | sparse lines, stacked, a height along each                 | lines         |

A dense field is a raster: the same memory whatever it holds, a point
answered by an index, and a fold that is a majority or a mean that the
client can run on what it holds. A sparse thing is kept in its own shape.
Moving the ground to vectors would put the fill of thousands of building
polygons a page on the device at every chunk build, and its coarse zooms
would need cartographic generalization in place of a majority.

What a deck costs at a chunk build is the decks that cross the chunk: a
deck six cells wide across a 64-cell chunk writes about 400 columns,
against the 66 × 66 columns and their bicubic samples every build already
reads, and a chunk no deck crosses tests a few boxes. A coarser page's decks
are its children's, simplified, with any narrower than half its cell left
out, the same rule by size the ground's majority keeps.

## Why the terrain page carries it

A layer is split by when it is needed, kept and read, not by what kind of
data it is:

| Payload                           | Wanted                   | Read by                 | Rebaked               |
| --------------------------------- | ------------------------ | ----------------------- | --------------------- |
| terrain: elevation, ground, index | the ground the view sees | chunk build, ray, index | when the bake changes |
| POI                               | its own demand           | marks and billboards    | on each data refresh  |
| geocode                           | its own demand           | address search          | monthly, in place     |
| decks                             | with the ground          | chunk build, ray, index | with the terrain      |

Decks are wanted with the ground, drawn in its chunks, and their tops
belong in the terrain's index lane, which is what lets a ray pass a region
and what bounds a chunk's heights. Apart, they would cost a request a page,
including the pages with none, and two indexes to keep in step. Together,
a change to the deck solve is a rebake of the terrain — which a change to
the bake already is.

In the packet they are one more branch, structure-of-arrays, copied into
the record as they arrive:

```text
terrain = [ele, cells, extent, occupancy, decks]
decks   = [offsets, points, tops, width, kind]
```

## Out of scope

Buildings stay flat: extruding them is the same kind of span, and the mesh
a whole city of them would put on the device is a decision of its own.
Nothing draws what is under the ground; a cut-away view would put tunnels
in the same deck records, below it.

## What came after, the same evening

Seen on the screen, the first decks stood, and three things were wrong
with the ground around them, each decided from what the precedents do.

- **Widths.** A way was painted at a square kernel's odd width and at its
  class's guess, so a residential street was six metres and a diagonal
  road wider than an axis-aligned one. A way now covers a cell whose center
  lies within half its width of its line, never under half a cell's
  diagonal so it stays connected (`pxl-spec bake/way.rs`), on the ground
  and on a deck alike. The width is `width_rules` where the source gives
  one — on about one segment in a hundred, much of it an import's width
  classes — and else its lanes: 3.5 m a lane, osm2lanes' default, the
  lanes and margins of the class's usual cross-section as Arnis lands on
  them, one way for a carriageway of a road drawn as two
  (`terrain/pipeline/overture/width.rs`).
- **Order and roles.** Ways paint in OpenStreetMap Carto's `z_order`: paths
  under service lanes under every link, links under every through road,
  railways over all. Overture's `subclass` says a footway is a sidewalk or
  a crossing: a crossing is not painted over the road it crosses, and the
  gap between a sidewalk and its road, or between a road's two
  carriageways, is paved where it is no wider than osm2lanes' 2 m buffer
  and left as land where it is wider, as osm2streets models a separated
  sidewalk with a planted buffer (`terrain/pipeline/corridor.rs`).
- **Classes.** The transport types follow OpenMapTiles' transportation
  classes: trunk, tertiary, service and track are types of their own, a
  living street is a road and a tram a transit line, in Carto's colors.

Every cell was painted half a cell north-east of where its feature lies,
lines and polygons alike, against the elevation's and the decks' centers;
it is painted at its center now. Buildings stand a cell above the ground,
as water lies one below it.

Decks running side by side — two carriageways, a sidewalk beside a road —
are one structure with one profile, joined where their edges lie within
2 m and their headings within 20°, as Arnis unions parallel decks; a deck
over wide water clears a metre more for every hundred metres of it, up to
fifteen.

## Where it stands

**Done:** the ingest carries the flags, widths, one-way rules and
subclasses and keeps its extract; the bake paints only what is seen, at
the cells' centers, with real widths, Carto's order and the corridor's
gaps; decks are solved, baked as lines, folded, drawn and met by the ray;
terrain is v8 on the dev stack. **Next:** a tunnel's mouth carved where
it meets a hillside; `Access::at`'s callers, each asked what it wants now
that a column can hold more than its ground; the surface named in the
`Terrain` type; terrain v8 baked whole and published.
