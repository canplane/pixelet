# A ground cell is fourteen bits

**2026-09-23** — measured against the bake in `out/terrain`, decided, not
built. A terrain page holds its ground as four planes, a byte a cell each, and
what those four bytes carry is fourteen bits. Packed into a `u16` the page
halves, and no reader changes: the four channels stay where they are, each
still answerable on its own.

The page is what the client holds resident. `.fixed.terrain` is 125 records of
one page each, placed when the module boots and never freed
(`layer/terrain/fixed.rs`), so the ground is **125 MiB of the module's
memory** — the largest single thing in it, against a heap whose live bytes
measure 47.

## Why there are four planes and why they stay

A page is 512 × 512 cells. Its ground is four channels — land, water,
transport, structure — each a dense `u8` plane in `GROUND_CH_KEYS`' order.
`shown_ground` paints a cell's four bytes in that order and the last non-void
one wins, less whatever a `ChannelMask` hides.

That mask is the reason the four are separate, and it is not a detail of
today's UI. Every map has layers a reader turns on and off — the terrain, the
water, the roads, the buildings — and the mask is already per channel
(`channel_bit`). Only `show_structures` uses it now; the shape is built for
the rest. **A format that composes the four into what a cell shows can never
serve a layer the composition has already discarded**, so the four survive
this whole note.

## Fourteen bits

Each channel holds one of its own types, or VOID where nothing painted it, or
UNKNOWN where the source data was missing — never AIR, which means something
only in the object encoding and which no raster can produce.

| channel   | types | with VOID and UNKNOWN | bits |
| --------- | ----- | --------------------- | ---- |
| land      | 14    | 16                    | 4    |
| water     | 3     | 5                     | 3    |
| structure | 5     | 7                     | 3    |
| transport | 10    | 12                    | 4    |
|           |       |                       | 14   |

```
 15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
+-----+-----------+--------+--------+-----------+
|     | transport |structur|  water |    land   |
+-----+-----------+--------+--------+-----------+
  free      4          3        3         4
```

VOID comes out of no channel — a cell has to be able to say it holds no
water. Moving UNKNOWN out to one bit the cell shares takes water from three
bits to two and spends the bit it saved, so it is exactly a wash. Fourteen is
the floor.

Two bits are left over, and a cell is a `u16` rather than fourteen bits packed
end to end: four cells in seven bytes would save an eighth and put a division
in the path of every single-cell read — the placement raycast, the camera's
ground-snap, `ground_ele`.

## The table the widths need

The catalog's numbers are not dense. Land runs `bare = 1`, `green_deep = 10`,
`sand = 12`, `residential = 20`, up to `sea = 129`: fourteen types spread over
a byte, because the number is an identity and a new type takes the next free
one in its group rather than the next one up. A placed cube in the object
store names its terrain style by channel and that number
(`TERRAIN_STYLE_BASE`), and that store is in a browser's `localStorage`
already, so the numbers cannot be shuffled to close the gaps.

So a channel's field is a **dense index** into a table of its physical
numbers, VOID first and UNKNOWN last, and the table is what turns one back
into the other. It is generated, not written: `script/spec.ts` already reads
`cube_type.json` and writes `cube_type.spec.rs`, and `check:spec` already
fails the gate when a generated file is not what its JSON writes.

The widths are generated with it, from the tables' own lengths, so a
fifteenth land type widens `LAND_BITS` and shifts the channels above it with
no hand-editing anywhere. What is written by hand is the line that fails when
they stop fitting:

```rust
const _: () = assert!(
    LAND_BITS + WATER_BITS + STRUCTURE_BITS + TRANSPORT_BITS <= 16,
    "a ground cell no longer fits a u16"
);
```

A build error, not a runtime one.

Packing and unpacking are one pair of functions over `GROUND_CH_KEYS` and the
generated triples of table, width and shift — not four channels' worth of
shifts written out. `pxl-geo-bake` and `pxl-kernel` both compile `pxl-spec`,
so the bake writes what the client reads from one implementation. The shell
never decodes a page, so nothing here is generated into TypeScript.

## The wire is a separate question

Measured over 195 real pages of levels 0 to 2, ground only:

| four planes | mean a page  |
| ----------- | ------------ |
| gzip -9     | 40.8 KiB     |
| zstd -19    | **32.4 KiB** |

A dense plane of a category runs the same byte for hundreds of cells, and
zstd takes 1 MiB of it 32 times down. Whatever the resident form is, the wire
wants planes and a strong compressor; the packing above is about the 125 MiB
the client holds, not the 32 KiB it fetches.

## Roads not taken

Both were measured over the same 195 pages before the layer mask ruled them
out. The numbers are kept because they say what composition is worth, should
the question come back with layers answered some other way.

**A cell as one composed value.** What `shown_ground` returns is one
(channel, type) out of the catalog's 44, of which 25 appear anywhere in the
bake: six bits where four bytes are spent now. It loses every layer but the
one on top, which is the whole of the objection.

**A palette per 8 × 8 block**, the way a Minecraft chunk holds its blocks,
with a `u16` offset a block. On composed values it measures 66.5 KiB a page
against 1,024 — 2.08 bits a cell, 34% of blocks holding a single value. Two
things sank it. Keeping the cells under a building needs a second palette in
**48.1% of blocks**, which is not a sparse table beside a plane but a second
plane with extra steps, and it costs 96.5 KiB. And a block palette spends the
redundancy zstd was going to eat: the packed page takes zstd only 1.6 times
down, to 40.7 KiB, where the planes it came from take it to 32.4. Entropy is
the compressor's job.

## What is decided

1. **A cell is a `u16`**, a dense index a channel, four channels independent.
2. **The tables and the widths are generated** from `cube_type.json` beside
   the catalog they come from, and a const assert fails the build when the
   channels outgrow the word.
3. **The wire keeps dense planes**, compressed, and the client packs them as
   it takes the page in — the pass that already copies a fetched page lane by
   lane into `.fixed.terrain`.

`.fixed.terrain`'s ground is then **62.5 MiB** against 125.

## Open for the build

- The fold. `fold_plane` votes per channel over a plane of bytes; on a packed
  cell it votes per field. The rule does not change but the loop does.
- `ele` is untouched, and is 0.3 MiB of the 125.
- Whether the two spare bits are worth naming now or left alone.
- The other half of `.fixed.terrain`'s size is that there are 125 records —
  `WINDOW_LEVELS × HELD_AXIS²`, a ring held one wider than the ring drawn.
  That is the same shape
  [the mesh arena is a grid where it could be a pool](./20260923-what-is-heavy-and-what-gives-way.md)
  argues about for the device, and it is not this note's.
