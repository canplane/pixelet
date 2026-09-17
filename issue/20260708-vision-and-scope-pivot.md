# Vision and scope pivot: from decentralized metaverse to a demo MVP

**2026-07-08** retrospective, MVP direction fixed **2026-07-30**, further
confirmations **2026-08-11** — what this project originally set out to be, why
that scope became unsupportable, and what was kept.

**Status: the original direction is parked, not discarded.**

---

## The original target

An OSM-backed "Minecraft-like" spatial-computing metaverse. The load-bearing
decision was architectural: **peer-to-peer, not client-to-server.**

Everything else followed from that one choice:

- P2P networking and an identity/signature layer — `net/gateway.ts`,
  `net/xport/central.ts`, `net/xport/mesh.ts`, crypto (Identity/PID/Signature,
  ed25519 via noble-curves)
- **Byzantine fault tolerance** designed for in advance, on the premise that
  peers cannot be trusted — the assumption seeped into several layers
- **WASM as obfuscation.** Pushing core logic into WASM was motivated by
  reverse-engineering defense, a separate motive from the performance one in
  [20260708-concurrency-model.md](./20260708-concurrency-model.md)

References being followed: Skype (its old P2P/supernode model) and Figma (WASM
core + cooperative multitasking — though Figma actually runs a trusted central
server model, so it was never evidence for P2P or BFT at all).

## Why the scope collapsed

**Web P2P doesn't close.** NAT traversal is not dependable without relay/TURN
infrastructure — which means a server exists anyway. The premise that motivated
every downstream system doesn't survive contact with the platform.

**WASM obfuscation doesn't defend anything.** As long as the client holds the
whole binary, it is not real protection. Worse, it _contradicts_ the BFT premise
sitting next to it: BFT says "do not trust the client", obfuscation says "hide
the client's code and you're fine". Those are opposite postures, and holding both
at once meant neither was being taken seriously.

**BFT consensus costs more than the project could carry**, on top of the above.

The conclusion at the time: on web technology, there is no clever way through
this.

## What the effort had actually been optimizing

A retrospective judgment worth keeping, because it generalizes:

The differentiating value of this project is the **OSM → terrain/voxel baking
pipeline** — which is a self-contained pure transformation that needed almost no
generalization. Yet the most design energy went into the runtime and network
layers, **the place where generalization was least needed.** The root cause was
trying to imitate systems-level architecture (real processes, real shared memory)
to route around constraints the web simply imposes.

The layered structure that produced (spec/bake/render/engine split, an `xpute`
runtime orchestration layer) was assessed as "ideal but scoped far too large".

A restart would compress toward monolithic and pipeline-centric — two apps and
one shared package, with the pipeline as the center of gravity and everything
else a thin layer consuming it, rather than a separate runtime-orchestration
tier and per-package `rust/` subtrees.

## Corrections to earlier readings

**BigInt/PVF3 was not over-engineering.** An earlier assessment called PVF3's
bit-exact BigInt determinism excessive. That was wrong. Governing a whole planet
at fixed precision in one coordinate system exceeds JS `number`'s 53-bit safe
integer range, so BigInt was necessary. Its slowness was known and accounted for:
**the plan was always to move that arithmetic to Rust** (which handles 64-bit
integers natively). TypeScript/BigInt was the spec and reference implementation;
Rust was to be the performant one — the same "core lives inside WASM" posture
Figma has.

_Implication of the no-WASM pivot:_ the problem Rust was going to solve reopens.
Either keep BigInt in plain JS and accept the cost, or replace it with
floating-origin rebasing onto float64 (52-bit mantissa gives sub-millimeter
precision over a wide region near the origin). The latter is this field's standard
answer — Cesium, Unreal World Composition — and fits the no-WASM direction better.
See [20260811-coordinate-system.md](./20260811-coordinate-system.md) for what was
actually built.

**XTP may be worth more outside this project than in it.** It describes itself as
"schemaless FlatBuffers-feeling, C-memory-layout friendly, relocatable across
languages". FlatBuffers and Cap'n Proto need a schema plus codegen; msgpack and
CBOR are schemaless but give no zero-copy lazy access. XTP offers relative-offset
lazy/random access _and_ relocation (subtree splice) without a schema — a real
gap. It already has both TypeScript and Rust implementations, so it is
cross-language validated. Plausible spin-out asset.

That comparison left out Apache Arrow, the one format whose whole premise is
typed buffers read in place across languages. It is weighed against how XTP
is actually used in the section at the end of this document (2026-09-17).

## Why `xpute` and `pxl` are separate packages

The intent was to grow `xpute` into a general-purpose
runtime — a "spatial OS" — not tied to pixelet, with the worker/IPC/transferable
design and XTP's cross-language format both serving that ambition.

**PVF3 was deliberately left in `pxl`, not moved to `xpute`**, because the
coordinate system is domain-specific, while execution and transport are general.

On the term: "spatial OS" is not standard academic vocabulary, but it isn't
invented either. The concrete precedent is Improbable's **SpatialOS** (~2016) —
not a kernel, but a cloud platform for sharding large persistent simulation
worlds across servers, using "OS" as a metaphor. `indexer.ts`'s unimplemented L5
(Shard) slot points at conceptually the same place. Note the honest gap:
SpatialOS ran a real distributed system, while L5 is a reserved bitfield with no
implementation behind it. The phrase describes a direction, not a current state.

## Why so much exploration happened

Worth recording because it changes how the commit history should be read:
pixelet doubled as a **testbed for pushing the web platform to its limits** —
Deno, WASM, WebGPU, workers, WebRTC, P2P, supernodes, and typed-array/DOD
techniques were partly being exercised for their own sake.

So the passages that look like "why on earth was this made so complicated" —
P2P/BFT, WASM+SAB threading, Rust ports adopted then abandoned — were not purely
product misjudgments. They were also deliberate technical experiments. Evaluate
them against both motives, not just the product one.

## Where it landed

The parked decisions became a concrete product direction on 2026-07-30: a
location-based (Pokémon GO style) + block-building (Minecraft style) + light
social (Cyworld style) hybrid MVP, with scaling — the original reason P2P was
ever considered — and moderation explicitly out of scope. See
[AGENTS.md](../AGENTS.md#scope-and-maturity).

**None of the parked code should be proactively deleted.** The
spatial-computing/metaverse niche is not considered dead, and the option to
resume is being kept open. When cleanup does happen, it goes to an archive branch
rather than `git rm`.

## 2026-09-17: XTP against Apache Arrow, by what XTP actually carries

Arrow is a columnar memory format — typed buffers, `Array`s over them,
`RecordBatch`es with a schema — with IPC and a file format that carry it
unchanged, and compute kernels over it. It has no mutable general-purpose
structures (an index set, an ordered set with ranks, a spatial hash, a handle
table, allocators), so it overlaps with XTP, the wire format, and not with
`xpute`'s `ds`. Arrow arrays are built through builders and then treated as
immutable; that suits a published page, not a lane the kernel rewrites every
frame.

What XTP carries today, read from the code:

| Use                              | Shape                                                                                                                                                   | Where                                                       |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| Terrain page (published)         | `[i16 lane, [u8 lane × 4]]` — fixed shape, one record a page; the elevation lane is 68×68 and each ground lane 1024×1024, so the lanes differ in length | `bake/format.rs` `encode_terrain`                           |
| POI page (published)             | `[f64×3, u16×2, u64, [str…]×4]` — an n-row table (x, y, z, category index, rank, id hash) and four string lists                                         | `encode_poi_page`                                           |
| Bake input (server-internal)     | `[i16 lane, [[row tuple]…]×4]` — buildings, segments, base and coasts as row-oriented tuples, geometry as WKB                                           | `apps/api/src/bake/terrain/bake.ts`                         |
| Object store save (localStorage) | `[[u64, f32×4, u32]…]` — five lanes a page                                                                                                              | `module/world/object/store.rs`                              |
| Ring commands and signals        | a message is a positional tuple of a few scalars and strings, written straight into a ring slot                                                         | `crates/pxl/wasm/src/lib.rs`, `apps/web/src/wasm/kernel.ts` |

Of what the comparison above named as XTP's own: GRAFT (splicing an encoded
subtree) has no use outside the codec — the last, the object store's
per-page Placement record, went on 2026-09-17; no writer outside the codec
records a typed null; and "any tree" is at most two branches deep in
practice, every shape fixed in code.

The same uses as Arrow:

| Use               | As Arrow                                                                                                                           | Fit                                                                                                       |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| POI page          | an n-row `RecordBatch`; `category` dictionary-encoded, `id`/`name`/`tags` as `Utf8` columns (a UTF-8 buffer and an offsets buffer) | Good. The backlog's XTP string-sequence leaf — one UTF-8 blob and a u32 offsets lane — is `Utf8`'s layout |
| Object store save | a `RecordBatch` with a `page_idx` column                                                                                           | Good                                                                                                      |
| Bake input        | four tables, geometry as a `Binary` column                                                                                         | Good; the columnar form is closer to Arrow than today's row tuples                                        |
| Terrain page      | lanes of different lengths cannot be one batch's columns: a single row of `List`/`FixedSizeList` columns, or buffers apart         | Awkward                                                                                                   |
| Ring messages     | an IPC message (schema plus Flatbuffers metadata) per command                                                                      | Poor                                                                                                      |

Two principles it sits against:

- **A specification is not sent** ([20260913-everything-is-a-file-the-placement-and-the-palette.md](./20260913-everything-is-a-file-the-placement-and-the-palette.md),
  undone 2026-09-17). An Arrow IPC stream or file carries its schema. XTP is
  not specification-free either — every node carries a type code — so the
  difference is type tags against a named schema, not none against some.
- **Immutable arrays.** Fine for what is published; not the representation of
  what the kernel mutates.

Not checked, and each bears on a decision:

- **Size.** Neither `arrow-rs` built for wasm32 nor the `apache-arrow` npm
  bundle was measured, against a floor of iOS 26 on an iPhone 11.
- **Zero copy across the boundary.** Arrow IPC aligns buffers to 8 bytes, but
  whether JavaScript and Rust can both open views without a copy over a block
  inside `WebAssembly.Memory` was not tried.
- **The bucket path.** Whether a published page as an Arrow file (Feather v2)
  changes anything on the Worker's route or its cache was not looked at.

In short: none of XTP's recorded distinctions — GRAFT, any tree, typed
nulls — is in use. The tabular things it carries (the POI page, the saved
store, the bake input) fit Arrow well; the terrain page's lanes of different
lengths and the ring's messages do not.
