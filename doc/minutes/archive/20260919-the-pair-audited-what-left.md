# The pair, audited: what left and why

**2026-09-19** — `@xpute/core` and `xpute-core` were read module against module,
each Rust file against the TypeScript it came from, with one question per
module: is this here because the problem needs it, or because JavaScript could
not do better? This records what left the pair, what stayed, and the rule the
answers came from.

## The rule

The TypeScript is the original and its semantics are the contract, but its
_shape_ is not: it says with names what it cannot say with types
(`u64 =
bigint`, `u8 = number`), splits typed arrays into two families that will
not union, carries comparators as closures because it has no `Ord`, and carries
streams as closures because it has no value to hold one in. None of that
crosses. A Rust module is judged on whether Rust already provides the thing —
std, the type system, a struct's fields — and on whether the project already has
it somewhere else.

Two things do not count. **"Something calls it" is not a reason to keep**:
ceremony is called too. **"Nothing calls it" is not a reason to delete**:
`Arena` had no caller and stayed, because a pool is a real technique (`truncate`
drops nothing, so a slot's own allocations survive the rewind) and `Vec` gives
it only through unsafe `set_len`.

Nor do the golden records decide anything. They are what the JavaScript runtime
printed, so they carry the very collapse being removed: `comp_str` ordered by
UTF-16 code unit because a vector script chose pairs the two orders disagree on
and the Rust was bent to match. Tests come after the structure stands.

## What left

| Module                 | What                                                                                          | Why                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ---------------------- | --------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `collection/arena`     | `Vector<T>`                                                                                   | It is `Vec<T>`. The TypeScript has it because a typed array has no `push`; with `T: Copy` there is no `Drop` and nothing a pool could save                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `collection/lanes`     | whole, both languages                                                                         | Named heterogeneous lanes under one length is what a struct of `Vec<T>` fields _is_; the Rust port had re-erased the types into a `Kind` enum, a string lookup per access and an `&mut` out of `&self`. The kernel's four lane tables were already plain structs                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `collection/map`       | the Rust side, with its test and golden                                                       | `IndexedMap` is `indexmap::IndexSet`: insert appends, `swap_remove_index` moves the last entry into the hole, and a caller keeps its lanes at the same positions — held to each other over 500k random operations with no divergence. Its six operations in use (`find`, `insert`, `remove_at`, `key_at`, `size`, `clear`) are `get_index_of`, `insert_full`, `swap_remove_index`, indexing, `len`, `clear`; `positions_seeded`, the seeded permutation the structure was written for in January 2026, had no caller in either language. Of the five Rust consumers, two only asked membership and are a `HashSet` (`poi/index`'s seen ids, the waker's per-topic seen), one kept a value at the position and is a `HashMap` (the change log's revisions), two walk positions with lanes beside them and are the `IndexSet` (the page table, the badge tables). All hash by `fnv`, so no random state. The TypeScript side stays: JavaScript has no set with positions |
| `collection/map`       | `KeyTraits::norm`                                                                             | Identity in both impls: `asIntN` and `>>> 0` are what `i64` and `u32` are. The width is the canonical form                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `collection/map`       | its own `splitmix64`                                                                          | Byte-identical to `kit::rng::mix64`; the TypeScript now points there                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `kit/rng`              | `RNG`, `make_rng`                                                                             | A `Box<dyn FnMut>` around `SplitMix64`, which is eight bytes and `Copy` and what the decor scatter already used directly                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `collection/treap_set` | whole, both languages, with its golden                                                        | Its one consumer was the peer directory's role sampling (`rpc/peer/` at `checkpoint/20260405-xpute`), which asked for a random start and a bounded window — O(1) on a dense index — and never for the sorted order the tree kept. Reduced to its general form, six of eight operations were `BTreeSet`'s; the two left were `kth` in O(log n) and a per-instance comparator the consumer passed as a constant. Comes back only if `rpc/` returns needing sorted order **and** `kth` **and** frequent change together; sorted with rare change is a sorted `Vec`, unsorted is `IndexSet`                                                                                                                                                                                                                                                                                                                                                                                |
| `ds/handle`            | `HandleTable` on the TypeScript side, with its test                                           | The table that issues handles is the kernel's; the shell only reads the slot off one to index what it keeps under it, and a second table there would be a second allocator of the same handle space. No TypeScript ever constructed one. What the shell needs is `handle_slot`, which the host side of `@xpute/runtime/abi/handle.ts` gives it. The table is `xpute_runtime::abi::handle` now: Rust-only for a reason of role — the guest issues, the host decodes — and the 16-bit split is written once in each language rather than generated, since it is half of a u32 with a `u16` generation lane to match and not a number anyone turns                                                                                                                                                                                                                                                                                                                        |
| `kit/encoding`         | the Rust side, with its test and golden                                                       | What was left in it — `Uuid`, `generate_uuid`, `ct_eq` — had no Rust caller anywhere; the hex pair the tests import went to the `hex` crate and base64url to `base64` earlier the same day. The TypeScript's `encoding.ts` stays whole: four shell files import it as a module for base64url, uuid↔b64url and the pid check                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `kit/rng`              | `set_entropy`, `entropy`, `seed64`, both hooks in `pxl-wasm` and the shell's `entropy` import | The one host-installed slot that lived in `core`: `pxl-wasm::install_host` set twelve hooks, eleven through `pxl_kernel::host` and this one through `xpute_core::kit::rng`, and its only reader was `generate_uuid`. The TypeScript `seed64` stays, since there `crypto.getRandomValues` is the language's own                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `kit/comp`             | the Rust side, with its golden                                                                | `comp_numeric` is `partial_cmp().unwrap_or(Equal)`, `make_comp` is `cmp().then_with()`, and a comparator in Rust is a closure or `Ord` at the call that sorts. `comp_str`'s UTF-16 order had no Rust caller at any point: the first port compared by code point (`0ab624c5`) and the same day's vectors (`d23ecfe6`) bent it to what the JavaScript printed                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |

The comparator module's TypeScript stays. The pair does not have to behave
identically here: what must agree across the boundary is a format — the ring's
layout, the message words, the frame, a page — and a kit function that sorts is
not one. If a string order ever does have to agree, it is
`a.encode_utf16().cmp(b.encode_utf16())` at that call.

## What stayed, and the fact it stayed on

- **`collection/arena`'s `Arena<T>`** — the pool. `truncate` moves the cursor
  and drops nothing; a `Vec` would free and re-allocate every slot's own buffer
  per rewind. The `_factory` closure is what `T: Default` cannot do when a slot
  is sized at construction.
- **`math/rng`** (`kit/rng` until today) — std has no seedable PRNG, so
  `SplitMix64` is the only way. `next_u32` still draws through an f64 —
  `U32(Number(z))` reproduced — because `golden/math/rng.tsv` and the decor
  scatter are placed from that sequence; changing it changes the world. It is
  under `math/` now, beside `scalar`, `vec3` and `mat4`, because a seed and the
  stream from it are arithmetic and nothing else: the host hook that shared the
  file is gone (below).
- **`collection/deque`** — `VecDeque` owns and grows; this is a ring over a
  buffer the caller owns. `Option<T>` per slot is how a value leaves a borrowed
  slice without unsafe.
- **`collection/heap`** — `BinaryHeap` wants `Ord` and `f64` is not; the array
  is the caller's. `MinHeap`/`MaxHeap` are one `Heap<T, const MIN: bool>` now:
  the TypeScript duplicated 106 lines because a comparator there is a call per
  comparison, and here the bool folds at instantiation.
- **`handle`** (now `xpute-runtime/sys`) — a slot allocator with generation
  lanes, `[u16; N]`, `[bool; N]`, `[u32; N]` inline under a const capacity:
  nothing in std, and already the Rust shape. Its consumers are the renderer's
  surface and object tables, which already held it in `sys::global::Global`.
- **`space/lattice/` in `pxl-spec`** (`grid_index`, `spatial_hash`, `hash`) —
  Mapbox GL's screen-space collision grid and Teschner's broadphase, with the
  lattice hash both bucket by. Nothing in std, and already the Rust shape; moved
  out of `xpute-core` because a structure over cells is spatial and the kernel
  is its only reader, beside `sparse_tree` which was already there. The
  TypeScript three left: the shell never holds the world, so it never asks what
  is near. `grid_index`'s three-way answer was an `i32` with two negative codes
  and is `enum Placement`; `NEIGHBOUR_CELLS_3D`, a constant nothing read, left
  both languages; `lattice_hash3/4` are `hash3/4`.
- **`alloc/` in `xpute-core`** (`buddy_tree`, `slab`) — the allocator, with no
  TypeScript at all. Out of `ds/` because a slot allocator is not a collection;
  in `core` rather than `runtime` because it runs over any `Range` and asks
  nothing of a doorbell or a tick — `pxl-wasm` installs it, `sys/mem` in the
  runtime says where the range is, and `Vec` and the runtime's `Block` API are
  its callers alike. `alloc`, not `heap`: this crate's `heap` is the priority
  queue, as Rust's `BinaryHeap` is.

## What the reading changed besides

Rust convention where the TypeScript's had come across: `Ordering` for `-1/0/1`;
`Option<T>` from a `pop` that returned `T` and lied; `is_empty()`/`is_full()`
for `empty()`/`full()`; `u32` for a `limit: i64` guarded by `<= 0`, so the guard
is the type; `impl<T> Deque<'_, T>` where `'a` was named and unused; accessors
dropped where they echoed a public field. Comments rewritten in the present
tense about this code, with the TypeScript's mechanism — `Number`, `BigInt`,
`asIntN`, `dispose`, `V8` — out of them; what was left of that across the crates
is queued in [BACKLOG.md](../BACKLOG.md) under P3, as is the leading underscore
the TypeScript used for privacy and Rust reads as "unused".

## The shelves, by what a thing is

`kit/` had been the shelf for whatever had no other shelf, in both languages,
and reading it whole showed four things on it that are not one:

1. **What JavaScript lacks as a language** — the machine word and the view over
   memory: `word` (the widths, `fits_*`, the masks), `array` (typed arrays over
   `ArrayBuffer`/`SharedArrayBuffer`), `cast` (a width's reinterpretation).
   `word.ts` names itself "ABI surface", and the three crates' `abi/` are each
   "what two sides agree on by number", so they are `abi/` now. Rust has none of
   this: the language is it.
2. **A representation and its bytes** — `encoding` (UTF-8, base64url, hex,
   uuid), `hash` (FNV-1a, sha256), `json` (`JSON.stringify` to UTF-8 and back,
   `io/json_codec` until today). AGENTS.md lists the kit as "collections,
   codecs, XTP, the errno table, the libm", and these are the codecs, so
   `codec/`; the wire formats, XTP and TLV, are `wire/` (`marshal/` until today
   — a verb for the act, where `wire` names what is on the line), and the line
   between them is whether the bytes are the wire (`tlv`, `tree/`) or a text.
   `lower` — the JavaScript value domain the tree encoder accepts, with the key
   order — is the tree's front and is `wire/lower`. Rust's `codec/` holds `json`
   alone: base64url, hex and FNV are crates there.
3. **The language's own generics** — `comp` (a comparator, `core::cmp` there),
   `iter` (`core::iter`), `type` (`Async`, `Nullable`). These stay `kit/`, which
   now means exactly this: what a language gives and JavaScript spells by hand.
   Rust has no `kit/` for the same reason it has no `abi/word`.
4. **Which runtime this is** — `detect_env`, now `env/detect`: Deno, Node or a
   browser, asked at run time. Nothing in `xpute-core` asks it, and nothing in
   Rust could.

Nothing was removed for having no importer. What each file is decided its shelf;
whether it is used decides nothing here.

## Where it stands

**Through** for all of `collection/` (`ds/` until today), for `alloc/`,
`handle`, `math/` (`scalar`, `vec3`, `mat4`, `rng`), the old `kit/` on both
sides (`encoding`, `comp`, and `json`, which stays: no crate writes JavaScript's
own-property order, and the reader is a cursor over borrowed text where a
crate's is a DOM), and the removals. `math.rs` is a barrel now, as
`collection.rs` and `kit.rs` are; the scalars are `math/scalar.rs` and `Vec3` is
`math/vec3.rs`, out of the matrix file four of its twelve users never needed.
`wire/` (`tlv`, `xtp/*`) read line by line against the 09-12 TypeScript on
09-19, and what differs is the language's edge — a shift past the word, a NaN
cast, a division by zero — never a byte of a packet; `abi/word` holds the
bit-field helpers that had been copied into six files. Not yet read as a pair:
`status/error` and `status/log`. The tests the removals and signatures broke
were the next thing after the structure stood, and are green.
