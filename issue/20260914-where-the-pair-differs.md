# Where the pair differs

**2026-09-14** — the kits had just been mirrored into crates file for file
([what stays in JavaScript](./20260914-what-stays-in-javascript.md)), and
the question was whether the Rust computes what the TypeScript does. This
is what was checked, what was wrong on either side and fixed, and what
differs because the languages do and stays.

## How it was checked

Three passes, each catching what the one before could not:

1. **The pair check** (`deno task check:pair`): every TypeScript file,
   export and test has its Rust counterpart at the same path. It runs one
   way on purpose — the Rust may carry more.
2. **Vectors** (`deno task vector`): the TypeScript writes what it
   computes for a spread of inputs into `crates/*/*/vector/*.tsv`, and the
   Rust test beside the module reads them back and must agree — to the bit,
   except where Math's transcendental functions are in the path (below).
3. **The lines no vector reached.** `cargo llvm-cov` over the vectors and
   tests listed 1,579 lines never run; each was read against its
   TypeScript. Tests only find what an input happens to exercise; this is
   how the error paths and the arithmetic at the edge of u32 were found.

Not read line against line: `kit/array.rs`, which has no TypeScript (it is
the typed arrays themselves), and msun's internal branches in
`kit/fdlibm.rs`, whose original is FreeBSD's C, not a TypeScript file.

## Wrong in the TypeScript, fixed there

Each is pinned by a vector now, so both sides answer the same.

- **`io/scalar.ts` `unpack96`** took each limb through `Number()` before
  masking, and a value past 2^53 lost its low limbs to rounding.
- **`ds/buddy.ts` `handle_at`** bounded the region index by
  `1 << (32 - order_bits - max_order)`. With a region of one minimum block
  the exponent is 31, `1 << 31` is negative in JavaScript, and every handle
  was refused.
- **`io/marshal/tree/spec.ts` `ALIGN`** aligned with int32 `&`: a size past
  2^31 came out negative, and one past 2^32 − 8 wrapped. It now refuses a
  size that does not align within u32 (EOVERFLOW).
- **`io/marshal/tree/cursor.ts` and `encode.ts`, BITSET** sized the payload
  as `(len + 7) >>> 3`; a length word of 2^32 − 1 wrapped to a size of 0,
  passed the bounds check, and the read went on.
- **`spec/space/sparse_tree/mem.ts`** sized the heap lane as `used << 2`,
  int32 again, past 2^30 entries.
- **`ipc/frame.ts` `align8`**, the same `&` wrap, where a negative body
  offset passes every "does it fit" check.
- **`kit/encoding.ts` `hex_to_bytes`** went through `parseInt`, which reads
  what it can and stops: `"1g"` was 1, `"zz"` stored as 0. A pair that is
  not two hex digits now throws.

## Wrong in the Rust, fixed there

- **Lengths off the wire in u32.** The TypeScript sums a length read from a
  packet with an offset in doubles and gets an honest EBADMSG; the Rust
  summed in u32, overflowed — a panic in a debug build, a wrap under the
  end and an out-of-bounds panic in release. For a module reading what a
  server sent, that is the whole module stopping on one bad packet. Now in
  u64: TLV's `_need`, the tree cursor's BITSET and STR, sparse_tree's
  `read_mem` (which refuses as ALIGN does), and the frame's `align8` and
  `lay_out`. The TLV and tree cases are vectors.
- **Growth by doubling** (`ds/arena.rs`, `ds/lanes.rs`) saturates, so a
  capacity past u32 reaches `max_cap`'s refusal instead of wrapping under
  it, as the TypeScript's doubles do.
- **`ds/buddy.rs` handle packing** shifted a u32 by 32 when the region can
  only be region 0; the shift is taken in u64.
- **`kit/comp.rs` `comp_str`** compared by code point (`str`'s `<`);
  JavaScript compares UTF-16 code units, and past U+FFFF the two disagree
  (`"｡"` is after `"😀"` there). It compares UTF-16 now.
- **`kit/mat4.rs` `or_one`**, the port of `x || 1`, missed that NaN is
  falsy too.
- **`io/marshal/tree/encode.rs`, scalars.** A scalar node given the other
  numeric family (a bigint for a u8, a number for a u64) was converted; a
  DataView setter refuses it. It is refused now (EINVAL). BOOL took NaN as
  true; `BOOL()` takes it as false.
- **`kit/encoding.rs` `generate_uuid`** drew from `seed64`, whose fallback
  without host entropy is a counter: the same UUIDs every run. It refuses
  without an entropy source, as a missing `crypto.getRandomValues` does.

## Different because the languages are, and kept

- **A throw is a `Result` or a panic.** A malformed packet or a refused
  size is `Err(MarshalError)` with the same errno the TypeScript throws. A
  programmer's error — a bad capacity, a double registration, a bad hex
  string, an index past a typed array (where JavaScript reads `undefined`)
  — panics. A JavaScript throw can be caught; a panic in the module stops
  it, which is why anything that reads outside input is a `Result`. The one
  refusal whose kind differs: an encoder scalar of the wrong family is a
  `TypeError` there and a `MarshalError` (EINVAL) here.
- **Numbers.** The TypeScript computes in doubles and reaches u32 through
  `>>> 0`, `| 0` and typed-array stores; the Rust has widths. Where the
  TypeScript's double could exceed u32 (a wire length plus an offset), the
  Rust computes in u64; `x | 0` is ported as ToInt32 (fraction dropped,
  modulo 2^32, NaN to 0).
- **Math.** `sin`, `cos`, `tan`, `atan`, `atan2`, `exp`, `log` are FreeBSD
  msun's in the Rust and LLVM libc's correctly rounded ones in V8: they
  agree within one ulp, and a result computed from them within a few. The
  Rust is the reference; a lattice address never rests on that last bit.
- **NaN's payload** is the engine's in JavaScript and not observable; V8's
  changed from run to run. Vectors write one canonical NaN.
- **Entropy.** `seed64` without a host source is a counter in the module
  and time plus `Math.random` in JavaScript. The module has no clock or
  entropy of its own until the host gives it one (`rng::set_entropy`).
- **Shapes.** A getter/setter pair is `x()` and `set_x()`; a static and an
  instance method of one name are `build_of` and `build`; `get(idx)` is
  `get_at`; a generator is an iterator struct (`Inorder`, `From`,
  `next_value`); a union is an enum (`Cursor`, `Numeric`, `SequenceVal`); a
  returned closure or object is a type (`Carver`, `Subscription`,
  `Compacted`). `Deque` borrows its buffer rather than holding a reference
  to it, `retype` consumes the tree and returns the retyped one, and
  `subscribe` returns a token `unsubscribe` takes. Each does what the
  TypeScript does; only the call site differs.
- **What only the Rust has.** `impl PoolKernel for BuddyAllocator`,
  `Ring::push_entry`, `rng::set_entropy`/`entropy`, `ds/fixed_heap.rs`,
  `kit/array.rs`, `kit/fdlibm.rs`, `kit/json.rs`, `sys/console.rs`. The
  kernel is where the work goes; a TypeScript twin is written when the
  shell needs one.
- **The host's JSON.** A record's tags are the text `JSON.stringify`
  writes, and the Rust must write the same bytes: `kit/json.rs` is that
  one shape — a flat string map — with the host's key order (array-index
  keys first, ascending, then insertion order), its escapes, and
  `undefined` left out. `JSON.parse` of anything else is refused there
  where the host would hand it back.
- **A catalog is a module.** `cube_types.land.SEA` is
  `cube_types::land::SEA`; a channel's entries in the object's key order
  are its `ENTRIES`, and the object of channels is `CHANNELS`. A
  `Record<number, …>` walked by `Object.entries` walks integer keys
  ascending, which a `BTreeMap<u32, …>` is. A string-keyed id
  (`"land.GREEN_DEEP"`) is a `String`.
- **A generated table is generated twice.** `record/ingested.ts` and
  `record/ingested.rs` are both written by `ingest_asset.ts`, from the
  same rows (`apps/api/src/bake/asset/table.ts` renders each), so the two
  cannot drift; the ingested vector holds them together anyway.
- **Mirrored and unused.** `spec/space/adapter/yup` is referenced by
  nothing on either side. It stays, as the rest of the mirrored tree does.

## The rest of @pxl/shared, mirrored (2026-09-14, later)

`spec/cell`, `record` and `bake` came over whole, at their paths, with
vectors for every file: what a client moving into the module will reach
for is there before it asks, and what turns out unreferenced is swept
later in one pass rather than guessed at now. What the read of them found,
recorded here so the reshaping that follows starts from it:

- **`bake/` is three things under one name.** The wire formats both
  sides read (`format.ts`'s terrain and POI page, `published.ts`'s
  versions) are a contract and belong in the shared package, though
  "bake" names the writer, not the format. `constant.ts` is the unit
  system, the grid spans and the elevation sampling contract — space, not
  bake — and `ground.ts` with `format.ts`'s `GroundChannelKey` is the
  cell model, which `spec/cell/mod.ts` imports _from_ bake, the
  dependency pointing the wrong way. And `format.ts`'s DB row shapes
  (`BUILDING_ROW_KEYS`, `BuildingRow`, …, `CellConfig`) with
  `model/feature.ts` (`Feature`, `CoastRawFeature`, `FeatureExtra`) are
  the server's own bake pipeline: `apps/web` imports none of them, and
  `POI` is the input to `encode_poi_page`, which only the server calls.
  `feature.ts` and the encoder side of the POI page are `apps/api`'s, and
  the client keeps the reader.
- **`constant.ts` names nothing bake-specific.** Four groups: units
  (`CELL_SHAMT`, `U_PER_CELL`, `XY_CELL_PER_M`, `Z_CELL_PER_M` — unused),
  grid (`PAGE_CELL_SPAN`, `CHUNK_CELL_SPAN`, derived from the indexer's
  shifts), the elevation sampling contract (`PAGE_ELE_SPAN`, `ELE_PAD`,
  `ELE_PADDED_SPAN` — the rule the server's SQL resample, the client's
  bicubic and the module's `bake/terrain.rs` all follow) and the object
  layout (`OBJECT_LAYOUT`, the placement model's). `crates/pxl/core/src/
  space/constant.rs` was a hand copy with the shifts as literals; it
  reads from the mirrored `bake/constant.rs` now that one exists.
- **`spec/cell` is the type numbering and colours.** The client reads it
  widely; the server reads `cube_types` only. The module's
  `bake/terrain.rs` restates the channel order, `SEA` and the water rule
  by hand, and is what the mirrored `cube.rs`/`ground.rs` replace.
- **`record/` is the client's alone.** The palette, the placement, the
  table, the shipped set and the texture pack: `apps/api` imports none,
  `apps/web` reads them from the object store, the texture layers, the
  decor kinds and the character variants. The module's `world/object.rs`
  keeps a palette of its own; when the object store comes into the
  module, `record/` is what it reads.

The mirror keeps the tree as it is. The reshaping — space constants to
`spec/space`, ground to `spec/cell`, the server's pipeline types to
`apps/api`, the formats under a name that says what they are — is done
on both sides at once, after the client's pieces have moved and before
the orphans are swept.

## Going forward

Moving an upper layer into the module — so the shell stops reaching in
through exports — brings its dependencies with it: for every TypeScript
file the moved code references, if the `.rs` pair is not there yet, the
whole file comes over at its path, under the pair check, and its vectors
with it. Guessing which will be needed is not the method: a file that
looked unreferenced is asked for the moment the next piece moves, so the
tree comes over whole and the orphans are swept later, in one pass, once
the structure has settled.

The line between the two crates is mechanism and policy, as an OS draws
it. `xpute-core` is the substrate: the scheduler that runs resumable jobs
within a budget and yields, the allocators and their refusal, the rings,
the change journal and the wake-ups, the lanes and containers, errno.
`pxl-core` is what is scheduled and in what order, what is reclaimed
first and what is protected, what a chunk, a page, a tier or a placement
is. WebGPU, the renderer, assets and the spatial model are none of
xpute's — at most a middleware layer on top of it later, never its
substance — and a name from that side appearing in `crates/xpute/core`
is a layer violation to fix, not a convenience to keep.
