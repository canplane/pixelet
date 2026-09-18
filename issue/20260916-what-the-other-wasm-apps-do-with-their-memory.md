# Where to look for how a fixed memory is divided, and where not to

**2026-09-16** — Working out whether the kernel's two heaps should be one, the
question came up of how other systems solve it. The large WASM applications
were the first place looked and the wrong one: four rounds of searching turned
up no public account of how a fixed-size linear memory is divided between large
buffers and small objects, and the one account that exists (Figma's) solves a
problem this client does not have. The right reference class turned out to be
elsewhere — kernels bigger than an embedded controller and smaller than a
general-purpose machine, which is to say game engines and RTOSes: fixed RAM, no
swap, no MMU to lean on, and failure is not an option. Those have solved it, in
public, for decades.

This records both halves: what the WASM references actually say and why they do
not transfer, then what the reference class that does fit has settled on.
Nothing here is built; it is the survey the decision cites.

## What was searched

- Figma's engineering writing and `IndirectBuffer`
- Google Earth's WebAssembly case studies
- Emscripten's memory layout and allocator documentation
- Rust/WASM allocators as of 2025–26 (dlmalloc, Talc, wee_alloc, lol_alloc)
- Memory64's status
- Rerun, wgpu-on-web architecture writing
- SharedArrayBuffer ring buffers and io_uring-shaped JS↔WASM boundaries
- Photoshop Web, AutoCAD Web

## Figma — separates, but across address spaces

Evan Wallace's 2018 account is the one source that describes the problem
directly. All C++ objects are reserved ranges in one pre-allocated typed array,
and large resources are held outside it:

> A trick that also helps is to use handles to out-of-heap data for large
> resources such as image and geometry buffers.

> Moving large allocations out of the main heap reduces memory fragmentation
> issues for long-running sessions, allows us to use more of the limited
> address space in 32-bit browsers, and allows us to break past the 31-bit
> typed array size limitation in 64-bit browsers.

The mechanism (`evanw/indirectbuffer`) is worth being exact about, because the
shape of it is what decides whether it transfers. The buffers live in
**JavaScript-side typed arrays**, in a global `Module._IB_` array. C++ holds an
integer handle, not a pointer, and reaches the bytes through `EM_ASM` blocks —
inline JavaScript. The API is `resize(size) -> handle` and
`handleForEmscripten()`.

So Figma does not divide its linear memory. It uses a **second address space**,
and that is why "more of the limited address space in 32-bit browsers" is one
of the three benefits: the buffers do not consume the WASM address space at all.

Two of the three reasons have since expired. Memory64 is in WebAssembly 3.0 and
ships on by default in Chromium 133+ and Firefox 134+, lifting 4 GiB to 16 GiB
and taking the 31-bit typed-array limit with it. **Safari does not have it**,
and the standing recommendation is to use it only when more than 4 GiB is
needed — every access pays a bounds check that the 32-bit fast path avoids. For
a phone-first client on a 512 MiB reservation, neither the limit nor the cure
applies.

Fragmentation is the one reason that survives.

Why Figma can pay the cost of the handle indirection: its large buffers are
opaque — image pixels and geometry handed to WebGL. C++ rarely walks them, so
crossing into JavaScript per access is affordable.

## Figma's author again, on a side project — the one close precedent

`evanw/buddy-malloc` is the same author writing an allocator for a WebAssembly
compiler of his own, and it is the nearest thing found to a precedent for the
arrangement here. The reason given for writing one at all is not performance
and not fragmentation:

> [I] didn't feel like attempting to port a standard memory allocator (they are
> many thousands of lines long and are really complicated).

The design, point for point:

- a **buddy allocator over a fixed linear address range**, spanned by a binary
  tree that tracks free space
- **one bit per node** for the free/used state
- **circular doubly-linked free lists**, the links threaded through the free
  blocks themselves, taken from the Linux kernel's implementation
- `brk` to ask for more, and explicitly: _"While archaic, this method is a good
  fit for WebAssembly's linear memory model"_
- `malloc` and `free` both O(log N) in the maximum number of allocations

Two things are worth taking from it. The first is that the same author chose
differently in the two settings — the product moves large buffers **out** of the
heap and into JavaScript; the side project runs a buddy **inside** the linear
memory — which is evidence that neither is a principle, both are answers to a
context.

The second is the reason he gives. A hand-written buddy over a fixed range is
what you reach for when porting a general allocator is not worth it, and that is
a statement about implementability, not about fragmentation. It is the same
position this kernel is in.

Where it stops transferring: it assumes the heap grows (`brk`, which is
`memory.grow`), and this memory does not; it is for a compiler, so the workload
is many small nodes and no large buffers; and the author states it "hasn't been
benchmarked at all and has barely been tested".

## Google Earth — does not discuss memory

The three public accounts (2019–2020, web.dev, Chromium Blog, the Google Earth
Medium posts) cover the Native Client → WebAssembly port, per-browser support,
WebGL2, OffscreenCanvas plans, and at length **threading**: streaming and
decompressing 3D data and imagery on background threads over SharedArrayBuffer
so the render loop stays free, falling back to single-threaded where
SharedArrayBuffer is disabled.

On memory there is one sentence:

> Earth may fail to start on devices with limited amounts of memory (such as
> 32-bit machines).

No layout, no allocator, no account of where streamed tiles are held. The one
transferable idea is on a different axis from this question — fetch and decode
off the main thread — and this client does neither today.

## Emscripten — a layout, for a memory that grows

`[static data][stack][heap →]`, dlmalloc over `sbrk`, 16-byte alignment,
a default stack of 5 MiB, `ALLOW_MEMORY_GROWTH` calling `memory.grow`. Useful
only as the shape of the convention; the premise is the opposite of this
kernel's, whose memory is imported with `initial === maximum` and never grows.

## The Rust/WASM allocator advice is about binary size

`wasm32-unknown-unknown` defaults to dlmalloc, "standard and battle-tested".
Talc (2024–) is a drop-in that benchmarks smaller and faster; `wee_alloc` is
unmaintained and leaks; `lol_alloc` is not production-ready.

The frequently-quoted line against custom allocators —

> Most Wasm written will be in conjunction with an app or a large library,
> where the presence of any allocator won't make much of a splash (and the
> maintenance of playing whack-a-mole with panics is too burdensome).

— comes from an article optimising **module size**, not fragmentation or
layout. It says an allocator swap is not worth the maintenance for a few KB of
module. It says nothing about what a heap should hold. (Recorded because it was
misread that way once in the conversation that produced this file.)

It does not apply here in any case: the reason this kernel has its own
allocator is not size. dlmalloc asks for pages when it runs out and would fail
at its first request past the initial size, and the memory here cannot grow —
`fixed_heap.rs` says so in its opening paragraph.

## Ring buffers — precedent exists, for a different purpose

io_uring's submission and completion rings, and `padenot/ringbuf.js`'s
wait-free SPSC ring over SharedArrayBuffer, are established. Both exist to
cross a **thread** boundary with Atomics.

This kernel's rings cross a **language** boundary on one thread: the glue
strobes `WORLD_UPDATE`, the core drains the submission ring and posts to the
completion ring, and the glue reads it dry between ticks. No SharedArrayBuffer,
no Atomics. The shape is a syscall interface, not a concurrent queue — which is
itself the signal that the glue has been demoted to a driver.

## Why none of it transfers

|                               | Figma (2018)          | Emscripten apps | Google Earth | here                       |
| ----------------------------- | --------------------- | --------------- | ------------ | -------------------------- |
| JavaScript's role             | host platform         | host            | host         | driver                     |
| memory                        | grows                 | grows           | grows        | **fixed at boot**          |
| large buffers                 | JS heap, by handle    | WASM heap       | not stated   | linear memory, by address  |
| how often the core walks them | rarely (handed to GL) | —               | —            | **every frame**            |
| glue boundary                 | `ccall` / `EM_ASM`    | `ccall`         | —            | **rings (SQ/CQ)**          |
| renderer                      | WebGL, driven by JS   | varies          | WebGL2       | WebGPU, driven by the core |

The two rows that decide it: the memory does not grow, so a standard allocator
is out; and the core walks the large buffers every frame — the scan and the
raycast step through band lanes, the mesher reads terrain lanes — so Figma's
handle-plus-`EM_ASM` indirection is not available. The repository had already
drawn that line for its own reasons
([20260912-linear-memory-what-is-left.md](./20260912-linear-memory-what-is-left.md)):
_a resource the canvas reads every frame is linear memory_.

## What the decision rests on

No external precedent was found for or against. Three constraints, all checked
against this code, decide it:

1. **The memory does not grow.** `reserve_memory` takes `initial === maximum`;
   the kernel owns the range it is given and nothing else.
2. **The large buffers are walked by address every frame**, so they stay in
   linear memory and stay directly addressable.
3. **A small long-lived block pins its whole ancestor chain.** `Region::release`
   coalesces only while the buddy is free, so one live 16-byte object in the
   middle of a 64 MiB span stops that span ever re-forming. Mixing the small and the
   large in one free list is the failure Figma's separation exists to avoid.

One buddy over the heap range satisfies all three if the small objects are kept
out of the large spans — large buffers taken as whole blocks of a high order,
small objects sub-allocated inside slabs the same buddy handed out. Figma buys
that isolation with a second address space; without one, an order does the same
work.

## The reference class that fits

Fixed budget taken once at startup, no growth, no swap, a mix of uniform
records and variable allocations, and an abort is a crash the user sees. That
is a game engine or an RTOS, and all three of the following divide **one budget
into regions by character** rather than running one allocator over everything.

### Quake's hunk, zone and cache — the closest shape

id Tech takes one large block at startup (`-hunk` sets its size) and runs
**two stack allocators from opposite ends of it toward the middle**: low for
permanent allocations, high for temporary, and they fail when they meet.

Inside that, the first hunk allocated is a fixed-size block called the **zone**,
reached through its own routines, whose purpose is the small and awkward —
dynamic strings and small buffers — that a stack discipline cannot serve.

What is left between the two hunks is the **cache**, and a hunk allocation made
after a cache allocation clears the cache up to that point. The cache is the
give in the system: it holds what can be rebuilt, and it yields to whatever
needs the space.

Laid beside this kernel, the correspondence is close enough to be worth naming:

| Quake                       | here                                                |
| --------------------------- | --------------------------------------------------- |
| hunk low — permanent        | fixed (rings, standing band stores), slab (terrain) |
| zone — small dynamic        | FixedHeap (`Vec`, `Box`, `String`)                  |
| cache — rebuildable, yields | mesh, and `RETAIN_BUDGET_BYTES`'s reclaim pass      |

And it answers the objection that stopped fixed partitions here. The problem was
never that the regions are separate; it is that `MODULE_BYTES` fixes the small
side at 32 MiB regardless of what was reserved. Two stacks growing toward each
other keep the characters apart **without** fixing either one's size.

### Zephyr — a near-miss worth recording as one

`k_mem_slab` looks like the terrain slab and is not it, and the difference is
worth writing down so the resemblance does not get cited again (it was, once,
in the conversation that produced this file).

Zephyr's slab does hand out fixed-size blocks with no per-block header, and its
documentation does say one size avoids "memory fragmentation concerns". But the
only use it suggests is a different problem entirely:

> Use a memory slab to allocate and free memory in fixed-size blocks. Use memory
> slab blocks when sending large amounts of data from one thread to another, to
> avoid unnecessary copying of the data.

The API says the same thing. `k_mem_slab_alloc(&slab, &ptr, K_MSEC(100))` takes
a **timeout**, and threads that find no free block queue on it by priority.
That is a synchronisation primitive — a fixed set of buffers cycled between a
producer and a consumer, the shape a driver hands DMA buffers round in — not a
policy about what belongs in a heap.

|              | Zephyr `k_mem_slab`               | the terrain slab                 |
| ------------ | --------------------------------- | -------------------------------- |
| alloc / free | yes, with a timeout               | **none**                         |
| waiting      | threads queue by priority         | —                                |
| which block  | any free one, off a list          | **computed from the coordinate** |
| purpose      | zero-copy handoff between threads | resident cache                   |
| threads      | several                           | one                              |

What they share is "fixed-size blocks, a known count", which is the shape of the
data structure and not the reason either exists. Zephyr separates slabs from its
heap because the **runtime API and the blocking semantics differ**; nothing in
its documentation argues for separating them in a memory map. The terrain slab
has no API at all — `slot_of` is arithmetic — so there is nothing there to
categorise.

### TLSF — the fragmentation number, for comparison

Two-Level Segregated Fit is the allocator real-time systems reach for when
allocation must be O(1) and bounded: size classes split logarithmically and then
each class into 32 linear bins, which caps round-up waste at 1/32 = 3.125%.
Four bytes of overhead per allocation, about 3 kB for the pool's own management.

It is worth recording as the number a buddy is measured against. A buddy rounds
to a power of two, so its worst case is 50%, and this kernel pays a real
instance of that: the elevation record is 9,248 B in a 16,384 B block, 44%
wasted, which is why the slab holds the elevation in a plane of its own rather
than through a pool. What a buddy buys for that is coalescing that is a single
XOR and an address arithmetic that is a shift — which is what
[20260916-the-page-table-the-pool-did-not-need.md](./20260916-the-page-table-the-pool-did-not-need.md)
turns on.

### What survives

Quake is the one that transfers, and TLSF is a number to measure against.
Zephyr's slab is a false friend.

What Quake's arrangement actually says is not "separate by character" — it is
that the only distinction a memory map needs is **whether the size is known at
startup**. The hunk and the zone are both taken once and never grow; the cache is
what is left, and it yields. Everything with a knowable bound is fixed; the rest
shares what is left over.

That reading also collapses a distinction this repository has been drawing.
`fixed` and `slab` are separate sections here, but from the map's side they are
one kind: both are settled at boot, neither grows, neither gives anything back,
and the map's whole job for either is "from here, this many bytes". What is
inside is that section's affair — `memory.rs` already says so of `fixed`. The
terrain slab is unusual in how it finds a record (arithmetic on the coordinate
rather than a search), but that is a property of the data, not of the section,
and it needs no entry in the map to be true.

## A caution on the negative result

Five rounds of searching the WASM applications turned up one precedent, and it
is a side project rather than a product. Companies publish about ports,
threading and performance, not about their allocators, and the one product
account that exists (Figma's) is eight years old. The absence of more is not
evidence that anything here is novel — only that the WASM literature will not
settle it.

What `buddy-malloc` does establish is that the combination — a fixed linear
range, a hand-written buddy, one bit a node, free-list links inside the free
blocks — is one that a second person reached independently under the same
constraints. That is worth more than it sounds: it is not a citation for the
design, but it is evidence the design is not eccentric.

The game-engine and RTOS literature will, and looking there first would have
saved the four rounds. The lesson worth keeping is that this kernel's reference
class is set by its constraints — a fixed budget, no growth, no MMU — and not by
the fact that it happens to compile to WebAssembly.
