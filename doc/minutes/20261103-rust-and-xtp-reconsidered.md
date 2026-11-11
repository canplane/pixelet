# Rust and XTP, reconsidered

**2026-11-03** — recorded with the user. Main was at `cad791eade`, as `original`
keeps it.

Nothing was resolved. Every recommendation below is the session's, a proposal
and no decision; what the user said is under its own heading. No code changed
and no benchmark ran: the comparisons are read from the code, the records and
each project's documents, and what comes from memory says so.

## What was asked

The user doubted two choices: XTP where FlatBuffers might have done, and Rust
where C++ might have made GPU programming easier. The user asked for XTP against
the FlatBuffers builder and FlexBuffers, whether to keep XTP even if only as a
portfolio piece, what C++ can lean on for CPU and GPU code from one source,
whether Slang has a Rust target, whether moving the ABI's types to FlatBuffers
makes sense, and what changes if other people use this.

## What the user said

- XTP was useful at the April checkpoint (`origin/checkpoint/20260405-rpc`):
  usable with no generator, made in the wish to build something like a binary
  JSON. With the ABI now fixed, the user asks what all of it is for.
- The user expected schemaless, in-place reading to matter in WASM.
- Coupling XTP to xpute may be the better place for it. The user said only that it may be.

## XTP

**The April use** is what the user recalls: `new TreeView().branch(...)` written
by hand (`apps/api/src/modules/world/terrain/service.ts:54`) and a tuple read
back with `.get<[...]>()` (`packages/pxl/bake/src/terrain/format.ts:123`). The
schema lived in the code, and the node's type tag was the check.

**The grounds of
[why XTP](./archive/20260910-why-xtp-and-what-would-end-it.md), today:**

- "No schema and no codegen" is gone for commands: `cmd.json` and xpute-tool's
  `spec::cmd::table_outputs` generate both sides.
- Named fields, the planned way to evolve, are not on these paths. Fields are
  found by position (`rust/spec/src/bake/tile/terrain.rs:596`; the generated
  `f.at(0)?` in `cmd.spec.rs`).
- That record said a hand-written verifier in two languages is where a hand-rolled
  format loses. There are three (`e8583c76c`): 5,350 lines of implementation
  without tests (TypeScript 1,873, Rust 2,059, C++ 1,418), held by goldens
  (`xpute/spec/golden/wire/xtp/`). The tree has no fuzz test.
- Bulk read in place without decoding still holds, and so does
  [what the writer and reader cost](../experiment/20260914-xtp-by-fixed-shape.md):
  the same bytes written by shape took 54 ns against 492, read by offset 5 ns
  against 303 (TypeScript, before the generator).
- Its end condition of a client writing what a server reads has not arrived:
  `rust/server` makes no XTP call. TypeScript reads XTP from outside the kernel
  in one place seen, the saved objects (`driver/namespace/object.ts:76`).

**The format is small; holding it correct was not.** A header, a length-prefixed
leaf and a branch with an offset table are nearly all of it. Writing the third
language found disagreements fixed in all three (`6d4e5836c`), and the rules a
reader refuses by came later (`e8583c76c`).

### Against the others

| | XTP | FlatBuffers | FlexBuffers |
| --- | --- | --- | --- |
| Schema | none in the format; `cmd.json` here | required | none |
| TS write | `PacketWriter`, in place in the caller's bytes | a builder; cost not measured | a stack object a value |
| TS typed array in | a packed leaf of its type | a generated vector helper | a blob: the type lost, the view's offset ignored (`writeBlob(value.buffer)`) |
| TS bulk read | a view | a typed-array view (from memory) | a `Reference` an element; typed vectors built into a JS array |
| Verifier | the reader refuses, three languages | C++; Rust, documented as experimental; no TS | none documented; a wrong type converts or defaults |
| Evolution | none, positional | vtables | keyed maps, by binary search |
| Bytes across languages | held by goldens | not guaranteed (from memory) | not checked |

FlexBuffers was set aside: its documents call it slower than FlatBuffers, and
its TypeScript does not suit bulk data over WASM memory.

The session first said it would choose FlatBuffers if starting again, then took
that back on the user's point: with a fixed ABI, vtables buy nothing either.

### By what each packet is

| Data | Shape | What it needs | Proposal |
| --- | --- | --- | --- |
| Ring commands | 28 packets: 19 fixed, 5 with only optional fields, 4 with strings; 9 commands carry none (`cmd.json`, counted by script) | a generated layout | XTP is more than they need |
| Baked tiles | columns of different lengths, rebuilt with their version | an offset table | keep; the table is small against the columns |
| Saved objects | left on a device, across builds | self-description and evolution | the one place the binary JSON's purpose remains |

### Ring commands: two ways

1. **The same XTP bytes, fixed shapes read by offset**, as the 2026-09-14
   experiment proposed, by xpute's generator. The contract stays, and so do the
   child entries, unread.
2. **Generated fixed structs.** The clearest, with two wire formats, and it
   changes the door's contract.

The session recommends 1, when the generator is next touched. Performance is no
reason for either: the difference measured is under 1 µs a call.

### Moving the ABI's types to FlatBuffers

Proposed against. **XTP is in xpute's guest contract**: the door's
`fn write(&self, w: &mut PacketWriter)` (`xpute/rust/guest/src/ipc/door.rs:24`),
xpute's own commands (`xpute/rust/guest/src/abi/cmd.spec.rs`), and the frame and
the packet (`xpute/ARCHITECTURE.md:142`).

- **What would move:** layout and alignment, the packets' generated code, and
  structural checks in Rust alone.
- **What stays:** the command numbers and directions, references to other specs'
  types (`SettingKey`, `BakeKind`), the enum's `unknown` rule and the refusal of
  an unlisted value, and value checks such as `finite`.
- **What it costs:** enums and records with two possible owners (`.fbs` and
  `cmd.json`); a root table's pointer and vtable where 19 packets need a struct
  (from memory); the in-place write in the ring slot; `flatc` and a runtime per
  language.

Moving commands alone is the worst case, two formats and two generators. Moved
whole (commands, tiles, saved objects, the door) it is one format with no
implementation of ours beneath pixelet, and the moment for it is when a saved
object or a server's wire needs fields to evolve.

### If others use it

Other people's ABIs still move, so writing packets with no generator has value
again. What stands in their way is that the format is imposed: every packet is
XTP, and the C++ ring reads a packet's size from its header
(`xpute/cpp/xpute/guest/ipc/ring.cpp:89`), a choice of policy held in the
mechanism ([mechanism and policy](./20260914-mechanism-and-policy-and-what-xpute-is.md)).
FlatBuffers in its place would impose a different one.

Proposed: XTP stays the default, the imposition goes, in this order:

1. Document XTP as an envelope: a root `u8` array carrying any other format.
   Its alignment and `PacketWriter`'s array path are unchecked.
2. Let a generated reader accept trailing optional fields that are absent. Today
   reading past the end refuses.
3. A format version rule. The header has no version field; magic or the reserved
   bits could carry one.
4. A dump tool over `get_deep`.
5. A fuzz test, before anyone else's input is read.

### As a portfolio piece, and alone

Proposed: keep it in `kit`, and retell it. "Schemaless FlatBuffers" no longer
describes it. What can be defended: relocatable packed columns read and written
in place over WASM memory, three languages held to the byte, with a generated
schema on top where the ABI is fixed, and the measurements and end conditions in
these records.

Alone, the gap is real and narrow. Of those read (from their documents, no
benchmark), none has all of schemaless, random access, typed arrays in place and
both JavaScript and Rust:

| Format | Short of it by |
| --- | --- |
| FlexBuffers | element objects in TS; alignment not checked |
| BEVE | sequential parsing only documented; Rust not confirmed |
| Lite³ | C11 only, typed arrays undocumented, API unstable; in-place mutation is its strength, which XTP lacks |
| CBOR with RFC 8746 | random access and alignment not in the standard |
| MessagePack, BSON | not zero-copy (Lite³'s README table) |

Someone who already knows FlatBuffers has little reason to move. It is better
introduced as xpute's default part than as a library of its own.

### A cleaner XTP and ABI

The user asked how the session would clean them up. Proposed, in this order,
with the wire bytes, the door and the ring unchanged and the goldens holding the
first four:

1. **Read the declared type.** The writers already write it (`.f64(`, `.u32(`),
   but the generated Rust reads every number as `get_number` through an f64 and
   so takes any numeric tag (`xpute/rust/tool/src/spec/cmd/rs.rs:75`), and the
   generated TypeScript's `.get<f64>()` is a cast that checks nothing
   (`cmd.spec.ts:214`). Each generator emits an exact accessor (`.u32()`,
   `.f64()`), which the TypeScript cursor needs added.
2. **Tiles and files in the table.** Their layouts are written by hand on each
   side: 35 positional reads in `rust/spec` and the kernel's layers, the column
   order held by a `const _: () = assert!`
   (`rust/client/kernel/src/domain/layer/object/repository.rs:265`), the types in
   `host.json`'s prose, and `object.ts` again. A spec declares each packet's typed
   columns and the generator emits both ends. It already emits lists and `strs`
   as views.
3. **One owner for a packet's size.** `pkt` (`rust/spec/src/bake/tile.rs:23`)
   restates the writer's layout in pixelet. It moves to `kit`, the generator
   emits the bound from the columns' limits, and these packets are written with
   `PacketWriter` directly rather than through `TreeView` and `walk`. `TreeView`
   stays in `kit`.
4. **A shape constant for fixed packets**, the 19 above: the generator emits the
   header and child table as bytes, a read compares them at once and takes its
   fields by offset, and a write copies them and overwrites the leaves, as the
   2026-09-14 experiment did. Optional fields and strings stay on the cursor.
5. **The format byte, last.** A one-byte prefix misaligns a file's packet, so both
   sides copy it to read it (`repository.rs` `packet`, `object.ts` `packet`). A
   word-long prefix removes the copy. It changes saved files, so their format
   numbers move.

## Rust and C++

**The GPU's pains are real:** rust-gpu 0.10.0-alpha.1 vendored and patched at 10
sites (34,589 lines), no enum with data, the build in a Linux container, 30
generated WGSL files of about 3 MB. rust-gpu's README calls it not production
ready.

**The cause is less Rust than rust-gpu.**
[The decision to compile one source for both](./20261014-the-gpu-casts-a-ray-a-pixel.md)
came on 2026-10-14, after
[Rust was chosen with the domain model as the wire contract](./20260918-the-server-in-rust-and-the-line-already-drawn.md).

**What C++ has.** Nothing as direct and mature reaches WGSL; the narrow end is
WGSL, not the language.

| Path | State |
| --- | --- |
| Slang | WebGPU "experimental", "WGSL support is still work-in-progress"; CPU experimental, no barriers or atomics there; HLSL-like, not C++ |
| Vcc (shady), C/C++ to SPIR-V through Clang | "for research purposes"; no WGSL mentioned |
| Circle | examples public; closed compiler by one author (from memory); no WGSL mentioned |
| clspv | passes OpenCL 3.0 conformance; a subset of OpenCL C 1.2, compute only; not C++ |
| HLSL with hlslpp | one source, built by DXC for the GPU and by a C++ compiler for the CPU; sound parts, assembled by hand. SPIR-V to WGSL remains, and whether naga, Tint and Safari take it is unchecked |
| C++ and WGSL written twice | sound tools; one source lost |

WGSL's limits (no u64, f64, u8, i16 or recursion) and WebKit's translation to
Metal stay on every path.

**Had it been C++ from the start**, the session's judgment, unmeasured:

- **Better:** the shader build, since most of the panic strategy, the aborts'
  removal and the vendored codegen exist to bring Rust down to the GPU.
- **Worse:** memory errors, in code agents mostly write, where WASM's linear
  memory stops no bad write inside its bounds; the server and bakes' share of
  the domain model; one Cargo workspace; 106,505 lines of Rust without the
  vendored codegen to move now.
- **So:** no decisive gain, a different kind of pain: the tools' now, memory at
  run time then. The session prefers the tools'. To undo the cost, Rust with
  WGSL written by hand and tests holding the two equal comes before C++.

**Slang has no Rust target.** Its CPU output is C++; the Rust side is bindings
that call the compiler (`shader-slang` 0.1.0, unstable). The session first
suggested keeping Rust and replacing rust-gpu with Slang, then took it back: it
breaks the one source.

| Keeping Rust | One source | Short of it by |
| --- | --- | --- |
| rust-gpu | kept | the vendored codegen |
| WGSL, with `naga-rust` generating the Rust | kept | 0.1.0; no pointers, atomics, workgroups or bindings; no generics for `cast::first` over `Query` |
| WESL | the WGSL side only | generics future work as of February 2026; no CPU Rust |
| Slang with bindings | lost | the CPU written apart |
| WGSL and Rust twice, tests holding them equal | lost | two to keep equal |

The session recommends keeping rust-gpu: nothing mature to move to.

## Open

- Whether one source for CPU and GPU (`README.md:26`) must hold. If it may go,
  the session proposes moving one compute entry to Slang and comparing the WGSL's
  size and Safari's compile. Closed the same day: rust-gpu is to be removed
  ([rust-gpu goes](../gpu/minutes/20261103-rust-gpu-goes.md)).
- Whether xpute is meant for outside use, or a portfolio piece. The first calls
  for the envelope and trailing optionals first, the second for the envelope and
  the dump tool.
- When to read fixed shapes by offset, if at all.
- What the FlatBuffers builder costs TypeScript's commands.

## Sources

- [FlexBuffers](https://flatbuffers.dev/flexbuffers/),
  [FlatBuffers' language support](https://flatbuffers.dev/support/),
  [Rust `root`](https://docs.rs/flatbuffers/latest/flatbuffers/fn.root.html),
  [FlexBuffers' TS builder](https://raw.githubusercontent.com/google/flatbuffers/master/ts/flexbuffers/builder.ts),
  [and reference](https://raw.githubusercontent.com/google/flatbuffers/master/ts/flexbuffers/reference.ts)
- [BEVE](https://github.com/stephenberry/beve),
  [Lite³](https://github.com/fastserial/lite3),
  [RFC 8746](https://www.rfc-editor.org/rfc/rfc8746.html)
- [Slang](https://github.com/shader-slang/slang),
  [its WGSL target](https://docs.shader-slang.org/en/latest/external/slang/docs/user-guide/a2-03-wgsl-target-specific.html),
  [its CPU target](https://docs.shader-slang.org/en/latest/external/slang/docs/cpu-target.html),
  [`shader-slang`](https://lib.rs/crates/shader-slang)
- [rust-gpu](https://github.com/rust-gpu/rust-gpu),
  [naga-rust-embed](https://docs.rs/naga-rust-embed/latest/naga_rust_embed/),
  [wesl-rs](https://relatedrepos.com/gh/webgpu-tools/wesl-rs),
  [WESL's Khronos talk](https://www.khronos.org/developers/linkto/wesl-a-pioneer-language-for-webgpu)
- [shady and Vcc](https://github.com/Hugobros3/shady),
  [Circle's shaders](https://github.com/seanbaxter/shaders),
  [clspv](https://github.com/google/clspv),
  [hlslpp](https://github.com/redorav/hlslpp)
