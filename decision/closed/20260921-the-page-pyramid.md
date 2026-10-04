# The page pyramid: coarse pages the bake writes and the client holds

**2026-09-21** — a design note, and nothing here is built. The page is 4 MB
decoded whether it lies under the camera or at the fog line, and every page
the range reaches is held at that size. The proposal is a pyramid over the
pages — a level-`k` page covering `2^k × 2^k` of them at `1/2^k` the
resolution, in the same format — so that what is far is held coarse, what is
first shown is shown before the fine pages arrive, and the coarse tiers the
client folds today are folded once, by the bake.

## What a page costs today

- **Decoded: 4,203,672 bytes**, the four ground planes at `1024²` bytes each
  plus the padded elevation at `68² × 2`. Compressed it travels at ~163 KB
  (v3, gzip; brotli would take a third off —
  [20260913-compression-by-traffic.md](../20260913-compression-by-traffic.md)),
  so the wire is not the cost. The decoded footprint is.
- **Held: 49 records, 196.43 MiB** (`terrain/fixed.rs`): `KEEP_DIST` is
  3 pages, the axis 7, and every record is a full page. That is the number
  behind `MIN_HEAP_BYTES` and the phone's 27 MiB of headroom
  ([mem.spec.rs](../../crates/pxl/web/kernel/src/mem/mem.spec.rs)), and it is
  why `RENDER_DIST` stopped at 1,536 u — 49 pages discarded the tab on an
  iPhone 13 mini
  ([20260912-what-to-render-when.md](../20260912-what-to-render-when.md)).
- **Reached: 3 pages, ~6 km on the ground.** Past `KEEP_DIST` there is no
  terrain at any resolution; the fog is what ends the view.
- **Folded on the client, every time.** A chunk at step 16 is scanned from
  the same 64×64 columns as a chunk at step 1, at ~3.1 ms a scan whatever
  the tier, and a wide zoom shows a thousand of them
  ([20260912-coarse-tiers-need-a-larger-unit.md](./20260912-coarse-tiers-need-a-larger-unit.md)).
  The remedy settled there — a unit of `s` chunks on a side at step `s` —
  still reads fine cells and folds them at build time.

So three things pay for the flat page: the phone's memory, the reach of the
view, and the coarse tiers' build cost. They are one thing: resolution is not
a function of distance anywhere between the bake and the mesher.

## The shape

A quadtree over pages, with the page as its leaf and the same record at every
level.

```
level 0    ┌──┬──┬──┬──┐   one page, 1,024 cells, 1 cell = 1 cell
           ├──┼──┼──┼──┤
           ├──┼──┼──┼──┤
           └──┴──┴──┴──┘
level 1    ┌─────┬─────┐   four pages, 1,024 coarse cells, 1 = 2×2 fine
           ├─────┼─────┤
           └─────┴─────┘
level 2    ┌───────────┐   sixteen pages, 1,024 coarse cells, 1 = 4×4 fine
           └───────────┘
```

A level-`k` page is 4,203,672 bytes like any other: four `1024²` planes and a
`68²` elevation, over `2^k` the span. Its coordinate is the fine coordinate
of its south-west page shifted right by `k` on both axes, and its key is that
coordinate packed the way a page's is (`X<<40 | Y<<16 | Z`, `Z` still zero);
the level goes in the path, `terrain/v4/{level}/{key}`, as a version does —
`route.json` folds a branch into one constant and the level is a segment of
it, not a bit stolen from a key whose `Z` is reserved for a meaning it does
not have yet.

**Elevation needs no new rule.** The `68²` grid is already 16 cells a
sample at level 0 with a two-sample skirt for bicubic
([constant.rs](../../crates/pxl/spec/src/bake/constant.rs)); at level `k` it is
`16 · 2^k` cells a sample, under the page's own center-sample convention. A
level-0 sample is 16 u, about 30 m on the ground at this latitude, which is
the 30 m source's own resolution: level 0 already tracks the source, and
every level above it is a true reduction rather than a resampling.

**Ground is a fold, and the fold already has a definition.** The client's
coarse tier defines a coarse cell as the fold of every column under it, and
that is the definition the bake takes: a level-`k` cell of a plane is the
fold of the `2^k × 2^k` fine cells under it, by the same rule the mesher
applies at step `2^k`, so that a coarse page and a coarse tier built from
fine pages say the same thing. Which rule that is — the priority order among
types when they disagree, whether `void` yields to anything — is the one
decision in this note that has to be settled before a byte is written, and
it is settled by reading `chunk_mesher.rs` rather than by choosing afresh:
the fold exists, it is only being moved.

**The ocean aliases at every level.** A coarse page that covers only open
water is bit-identical to any other, as the fine `_ocean` page is
([tile_repository.rs](../../crates/pxl/web/kernel/src/layer/terrain/tile_repository.rs)),
and a 404 under `{level}/` means the same thing it means under `v3/`.

## What the bake does

The pyramid is built from the pages, not from the database: a level-`k` page
is four level-`k-1` pages folded, which is a pass over `out/terrain/v4/0/`
and no PostGIS query. 43,506 land pages fold to at least a quarter as many
at level 1 and somewhat more in practice, since a coarse page with one land
child is a page; the pyramid to the coarsest useful level is at most a third
again of the leaf bake's bytes and a fraction of its time — the leaf
bake is the 32 minutes ([BACKLOG.md](../BACKLOG.md)), and a fold of 43,506
pages is I/O. A page missing at level `k-1` — open water — folds as the
ocean page.

`script/decode-tile.ts` reads a level-`k` page as it reads a leaf, since the
format is the same; what it would gain is a `--level` and a check that a
coarse page is the fold of its children, which is the byte-diff the pipeline
already uses between its two implementations, one level up.

## What the client holds

A ring a level: the arrangement of a geometry clipmap, which is what
`fixed.rs` already is at one level (its own words: "the way a clipmap
addresses its ring"). Level 0 keeps the pages within its `KEEP_DIST`; level 1
keeps the coarse pages within twice that, of which the inner ones lie under
level 0 and are held anyway, because a record is its own coordinate and a
coarse page is what the fine pages under it fall back to while they are on
their way; and so on up.

Four levels of the same 7×7 range are 196 records, four times today's
memory, and that is the wrong way to size it. The range is what `KEEP_DIST`
derives, and `KEEP_DIST` is what a _fine_ page is worth holding for; a level
above it wants its own, smaller radius in its own pages — the axis at level
`k` is `2·KEEP_PAGES_k + 1`, and `KEEP_PAGES_k` is a level's own number in
`terrain/config.json`, the way `KEEP_DIST` is the one number today. A
plausible setting, once a chunk can mount from any level (step 3 below):

| level | pages a record spans | records | held    | reaches         |
| ----- | -------------------- | ------- | ------- | --------------- |
| 0     | 1                    | 3×3 = 9 | 36 MiB  | 1 page, ~2 km   |
| 1     | 2×2                  | 3×3 = 9 | 36 MiB  | 2 pages, ~4 km  |
| 2     | 4×4                  | 3×3 = 9 | 36 MiB  | 4 pages, ~8 km  |
| 3     | 8×8                  | 3×3 = 9 | 36 MiB  | 8 pages, ~16 km |
|       |                      | **36**  | **144** |                 |

Against 49 records, 196 MiB and 3 pages today: less memory, nearly three
times the reach, and the far three quarters of it at a resolution the eye
cannot tell from fine at that distance. The `3×3` at level 0 is the number
for what the fine tier needs — `REFERENCE_THRESHOLD_0` hands step 1 off at
192 u on a 1080-pixel viewport, well inside one page — and it is the number
the first chunks mount from. It is not available before step 3: `fixed.rs`
asserts at compile time that `CHUNK_RENDER_DIST` plus the range hysteresis
(1,920 u) sits inside `KEEP_DIST`, because today every chunk mounts from a
level-0 page. Until a chunk past that distance can mount from level 1, level
0 stays at 5×5 (2 pages, 100 MiB) at the least, and the rings above it buy
reach rather than memory. Whether level 0 ends at 3×3 or 5×5 is a
measurement, not a decision, and `KEEP_DIST` is where it is made.

`.fixed.terrain` becomes one range a level, each placed once at boot and
sized at compile time as the one range is now; `slot_of` takes a level; the
fetch set is the union of the rings, each derived from its own radius the way
`range.rs` derives one today. Nothing about a record changes — it is the same
lanes at the same offsets — which is what makes the level a parameter rather
than a second store.

## What the mesher reads

This is where the pyramid and the coarse-tier unit meet, and it is the
reason to build one rather than the other. A chunk mounted from a level-`k`
record is 64 coarse cells on a side, which is `64 · 2^k` fine cells: the
"unit of `s` chunks on a side at step `s`" that the coarse-tiers note asks
for, with `s = 2^k`, and its scan is a scan of 64×64 columns like any
other — **the fold is already in the record**. The per-unit cost that note
wanted flat is flat because the unit never changes; what changes is which
record it reads from.

The tier ladder (`lod.rs`) keeps its steps; what it gains is that a step of
`2^k` or beyond is served from the level-`k` record where one is held, and
folded from level 0 only where the view is close enough that a level-0
record exists and no coarser one is wanted. Between two levels the seam is
the one `20260909-lod-look-and-seams.md` already draws between tiers, and the
dissolve `pixel_dither` does between steps does the same between records.

The elevation a chunk reads from a coarse record is coarser too, and that is
the one place the picture changes rather than only its cost: a hill under
level 2 is sampled at 64 cells and rises where a level-0 sample would have
had it rise a few cells over. Under fog at 8 km that is the intent; at the
level boundary it is a step in the ground, and it is the same step the
clipmap literature spends its blending on. The measurement before anything
is tuned is whether the ladder's existing handoffs put the boundary far
enough out that it is under the tier dissolve already.

## The order to build it in

1. **The bake, alone.** `terrain/v4` with `{level}/`, the fold rule taken
   from the mesher and written down in `format.rs`, `decode-tile.ts`
   checking a parent against its children. Published beside v3, read by
   nothing; measured for bytes and time. This is the step that can be
   wrong cheaply.
2. **The client holds the rings and reads only level 0.** The ranges, the
   rings, the fetch set — with every chunk still mounted from level 0, so
   that nothing drawn changes and what is measured is memory and traffic.
   `KEEP_DIST` shrinks here; that is a visible change and a separate commit.
3. **The mesher reads the record its step wants.** The coarse-tiers unit,
   delivered by the pyramid. This is the step that pays: the wide zoom's
   thousand scans become the count of coarse records on screen.
4. **The seam.** Only after 3, and only if it shows.

Each step leaves the previous one deployable, and 1 needs nothing from the
client at all.

## What this does not do

- It does not compress a page. A record is 4 MB at every level on purpose,
  so that one code path holds them; the plane that is `void` from edge to
  edge costs the same as a city. Sparse planes are a different change and
  probably a later one — the pyramid pays its way on residency alone.
- It does not touch objects, decor or POI, which have their own pages and
  their own reasons. The terrain page is the unit this note is about.
- It does not decide the fold. It says where the decision already lives.

## Where this stands

**Superseded in part, 2026-09-21,** by
[20260921-level-step-and-zoom-are-one-number.md](../20260921-level-step-and-zoom-are-one-number.md):
the page is 512 cells rather than 1,024, which puts a level on Web Mercator's
own zoom and a level-0 cell on its z16 pixel; and the fold this note says
already exists in `chunk_mesher.rs` folds surface color and coverage, not the
ground channels' categories, so the rule had to be decided rather than moved.

What is unchanged: the diagnosis — that the phone's memory, the reach of the
view and the coarse tiers' build cost are one thing, because resolution is not
a function of distance anywhere between the bake and the mesher — the shape of
the pyramid, the ocean aliasing at every level, and the order to build it in,
the bake alone first.
