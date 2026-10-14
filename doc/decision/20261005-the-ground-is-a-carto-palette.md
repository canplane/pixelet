# The ground is a Carto palette

**2026-10-05** — decided in the conversation; built on the `carto-standard`
branch.

pixelet is a paint canvas in the shape of the world, so what a ground cell
needs to say is which color of block stands there, not what the land is used
for. A cell now holds one look from OpenStreetMap Carto, the style
openstreetmap.org draws with: one byte, numbered in the order Carto paints.
What the data says — Overture's classes, and the OpenStreetMap tags under
them — is read by the bake and left there.

## What a cell was

A ground cell was four channels packed into a `u16`: land, water, transport
and structure, each the ordinal of a type of its own (`pxl-spec bake/pack.rs`).
The bake painted each feature into its own channel, and every reader took the
last channel painted (`bake/ground.rs` `shown_ground`), so what showed was one
value and the other three were hidden under it. Three places read a hidden
channel: the fold, which took a majority per channel; cars, which stood on the
transport channel under a building; and the bake's coast mask. Each can read
the shown value instead.

The types were this tree's own words (`green_deep`, `road_default`), with
colors taken from Carto for some and from a satellite's reflectance for
others, and a building stood over a road, as it does again here.

## The palette

Carto names each look it paints by a variable (`@forest`, `@residential-fill`)
or, where it has none, by the feature it paints (`railway_rail`). A look is
one key: two Carto variables of one color are one key, and a key is named
after the variable it is painted as. Tertiary, residential and service roads
are all white in Carto, which tells them apart by width alone
(openstreetmap-carto#1736), so they are one key, `residential_road`, and
differ on the ground in width and paint order as they do there. Carto changes its colors between
releases, so the palette is held to one release, and the catalog's `doc` cites
the variable each value comes from.

The keys are numbered in the order Carto paints: its layers as `project.mml`
stacks them — landcover, water, buildings, roads by Carto's `z`, aeroways —
and within each, the order its style draws. So one order is said once and read
twice: what paints over what in the bake, and where a key sits in the drawer.
The fold keeps its majority and its tie, the corner that alternates with the
zoom (`pxl-spec bake/fold.rs` `majority`), now over the one value.

Carto has one water fill, so water is one key, river and lake alike. A
wetland is filled by its `wetland` tag, which Overture does not carry, and
Carto fills a wetland without one not at all, so it paints nothing.

## One value space

A ground cell is a `u8`, and the object layer's values a `u16` whose byte
range is the same palette:

| Values     | What                            | Held by            |
| ---------- | ------------------------------- | ------------------ |
| 0          | `void`: nothing painted         | ground and objects |
| 1 – 254    | Carto's looks, in paint order   | ground and objects |
| 255        | `unknown`: a byte no bake wrote | ground and objects |
| 256 and up | `air`, the user's colors, decor | objects alone      |

`void` is where the terrain gives no surface: past the source's coverage,
where a page has no object and the fold reads it as void, and the column there
draws no ground and stops no ray, while an object placed there stands as any
other does. Inside the coverage a page with no object is open sea, since only
a page with nothing drawn in it is left unbaked; a coarser one is the fold of
those under it, so absence folds as every page does and no shared ocean page
is baked or published (`geo-bake terrain/absent.rs`). Whether a page has
arrived is the query's to say (`world/access.rs`), never a cell's.

The ground is the palette's subset an object can also be: placing a block of
forest stores the forest's number, so a terrain-styled cube needs no encoding
of its own. What only an object can be lies above the byte, where no ground
cell can hold it. `unknown` is `0xFF` so that memory filled with `0xFF` reads
as unknown, as a word the pack did not write did before.

## The bake

Carto is read at one commit, gravitystorm/openstreetmap-carto `1cc4b89`
(2026-09-20; its styles are v6.1.0's), and each rule cites the style it comes
from beside the code that applies it (`geo-bake overture/policy.rs`).

A cell shows one look, so painting is an order: every feature has a rank, a
later rank paints over an earlier one, and only the parts of one source share
a rank, which paint one look. The rank is Carto's sort, compared field by field
(`terrain/pipeline/feature.rs` `Rank`):

1. The layer, in the order `project.mml` stacks them: landcover, the
   cutlines (`landcover-line`), the waterways drawn as lines, the water
   areas, dams and weirs (`water-barriers`), piers and breakwaters
   (`piers`), the areas of highways, platforms and aeroways
   (`highway-area-fill`), roads, buildings, aeroway lines.
2. The layer's own order: an area layer larger `way_area` first, the whole
   source's, so a page that clips it or a part of it ranks as the rest do;
   roads by Carto's `z`; a runway over a taxiway; a water barrier's or a
   pier's areas under its lines, and a pier over a breakwater.
3. Carto's `feature`, which breaks a tie in landcover (`landuse_forest`), kept
   by the bake beside the look, since several features share one look and the
   look's number orders nothing.
4. The source's id, `gers_id`, where Carto falls to `osm_id`, so a page's
   ground never depends on the order its rows arrive in. What the bake makes
   from the rows, the strip paved between two ways, picks among equals the
   same way.

Some of Carto's keys are left out on purpose. It compares OpenStreetMap's
`layer` first in water, buildings and highway areas; Overture carries it as an
optional `level`, which the bake does not read for an area, since what stands
above or below the ground is not painted (below). Among roads it orders by access and surface after `z`, and
those only order roads of one `z` that overlap, which `gers_id` decides
instead.

One layer is out of Carto's order: buildings stay over roads. A way is painted
at its width, where Carto draws a hairline at a zoom a building is a few
pixels, so Carto's order would bury a small building under the street beside
it.

The order decides only what lies on the ground at one height. What stands
over or under it is not painted: a bridge is a deck, a tunnel a bore, both
lines the bake writes beside the ground (decision/20261001-ways-off-the-ground.md),
and an indoor passage is left off. A road with none of those flags that
crosses a building's footprint is where the order speaks, and the building
shows. For the same reason a bridge's own area (`man_made=bridge`, Carto's
`bridge` layer) is not painted: the deck is drawn from the ways on it, and
the ground under it shows what is there.

A line is painted at Carto's width at z16, the zoom whose pixel is a cell,
casing included: a way's (`overture/width.rs`) as a waterway's, a runway's
or a pier's. Carto reads no measured width and counts no lanes, so the bake
reads neither, and a deck is as wide as the way it carries.

On a shore the sea is what the coastline leaves, fixed before any feature
paints, and landcover and water do not paint it. It is read from the coast,
never from a cell's look: a salt pond paints water too, and the land over it
must not depend on whether the page touches the sea.

The policy maps Overture's type and class to a look the way Carto maps the
OpenStreetMap tag the class comes from; Overture's `land_use` classes are the
tags' values. A class Carto draws no fill for (a military area, a nature
reserve, a dune) paints nothing, and the land under it shows Carto's
background, `@land-color`, which is also what land no feature covers shows.
The palette holds only what can be painted: a look no Overture class
reaches (a fire station's area, a rest area, a bus guideway, a raceway, mud,
a prison) is not defined, since Overture's conversion from the tags drops
every source of it. Nor is what is not in use painted, a construction site
or a brownfield, a way under construction, abandoned or disused, so neither
has a look. A new look joins the palette only where no look has
Carto's color: a glacier
is Carto's `@glacier`, drawn with the water areas, a dam `@dam`, a
breakwater and a weir `@breakwater-color`, and each railway Carto draws in a
gray of its own (tram, light rail, subway, monorail, rail) its own key; a
pier is `@land-color`, painted over the water. Carto's golf areas are read
by Overture's `golf` subtype, so a bunker there is sand and a military one
nothing.

## What it costs

The terrain format changes and every page is baked again. A page's ground
lane halves, and so does the ground's share of the memory a device holds
pages in, the larger part of it. Placements stored with the old numbers are
not carried over: the objects' stored file moves to a new path, and what
was placed before starts empty.

POI and reverse geocode keep Overture's own keys; that their icons and labels
follow Carto too is a separate change.
