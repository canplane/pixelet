# The record layer the migration dropped

**2026-09-21** — recorded, not scheduled. A reading of the storage the client
holds, made while looking at what a server and a second player would need, and
one finding worth keeping: the layer that was supposed to carry a placed thing
across a boundary was decided, built, and then lost.

On 2026-09-13 this project settled how its records are shaped
([20260913-everything-is-a-file-the-placement-and-the-palette.md](./20260913-everything-is-a-file-the-placement-and-the-palette.md)).
The control plane is unified — a terrain page, an object band, a POI page and
a mesh are all a handle with an identity, an owner, a lifetime, a revision and
a residency. The data plane is not: a terrain page stays dense lanes, an
object band a sparse tree, a POI page point records. What is unified is one
level down, and it is two records. A **Placement** is one row for one placed
thing — id, position, size, heading, and a palette value — so that what
differs between a tree and a cube is the value and not the row. A **Palette**
is the table that value indexes, one row per kind of thing, pointing at a
model by content hash. Both were written that night, and the object store's
saved form became one Placement record per page.

Neither is in the Rust tree. There is no `Placement`, no `Palette`, and no
column layer under them. XTP is explicit that it does not carry a schema —
its README asks "are keys encoded? in what order? are missing fields
allowed?" and answers that a higher layer owns all three — and that higher
layer no longer exists on this side.

What the object store saves today is five bare lanes grouped by page:

```text
[[page_idx u64, x f32[], y f32[], z f32[], size f32[], value u32[]], …]
```

base64url into `localStorage` under a key carrying the layout version, now 9.
No id, no heading, no room for a column a reader could skip. The
`(corner, size, state)` the store holds is a Placement already, minus the
heading — which is exactly what the 09-13 note said the unification would cost
it.

## Why it matters now

That note left the Placement's own emission unfinished, and said what would
finish it:

> It is read the moment a placement crosses a boundary — a server. Until then
> it is a copy nobody opens, so it waits for its first reader.

A second player is that reader. And the slot it arrives into is already
standing: `pxl-api` answers nothing and exists anyway, with `pixelet-db-main`
behind it, on the same reasoning
([20260918-the-server-in-rust-and-the-line-already-drawn.md](./20260918-the-server-in-rust-and-the-line-already-drawn.md)).
Nothing about the world is stored server-side today; placed objects live in
`localStorage` and nowhere else, which is a deliberate omission and not a gap.

## The three things a second player unifies

**The address.** Today each layer names its own: terrain by L3 page index,
POI by the same, objects by the L2 column with Z dropped and a band Z beside
it, decoration by nothing at all. One shape covers them — `(kind, level, key)`
— and the level half of it is settled in
[level, step and zoom](./20260921-level-step-and-zoom-are-one-number.md).
`DemandKey` is `(category, subject)` and wants the level; the world change log
is already `(topic, key)` and wants it too.

**The record.** The object store's lanes and the POI page's lanes reached the
same conventions independently — columns, a `u64` identity from a hash, a
page-local float position — which is the condition the 09-13 note set for
lifting anything into a common layer. POI is the better starting point of the
two: it already carries a page-local dictionary of categories and a free-form
tag string, so there is somewhere for a thing a user made to go.

**The palette.** `cube_types` and `decor_types` are generated from `spec/` and
closed at build time, so a kind one player has and another does not cannot be
expressed. Models are already content-addressed at `/asset/<sha256>`, which is
half of what the note's palette row described.

## What must stay two

The baked half and the mutable half are two planes and must not be drawn into
one
([20260919-the-transport-already-there-and-the-one-that-is-not.md](./20260919-the-transport-already-there-and-the-one-that-is-not.md)).
Terrain and assets are immutable prefixes behind an edge cache and a private
bucket, and that composition is also what keeps the deployment unable to bill.
So the unification is of the record's grammar and its addressing, never of the
transport: the same Placement arrives as an immutable object on one side and
on a stream on the other.

The subscription unit needs nothing new. `world/change.rs` already bumps
`(topic, key)` per column, per cell and per page, and readers already compare
revisions rather than being told. A server pushing "this key moved" lands on
the shape the client is built around. The transport is compiled in too: `ws`
is a default feature of axum 0.8, and nothing has walked the path.

What no current format carries is the rest of what a shared edit needs: an
author, and an order. The identity is already answered — the 09-13 convention
makes an id the FNV-1a of the address, and because a write touches exactly one
cell, the conflict unit is already as small as it goes.

## What POI is, and how much of it is reusable

POI is three layers, and they are worth telling apart because only two of them
generalize.

The **page** is a column table: `x`, `y`, `z` as `f64`, a category index and a
rank as `u16`, and the id's FNV-1a as `u64`, all as lanes; then the category
dictionary and three string tables — ids, names and tags — read one record at
a time through a cursor. That is the line
[20260912-linear-memory-what-is-left.md](./20260912-linear-memory-what-is-left.md)
draws: the lanes are the canvas's, the strings are read on demand.

The **placement** is a job of the frame's content phase: query a disc, file
the hits into an implicit lattice whose tier follows camera distance, offer
the nodes with estimated widths, place greedily on a screen grid in distance
order — a covered badge is absorbed into the one standing on its pixels and
becomes a count — then commit the field whole. The problem it solves is
"draw world-anchored labels inside a budget", which is also the problem a
player's name tag and a chat bubble are.

The **index** is the part that does not generalize, and it says so:

> POI is static. It changes when a page arrives or leaves and at no other
> time, which is the axis this indexes along.

It rebuilds when the page set changes, when the frame origin moves, or when
the camera passes its margin, and a rebuild sweeps the whole disc grounding
each record against the terrain. One record moving is not something it can
express. What is reusable is the structure underneath it —
`space/lattice/spatial_hash.rs`, which is built for the opposite regime: a
per-frame rebuild that allocates nothing, three passes of counting sort over
flat integer arrays, a generation stamp instead of a clear. Its own header
draws the line for us:

> The regime this is for is the one an octree is worst at: many points in a
> small volume, moving.

and

> A different consumer with a different radius wants a different index rather
> than a compromise in this one.

So: things that stand still keep the POI index, things that move get their own
`SpatialHash`, and neither gets a tree.

Two more pieces of a second player are already in the tree and unused. The
billboard field draws sets in a fixed order — `poi`, `chat`, `debug` — and a
set can follow an actor at a lift; and `io/load.rs` keeps `actors` as a
`Vec<Option<Actor>>` with slot reuse, so more than one has always been
possible. The companion is the only one standing in it.

Two gaps are real. POI has no click-to-select and no hover state, which a
social layer needs. And its badges are whole bitmaps cached by id hash, which
cannot serve arbitrary names; the atlas README already names the successor and
its condition — "a glyph atlas in place of a badge atlas … belongs after
instancing, which is now in, and before a much denser dataset". Player names
are that denser dataset.

## Where this stands

Nothing is built and nothing is scheduled. The finding to keep is that a
decision made and implemented on 2026-09-13 is not in the tree that replaced
it, and that the condition it was waiting on has arrived. The order it implies,
if it is taken up: the column layer and `Placement` in `pxl-spec`, the object
store's saved form onto it, the palette as rows with an ingest behind
`pxl-api`, and only then a subscription over the change log's own keys.
