# The kernel heap and what xpute has

**2026-09-15** — the heap-removal work on `wip/kernel-heap` (checkpoint
`ce65571`, uncommitted work on top) turned out to have replaced the kernel's
`Vec`/`String` with containers invented for the purpose, not with what
`crates/xpute/core` already had. Before anything is changed back, this records
what those containers are, what xpute on `main` has for the same role, and
how far each reaches, so the next step is decided from the facts. No approach
is chosen here.

Baseline: `main` (the checkpoint's parent `d29c3ce` is `main`). Reference
counts are rust-analyzer's (LSIF index of the workspace), uses only,
definitions excluded.

## 1. The bottom types: absent from `main`, or different there

| Type                                  | Where                                | On `main` | Role                                                        | What `main` xpute has for it                                                                                                 | References (uses / files)                  |
| ------------------------------------- | ------------------------------------ | --------- | ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| `FixStr<N>`                           | xpute `kit/text.rs`                  | absent    | inline fixed-capacity string                                | nothing; text is `String`, bytes can sit in a `TypedArray<u8>` lane                                                          | 77 / 24 (incl. xpute `sys/error.rs`)       |
| `Text<'a>`                            | xpute `kit/text.rs`                  | absent    | `fmt::Write` over caller bytes                              | nothing                                                                                                                      | 8 / 3                                      |
| `Quoted<T>`                           | xpute `kit/text.rs`                  | absent    | JSON string escaping as a formatter                         | `json.rs`'s private `quote(&mut String)`                                                                                     | 37 / 5                                     |
| `Span`, `TextLane<A>`                 | xpute `kit/text.rs`                  | absent    | strings laid one after another in a lane                    | `Lanes` with a U8 lane (internally `Vec`/`Box`)                                                                              | 17 / 5, 5 / 2                              |
| `InlineVec<T, N>`                     | xpute `ds/inline_vec.rs` (untracked) | absent    | `[T; N]` + len inside a struct                              | no generic type                                                                                                              | 110 / 29                                   |
| `HandleTable<N>`                      | xpute `ds/handle.rs`                 | absent    | slot + generation handles                                   | buddy's address handles (`handle_at`/`handle_base`) only                                                                     | 14 / 4 (`render/gpu/api.rs`, `surface.rs`) |
| `Vector<T: Copy, A>`                  | xpute `ds/arena.rs`                  | different | lane of records, allocator as a type parameter              | `Vector<T: AnyElement>`: numeric elements over a `TypedArray`, holding a `Box<dyn TypedAlloc>`                               | 36 / 7                                     |
| `TypedAlloc::take_bytes`/`give_bytes` | xpute `ds/typed_alloc.rs`            | absent    | byte room for record lanes                                  | `take_view`/`give` (typed views only)                                                                                        | 1 / 1, 2 / 1 (`arena.rs` only)             |
| `IndexedMap<T, V, A>`                 | xpute `ds/map.rs`                    | different | key and value lanes; `get`/`set`/`remove`/`fixed`/`keyed`   | `IndexedMap<T: KeyTraits>`: keys only; `new(traits, cap, Option<Box<dyn TypedAlloc>>)`, `find`/`insert`/`remove_at`/`clear`  | 35 / 8                                     |
| `KeyHash`, `HashKeys<K>`              | xpute `ds/map.rs`                    | absent    | tuple / arbitrary keys                                      | `I64Keys`, `U32Keys` only                                                                                                    | 9 / 1, 13 / 3                              |
| `Deque<T, A>`                         | xpute `ds/deque.rs`                  | different | owning ring                                                 | `Deque<'a, T>` over a caller's `&mut [Option<T>]`                                                                            | 4 / 2 (tests only; no pxl use)             |
| `FixedAlloc`                          | pxl `memory.rs`                      | absent    | takes from the fixed section's pool, never gives back       | `TypedAlloc` impls: `buddy_alloc(Rc<RefCell<BuddyAllocator>>)`, `HeapAlloc`, `Carver`; as a global allocator, `FixedHeap<R>` | 12 / 5                                     |
| `FixedVec<T>`                         | pxl `memory.rs`                      | absent    | alias of `Vector<T, FixedAlloc>`                            | see `Vector`                                                                                                                 | 256 / 30                                   |
| `filled()`                            | pxl `memory.rs`                      | absent    | a filled fixed lane                                         | nothing                                                                                                                      | 44 / 8                                     |
| `LaneMap<K, V>`                       | pxl `memory.rs`                      | absent    | alias of `IndexedMap<HashKeys<K>, V, FixedAlloc>`           | see `IndexedMap`                                                                                                             | 15 / 3                                     |
| `TextLane` (pxl alias)                | pxl `memory.rs`                      | absent    | `TextLane<FixedAlloc>`                                      | see `TextLane`                                                                                                               | 9 / 3                                      |
| `ChangeLog<A>`                        | xpute `ipc/change.rs`                | different | allocator as a type parameter, revisions in an `IndexedMap` | `ChangeLog` with its internal `Shared` allocator                                                                             | —                                          |
| `XputeError.message`                  | xpute `sys/error.rs`                 | different | `FixStr<256>` inline message, `of(errno, format_args!)`     | `new(errno, Option<&str>, opts)`                                                                                             | —                                          |

Added after the checkpoint and uncommitted: `InlineVec::insert`/`swap_remove`/`Eq`,
`Vector::pop`, `KeyHash for FixStr`.

## 2. In the checkpoint, absent from `main`, not among the bottom types

| Item                                                                  | Heap inside                           |
| --------------------------------------------------------------------- | ------------------------------------- |
| `sched/{tick, quantum, io, frame_budget}.rs`                          | none (`[T; N]` arrays, `fn` pointers) |
| `sys/{memory, clock}.rs`                                              | none                                  |
| `kit/json.rs`'s in-place reader (`read_json`, `JsonValue`, `JsonStr`) | none                                  |
| `io/marshal/tree/encode.rs`'s `PacketWriter`                          | none                                  |
| `ipc/ring.rs`'s `push_in_place`                                       | none                                  |
| `cursor.rs`'s `get_str_ref`                                           | none                                  |

## 3. Heap that xpute on `main` still uses itself

| File                                              | Heap                                                                                                                |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `kit/array.rs`                                    | `ArrayBuffer(Rc<Storage>)`; `new` owns a `Box<[UnsafeCell<u8>]>`, and `from_raw` still allocates an `Rc` per handle |
| `ds/arena.rs`                                     | `Arena<T>`: `Vec<T>`, `Box<dyn Fn>`                                                                                 |
| `ds/buddy.rs`                                     | `Region`'s `Vec`/`Vec<Vec<u8>>`, `HashMap`/`HashSet`, `Box<dyn PoolKernel>`, `format!`                              |
| `ds/lanes.rs`                                     | `LaneSpec = Vec<…>`, `LaneViews = Vec<…>`, `Box<dyn TypedAlloc>`                                                    |
| `ds/spatial_hash.rs`                              | `Option<Box<dyn TypedAlloc>>` (the lanes themselves come from the allocator)                                        |
| `ds/grid_index.rs`                                | every lane a `Vec`                                                                                                  |
| `ds/heap.rs`                                      | the caller's `&mut Vec<Element<T>>`                                                                                 |
| `ds/treap_set.rs`                                 | `Box<Tree<T>>`, iterator `Vec` stacks                                                                               |
| `ds/typed_alloc.rs`                               | `HeapAlloc` (`ArrayBuffer::new`), `Rc<RefCell<BuddyAllocator>>`                                                     |
| `io/marshal/tree/{spec, view, cursor, encode}.rs` | `String`/`Vec` in `Node`/`NodeValue`, `TreeEncoder` growing `TypedArray::new`, `Vec<u8>` payloads                   |
| `io/marshal/tlv.rs`                               | `TLVValue::Str(String)`, buffer growth, `read_all`'s `Vec`                                                          |
| `kit/encoding.rs`                                 | `TextDecoder`'s `String`, base64/hex `String`/`Vec`                                                                 |
| `kit/json.rs` (the `main` part)                   | `StrMap = Vec<(String, Option<String>)>`                                                                            |
| `kit/hash.rs`                                     | `fnv1a_64_final`'s `TypedArray::new`, `fnv1a_64_hex`'s `String`                                                     |
| `kit/comp.rs`, `kit/rng.rs`                       | `Box<dyn Fn>`                                                                                                       |
| `ipc/change_waker.rs`                             | `Vec`, `Box<dyn FnMut>`, `Rc<RefCell<J>>`                                                                           |
| `ipc/port.rs`                                     | `Vec<Option<Box<dyn FnMut>>>`, `format!`                                                                            |
| `ipc/frame.rs`                                    | `ArrayBuffer` handles                                                                                               |
| `sys/error.rs`                                    | `cause: Option<Box<dyn Error>>`                                                                                     |
| `vector.rs`                                       | tests only (`HashMap`, `format!`)                                                                                   |

## 4. What `main` xpute offers with no heap

| Mechanism                                                                   | Note                                                               |
| --------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| `FixedHeap<R: Range>`                                                       | a `GlobalAlloc` over a fixed range, a free list per size class     |
| `TypedArray<T>::over(buffer, offset, len)`                                  | numeric views; the `ArrayBuffer` handle itself is an `Rc`          |
| `Carver`                                                                    | typed views cut one after another from one block                   |
| `TypedAlloc` / `AddressedAlloc`                                             | the view-at-a-time allocation interface                            |
| `BuddyAllocator` through `PoolKernel`                                       | addresses in and out; the allocator's own state is `HashMap`/`Vec` |
| `Ring::new`/`init`                                                          | a ring laid out as views over a buffer                             |
| `sys/errno.rs`, `kit/{math, mat4, fdlibm, lattice_hash}`, `rng::SplitMix64` | no heap                                                            |

## Roles no existing structure fills

- **Text**: the role of `FixStr`/`Text`/`TextLane`.
- **Lanes of records**: `Vector` and `Lanes` take numeric `AnyElement`s only.
- **A map with values**: `main`'s `IndexedMap` holds keys only.
- **A short inline list**: the role of `InlineVec`.

Which existing structure is extended for each is not decided here.

## The removal itself is dropped

Decided the same day, once the question was asked of the heap rather than
of the containers:

- **The Rust heap is already in the core memory.** `pxl-wasm`'s global
  allocator is xpute's `FixedHeap<ModuleHeap>`, over what is left of the
  module section after the stack and the data (`__heap_base` to
  `MODULE_BYTES`, about 31 MB). It never grows the memory; past its range
  an allocation traps.
- **It has no GC.** A block is a pop or a push on its size class's free
  list. The collector stalls on record
  ([where the allocation goes](./20260910-where-the-allocation-goes.md))
  are the JavaScript heap's and Blink's, not this one's.
- **A section fills up just the same.** Moving allocations from the heap
  into the fixed section changes where exhaustion happens and how it
  fails (a refusal instead of a trap), not whether. The containers above
  made it worse on one count: `FixedAlloc` never gives a block back, and
  the lanes were reserved at their worst case.
- **No issue gave a reason to remove it.** The one trace is `memory.rs`'s
  "its heap for as long as the kernel still has one".

What stays open is not the heap but the shape of the data: the kernel's
`Vec`/`String`/`HashMap` came in with the line-by-line translation of the
TypeScript (`d23ecfe`, `40d76ab`, `b13a0e6`), an object per thing, where
the move wants lanes and indices. That is done structure by structure,
from how each is read, not as a sweep over the heap.

## The containers are gone

Also the same day. The uncommitted layer on top of the checkpoint was
discarded, and the checkpoint's own invented types removed by deleting them
and following the compiler: xpute's `kit/text.rs` (`FixStr`, `Text`) and pxl
`memory.rs`'s `FixedAlloc`, `FixedVec`, `InlineVec`, `LaneKey`/`LaneMap` and
`Span`/`TextLane`. Their sites went back to `Vec`, `String` and `BTreeMap`
(a `BTreeMap` where the walk's order was part of the behavior), and the POI
index's `SpatialHash` and `IndexedMap` to their default allocator. `Quoted`,
a formatter rather than a place to keep text, moved into `kit/json.rs`.

Where a capacity carried meaning it stayed as a length check: the ledger's
rings, a chunk build's decor instances, the scheduler's slots and pages, a
search's results. Everywhere else, what was refused past a capacity now
grows; a geocode language tag past sixteen bytes is no longer dropped.

## Where this stands

**Superseded 2026-09-17** by
[20260917-the-slab-over-the-buddy.md](./20260917-the-slab-over-the-buddy.md),
in the one part that has moved.

What was found here stands, and is why the removal was dropped: the branch had
replaced `Vec` and `String` with containers invented for the purpose rather
than with what `crates/xpute/core` already had, and the counts above are what
that was decided on. `Quoted` earning its place is the other half of it, and
the last three copies of a hand-written `json_string` went to it on 09-17.

The heap itself is no longer the one described. `FixedHeap` over the module
section is gone: the module's allocator is a slab of size classes over a buddy
of 4 KiB pages, over the whole range past the fixed sections. A reader who
came here from the working rule that names `FixedHeap` should read that
document for what the heap is now; this one is the record of a branch that was
not taken, not of the memory as it stands.
