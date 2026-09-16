# The slab over the buddy

**2026-09-17** — `a2b9bd4` made `ds/buddy_tree.rs` the module's
`#[global_allocator]`, and `ef4d6e2` routed every remaining block through it, so
one buddy tree now answers both a `String`'s twenty-four bytes and a band's
lanes. Its smallest block is 16 bytes and its split bitmap is 8 MiB of `static`.

**The shape this records is a slab of size classes over a buddy of 4 KiB
pages.** The buddy's smallest block goes back to being the page `memory.rs`
already names, and the classes a heap actually asks for come from the level
above it. Built the same day, in `ds/slab.rs`; the two sections at the end say
where the bookkeeping went and what the run sizes are, which is the part this
document left open.

## The bitmap is the ratio, not the device

`SPLIT_BYTES` is `(1 << (BUCKET_COUNT - 1)) / 8` and `BUCKET_COUNT` is
`MAX_ALLOC_LOG2 - MIN_ALLOC_LOG2 + 1`, so the bitmap is `2^(MAX-MIN) / 8` — a
function of the two exponents alone. It is a field of a `static`, sized at
compile time, so the phone carries the same array the desktop does:

| smallest block | `MIN_ALLOC_LOG2` | split bitmap |
| --- | --- | --- |
| 16 B | 4 | **8 MiB** |
| 64 B | 6 | 2 MiB |
| 256 B | 8 | 512 KiB |
| 1 KiB | 10 | 128 KiB |
| **4 KiB** | **12** | **32 KiB** |

Moving the bitmap out of the `static` and into the range — which is what
`ds/fixed_heap.rs` did, keeping "a bit for every place a block of every class
could start, about a sixty-fourth of the range" — makes it proportional to the
reservation instead of to 1 GiB. It does not make it small. Over the desktop's
763.57 MiB of heap, at a 16-byte leaf:

```
buddy_tree's split bitmap, static           8.00 MiB     2^(30-4)/8
buddy_tree's split bitmap, sized to range   5.97 MiB     range/128
fixed_heap's, as it was                    11.93 MiB     range/64
```

and a third of each of those on the phone's 251.57 MiB. A buddy that resolves to
16 bytes across a range this size costs MiB-scale bookkeeping wherever the
bookkeeping is kept.

That is the whole of the tension: one tree is asked to hand out both a 64 MiB
region and a twenty-four-byte string, and it cannot be cheap at both ends.

## 4 KiB is already the unit

`memory.rs` names it — `PAGE_BYTES`, `1 << 12`, which its doc-comment now ties
to `buddy_tree.rs`'s `MIN_ALLOC` in a `const` assert rather than in prose, the
two being the same number "for the same reason rather than by coincidence".
`20260916-the-page-table-the-pool-did-not-need.md` argues it at length and
against three axes: bookkeeping, internal waste, and the width of a page number.
None of that changed. What changed is that the global allocator was put on the
same tree, and a global allocator's smallest class cannot be a page — every
`Vec`, `String` and map node would round up to 4 KiB.

## The shape

One range, two levels. The buddy hands out pages and nothing smaller; the slab
above it hands out the size classes, and passes anything a page or larger
straight down.

```
the global allocator = a slab of size classes
    a request under a page takes a free slot of its class; a class with no
    free slot borrows a run of pages below, carves it into slots, and gives
    the run back when the last slot in it is freed.
    a request of a page or more is not carved at all — it goes down as it is.
        |  borrows / returns
        v
buddy_tree   MIN_ALLOC_LOG2 = 12, over [HEAP_AT, end)
    the page allocator. 32 KiB of split bitmap.
```

The slab costs what is live, not what the range is: a 4 KiB page cut into
16-byte slots is 256 slots, so 32 bytes of bitmap, and that fits inside the page
it describes. Nothing is reserved for small objects that small objects are not
using.

This is buddy + slab as Linux has it, and mmap + size class as glibc and
jemalloc have it. It is also what this tree already had before `a2b9bd4`:
`BuddyAllocator` for the sections and `FixedHeap` for the module's own
allocations. What it did not have was the return path between them.

### The pass-through is the common case, not an edge

`bake/volume.rs`'s `take_bytes` is `std::alloc::alloc` since `ef4d6e2`, so the
global allocator is what a band's material lane (`BAND_VOXELS * 4`), its layer
lane (`BAND_VOXELS`) and a fetch body all ask. Every one of those is far past a
page. The slab is not an optimization bolted onto a page allocator for the rare
small object — the two paths carry different traffic, and the large one is the
one the kernel's own blocks take.

### The header is not needed at either level

`HEADER_SIZE = 8` was in `buddy_tree.rs` because C's `free` is told an address
and nothing else. Rust's `GlobalAlloc::dealloc` is handed the `Layout` the
allocation was made with, so a free already knows its own size: the slab reads
its class off the layout rather than off a header, and finds the page the slot
is in by masking. The commit messages disagreed about whether the header
survives — `b2e207b` said it goes, `a2b9bd4` said it stays — and it is gone:
`BuddyMalloc::free` takes the size as an argument, and `POOL_RELEASE` reads the
packet's third child again, which is what it carried all along.

### What `fixed_heap.rs` objected to, and why this is not it

Its doc-string rejected per-class free lists: "a list per class only ever serves
its own size … the lists held what one size had given back while another size
cut fresh space from the top". That is an objection to class lists that own the
range permanently. A class whose emptied page goes back to the buddy is the case
the objection does not cover — what one size gives back becomes available to
every other size, through the level below. The return path is the whole
difference, and it is the one thing the pair here never had.

## Where the bookkeeping went: SLUB, not SLAB

A bitmap in the page was the first thought — "a 4 KiB page cut into 16-byte
slots is 256 slots, so 32 bytes of bitmap" above. SLUB does not keep one, and
the reason it does not is worth taking: a free slot is free, so its own first
word can hold the address of the next free slot. No bitmap, no per-object
header, and the run is its slots and nothing else. It is the same trick
`buddy_tree.rs` already plays with its free lists, one level down.

What a run needs besides is three fields — the head of that free list, how many
slots are out, and the links of its class's partial list. SLUB puts the
equivalent in `struct page`, which Linux has for every page anyway. Nothing
here has a page descriptor array, and one would be proportional to the range
rather than to what is live: 195,473 pages over the desktop's heap, which is
the cost this whole arrangement exists to avoid. So the three fields live in
the run's own leading slots — `head_slots` of them, one for a class of 16 bytes
and never more than two — and the run is `MIN_SLOTS_LOG2` = 16 slots at the
least, so that header is at most a sixteenth of what it describes.

That sizes the runs of the classes at and above a page's sixteenth:

| class | 16 | 32 | 64 | 128 | 256 | 512 | 1 KiB | 2 KiB |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| run | 4 KiB | 4 KiB | 4 KiB | 4 KiB | 4 KiB | 8 KiB | 16 KiB | 32 KiB |
| slots | 256 | 128 | 64 | 32 | 16 | 16 | 16 | 16 |

## How a slot finds its run: a mask, not a table

The buddy places a block of size `S` at an offset from the range's base that is
a multiple of `S`, so a slot's run begins at
`base + ((slot - base) & !(run_bytes - 1))` — and `dealloc` knows `run_bytes`
before it needs the mask, the class coming off the `Layout`. The mask is of the
**offset** and not of the address: `HEAP_AT` is where the fixed sections
happened to end, rounded up to a page, and 273,084,416 is not a multiple of the
32 KiB a 2 KiB class's run takes.

The same fact bounds what the pass-through can align to. A block lands at a
multiple of its own size from a base that is only known to be a page boundary,
so the pair satisfies any alignment up to a page and refuses one past it.

## What was considered and not taken

- **Two allocators over two fixed sub-ranges, side by side.** The split point
  has to be chosen at boot, and a shortfall on one side fails while the other
  side has room. That is `FixedHeap`'s own rigidity — it "owns the range its
  `Range` names and nothing else" — which merging the two was meant to end.
- **One buddy at 16 B.** The bitmap above.
- **The buddy at 4 KiB with no slab over it.** The page size is right and it is
  half of what is taken, but on its own it rounds every `Vec`, `String` and map
  node in the module up to a page.

## Two things independent of all of the above

- `MAX_ALLOC` is `1 << 30`, exactly `DESKTOP_BYTES`, with nothing tying them
  together. A change to either alone is silent.
- `4da3ff1` and `7c11e18` change only `ds/buddy.rs`, which `a2b9bd4` deletes.
