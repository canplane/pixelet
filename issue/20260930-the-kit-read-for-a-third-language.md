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
against it. All seven were fixed the same day, after the C++: `libm` left
`xpute-core`, which never called it, and the comments that cited it; the
`rng` and `FIELD_MASK64` comments say what the code does, and `FIELD_MASK`
gives Rust's answers at the word's edge; a deque's pop from empty is none and
its push onto full a bug, in all three languages; `lower.ts` keeps only the
tuple helpers the XTP README recommends; the headers name no absent pair;
and `codec/json` is filed across a protocol, the host's own JSON held to the
record by a TypeScript test.

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
  none either. ~~`codec/json` waits for a C++ guest that reads the records it
  was written for.~~ Withdrawn 10-01: see below.
- **Errors, as Rust's.** Exceptions are off, so what came from outside is a
  small `Result<T>` (C++20 has no `std::expected`), a broken invariant is `bug`,
  and absence is `std::optional`.
- **The records.** `tlv` and `xtp` are read by C++ as by the other two, to the
  byte. `rng` and `scalar` give the same bits wherever the double is IEEE, so
  C++ can be held to Rust's records for them as well.

## What writing it found

The C++ XTP was held to both records and turned up three places where the pair
disagrees with itself. All three are fixed in the three languages the same
day, each with a test of the law it broke; the first changed the `graft`
vector's root type from GRAFT to BRANCH.

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

- **Where the records live.** They were under `crates/xpute/core/golden/` and
  the traces under `crates/xpute/conformance/golden/`, both the three
  languages' and belonging to none. Moved 09-30: the records to
  `spec/xpute/golden/`, the traces beside `guest.json` in
  `spec/xpute/conformance/`, and every reader with them.
- **A guest built to WebAssembly.** Built 09-30, both guests: the module
  imports its memory and `env.now`, its stack (1 MiB, said in each build)
  and data come first, and its range begins on the page past them. The
  guest.json number for the directory went: boot answers where the directory
  is, the contract's rule 0 taken the other way it allows, so the host finds
  a native guest's at 0 and a module's past a megabyte by the same call.
  Every pointer is made from an address, the base being address 0 on a
  module. The C++ is built with the WASI SDK, and libc++'s abort on its own
  broken preconditions is the guest's `bug`, since its default printed to
  stderr and imported WASI's file calls; a module that imports anything but
  its memory and the clock fails the traces.

## C++ is held to Rust, item for item (10-01)

Two things C++ was let off were not the language's or the platform's but a
judgment that nothing needed them yet: `codec/json`, "until a C++ guest reads
the records", and the tree builder, XTP's `TreeView` and encoder, since C++
had its one writer in place. That is not one of the three reasons, and it is
withdrawn: what Rust's `core` has, C++'s has. What one side lacks for its
language or its platform — TypeScript's allocator, Rust's typed-array
helpers — is still not a gap.

Read item for item against Rust, C++ then took, beside `codec/json`
(`json.{hpp,cpp}`, held to `golden/codec/json.tsv`) and the tree
(`xtp/view.hpp`, held to `golden/wire/xtp/encode.tsv` through the tree and a
buffer that grows): a string read where it lies checked as UTF-8 and one
decoded into its own with U+FFFD where it is not, as Rust's `get_str_ref`
and `get_str`; the same check for a string sequence; `get_deep` and a
branch's values and children; `Strs`'s checked access; TLV's string decoded
as Rust's reader decodes it; the allocators' `stat` row; a deque's slot let
go of on a pop and a clear; a slot past the arena's end a bug; `or_bug` for a
result; errno names and `ErrnoResult`; ties to even by arithmetic rather than
the FP environment's mode, with the contraction held off in the header; a
record's number read as Rust reads one; `elem_sz` of a string sequence its
offsets' word; offsets stored little-endian. Two went the other way, where
Rust was the one that differed: its `PacketWriter` now judges a string by its
UTF-16 units as the tree encoder and C++ do, and `get_array` reads an array
only as the type it was written as. A unit of 0 is EINVAL to `align` in both.

What stays apart is said where it is: a Rust packet must lie on a word to be
read, since Rust views elements in place, and C++ loads them where they lie;
and a shallow value's shape follows each language — a bitset packed or a
list of bytes, a string sequence its own type or a list.

## `codec/json` is a reader, and nothing more (10-01)

What made `codec/json` a protocol was a string map written by one side and
parsed by the other, the POI's tags in a packet. That left with the tags on
09-25, and the map's `stringify` and `parse` stayed behind with no caller
but their record, as did a TypeScript `codec/json.ts` nothing imported. All
three are gone; the bake's coast tags are a list of pairs of its own.

What is left is the reader: a glTF's document and the settings a guest
kept, read where they lie. Its numbers had been scanned as Rust's `f64`
parse takes them, so `+1`, `.5` and `1.` were JSON to both guests and not to
the host. They are read by JSON's grammar now, and the record holds which
texts are JSON at all, judged by the host's own `JSON.parse` and by both
guests' readers.

## Where it stands

**Built** 09-30. The C++ `core` is the kit: `abi`, `alloc`, `codec`,
`collection`, `math`, `status`, `wire` and `golden`, tested by
`deno task test:cpp` against the records the other two read. Its XTP has a
writer in place that gives the tree encoder's bytes, the tree and its
encoder over it (10-01), and a reader that loads elements where they lie, so
a packet need not be aligned.

The Rust harness is gone. Each guest is a library of the same two C calls, its
state in the memory it is booted over, and one host —
`packages/xpute/conformance/`, the TypeScript runtime over Deno's FFI — runs
the traces against both. The C++ is built by `script/cpp.ts` with the
platform's compiler, and `cc` has left the workspace.
