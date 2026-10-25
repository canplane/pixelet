# The mesh step and the zoom are one number

**Status: superseded** by
[a zoom names a page grid](../20261006-a-zoom-names-a-page-grid.md) — a zoom no
longer names a cell's scale: it names the page grid, a leaf is z15, and a cell
is named by its size. The mesh step and the cell are still one scale; the zoom
is no longer that scale's name.

**2026-09-21** — a design conversation, built since; what the building changed
is at the end, under "Where this stands". It starts from
[the page pyramid](./20260921-the-page-pyramid.md), written the same morning,
and supersedes two of its claims: the page is half as wide, and the fold is not
what that note said it was.

A terrain page is one resolution, and the client folds it down at build time
whenever a chunk is far enough away to want less. So three separate ladders
describe how coarse a thing is — the mesher's `lod_step`, the single `KEEP_DIST`
a page is held within, and the zoom a pyramid would give a page — and none of
them knows about the others. The proposal is that they are one number, and that
the number is Web Mercator's zoom.

## The identity

Make a page 512 cells on a side. Then, because the X lane of a coordinate is one
full EPSG:3857 world (`space/adapter/3857.rs`) and a z16 cell is one unit:

```text
world           2^24 u   = 40,075,016.68557849 m = 2π · 6378137
cell at z       2^(16 − z) u
page at z       512 · 2^(16 − z) u
pages across    2^24 / (512 · 2^(16 − z)) = 2^(z − 1)
```

A z16 cell is 2.3887 m, which is the standard z16 pixel exactly and not nearly:
that pixel is `156543.03392804097 / 2^16`, and the constant is the world's width
over 256, so both are the same division of the same number. A 512-cell page is
2×2 of the 256-pixel tiles that OGC's `WebMercatorQuad` defines, and our finest
resolution is z16.

| z   | cell   | page      | South Korea spans |
| --- | ------ | --------- | ----------------- |
| z16 | 2.4 m  | 1.2 km    | 209,293 cells     |
| z12 | 38 m   | 19.6 km   | 13,081 cells      |
| z10 | 153 m  | 78.3 km   | 3,270 cells       |
| z6  | 2.4 km | 1,252 km  | 204 cells         |
| z1  | 78 km  | 40,081 km | 6 cells           |

The alignment is not inherited. The projection is a standard and the tiling
scheme is a separate one, but the lattice under them is this project's own: what
makes the two line up is the single decision that all 24 bits of X are one
Mercator world, plus `CELL_SHAMT = 0`. Three constants agree today and nothing
records that they must, so the identity above belongs in an assert.

## The page is 512 cells

Halving the page from its current 1024 costs no resolution. The finest cell is
fixed by `CELL_SHAMT` and the world's width; the page only decides how that
resolution is cut into files. One 1024-cell page is four 512-cell pages, and the
cells in them are the same cells.

What it buys:

- A page is **8×8 = 64 chunks**, which is exactly `MAX_MOUNTS_PER_FRAME`. One
  page is one frame's worth of mounting; today's 256 chunks take four.
- A record is **1,051,168 bytes** — four 512² planes and a 36² elevation grid —
  against 4,203,552 today.
- The transfer unit drops from 4.2 MB to 1 MB, against a first load on a phone
  measured at p50 9.7 s from request to adopt, of which the network was 1.4–1.9
  s and the rest was queue wait behind `MAX_INFLIGHT` 6 (BACKLOG).
- The grid and the zoom land on the web map convention at once.

Elevation density is unchanged: `PAGE_ELE_SPAN` is `1 << (L3_SHAMT + 1 - 5)`, so
a 512-cell page carries 32 inner samples and a page still has one sample per 16
cells — 38 m at z16, which is the 30 m source's own resolution. The two-sample
skirt still reaches 32 cells past the page's own span at every zoom.

`L3_SHAMT` becomes 9, and L3 is the unit of more than terrain: POI pages and the
object store's saved form are keyed by it too, so they re-bake and re-version
with it.

## Terrain stops at z16; finer is the object tree

Terrain is a 2.5D raster at exactly one finest resolution, and nothing under a
cell belongs to it. What is finer is the object layer's sparse tree, whose
`OBJECT_LAYOUT` already reaches three zooms finer than the cell — `OBJECT_SIZES`
is `[1.0, 0.5, 0.25, 0.125]`, which is z16 through z19. So the zoom axis is one
axis with two owners and no overlap:

```text
z19   1/8 cell   0.30 m   ┐
z18   1/4 cell   0.60 m   ├ object, a sparse tree per band
z17   1/2 cell   1.19 m   ┘
z16   1 cell     2.39 m   ← terrain's floor, the tree's root
z15   2 cells    4.78 m   ┐
z14   4 cells    9.55 m   ├ terrain, a record per zoom
 …                        ┘
```

Deciding when a sub-cell entry is drawn as itself rather than folded into its
cell needs no new rule: an entry of size `2^-j` cells reaches the ladder's own
target pixel count at `192 · 2^-j` units, which is 96 u for a half cell, 48 for
a quarter and 24 for an eighth. Past that the coarse pack already absorbs it as
coverage.

## Counting in cells, measuring in units

One rule decides which things a zoom may rescale:

> What the world is divided into is counted in cells. What exists in the world
> has a size in units. A zoom changes only the exchange rate,
> `1 cell = 2^(16 − z) u`.

Counted in cells, and so the same at every zoom: the page (512), the chunk (64),
a band's height (64), the elevation's sample spacing (16) and its skirt (2
samples), an object entry's ceiling (1 cell). Measured in units, and so not: a
tree's extent, a placed cube's footprint, the camera's distance, the fog.

The rule decides what may be coarser than z16 at all, and it has one immediate
consequence. Decoration is drawn while `extent / step ≥ 2`, so a kind of extent
`E` reaches `96 · E` units. The ingested models run 0.77 u to 4.74 u, which puts
every kind inside 455 u of the camera. That is past where the leaves themselves
are drawn, which ends at 384 u, so a z15 chunk can carry decoration too, and it
scatters it from the leaves under it rather than from its own record.
**Decoration is always a function of the z16 ground**, which is why it needs no
aggregation of its own. What holds it there is that the leaves are resident
wherever it reaches: z16 is held while any ground within 512 u of the view
center is in view, which is how far the leaves' ring draws (below), and an
assert holds `96 · max_extent` inside those 512 u.

## Rings, and the window

What the eye takes in is the same per unit of screen whether it is looking near
or far, and the arrangement below is that carried into the data. Holding a
cell's screen angle fixed means its size grows with distance — `cell ∝ r`, which
is the ladder. Count what that leaves in a band from `r` to `2r`: the band's
area goes as `r · dr` and a cell's as `r²`, so the cells in it are `dr/r`, and
an octave integrates to `ln 2` whichever octave it is. In three dimensions the
shell goes as `r² · dr` and a cell as `r³`, and the answer is the same. **Every
octave of distance costs one constant** — which is the counting behind the
complexity below, and the reason a zoom holds a fixed number of pages rather
than a radius.

Each zoom keeps a ring of pages around the view: 5×5 held, 3×3 rendered, which
guarantees one page of radius at that zoom. Geometry clipmaps are the precedent,
and `terrain/fixed.rs` already calls its single range "the way a clipmap
addresses its ring" — this is that arrangement at more than one zoom.

A zoom is drawn one step coarser than its own cells: its cells averaged 2×2, so
z draws step `2^(17 − z)`, and only the leaves are also drawn as they are, for
step 1 ([below](#a-zoom-is-drawn-one-step-coarser) says why). The ladder and the
page widths both double, so their ratio does not:

```text
step 2^(17 − z) is wanted out to  384 · 2^(16 − z)
ring z draws past the view        512 · 2^(16 − z)
margin                            512 / 384 = 1.33, at every zoom
```

Two things follow. The zoom a chunk wants is resident with the same slack at
every zoom. And **the client always holds data one zoom finer than it draws**,
by construction, since what it draws is that data averaged.

Chebyshev distance, not Euclidean: a square ring under a radial criterion wastes
its corners, and the corner of a ring is 1.41× its edge. Measured as L∞ the ring
_is_ the zoom's domain. The tier criterion stays Euclidean from the eye, and the
two do not fight: a chunk at a ring's corner is further from the eye than one at
its edge and wants a coarser step, which the next ring out supplies.

Zooms are not all resident at once. Only the zooms a screen shows simultaneously
are, and that count is the projection's, not the world's:

```text
W = log2( sin(θ + 25°) / sin(θ − 25°) )     50° vertical field of view
```

| tilt θ | ratio | octaves  |
| ------ | ----- | -------- |
| 90°    | 1.10  | 0.14     |
| 60°    | 1.74  | 0.80     |
| 30°    | 9.40  | 3.23     |
| 27°    | 22.6  | 4.50     |
| ≤ 25°  | ∞     | diverges |

At 25° the top ray grazes the horizon. So `W = 5` covers everything down to a
tilt of 27°, and below that how far to draw is a choice rather than a
measurement — the coarsest ring is where the haze is. The window slides with the
camera instead of growing: its finest zoom is

```text
r_near = d · sinθ / sin(θ + 25°)
z_fine = min( 16, 16 − ⌈ log2(r_near / 192) ⌉ + 1 )
```

one finer because a zoom draws the step coarser than its own, clamped at z16
because terrain has nothing finer, and held at z16 while any ground inside the
leaves' drawn ring is in view, which is where decoration reaches. Memory is then
constant in the camera's distance. `.fixed` holds a group of records per zoom
modulo the window's width, so a zoom entering the window takes the group of the
one that left — the layout stays settled at compile time.

Its coarse end is the zoom the farthest ground wants, and at least the zoom
whose ring covers all the ground in view. The slack is only a third, and a wide
screen's upper corners see further than its middle, so the first can fall short.
The ground in view is measured from the view center at the frustum's four
corners on the ground — a frustum meets a plane in a convex footprint, so its
farthest point is a corner — or, where a corner meets no ground before fog, on
fog's circle around the eye's foot as far round as that corner's bearing. The
half of that circle behind the eye is not in view, and counting it would hold a
tilted view at the farthest distance a zoom too coarse: z13, for ground that
reaches 1,100 u.

The camera's orbit runs from `MIN_DIST` 2 u to `MAX_DIST` 1,024 u, which at a
1080-pixel viewport is 0.0017 to 0.88 units a pixel, a 256-pixel tile's z24.2
down to z15.2: a pixel is never larger than a leaf cell, so every zoom coarser
than z16 on screen is the ladder's coarsening rather than the screen's
resolution. The cap is not arbitrary: at `MAX_DIST` one terrain cell covers 1.13
pixels, so pulling back sits exactly where the data runs out of resolution,
which is the condition a coarser zoom answers.

Every zoom holds its ring whole, the middle the finer ring covers included. A
clipmap renders hollow rings but keeps every ring's data whole, and for the
reason that holds here: the middle is what is drawn where the finer page under
it has not come, so the ground coarsens there rather than opening. Leaving it
out saves one page of the twenty-five or four, as where the view stands in its
page decides, and `.fixed` has a record for each anyway.

Three rules fall out for the stream:

- **Coarse first.** A coarse record is the fallback a fine one refines, so it is
  worth more early. Every page drawn from comes before any rim page, and within
  each the order is by zoom, coarsest first, then by distance, which is what
  progressive refinement means.
- **A fine page is dropped only once something else draws its ground** — the
  page over it, or the four under it. Per zoom one ring decides fetch, keep and
  record together, so a page cannot be evicted from under its own request;
  across zooms this is the rule that replaces a hole with a blurrier answer. A
  page kept past its ring goes when another page takes its record.
- **A coarse request is not canceled by moving closer.** It is small, it stays
  valid far longer than a fine one, and pulling back again draws it first. It is
  kept while it runs, and only while its zoom's record group is one the window
  leaves free: arriving, it would otherwise take a held page's record, and a
  page that does arrive into a held record is dropped. A pan keeps a running
  request the same way, one page past the ring; a jump withdraws.

`RENDER_DIST` dissolves into this. It is a link in a chain the camera's own
`MAX_DIST` anchors — geometry has to exist as far as fog can see — and that
chain was fixed at the bottom because one resolution had to cover the whole of
it. With rings, the coarsest one is the edge, and fog becomes a choice rather
than a cover.

## What it costs

`R` is the reach, `P` the page span, `A` the ring's side in pages, `N` the leaf
page count.

|                  | z16 alone | window of `W`                   |
| ---------------- | --------- | ------------------------------- |
| resident space   | **Θ(R²)** | **Θ(P²A²)** — constant in reach |
| build per frame  | **Θ(R²)** | **Θ(1)**                        |
| cold fetch       | Θ(R²/P²)  | Θ(1)                            |
| bake and storage | Θ(N)      | Θ(N) · 4/3                      |

Space is quadratic with z16 alone because every page is the same size and the
held set grows with the radius: doubling the reach costs four times the memory.
A ring holds a fixed page count and the reach is exponential in the zoom, so the
total is the window's width and nothing else.

The frame cost is the same argument from the other side. A chunk is 64 cells at
every zoom, so it covers `(64 · 2^(16 − z))²` of ground, and the ladder picks
`z` to keep a chunk's projected area roughly constant. The count on screen is
therefore constant. With z16 alone a chunk was 64 units wide at every tier, so a
wide view put a thousand of them on screen at ~3.1 ms a scan
([20260912-coarse-tiers-need-a-larger-unit.md](./20260912-coarse-tiers-need-a-larger-unit.md)),
and that cost read as constant only because `RENDER_DIST` capped `R`.

Panning is the one that looks like it should get worse and does not. Moving `δ`
crosses `δ / (P · 2^(16 − z))` page columns at z, each `A` pages of `P²` cells:

```text
at z        A · P · δ / 2^(16 − z)  cells
all zooms   A · P · δ · Σ 2^(z − 16)  =  2 · A · P · δ
```

The series converges at twice the base ring, so five zooms cost less than z16
alone did in 1,024-cell pages: `2 × 5 × 512 = 5,120·δ` against
`7 × 1,024 = 7,168·δ`, 29% less.

Storage is the only axis that grows, and it grows by a constant. Folding terrain
v4's 167,499 land pages gives 43,681 at z15, 11,646 at z14, 3,172 at z13, 902 at
z12 and 274 at z11, down to one page at z6 — **227,309 in total, 1.357× the
leaves**. The fan-in is 3.83 rather than 4 because a coarse page with one land
child is still a page, and it falls to 3.29 by z11 as the coverage thins against
the coarse cell. Bytes are not the same ratio: a coarse page is denser in
content per cell than a leaf. The leaves measure 1.1 KB at the minimum, 6.6 KB
at the median and 62.4 KB at the maximum, gzipped, for 1.30 GB, and the zooms
above add 0.81 GB, 62% more. A page's bytes are bounded above by the densest
leaf's, since the format and the decoded size are the same at every zoom. R2's
free tier is 10 GB and the binding ceiling is the Workers plan's 100k requests a
day ([20260816-deployment-hosting.md](../20260816-deployment-hosting.md)), so
the storage is not the constraint; the request count is, and a smaller fine ring
lowers it.

## The fold

One rule, and every channel runs it:

```text
parent cell = the type that fills at least half the four children
              a tie, or no majority, takes the nearest child
              (the sample alternates (0,0)/(1,1) by zoom, or it drifts
               half a coarse cell toward the origin)
```

It is Earth Engine's `MODE` pyramiding policy with one change. Earth Engine
builds each zoom from 2×2 pixels of the one finer and takes their most frequent
value, or the upper-left pixel when there is none — which is this rule for three
agreeing, for two against one and one, and for a tie (Importing Raster Data,
developers.google.com/earth-engine/guides/image_upload). The change is the tie's
corner: always the upper-left moves every tie the same way and the map drifts,
so the corner alternates. Earth Engine weights by the mask and a masked pixel
does not vote; here nothing is masked, and a channel's VOID votes like a type,
or one road cell in three empty ones would be road.

The rule is an approximation above z15, as Earth Engine's is. A mode is
holistic: nothing of fixed size summarizes a block so that the mode of the union
follows. A mode of modes therefore sees each child as wholly its winner, and a
line one leaf wide that wins the tie at z15 is half of a z14 cell rather than
the quarter it is, and may win again; in the worst shape, three quarters of
three quarters, a type filling 27 of 64 leaves takes a z13 cell. "Narrower than
half a cell rounds away" holds of the zoom one finer, not of the leaves.
Contiguous features only differ from exact at their edges, which is where the
error stays.

No channel knows it is a channel. There is no per-type table, no rank, no
threshold, and no split between area features and linear ones. That is the rule
the fold is held to:

> A channel is a list with an order. Every other difference is a column of a
> type. Neither the fold nor the ladder asks which channel it is looking at.

The fold and the ladder keep it. The paint does not, in four places, and they
are worth telling apart because only one is a problem. `LAND_BASE_TYPE` gives
the land channel a fallback the others have none of; `show_structures` skips one
channel for a dev toggle; `sea` is a land value with its high bit set so a
channel stays one byte. All three are meant to be there, and each becomes a
column of the catalog rather than a branch when it is worth the move. The fourth
is `WATER_RECESS_CELLS`, and its own neighbor says why it is not really a
channel's business: `is_water_family` checks `land.sea` separately "because
cube.rs models it as land.sea", so water-family already crosses the channel
boundary and is a property of the type.

The composite does not vary with the zoom either. The channels are painted in
one order, a later one's non-void type winning, at every zoom — so the code that
draws z16 draws z10 unchanged. What is missing at a coarse zoom is missing
because its cells lost the majority, not because anything switched a channel
off, and `shown_ground` already steps over a channel that reads void.

**What disappears is a consequence, not a policy.** A feature of width `w` falls
under half a cell when `c > 2w`, and `shown_ground` already skips a channel that
reads void, so the layer beneath simply shows:

A building is an area and a road is a line, so the half falls at a different
width for each: a building of extent `b` covers `(b/c)²` of the cell it sits in,
a road of width `w` covers `w/c` of every cell it runs through.

|           | extent or width | gone at |
| --------- | --------------- | ------- |
| building  | ~15 m = 6.3 u   | z12     |
| road      | ~12 m = 5.0 u   | z12     |
| river     | ~100 m = 42 u   | z9      |
| lake, sea | km              | never   |
| land use  | covers the cell | never   |

**Nothing in the rule orders one layer against another.** Which goes first is
whichever is smaller against the cell, feature by feature, and the widths above
are representative rather than a rule — swap the two figures and the two rows
swap with them. At these sizes both survive 8 u and neither survives 16, so they
go at the same zoom. An explicit order was written down while this was being
worked out — buildings, then roads, then water, then land — and then dropped: an
order per layer is a policy to keep in step, and the fold answers the question
without one.

Nor does a layer switch off at a zoom, because its features are not one size. An
8 m house goes at z13 and a 40 m warehouse at z11; a 6 m alley goes at z13 and a
30 m motorway at z11. So `structure` and `transport` thin across the same zooms
and interleave while they do it, and each row above is one feature of that size
rather than a channel. The land channel carries `residential`, `commercial`,
`industrial`, `education`, `medical` and `public`, so a city that has lost both
still reads as a city. Past z12 the world is terrain, water and land use — which
is what a terrain view at that scale should be, and not what a map at that scale
is.

Elevation folds by box average, and that is forced rather than chosen. A
column's occupied volume is `Σ (h_i − floor)`, which is linear in height, so the
mean is what preserves it; folding by maximum would inflate the volume and grow
the mountains. The average rounds to even — the project's `round` already is
`round_half_even`, so that a value the CPU and a shader round lands on one
integer — and here it earns its place twice over: the mean of four integers is a
half exactly a quarter of the time, so rounding half up would drift `+0.125` u
per fold and about `+0.75` u over six.

The elevation lane stays in units at every zoom, not in the zoom's own cells.
Store it in coarse cells and the rounding eats the fraction; keep it in units
and an `i16` still covers ±78 km.

That fraction is kept by drawing the height in units at every zoom. Only a
column's footprint coarsens: a coarse page's column, and a coarse tier's, is a
run from the floor to the mean height of the ground under it, in units, so its
volume is the ground's and nothing is a coarse cell part filled. A part cell was
first drawn as a cube inset by the cube root of its fill, the way the coarse
pack draws a sparse cell of objects, and that is right for a volume and wrong
for a height field: the ground covers the whole cell and is only thin, and a
cube of the same volume leaves a gap on every side. Where the ground is thin
against a coarse cell — anywhere flat, a few zooms past z16 — the view turned to
cubes standing apart, and each cube, merging with nothing and hiding no face,
cost a mesh four times its fine tier's.

The cost of the rule is that a road has no thin-and-connected regime. A fold
that takes the maximum keeps a line one cell wide far longer, which is what a
map wants; half-or-nothing drops it at once. This project is not a map.

## A zoom is drawn one step coarser

z10 is not shown as the z10 page but as z11 averaged. Step `2^(17 − z)` is
always z's cells averaged 2×2 — a coarse tier of the mesher, which averages
color, volume, objects and textures alike, as every coarse tier did before there
was a pyramid — and only the leaves are also drawn as they are, for step 1.

A page is data, and a view is a picture of it. The fold quantizes: a vote is
what keeps a zoom a function of the one finer, and a vote is what makes a thin
thing vanish at one zoom or swell at the next. Averaged one zoom down, what
reaches the screen is what a pixel of that ground would show — a road a tint
rather than a cell or nothing — and the vote's quantization sits under a mean,
where it reads as texture.

Drawn at its own cells, a zoom would also disagree with the one finer. It would
show step `2^(17 − z)` as the vote, where the zoom finer shows the same step as
the mean of the same four cells, and every zoom's edge would change color. With
one rule each step is drawn one way, and a split or a merge moves between
adjacent steps drawn from adjacent zooms, each the mean of its own.

What it costs is the ring's slack, which drops from 2.67 to 1.33, and the
window, which moves one zoom finer
([Rings, and the window](#rings-and-the-window)). Elevation is the one thing
that agreed either way: it folds by mean, and a mean of means is the mean.

## Objects fold too, one dimension up

A placed cube is a real thing with a real size, so its address does not move
with the view: its column stays the z16 64-unit column, and a chunk at z asks
the `4^(16 − z)` columns and `2^(16 − z)` bands under it — `8^(16 − z)` z16
bands — what they hold. What does move with the zoom is the same fold, with one
more axis:

```text
terrain   2×2   → 1      half of four children
objects   2×2×2 → 1      half of eight
```

And the pyramid is already there. `sparse_tree` runs from its finest entry to a
band's root, so its internal nodes _are_ the coarse zooms: nothing has to be
built, only decided — what an internal node reads as when its children disagree,
which is the rule above. Past a band, 2×2×2 bands make one, and that is the same
containment.

Being local is what lets this work on a layer that changes. A terrain fold is
offline; an object fold has to survive an edit, and a 2×2×2 fold means one write
invalidates its own ancestors and nothing else — six steps to the band's root,
then one a zoom — against re-baking a page. The unit it is invalidated by is the
change log's `(topic, key)`, which is already there.

What the rule has to get right is what a coarse zoom should show, and it does. A
lone 1 u cube is an eighth of a z15 cell and goes at the first fold; a wall or a
tower keeps whole cells and stays. **Scattered blocks go and built things
stay**, decided by size, with nothing saying objects stop at a zoom.

The composite gains one step and loses nothing — `land`, `water`, `transport`,
`structure`, then objects — which is the order the chunk build already paints
in: terrain into the volume, objects over it, an `AIR` entry carving it.

One thing does not merge, and it is the honest kind of difference. Terrain is a
height field, so a coarse cell of it is the mean height, a whole run; objects
are a volume, so a coarse cell of them is `accumulate_run`'s coverage summed,
which `cbrt` turns into an inset. The coarse pack takes the ground first and
stacks what objects add above its top.

The cost is storage rather than rules. A `sparse_tree` slot is a branch pointer
when its high bit is set and a leaf value when it is not, so it cannot be both:
an internal node carrying the majority of its children wants a lane of its own.
And `col_idx = L2_IDX(...)` keys the render chunk and the object column with one
number today, which a chunk carrying its zoom separates.

**Decoration is what does not fold.** It has no address at all — a pure function
of the z16 ground, a seed and the object layer's occupancy — and the extent
argument above keeps it inside the innermost ring. It is a consumer of the unit
system rather than a participant in it.

## The bake

Bake z16 from PostGIS once, as today, and fold upward — z15 from z16, z14 from
z15 — to z6. No query runs above the leaves.

Building incrementally is what makes the fold rule free. The bake and the client
compose the same one-zoom function the same number of times, so any
deterministic rule agrees between them; a rule would have to be a homomorphism
only if some zoom were produced by a different route, which is what
re-rasterizing per zoom would have been. Verification follows from the same
fact: a parent must equal the fold of its children **byte for byte**, and the
check is Rust's (`pxl-geo-bake terrain/pyramid.test.rs`), since the model has no
TypeScript twin to hold it to.

One asymmetry survives. Ground folds from the 2×2 block alone. Elevation does
not: a parent's two-sample skirt reaches 64 child cells past the block and a
child carries only 32, so the fold reads a rim from the neighbors. Offline that
is a window over a sorted key space. In the client it means a page is folded
only where the sixteen pages around its children are held, not just the four
under it — so pulling back is served without the network where the held rings
reach that far, and moving closer never is.

## Roads and buildings

The channels paint in a fixed order and a later one's non-void type wins. That
order put transport last, so a road painted over a building. It is the other way
now.

The two overlap on 1.30% of cells in the densest page and 0.72% in the next,
which is 4.8% of building cells lost to roads under the old order, or 4.4% of
road cells lost to buildings under the new. By area it is a wash. It is not a
wash by shape: a line that loses its rim is still a line, and connectivity is
topological, while a small building that loses its street-facing cells can stop
existing. A city's buildings almost all face a road, so the present order erases
the smallest of them first.

```text
[land, water, transport, structure]      structure paints last
```

The order was written in three places — the paint loop's index constants in
`mesh/paint/terrain/adapter.rs`, `GROUND_CH_KEYS` in `bake/format.rs`, and
`is_water_family` in `mesh/paint/terrain/elevation.rs`, whose own comment said
it "mirrors the terrain paint's priority". Three copies is how they drift, so
the precedence is one array, `GROUND_CH_KEYS`, that the paint reads.

The same array is also the storage order — the wire format's lanes and the
`.fixed` planes. Keeping two arrays would buy the freedom to reorder precedence
without re-baking, and that freedom is worth nothing: adding or moving a channel
is a format change either way. So there is one array, and changing it is a
re-bake.

## Roads not taken

**Re-rasterizing each zoom from the source.** Proposed to keep roads alive: draw
the features again at the coarse scale with the bake's existing rank order and
its `round(width_cell * 0.5).max(1.0)` floor, which is a minimum width rule
already. It would have kept a road network to continental scales, the way a
vector tile pipeline's rank filter does. It died because a parent would then not
be a function of its children: the client could not derive a coarse page from
fine ones, and the byte-diff check would fail by construction. It also assumed
this project wants what a map wants.

**Requiring the fold to be a homomorphism.** With re-rasterizing gone, a
thresholded maximum by rank was proposed instead, because `threshold(max(·))`
composes and so a fold over several zooms is the one-zoom fold composed exactly.
The proof holds, and the requirement turned out to be unnecessary: once the bake
is incremental, both sides compose the same function and only determinism is
needed.

**A per-type table of zooms.** Measured by folding real pages with a per-type OR
and finding where each type's coverage passes half — building at z15,
`road_default` at z13, `pedestrian` z12, `parking` z11, `road_secondary` z10,
`road_primary`, `road_motorway` and `rail_main` past z8. The hierarchy is the
one cartographic generalization produces, and the measurement was an artifact of
the rule being measured: a maximum spreads a type, which is why buildings
saturated at z15. Under half-or-nothing nothing spreads, and the table is not
needed.

**Majority against nearest-neighbor for the land channel.** Both were run over
real pages. Area error after six folds: 2.23% for nearest-neighbor, 4.89% for
majority, with the two disagreeing on where they put things by up to 36.7%. A
first run had majority at 57.76% because ties were broken toward the smaller
type id, which favors `bare`; broken toward the nearest child it is a wash. Area
fidelity does not choose between them, so the tie rule is what survived into the
rule above.

**Ties to the background.** Sending a tie to VOID (round half down, the strict
majority of ArcGIS's Majority Filter) ends the one-leaf line at z15, but does
not make the mode exact — the strict majority compounds too, to 27/64 at z13 —
has no background to go to in the land channel, whose every cell is some type,
and leans every channel toward empty on top of the bias a majority already has
against rare types. The alternating corner keeps what Earth Engine does.

**Counting instead of voting.** Per-type counts sum, so carried through the
pyramid they make the mode exact at every zoom. Carried in the page they are 32
types a cell and 4^10 leaves deep, four bytes each, which is 32 MiB a page
against the record's one, and a coverage lane per channel alone would add 500
MiB to `.fixed` at u32. Carried in the bake only, the page is unchanged but a
parent stops being a function of its children, and the client could fold z15 and
no further. Neither is worth an exactness a view at that distance does not show.

**Vector tiles.** The basemap world moved to them in 2013 and kept raster only
for fields — imagery, and elevation encoded into a PNG's channels. The split is
not old against new: a field has a value everywhere and a feature set does not.
Terrain is a field, so it is raster for the same reason Terrain-RGB is. What
makes the case one-sided here is the second half: `bake_ground` fills polygons
even-odd with holes and draws lines through a thickness kernel, and
`ARCHITECTURE.md` calls rasterizing "the bulk of" the bake. Handing the client
vectors moves that cost onto the phone at every page and every zoom, and with it
the geometry the bake absorbs — the clip that must preserve ring closure is
already a deliberate omission naming Weiler–Atherton by name. One page of
Overture buildings is about 96 KB of GeoJSON against a 28.6 KB mean for the
whole baked page, and cells are 2.39 m, so the vectors carry precision the world
cannot draw. Rasterizing _is_ the generalization, done once.

**A channel packing.** The four channels are four bytes a cell, and the distinct
combinations in the densest page number 108 — under a byte, at 4.28 bits of
entropy against a median page's 0.88. A palette index a cell would take the
record from 4.01 MiB to 1.01 MiB and `.fixed` from 196 to 49 MiB, and the four
channels of one cell sit `GROUND_PLANE_BYTES` apart today. It is left out on
purpose: it is a question about what is inside the record, and this document is
about what the record is named and how it composes.

## What this does not do

- It does not switch projection. The globe is a vertex transform over the same
  tiles — Mapbox reprojects the same Web Mercator tiles on the client and drapes
  raster ones over a mesh — not a different data model.
- It does not change what a record holds, only which record exists.
- It does not touch POI, which shares L3 and so re-bakes with the page size, but
  whose own generalization is its `rank` lane and a separate question.

## What the session turned up

Findings from reading, not decisions:

- `decision/README.md` indexed 71 of the 72 documents here; the one it left out
  was [the page pyramid](./20260921-the-page-pyramid.md), committed this
  morning. It is indexed now, with this document and its sibling.
- `spec/…/environment/config.json` says the held set is "the nearest 32 (~129MB
  decoded), against the 49 pages (~197MB) that discarded the tab on an iPhone 13
  mini". `terrain/fixed.rs` derives 49 records and 196.43 MiB from `KEEP_DIST`,
  which is what is held.
- `KEEP_DIST` is 3,072 u and the compile-time assert it answers to needs 1,920.
  A whole number of pages above that is 2,048, which is a 5×5 range of 25
  records and 100 MiB rather than 49 and 196. Nothing records why the third ring
  is there; the likely reason is fetch margin on a fast pan, which is
  measurable.
- `camera/config.json` puts the scale bar at `0.21 · dist` and points at
  `scale_bar.tsx`. The reading is `0.092 · dist`, the ratio is the bar's width
  going 128 px to 56, and the computation now lives in the kernel's
  `shell/ui/scale_bar.rs` while the `.tsx` only draws it.
- `bake/format.rs` opens with "each ground channel a sparse tree, encoded as
  words". `encode_terrain` writes four dense `u8` arrays.
- `WATER_RECESS_CELLS` is one cell, applied after the fold at the shared
  elevation query. Read as one unit it needs no zoom-dependent handling — it is
  1/16 of a cell at z12 and quantizes away — but the name says cells, which
  stops being unambiguous once cells come in more than one size.

## Where this stands

Updated 2026-09-22. Built:

- **The bake.** Terrain v4 is 512-cell pages with the pyramid folded up from
  them to z6, the channels in one order, the elevation in units. The fold is
  pxl-spec's and a parent exists exactly where one of its children does. The dev
  tile server demand-pages the same functions, so what it serves unpacks to what
  the bake writes, at every zoom.
- **The rings and the window.** The client holds a 5 × 5 ring of every zoom of
  its window and draws from the inner 3 × 3, fetched and kept by the three rules
  above. `.fixed` is 125 records, a group a zoom, addressed by zoom modulo five.
  The window runs one zoom finer than the steps the ground in view wants, its
  coarse end at least the zoom whose ring covers that ground and its fine end at
  z16 while decoration can reach any of it: looking down at a street it is the
  leaves alone, and five is only its widest.
- **Chunks at every zoom.** A chunk is 64 cells of its zoom. The mount pass
  walks from the coarsest zoom's drawn ring inward, drawing a chunk whole or as
  its four children where it lies whole inside the ring the zoom finer draws and
  the page under it has come. The rings nest as a clipmap's do, so a zoom's
  chunks lie inside its own drawn ring and nowhere else, whatever the eye does.
  A coarsest page not yet here is a hole until it comes, and it comes first. A
  coarse chunk's runs are its page's columns, `2^(16 − z)` units across and as
  tall as the elevation in units, and the objects folded to its zoom over them,
  which its coarse tier then averages 2×2
  ([A zoom is drawn one step coarser](#a-zoom-is-drawn-one-step-coarser)).
- **The objects fold.** By an eight-way majority, the terrain's rule one
  dimension up, from the finest entry to the chunk's zoom, empty and air counted
  as states.

Decided while building, on top of the above:

- **Only the leaves have two tiers.** A z16 chunk builds step 1 and step 2, and
  the existing dither blends between them; a coarser chunk builds only its
  coarse tier, so every handoff above step 2 is a split or a merge, and it falls
  at the edge of a ring rather than at a distance from the eye. z starts at
  least a page of z + 1 past the anchor's page, `256 · 2^(16 − z)` units, where
  the ladder would have handed off at `192 · 2^(16 − z)`: the ground is never
  coarser than the ladder asks. A split by the eye's distance put a zoom's
  chunks wherever the eye's reach did, past its ring, and the mesh slots below
  need them inside it.
- **A mesh lives on the device, in a slot of its own.** The renderer keeps a
  fixed storage buffer a group, and a chunk's tier is a slot in it by the
  chunk's coordinate modulo its drawn ring's 24 chunks — `.fixed.terrain`'s
  addressing, one step down. A bake writes into `.fixed.bake`, 1 MiB, and is
  sent to its slot when it is done, cut to the slot's capacity. A slot is taken
  over only once the chunk holding it is no longer drawn, so a mesh outlives its
  page's record and there is nothing to free, and no mesh is in the module's
  memory.

  The numbers moved twice after this was written, both under
  [a coarse cell is isotropic](./20260923-a-coarse-cell-is-isotropic.md). There
  are six groups, not five: a group is a zoom of the window modulo five, and the
  leaves' own tier has one to itself, because a group is one storage binding and
  with both in one what either could hold was what the other left. Measured
  under the isotropic rule, a slot holds 2,730 boxes for a leaf's own tier and
  4,551 for every other. And a box is not one size: a leaf's is six words and
  every other four, since a box that merges cells carries no texture taps and is
  never drawn near enough to need a leaf's resolution across a cell. 236 MiB of
  the device, where the same slots at one 36-byte box would be 529.
- **A chunk's runs last as long as its bakes.** They are one of two sets of
  `.fixed.runs`, 1 MiB each, one filled while the other is baked from; every
  tier the chunk has is baked while its set is there, then the set is let go.
  With the tiers kept on the device, runs kept for a tier asked later were a
  cache of a cache, and the heap's largest tenant.
- **Nothing is built from a record that has gone.** When a record goes — let go
  by the eviction above, or taken by another page arriving at its residue —
  every chunk of that page goes stale at the start of the next tick and its scan
  and bake are given up, before anything can build one: a build would read
  another page's bytes. What it has built stays drawn until something covers its
  ground, as any stale chunk does; letting it go with the record opened a hole
  wherever nothing else was ready.
- **A split or a merge is never a hole.** A chunk no longer wanted is stale,
  drawn until every chunk replacing it has a layout; the replacements wait under
  it. One rule serves both directions and the view moving on.
- **The object fold is made, not kept.** It is the same function whether held in
  the band trees' internal nodes or computed when a coarse chunk is built, so it
  is computed, with a coarse chunk's revision the sum of the columns under it.
  Holding it is a cache, for when edits or counts make the computation show.
- **The raycast reads the leaves.** It composes the leaf at a position from the
  resident leaf page whether or not a leaf chunk is drawn there, so the pointer
  reaches ground drawn at any zoom wherever its leaf is held.
- **The minimap reads z12.** A minimap pixel is 16 units, which is a z12 cell,
  so the minimap draws that zoom's page a cell a pixel instead of every
  sixteenth leaf cell.
- **The client folds what it holds.** A page whose sixteen pages a zoom finer
  are all resident is folded by the client with the bake's fold rather than
  fetched, a zoom at a time from the finest; the leaves a pull-back has just
  left the window with are what z15 is made of. A page it cannot make is one
  fetch of the coarse page, never the fine pages gathered to make it — a page at
  z stands for `4^(16 − z)` leaves. The pages it makes are the bake's byte for
  byte: 606 parents across z15 to z12 of terrain v4, 150 of them beside ocean,
  folded from their sixteen as the client reads them.
- **Fog follows the camera.** The band, the clip planes and the sky grew with
  the camera's distance past 1,024 u of its pivot, `FOG_PER_DIST` = 1.5 of it.
  Superseded the same week by
  [the ground runs to the horizon](./20260923-the-ground-runs-to-the-horizon.md):
  the reach is no longer a number tuned against the distance but the nearer of
  the planet's own horizon and the distance the haze takes the ground, both read
  off the camera's height, and the band and the planes are read off that. What
  is unchanged is the consequence — the window and the rings follow the reach
  without being told.
- **The camera reaches the pyramid's top.** `MAX_DIST` is 2^18 u, where a view
  straight down wants step 2^11, which z6 draws. The on-demand dev tile server
  makes nothing coarser than `--top-z` (z13 unless told), answering 503 rather
  than the 404 the client would paint as sea, since a z6 page is a million
  leaves; `deno task dev:baked` serves the finished bake instead, every zoom at
  once.
