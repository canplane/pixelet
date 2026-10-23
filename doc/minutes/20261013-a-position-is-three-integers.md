# A position is three integers

**2026-10-13** — decided in the conversation, and in the code: the tile's
one key (`c70cf3e0e`), then the position and the seeds in one commit with
this record.

A position is three `i32`, one an axis, each counting 1/256 of a unit: a
`Q3`, beside `V3`'s three doubles. The lanes' widths and their meaning stay
as they are; what changes is how they are held. The packed form, PVF3, goes:
two words holding three lanes each (`pxl-spec space/coord/pvf3/`), `IVec`,
`FVec`, `Pvf3` and the arithmetic over them. It came with the platform
rewrite (`361c796e6`, 2026-01-19), the packed 64-bit world coordinate a
2026-01-09 note had proposed (decision/closed/20260910-january-origin-salvage.md).

## Why the packing goes

PVF3 packs the cell index into one `u64` (X 24, Y 24, Z 16 bits) and the
fraction into one `u32` (8 bits an axis). Counted in the tree, 567
references in 76 files, the packing does no work of its own:

- **Its arithmetic is unused.** `ivec`, `fvec` and `pvf3` each carry the
  four operations, comparisons, dot products and distances, with a trait
  (`PVecTraits`) and two markers over them. Outside the module, `ivec::sh`
  is called twice (`zoom.rs` `idx`, `from_idx`) and `pvf3::sub` once
  (`control/navigation/off_center.rs`); the rest only in tests.
- **Every caller unpacks.** A caller reads `X`, `Y`, `Z` out as `i64` or
  `f64`, computes, and packs the result back with `gather`. The packing is
  a container, never the computation.
- **What it does on its own repeats something else.** As a key, the packed
  word named a tile beside the Morton key, now gone (below). Its X lane
  wraps at 2^24 units, which is the world's round, and the world states
  that round again (`world.rs` `nearest`, `wrap_index`, the walk's
  `wrap(…, ROUND)`).

The one argument for packed words, arithmetic over all lanes in one
instruction, was weighed and rejected in the module's own note
(`coord/pvf3/README.md`): the lanes carry no guard bits, so a borrow crosses
into the next lane, and three lanes of uneven width leave no loop to amortize
over.

A position also gets smaller: 12 bytes, where PVF3's `u64` and `u32` take
16 with their alignment.

## What a position is

A `Q3` is `[i32; 3]`, each value a unit times 256 (`FRAC_W`, 8 bits). An
axis is its lane's integer bits and the fraction's 8 together: X and Y are
24 + 8 = 32 bits, all of an `i32`; Z is 16 + 8 = 24 bits, inside one. Its
cell at zoom `z` is a shift, `v >> (FRAC_W + shamt(z))`, as `zoom.rs`
`index_of` already computes. Its place inside a unit is `v & 255`. The
fraction and the index are not two types. A computation that grows past an
axis, a product or a far Y apart, is done wider, as every caller already
does once it reads a lane out.

## What the widths are

The widths 24, 24 and 16 were the lanes', and each has a reason of the
world's own; they move to `world.rs` with that reason, not as a format's:

- **X, 24.** A unit is a z24 cell's side, so the world's round is 2^24
  units (`UNIT_ZOOM`); that is what a unit means, and a web map's z16 pixel
  is a z24 cell (ARCHITECTURE.md, the 3857 adapter).
- **Y, 24.** The Mercator plane is a square, as wide as it is tall.
- **Z, 16.** The world's height is ±2^15 units: the terrain stores a
  height as an `i16` in ground cells, and a ground cell is a unit
  (`bake/constant.rs`). That is enough: a unit is 2.4 m at the equator and
  shrinks with latitude, so 2^15 of them reach 78 km there and 39 km at 60°,
  where the Earth's relief spans some 20 km.

`SHARD_ZOOM` follows from the last two: the coarsest cube whose side fits
the world's height, 2^16 units, is z8. It is where the pyramid becomes 3D,
and the unit the data is ingested and baked in. Nothing in it depended on
a lane; it moves with the height it is made of. The Morton key's digits
(`hierarchy/morton.rs` `LANES`) and `REACH` follow the same three numbers.

## The one door, and the short way

PVF3 wrapped every lane on every `gather`, silently: a value past a lane's
width came back as the lane's residue. `world.rs` already says a position
enters the world only through `point`, which sends X round the world and
holds Y and Z at its ends. Ten sites outside the tests build one with
`gather` directly instead, and some lean on the silent wrap. The walk's end
(`query/support.rs` `step_slide`) can cross the antimeridian that way.

Every `Q3` that names a position passes the world's door, which holds Y
and Z at the world's ends. X needs nothing more: the round is 2^24 units of
256 steps, 2^32, all of an `i32`, so X held in one goes round the world as
the type wraps.

A displacement is not a plain difference. With L the round, two positions
at L − ε and ε are 2ε apart, and their difference is 2ε − L. PVF3's `sub`
gave the short way only because its X lane wrapped, and `off_center.rs`
relies on that, as its comment says; so does a 3857 test. In an `i32` the
same wrap is the type's: X's difference taken wrapping is the short way,
which the world states in doubles too (`world.rs` `nearest`, the walk's
`wrap(…, ROUND)`). Y does not go round, and two Y as far apart as the
plane's two edges differ by more than an `i32` holds, so a Y difference is
taken wider.

## One key for a tile

Done in `c70cf3e0e`. A tile is named by its Morton key alone, and `Tile`
holds its index as `[i64; 2]`.

The demand queue's `subject` stays one `u64` word, shared with the asset
loads (`io/load.rs` `subject_of`). For a tile, the word is the Morton key's
high half. A 2D key at zoom z is 2z digits, left-aligned, then an end bit.
Every zoom to 31 lies wholly in the high 64 bits, so the low half is zero
and the word gives the key back exactly; `node_of` finds the zoom from the
end bit. The bound is asserted against `LEAF_ZOOM`. A tile's word and an
asset's never meet: the queue's key is the stream and the word together.

The separation memo (`geo-bake separation/source.rs`) named its files by the
packed word's decimal. They are now named by the leaf's name, so a memo
written before is read again from the database.

The geo-bake now orders tiles as `[i64; 2]` and `Tile` sort: X signed,
then Y. The packed word sorted a negative X after a positive one, so the
two orders part only across X = 0 or Y = 0. Whether the separation solve's
result depends on the order its ring loads in (`arnis.rs`) was not checked.

## The seeds

Two seeds were drawn from the packed index word: placed decor's variant
and turn (`pxl-kernel layer/object/meta.rs` `user_decor_pick`), from its
corner's unit cell, and the ground's scattered decor (`procedural.rs`
`each_candidate`), each block of the scatter seeding its candidates from
its index as that word. The caves drew theirs another way: the zoom and
each index folded into a seed of their own (`draws`).

A seed is a node's hash for one use, and the packing only supplied an
encoding of the node. Two encodings for one role are one too many, so
there is one function, `procedural.rs` `node_seed`: a tag names the use,
and the node's zoom and indices are folded in, as the caves already did.
Each use keeps a tag of its own, so no two draw one stream: the caves
theirs, each decor kind its catalog seed, placed decor a tag of its own
(`meta.rs` `PLACED`). A seed comes from the node that names where a thing
is placed or born, never from the zoom it is seen at.

The caves' values do not move: `node_seed` is the fold they used. Placed
decor may show another variant or turn, and the ground's decor stands
elsewhere. Nothing is stored that converts: what is placed stays placed,
and only what is computed from it changes.

Weighed against it: keeping both seeds' bits as they were, which moves
nothing on screen but keeps a second encoding of a node for one hash.

## Documents

- `coord/pvf3/README.md` is deleted with the module: the question it
  answers does not arise for three integers.
- `space/README.md` loses its link to that note and its `IVec`, `Pvf3` and
  `pack.rs` paragraphs, rewritten for `Q3`.
- ARCHITECTURE.md's "A position — PVF3" loses its two-word figure.
- README.md lists PVF3 as a format made here (the formats paragraph and the
  `pxl-spec` row). The write-documents skill uses it as an example of a
  coined name.

## Order

1. The tile's one key (`c70cf3e0e`).
2. `Pvf3`, `IVec`, `FVec` to `Q3`; the widths to `world.rs` with their
   reasons; the raw constructions through the door; displacements the
   short way; the module and its note deleted.
3. One seed for a node, `node_seed`.

The second and third are one commit.

The tree was held still while they ran: every other session stopped first.
