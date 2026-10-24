# A zoom names a page grid

**2026-10-06** — decided on 2026-10-05, built today. It supersedes
[the mesh step and the zoom are one number](./closed/20260921-the-mesh-step-and-the-zoom-are-one-number.md)
on what a zoom names.

A zoom used to name a cell's resolution: terrain's 1u cell was z16, and a
page of 512 of them was named z16 too, though it is the standard's z15
tile. The same number then meant a cell in one place and a page in the
next, and three readers of the fold's tie (the terrain bake, the objects'
store, the ray) each carried a different number that agreed only in parity.
Bake, globe and render are to share one unit system, so the word goes to
one thing.

## What a zoom is now

- **A zoom names the page grid alone**: 2^z pages around the world. The
  leaves, L3 pages, are z15 (`pxl-spec space/pyramid.rs` `LEAF_Z`, from the
  X lane's width and L3's); the pyramid stops at z5 for now.
- **A cell is named by its size**, a `Scale`. A layer's cell is its page's
  side over the layer's own count of cells, the same at every zoom
  (`Lattice`); objects' cells go finer than a leaf's, and no zoom names them.
- **The distance chooses a cell** (`pxl-spec space/lod.rs` `cell_at`), and
  terrain's zoom is the one whose cell that is, held to the pyramid
  (`Lattice::z_of`, `Tau::z_at`). The fold's tie is the parent cell's
  (`bake/fold.rs` `majority`), so every reader keys it alike.
- **A web map's zoom is the 3857 adapter's** (`zoom_of`, `side_at_zoom`),
  for comparing with one — what a POI is shown from — and nothing in the
  pyramid counts in it.

## What it changes and what it does not

A page's key, its quadkey name (whose length is the page grid's bits) and
its bytes are unchanged; every zoom's label drops by one. The bake's
directories are `z15/` to `z5/`, a demand's subject carries the new zoom,
the published `top_z` is 5 and the dev tile server's default `--top-z` 12,
the same pages as before. The profiler counts its zoom slots by
`Lattice::z_of`. The floating origin is unchanged: its square scales with
the page zoom and its height stays an L4.

POI and reverse geocode are turned off meanwhile, one problem at a time
(`apps/client/src/kernel/boot.ts` leaves their roots unbound, which the kernel
takes as no page); BACKLOG.md holds turning them back on.

## Not yet

A chunk is still 64 ground cells of its zoom's page, objects drawn inside it
at their finer cells. Making every chunk 64 cells of its own scale, objects'
too, makes near chunks eight times narrower and many more of them; that is
a change to the frontier and the slots, decided apart with its cost.
