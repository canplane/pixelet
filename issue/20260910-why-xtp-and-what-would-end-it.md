# Why XTP rather than FlatBuffers, and the two conditions that would end it

**2026-09-10** — prompted by asking whether a hand-rolled binary format
earns its place when serde and FlatBuffers exist, and sharpened by the fact
that a Rust implementation of both halves is intended. The answer is to keep
it, but the reasons are narrower than "it works", and the conditions under
which they stop holding are worth naming now rather than discovering.

Also records the two extensions that would give it named fields, since the
shape of that was the open part.

## What is actually there

`packages/xpute/core/src/io/marshal/tree/` — 1678 lines across four modules,
plus an 818-line README. A relocatable binary tree: little-endian, child
offsets relative to the enclosing branch base, `child_rel_off == 0` meaning
physical absence with the type preserved, lazy reads rather than an upfront
decode. Used today by the terrain bake alone — `source.ts` encodes,
`bake.ts` reads.

The design intent, stated by its author: schemaless with a C memory layout,
built specifically to avoid FlatBuffers' compile-time boilerplate.

## serde is not the comparison

serde is a Rust framework rather than a format, and has no JavaScript side.
The encoder lives in the Deno bake and will keep living there whatever the
client becomes, so a Rust-only serialization stack cannot cover the write
half at all. One format that two languages read _and write_ is the
requirement, which is the thing XTP is already trying to be.

## FlatBuffers is the comparison, and it loses on this payload

FlatBuffers wants the same three properties — relocatable, direct indexed
access, lazy reads — and brings a verifier, worked-out schema evolution and
generated readers for TS, Rust and C++. Against that:

**The payload is bulk, not records.** A terrain tile is a header and a few
typed arrays at a fixed shape, 4,203,672 bytes every time. FlatBuffers'
vtable indirection is what buys schema evolution, and a tile that is
regenerated whenever `PUBLISHED_VERSION` moves has no evolution to buy. The
production traces measure `decode 0.0 ms` on those 4.2 MB, so there is also
no headroom to win: the current path is already a typed-array view over
received bytes.

**Schemaless matches the thing that changes.** The bake pipeline is the
fastest-moving part of this repo — OSM to Overture, ground channels added,
names reorganised, all inside one week. A schema file plus `flatc` plus
regenerated readers in two languages puts the heaviest procedure on the part
that changes most.

**The generated TS is object-shaped.** FlatBuffers' TypeScript bindings hand
back an accessor object per table, which is the array-of-objects layout this
project spent the same day removing from its render path.

**Alignment is already right.** This was the real risk to a Rust port and it
is handled: `spec.ts`'s `ALIGN_SZ` aligns a sequence leaf to its element
size and structural nodes to `WORD_SZ`, and `view.ts` rejects a misaligned
packet base outright. A Rust reader can cast a `f64` leaf to `&[f64]` with
no copy. A format that had not thought about this is one where the second
implementation discovers it, painfully.

## What it costs, honestly

1678 lines that are entirely ours: endianness, offset arithmetic, the fuzz
surface, version discipline. And the README says plainly that compatibility
is delegated upward —

> This layer is **not** a schema system by itself. Higher layers define
> field meaning, ordering, compatibility, and validation.

Nothing enforces that delegation. `OBJECT_STORAGE_VERSION` has already moved
four times, and "the bake regenerates anyway" is what makes that survivable
today.

## The two conditions that would end it

Not "the codebase grew". Two specific boundaries:

**Untrusted input.** Every XTP packet today comes from our own bake. The
moment one arrives from anywhere else, a hardened verifier is required — and
writing a hardened verifier twice, by hand, in two languages, is where
hand-rolled formats actually lose rather than where they theoretically do.

**Two writers of the same data.** There is one producer and several
consumers today. When the client _writes_ XTP that the server reads — which
is what object persistence means — both ends evolve independently and the
packet becomes a wire contract. That is the case vtables exist for.

Those are the same two triggers
[20260910-asset-identity-and-storage-model.md](./20260910-asset-identity-and-storage-model.md)
names for moving to a runtime asset catalog: content supplied by something
other than a deploy, and objects becoming server-persisted. Not a
coincidence — both are the moment data crosses a boundary whose far side we
do not control.

## Writing the Rust half: corpus first

Two hand-written implementations of one format drift, and they drift
silently: a disagreement about padding or string termination produces output
that looks like corrupt data rather than like a bug. The standard answer is
cheap and buys most of what FlatBuffers' codegen buys.

**Freeze a conformance corpus from the JavaScript side before the Rust side
exists.** Packets plus the values they must read, committed. The ordering is
the point: a Rust implementation developed against a fixed target converges
on the format, while two implementations developed against each other can
converge on being wrong together.

**Assert byte identity, not just read agreement.** The strongest check is
that both encoders produce the same bytes from the same input, because that
pins padding, alignment and ordering decisions as well as values. Comparing
only reads lets two encoders emit different-but-valid packets forever.

The edge cases worth fixing in the corpus are half-listed by the README's
own error model already: an empty branch, `child_rel_off == 0`, an empty
string, a string containing NUL, a zero-length typed array, a leaf landing
exactly on an alignment boundary, and the packet size limit.

## Named fields: two extensions, and they do not compete

Branch children are positional today, so adding a field in the middle breaks
every reader. The general fix is named fields, and the sharp version splits
by whether the data is homogeneous.

### Bulk and homogeneous: name the columns

Thousands of records with identical fields — POI, tile layers, cluster
leaves. Naming per record pays the name cost per row for no reason:

```
branch {
  keys:    ["id", "x", "y", "rank"]     once
  count:   n
  columns: [str_seq, f64_seq, f64_seq, u32_seq]
}
```

**The columns are the SoA lanes.** A `Float64Array` view over the received
bytes is the same array the spatial index reads, so there is no decode step
and no intermediate object — wire to memory with nothing in between. That is
the layer
[20260910-storage-model-precedents.md](./20260910-storage-model-precedents.md)
identifies as the one that cannot cross to WASM, removed rather than ported.
The existing alignment discipline already lands an `f64` column on an
8-byte boundary, so the Rust side casts rather than copies.

It is also Arrow and Parquet's layout, and it compresses better, since like
values sit together.

Evolution comes free and needs no schema discussion: the key list is inside
the packet, so a reader skips a column it does not know.

### Few and heterogeneous: a shape table

Manifests, configuration, page metadata — the JSON-object case, where
columns do not apply. The move is to separate _which fields exist_ from
_what the values are_:

```
shapes: [
  0: [key 3, key 7, key 12]
  1: [key 3, key 7, key 12, key 20]
]
record: { shape_id, payload }
```

Resolve the shape once, then read by direct offset. This is FlatBuffers'
vtable with one difference that matters here: the shape is **discovered at
write time rather than declared at compile time**, so there is no schema file
and no codegen. A new field produces a new shape id; an older reader finds
the keys it knows and misses the rest, which is exactly what a vtable buys.

It is also the array-of-structures / structure-of-arrays split applied to
serialization: the shape once, the values every time.

### Order

Columnar first. It reaches the part that is actually expensive today
(thousands of POI), it is the same layout the in-memory refactor is building
so it is not built twice, and it opens no schema-evolution question.
Positional `branch` stays as it is — it is not wrong, it is only wrong for
heterogeneous records, and both of these are new node kinds beside it rather
than changes to it.

## What was read

The README, the usage sites, and `spec.ts`'s alignment rules. Not the
encoder or cursor internals — so "specified well enough to bear a second
implementation" rests on the alignment discipline and the length of the
README, not on an audit.
