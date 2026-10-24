# Places follow Carto

**2026-10-09** — decided in conversation and built the same day; what is left is tuning.

A place is what Overture's schema says it is, and it is drawn as
OpenStreetMap Carto draws a point of interest, and behaves as Carto does:
the same icon, color, label and zoom, the same places kept where they would
meet, and nothing Carto does not draw. The one thing kept of our own is that
places are held in the pyramid. Everything else this tree had invented for
places — its 35 kinds, the emoji badges, OpenMapTiles' class rank, the count
of a crowded cell — goes, so that a question about how a place looks is
answered by Carto and not by us.

## What a place is

Overture's places, read as its schema gives them: the GERS id, the names
(primary, and common by language), and the kind as `basic_category`. In the
geo database today 777,030 places carry 256 basic categories (12k carry
none), against 1,245 values of `taxonomy.primary` under 13 roots.

`basic_category` and not the finer taxonomy, because it is the level Overture
made for display across sources: a provider's own tree (Naver, Kakao) maps
onto 256 kinds far more surely than onto 1,245, and every finer distinction
would only be folded back to one Carto icon anyway. The list lives in one
JSON under `spec/`, generated into Rust and TypeScript. A tile carries the
kind as its number in that list, a `u16`: 256 kinds and none do not fit a
byte. A release whose taxonomy moved is a purge by command, as every source
change is; a category the list does not hold has no row and is not drawn.

`confidence` is not read. Carto draws OpenStreetMap, which has no such
field, so there is no precedent for a threshold and none is invented.

## How a place is drawn

One table, the theme's, from a basic category to what Carto draws for it:
the OpenStreetMap tag Carto styles that kind under (kept so each row can be
checked against `amenity-points.mss`), the icon (`symbols/`), the icon's
color, the label's color, and the zooms at which the icon and the label
show. Carto's point rules give a lower zoom and no upper one; a rule with a
`way_pixels` condition is an area's, which a point never meets, so a row
takes the rule a point falls to. A category with no row is not drawn and
not carried in a tile, as the
ground carries only the classes its policy paints (`terrain/repository.rs`).

The icon is Carto's SVG, rasterized into an atlas by the asset pipeline and
filled with its row's color. The label is the place's `names.primary`, which
is what Carto's `name` is, in Carto's text style but a marker's font, so the
world has one size of text: a halo, set under the icon (`text-dy`), wrapped
at the cell a place keeps to itself (below; `pxl-kernel scene/place.rs`
`spacing_px`), where it was first six em, twice Carto's, which broke a
Korean name of five syllables in two, in its row's label color. A map label
is Carto's, so it is not translated; the address line the shell writes from
the divisions is UI text and keeps the language setting.

## Zoom, as the grid's cell

A Carto zoom is a ground resolution: at zoom c a web tile's 256 pixels span
one tile of the pyramid at c, so its 16-pixel icon is a cell of that tile at
c + 4. Places are kept in that lattice (`PLACE_LATTICE`) and read as every
layer is, at its scale on the camera grid (`Lattice::scaled`), two steps
coarser than the grid (`pxl-spec bake/tile/place.rs` `SPREAD_STEPS`): an
icon's cell spans four grid cells, some 48 pixels round a drawn icon of 22
against Carto's 16, so the kinds shown at a distance are about Carto's at
that size on the screen. The zoom a place is drawn at is the tile zoom of
the grid's cell there, past the leaf too. The grid's cell is the distance's
from the eye, so which Carto zoom a place is drawn at is decided in 3D, by
the chunk the top of what stands there is in, as everything drawn is. Nothing measures pixels: the grid's
cell, an angle, stands for Carto's icon, so the demand is the grid's and
bounded as every layer's is, and the quality setting that coarsens the grid
coarsens the places with it, as it does the ground. Measuring Carto's pixels
on the screen instead would make the tiles a zoom asks for grow with the
screen, which nothing bounds.

## Which places, where they would meet

Carto lets no two icons overlap and places them in its query's order,
`score DESC NULLS LAST, way_pixels DESC NULLS LAST, osm_id` (`project.mml`,
`amenity-points`); a point has neither score nor pixels, so the order is the
id's. The later of two that would meet is not drawn at that zoom, and two at
one spot never both are: Carto has no count, cluster or spread for them.

Ours is the same rule quantized to the pyramid, so that a tile holds only
what lies in it: at Carto zoom c a cell four icons wide — an icon 16 pixels,
Carto's 14-pixel symbols rounded up to a power of two, so the cell at zoom
c + 2 of the web map's 256-pixel tile — keeps one place, the first in order,
and every other in it is not drawn at c. Four icons, some 190 pixels on
the screen, is a product choice: at an icon's own cell, twice the places
Carto shows were kept, which drawn showed as icons and names piled on each
other, and places a few metres apart stood side by side as near as the view
came. The cell also holds a name of a few words, so a name shows wherever
its kind's label zoom has come and needs no rule of its own; where Carto
places labels after the icons, colliding with each other, ours comes with
its icon. That differs from Carto, and nothing is read past a cell to close
it: two places a pixel apart across a cell's edge are each first in their
own cell, so their icons and names may meet, where Carto would drop one;
and fewer places show than Carto's, which spaces icons only an icon apart.

The order is the icon zoom, then the GERS id: a place shown from a coarser
zoom goes before one that appears only finer. Carto orders by id alone, so a
restaurant appearing at z17 could take a hospital's room there; ours keeps
the hospital. That is what makes a cell's first place stay first in the
cell it falls in a zoom finer — a cell's places at c + 1 are its own at c
and those of icon zoom c + 1, which all rank after — so what is shown only
grows with the zoom. It holds because cells nest, which the power-of-two
width gives. A parent is its children folded: per cell, the first of their
firsts shown from its zoom or coarser.

The placing is done when the tiles are made. A place carries the zoom it is
shown from — its icon zoom, or the first finer zoom at which it is first in
its cell — and no other state; one never first is not carried. The client
draws what that admits and decides nothing.

## How a tile holds them

A tile at zoom z covers what the web map's tile at z covers, and serves
Carto zoom z; that is the one map between them, and the zoom the client
reads at (above) picks the tile by it, the leaf for every Carto zoom past it.
A tile is 16 icon cells a side; the leaf places its finer zooms in the
ground's leaf cells it stands in, which nest in them through z19's.
A tile at Carto zoom z holds the places shown from z or coarser within it.
The leaf, at z15, holds every place of its area with the zoom it is shown
from, through Carto's last (z19), so what is placed finer than the leaf is
placed inside it. A child is kept only if it holds a place its parent does
not, or has a child kept, so a place first shown deeper stays reachable; a
reader at any zoom reads the deepest tile held for its node.

## The billboard

Places and the markers someone puts up stay one billboard pass — gathered,
culled to the screen, drawn far to near, tapped — since a billboard is a
behavior. What each draws is its own sheet: a marker its words, as now; a
place Carto's icon and label. A place's count badge goes; markers crowded
in one marker's cell keep theirs (`pxl-kernel billboard/badge.rs`
`Label::Count`).

## What goes

`place_badge.json` and its 35 kinds with their emoji and colors, the
`category::map` onto them, `poi_class_rank` and the near/far zooms it
served (`PLACE_ALL_ZOOM`, `PLACE_MAJOR_*`), and the count of a crowded cell
with its `partial` flag.

## Divisions

The divisions' format is Overture's already. Their theme does not carry
over: what the shell writes is the focus's address, UI text, and Carto has
no precedent for how deep an address reads. Its depth stays overture-tiles'
rule (`center.ts`). Drawn on the map one day, division names would follow
Carto's `placenames.mss`.
