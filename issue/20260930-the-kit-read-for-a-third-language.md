# The kit, read for a third language

**2026-09-30** — xpute's guest end is now written in C++ as well as Rust, as far
as a conformance guest needs (`cpp/xpute/`), and the next step is to give it the
same `core` the other two have. Before writing it, this reads how TypeScript and
Rust each made the one kit their own: which parts are the kit twice, which are
one language's, and why. The C++ `core` is shaped from that reading rather than
from either implementation. It follows [the pair audited](./20260919-the-pair-audited-what-left.md)
and [inside a language, and across a protocol](./20260919-inside-a-language-and-across-a-protocol.md),
which drew the lines this applies.

## C++ is a third world, not a second host

Rust and C++ are parallel implementations, not one serving the other. The
first C++ guest was driven by a Rust test harness that played the host, which
made Rust a host of C++: a second host, written for the test, beside the real
one in TypeScript. That goes. Every guest, in whatever language, is driven by
the one host that exists, and every language keeps the same shape — `core`,
`runtime`, a conformance guest — so none of them is where the others are held.

## The kit twice, by design

These are one kit in both languages, differing only in how each language says
it: `abi/cmd`; the bit fields of `abi/word`; `collection/{arena,deque,heap}`;
`math/{rng,scalar}`; `status/{errno,error}`, errno generated from `spec/`;
`wire/{tlv,xtp}`; and `golden`, the record reader.

Where the pair differs inside them, it is the language's idiom:

- **Errors.** Rust returns `Result<_, MarshalError>` for what came from outside,
  stops on a broken invariant through `bug!`/`ensure!`, and says absence with
  `Option`. TypeScript throws both kinds (`MarshalError`, `InvariantError`) and
  says absence with `undefined` or `null`.
- **Widths.** Rust carries a width in the type, so a TLV value read back keeps
  the width it was written at. TypeScript has `number` and `bigint`, so it
  infers a width when it writes and loses it when it reads.
- **Checking.** Rust's `deque` and `heap` answer an empty pop with `None`;
  TypeScript's do not check.
- **Additions.** Rust's XTP has `PacketWriter`, which writes a packet into the
  caller's bytes with no allocation; TypeScript's `arena` has `Vector`, a
  typed-array store Rust has no need of beside `Vec`.

## One language's, and the three reasons why

Every module present on one side only is there for one of three reasons.

1. **The language already has it.** TypeScript needs `abi/array`, `abi/cast`,
   `kit`, `wire/scalar` and its own indexed map (`collection/map`); Rust has
   slices, `as`, traits and `indexmap`. TypeScript writes `codec/encoding` and
   `codec/hash`; Rust takes `base64`, `hex` and `fnv` as crates.
2. **The platform lacks it.** `alloc` (buddy and slab) is Rust's alone, since
   JavaScript has no heap to hand out; `status/bug` is Rust's alone, since a
   thrown JavaScript error carries its own stack; `env` is TypeScript's alone.
3. **A protocol needs the same bytes.** Rust's `codec/json` is a byte-exact
   port of the host's `JSON.stringify` and `parse`, for the records' tags;
   Rust's `rng` draws through a double, as JavaScript's does, so the scatter the
   decor is placed from is the same; `tlv` and `xtp` are held to each other by
   the records both read.

## What the reading found

None of these is fixed here; each is recorded so the C++ `core` is not written
against it.

- `xpute-core` declares `libm` and no source in it uses it, while `golden.rs`
  and `lib.rs` say Rust rounds through it. Either the dependency or the
  comments are left over.
- TypeScript's `rng` says its draw is the word's low 32 bits. It goes through a
  double, so past 2^53 it is not; Rust's comment says so and is right.
- TypeScript's `deque` pops without checking, and an empty one's length is left
  wrong.
- TypeScript's `FIELD_MASK(32)` is 0, since `1 << 32` is 1 in JavaScript; Rust's
  is `u32::MAX`. Its note says the case is not guarded.
- `wire/lower.ts` is imported by nothing but the XTP README.
- Rust's `tlv`, `xtp/cursor` and `xtp/encode` tests still open on "no pair: the
  TypeScript has no test file", which stopped being true on 09-19.
- `golden/codec/json.tsv` is filed as a record inside a language, and what it
  holds is the bytes JavaScript writes.

## What follows for C++

The three reasons, applied to C++ rather than copied from either side:

- **The kit twice becomes the kit three times.** All of it, in the same modules.
- **The platform.** C++ has a heap to hand out, and a guest must hand out its
  own rather than take the system's, so `alloc` is C++'s as it is Rust's. With
  exceptions off, an error carries no stack, so `status/bug` is C++'s too.
- **The language.** The standard library has no base64, hex, FNV or
  order-keeping map, so `codec/{hex,base64,fnv}` is written, as TypeScript's
  is; the map waits for a runtime that asks for one, since Rust's `core` has
  none either. `codec/json` waits for a C++ guest that reads the records it was
  written for.
- **Errors, as Rust's.** Exceptions are off, so what came from outside is a
  small `Result<T>` (C++20 has no `std::expected`), a broken invariant is `bug`,
  and absence is `std::optional`.
- **The records.** `tlv` and `xtp` are read by C++ as by the other two, to the
  byte. `rng` and `scalar` give the same bits wherever the double is IEEE, so
  C++ can be held to Rust's records for them as well.

## What writing it found

The C++ XTP was held to both records and turned up three places where the pair
disagrees with itself. None is fixed here; the C++ writes the bytes the records
hold.

- **A grafted root does not read back.** The tree encoder writes GRAFT as the
  header's type when the root is a graft, where a grafted child takes its
  root's type; `golden/wire/xtp/encode.tsv` records it, and the reader refuses
  the packet (EBADMSG). The fix is the three languages' and the record's
  together.
- **Rust's `PacketWriter` and its tree encoder lay one tree two ways.** The
  encoder ends a branch on a word; the writer does not, so a byte after a
  branch whose last child ends off a word lies at 57 from one and 64 from the
  other. The C++ lays it as the encoder does.
- **The tree encoder refuses packets that fit.** Its buffer doubles from 1 KiB,
  and a doubling past the cap is refused even when the packet is under it: a
  cap over 1 KiB that is not 1 KiB times a power of two refuses what fits, and
  at the default cap every packet between 1 GiB and 2 GiB. The records hold
  only caps under 1 KiB, where the buffer never grows. The C++ writes in place
  and refuses only what does not fit.

## Open

- **Where the records live.** They are under `crates/xpute/core/golden/` and
  every language reads them there; the traces are under
  `crates/xpute/conformance/golden/`. Both are the three languages' and belong
  to none, which argues for `spec/xpute/` — a move every language's reader
  follows.
- **A guest built to WebAssembly.** The host drives each guest as a native
  library; the same two calls over a guest built for `wasm32` is the
  arrangement a page runs, and asks a C++ build for `wasm32` with `memcpy` and
  `abort` supplied, or a WASI SDK.
- **The findings**, both lists, fixed before the C++ is written against the
  pair, or after.

## Where it stands

**Built** 09-30. The C++ `core` is the kit: `abi`, `alloc`, `codec`,
`collection`, `math`, `status`, `wire` and `golden`, tested by
`deno task test:cpp` against the records the other two read. Its XTP is one
writer, in place, that gives the tree encoder's bytes, and a reader that loads
elements where they lie, so a packet need not be aligned and nothing is
allocated.

The Rust harness is gone. Each guest is a library of the same two C calls, its
state in the memory it is booted over, and one host —
`packages/xpute/conformance/`, the TypeScript runtime over Deno's FFI — runs
the traces against both. The C++ is built by `script/cpp.ts` with the
platform's compiler, and `cc` has left the workspace.
