# The page table the pool did not need

**2026-09-16** — Checking the slab's map against `REGION_BYTES` asked what a
region is for now that terrain is a modulo-addressed slab rather than a pool.
Four jobs came back, only one of them the region's own, and following the other
three led to twenty lines in `ds/buddy.rs` that hold the whole layout by the
throat: `_locate`, which answers "which region is this address in". This
records what it does, what it costs, and what the arithmetic is that replaces
it. Nothing here is built yet.

## What a region is, and what it has been doing besides

`REGION_BYTES` is 64 MiB. Every reader of it:

| use | code | the region's own job |
| --- | --- | --- |
| the mesh pool's largest block | `mesh.region_bytes`; the buddy refuses `bytes > region_bytes` under `no_spill` | **yes** |
| the fixed section's size | `bytes: REGION_BYTES - MODULE_BYTES` | no |
| the slab's start | `slab.offset = REGION_BYTES` | no |
| where the mesh begins | `MESH_AT = SLAB_END.div_ceil(REGION_BYTES) * REGION_BYTES` | no |
| the reservation's rounding | `regions = bytes / REGION_BYTES` | no |

Only the first is inherent — a buddy region is the largest block that pool can
hand out. The other four are what is left of counting every section in regions,
and one of them is expensive.

## The cost: 59.57 MiB nobody owns

The slab is exactly the terrain the load radius reaches — 7x7 records of
4,203,552 B, so 205,974,048 B = 196.43 MiB — and it starts at 64 MiB, so it
ends at 260.43 MiB. The mesh cannot start there:

```
slab end   260.43 MiB
MESH_AT    320.00 MiB        <- rounded up to a multiple of 64 MiB
no one's    59.57 MiB        <- 11.6% of the phone's 512 MiB reservation
```

`memory.rs` already names the gap ("no one's") and calls it the price of the
slab being the set rather than the set being the slab. It is not. Rounding the
mesh's start to any smaller unit costs almost nothing:

```
align to   4 KiB -> 0.001 MiB lost
          16 KiB -> 0.005 MiB
          64 KiB -> 0.005 MiB
          64 MiB -> 59.568 MiB       <- today
```

Shrinking `REGION_BYTES` instead recovers less than the alignment does — 8 MiB
regions still lose 3.57 MiB — and it would drag the fixed section down with it,
since `fixed.bytes` is `REGION_BYTES - MODULE_BYTES` and goes to zero at 32 MiB
and underflows below that.

## Why the mesh has to start on a region boundary

Because of the lookup, not because of the pool.

```rust
fn _locate(&self, buf: &ArrayBuffer, byte_offset: u32) -> i64 {
    if self._backing.as_ref().is_some_and(|b| buf.is(b)) {
        return self._backed_at.get(&(byte_offset - (byte_offset % self.region_bytes))).map_or(-1, |&i| i as i64);
    }
    self._plain_at.get(&buf.id()).map_or(-1, |&i| i as i64)
}
```

A block comes back as an address alone, so the pool has to recover the region
from it. For a region carved from the backing it floors the address to a
multiple of `region_bytes` **counting from address 0** and looks that up in
`_backed_at`.

The keys in `_backed_at` are not counted from zero. A region's base is
`_backing_used`, which starts at `_backing_offset` and steps by `region_bytes`:

```rust
_backing_used: offset,                           // = _backing_offset
Region::new(..., self._backing_used)             // base = that
self._backing_used += self.region_bytes;
self._backed_at.insert(region.base, i);          // key is offset-relative
```

So the key is written offset-relative and read zero-relative. The two agree
only when `_backing_offset` is itself a multiple of `region_bytes` — which is
the whole of the alignment requirement, and today it holds by arithmetic
accident rather than by construction: module 32 MiB, fixed at 32 MiB with
16 MiB regions, slab at 64 MiB, mesh at 320 MiB with 64 MiB regions all happen
to divide.

## What happens when it does not hold

Nothing loud. Every caller of `_locate` swallows the miss:

```rust
// PoolKernel::release
if self._locate(&backing, at) < 0 || is_free(...) { return false; }   // the bool is dropped by callers

// release_at
if i < 0 { console::warn("... never lent ignored"); return; }         // a warning, then nothing
```

A block whose region cannot be found is never returned and never reported. The
terrain audit earlier today left this as the one unclosed hole in the release
path, and the reason it is not reachable is the accident above, not a guard.

## The mapping is affine, so there is no table to keep

Region bases are an arithmetic progression, and the indices follow them. Backed
regions are cut before plain ones while the backing has room, and
`_reclaim_empty` skips everything in `_backed`, so a backed region holds its
slot in `_regions` for the life of the pool: the nth region cut is index n.

The region index is therefore a shift:

```rust
i = (byte_offset - self._backing_offset) >> region_shift
```

`_backed_at` is read in exactly one place (`_locate`) and written in exactly one
place, and on the backed path it is a `HashMap` standing in for a subtract and a
shift. A page table earns its keep when the mapping is arbitrary; this one is
contiguous and in order. The plain (spill) path still needs `_plain_at` — a
spill carries its own `ArrayBuffer` and has no address to compute from.

Half of this is already arithmetic. `handle_at` splits an address into a region
and a page with `(byte_offset - region.base) >> _min_shift` and packs
`region | order | page` into a `u32`; only the upper half of the same split is
done through a map.

## Where 4 KiB comes in, and why it is not the host's

The page here is a **borrowed analogy, not a hardware number**. This kernel is an
operating system inside an operating system: it is handed one fixed range, it
addresses it with 32-bit offsets, it has no MMU, and nothing underneath will
give it another byte. That is the situation a 32-bit machine was in, and 4 KiB
is what that situation settled on.

The scale matches closely enough for the analogy to be worth taking literally:

```
386, 1985     4 GiB address space / 4 KiB  = 1,048,576 pages by design
here          1 GiB reservation  / 4 KiB  =   262,144 pages actually managed
```

A quarter of what that architecture was drawn for, which puts this kernel inside
the range 4 KiB was chosen against rather than outside it.

**The host's page size is deliberately ignored.** It is 16 KiB on Apple silicon
and increasingly on ARM64 Android, 4 KiB on x86-64, and a WASM page is 64 KiB by
specification — three different numbers, none of which this kernel can see, act
on or benefit from. Linear memory is one flat array; there is no MMU underneath
it for the kernel to talk to, and the engine's own mapping of that array to host
pages is invisible and unaffected by anything decided here. Matching it would be
a coincidence dressed as a reason.

16 KiB is the specific temptation worth naming, because it has a real argument
that is not about the host: on a 1 GiB reservation it gives exactly 65,536
pages, so a page number is a `u16` and `[page:16][offset:16]` halves a `u32`
cleanly. That is genuinely tidy. It is rejected anyway — the reason to pick a
page size here is the analogy to the machine this kernel resembles, and taking
16 KiB would be picking a number because the hardware underneath happens to use
it, which is the thing this whole choice is not doing.

### The three axes it is chosen against

Whatever the number, it is pulled three ways:

| page | bookkeeping over 1 GiB (2 B a page) | average waste per request | page number |
| --- | --- | --- | --- |
| 512 B | 4.0 MiB (0.39%) | 256 B | 21 bits |
| **4 KiB** | **512 KiB (0.05%)** | **2 KiB** | **18 bits** |
| 16 KiB | 128 KiB (0.01%) | 8 KiB | 16 bits |
| 64 KiB | 32 KiB (0.003%) | 32 KiB | 14 bits |

Bookkeeping falls as the page grows, internal waste rises with it, and the index
narrows. Which end hurts depends on how much a sub-allocator takes at a time: an
arena that takes one page at a time pays the waste in full, one that takes
sixteen dilutes it. 4 KiB is the point where the table is already negligible
(0.05%) and the waste is still small enough not to need diluting.


### It is already the unit, unnamed

The pool's leaf already is a 4 KiB page: `MIN_BLOCK_BYTES` is `1 << 12`, and a
buddy block is a power-of-two count of them. So is almost everything else:

```
MIN_BLOCK_BYTES          1 page
ground lane            256 pages          (1024 x 1024, one byte a cell)
ground plane        12,544 pages
MODULE_BYTES         8,192 pages
REGION_BYTES        16,384 pages
ele lane                 2 pages + 1,056 B     <- the only exception
```

Counted in pages, an address on a 1 GiB reservation is 18 bits, which leaves 14
bits of a `u32` for a length — and 14 bits of pages is 64 MiB, exactly the
largest block a region can hand out. `Block = (usize, u32)` and the `at`/`bytes`
pairs in the scheduler's `Slot` collapse to one word.

The rule is about section starts and ends, not about what is inside one.
A section begins on a 4 KiB boundary and the next begins on the first 4 KiB
boundary past where this one ends. What a section does with its range is its
own affair — straddling costs nothing, linear memory being contiguous.

4 KiB and not more, because that is what the mesh pool's own arithmetic needs
and all it needs: a block lands at `backing_offset + n*region_bytes +
k*MIN_BLOCK_BYTES`, and `MIN_BLOCK_BYTES` is `1 << 12`, so a 4 KiB-aligned
section start makes every block 4 KiB-aligned.

The slab already obeys this internally without being asked. The ground is four
planes of 49 MiB, so every channel's plane starts on a page, and the elevation
plane starts at +196 MiB. Only the 49 elevation records inside that plane are
ragged, at 9,248 B each — and nothing counts them in pages, `ele_at` being
`slab + GROUND_BYTES + slot * ELE_LANE_BYTES`.

That raggedness reaches exactly one place: the slab's end, at 260.432 MiB. The
next section starts at 260.434 MiB and **1,504 bytes** go nowhere. Padding each
record to three pages instead would cost 148,960 B — ninety-nine times as much,
for nothing. Holding the elevation last, which `slab.rs` does for its own
reason, is already the answer to its size: the odd shape only ever touches the
end, and the end is only ever rounded up once.

## The order this falls in

1. `_locate` on the backed path becomes arithmetic. It recovers 59.57 MiB on
   its own, closes the silent-loss path, and is local to `ds/buddy.rs`.
2. Section starts stop being multiples of `REGION_BYTES` and become multiples
   of the kernel's page. `REGION_BYTES` goes back to meaning the mesh pool's
   largest block and nothing else; `fixed.bytes` says its own size (rings 3 MiB
   + 16 standing band stores 20 MiB = 23 MiB) instead of `REGION_BYTES - MODULE_BYTES`.
The first is worth doing whether or not the second is, and the second cannot be
done without it.
