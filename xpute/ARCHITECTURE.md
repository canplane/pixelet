# Architecture — xpute

[xpute](./README.md) is a foreign function interface between a program's
compiled **guest** and the **host** that runs it. It is a calling convention
over three host functions, through which the two pass frames laid in the guest's
memory; bindings for both ends, which a generator writes from the program's
protocols as WIT's tools write a component's; the turn, the one call through
which the host gives the guest its time; and the GPU stream, a command buffer
the guest records and the host runs, which moves large data while that memory
stays fixed. The guest owns nothing but its memory. The host implements only
the device that lends that memory across turns, the GPU, and nothing else.
Everything else belongs to the **program**, and it travels over one general
channel whose messages xpute delivers and does not interpret.

The guest is the program's compiled code, built with xpute's guest library into
one image, which runs as a WebAssembly module in a web page and as a library
linked into a native app. The host is xpute's TypeScript. The program is
what the app decides: what its commands mean, when its turns should run, and its
UI.

Each section below takes one part of xpute, in the order the parts depend on
each other: the memory the guest owns, the turn that gives it time, the channel
that carries everything else, the device that lends the memory, and the
languages, with the records that hold them to each other. The document ends with
the boundaries, the invariants every end keeps, what xpute took from earlier
work, and a map of the code. The contract's layouts, those of the frame, the
stream, the header and the GPU's ops, are written once in
`spec/`, and this document points there instead of restating them.

---

## The devices that lend the memory

A frame on the channel is complete in itself: what it names goes with it, and
once `send` returns the guest may write over all of it. Some work cannot be
said that way, because it reaches into the guest's memory after the turn that
asked for it has returned: a buffer the GPU uploads from, a word the GPU writes
when a request finishes. Such work needs a rule that says who owns which bytes
of the guest's memory and when they come back, and only xpute sees both sides of
that rule. So the host implements exactly the device whose work lends the
guest's memory across turns, the GPU, and the program implements everything else
over the channel.

### The GPU device

A renderer makes thousands of calls a turn to a device it cannot reach, and none
of them needs a reply. For those the guest writes a **stream**: records laid end
to end during a turn, each an op, a count and that many words, published at once
by storing their count with release. The host runs every record and then
stores the count back to zero, which hands the stream back to the guest. Until
then the records, and any memory a record names for the host to read, are the
host's, and the guest neither rewrites nor frees them. On a page the host
runs the stream after the turn returns; on a device the GPU host takes it as
the turn returns and runs it beside the guest. No turn waits on the device:
while the host holds the stream the guest records nothing and goes on turning.
Either way the order is the acknowledgment: a record may name memory for the
host to write into, and the guest reads that memory on a later turn.

The calls themselves are WebGPU's, written down once in WebGPU's own vocabulary.
An enumeration is its specification's strings in order, so a host hands the
string to the device as it is, and a call's words follow the descriptor WebGPU
takes, with nothing defaulted on the program's behalf. The guest writes the
calls through the **encoder**, which names each object by a handle from one
table. The **GPU host** runs them, on WebGPU in a page and on wgpu in a native
app, and whatever a run throws ends the guest, as a trap does.

Some calls finish on a later turn: an image copied in, a buffer read back, the
queue's work done, or a pipeline made by WebGPU's asynchronous call. Each such
call names a **fence**, a word of the guest's memory, and a **ticket**, the next
value of a sequence that the guest alone advances for that word. The GPU host
is the fence's only writer: once the call is done, it stores the ticket in the
fence with release, after the result the fence guards. The guest loads the fence
with acquire and holds the call done when it equals the ticket the guest asked
with, so a late answer to a request that was dropped carries an older ticket and
never matches. The device's block, which holds its limits and features, its
preferred format, the counts of its uncaptured errors and its unfinished frames
and its status, is written by the same rule, where the guest asked for it.

Both GPU hosts admit a frame by one rule: a turn draws only while fewer of
its frames are unfinished on the GPU than its frame latency, so taking a
surface's texture never blocks, and the GPU is never more than that many frames
behind. What a surface shows, a canvas on a page or a view's layer on a device,
is lent by the UI. The guest answers a surface's open with a handle, and the UI
binds its canvas under that handle, to the GPU host, which keeps the surface
from then on. The manifest's GPU ask says which adapter to prefer, WebGPU's power preference,
and which features the guest needs; the GPU plugin asks the platform for that
device, and `GPU_READY` tells the guest when the device has answered, the first
time and again after a loss, with the device block's status saying what the
device is now. The ops and their words, the fence's results and the device's
block are in `gpu/link/spec/api.json5`, and the stream's own rules are written out
beside the guest's stream. The GPU's calls stand apart from `kit` and from both
ends, so a program that never draws links none of them.

A GPU's floats are held only as far as WGSL holds them. Its addition,
subtraction and multiplication round half to even, as every language here does,
but its division may be 2.5 ulps off and its square root and transcendentals
have only bounds, and a GPU may be further off than the specification allows: a
division has been measured 13 ulps off. A compiler may also fuse a product into
the sum after it, and Metal's, in its default fast math, reorders a sum, so no
order written in a shader holds. So a rule the CPU and a GPU both decide is
decided in integers, or in floats the CPU rounded once and handed to the GPU in
a word, a reciprocal in place of a division; and a rule the GPU alone decides
is one function every pass calls, so that the GPU at least agrees with itself.

## The languages, held to each other

xpute is written in TypeScript, Rust and C++, and the implementations do not all
stand in the same relation to each other. Which relation they stand in decides
what must match.

The kit is symmetric. Each language implements the same collections, codecs,
wire formats, errno table and scalar math, held to each other wherever bytes
must match. TypeScript's kit has no allocator and no bug reporter, because it
has no linear heap of its own and a thrown error carries its own stack.

The two ends face each other, and they owe each other the contract and its
formats, not matching lists of modules. The guest end exists in Rust and in C++.
The manifest is Rust's; a C++ guest lays out its own front and writes its
header itself. The host ends hold what their platform needs. TypeScript's
boots a guest on a page, or on a device through the native module. The GPU
hosts are TypeScript's on WebGPU and Rust's on wgpu.

The C++ library is written from the contract and the formats, beside the Rust,
and where it differs, the language is the reason: it writes the codecs its
standard library lacks, its XTP reader hands back views into the packet rather
than copies, its tasks are C++20 coroutines where Rust's are futures, and it
uses RAII, templates and results, with no exceptions and no RTTI.

What holds the languages to each other is data, not a shared implementation.
Under each part's `spec/golden/` are lines of a path and a value, produced once, checked
once and then frozen. Each language that implements a part reads that part's
records bit for bit, so a record is never recorded again to make a test pass: a
difference means that one side moved, and a record changed on purpose changes
for every language at once. Some records cover bytes that cross a boundary, such
as the packet formats and the words each GPU encoder writes; others cover values
that cross none but must still agree, such as the random generators and the
scalar math. Every rounding in the tree is one rule, half to even as WGSL
rounds (`math/scalar`), so the guest, the UI and a shader round alike; a call
that rounds otherwise is refused by the tools. The kit still offers its other
roundings by their own names, as a library offers options.

---

## Boundaries

What xpute leaves out is part of its design.

- **The kit knows nothing of turns, hosts or memories.** It is generic code
  that any program can link and call. The guest end depends on it, and it never
  depends on either end.
- **No command of the program's has a meaning in xpute.** No domain type reaches
  the kit or either end, and the host routes a program's frame by its major
  and reads nothing of what it says.
- **The memory past the front is the guest's.** The manifest says the sizes;
  what the guest's ranges hold, where they lie, and what answers the heap are
  the guest's.
- **The policy is the program's.** When turns run, how long they may last and
  how much I/O may be out at once are decided outside xpute.
- **The devices are only those that lend memory.** Any other I/O the program
  does, a network's included, is the program's, over its own commands, and xpute
  itself reaches nothing outside the device it runs on.
- **xpute is not real time.** A deadline is how long a turn should take, and a
  turn that runs past it is not stopped.
- **xpute does not enforce its grants.** The host grants a region and a
  deadline, and nothing stops a guest that ignores them. The contract describes
  one host and one guest; several guests under one host would have to bring
  what the contract does not, isolation and a way to stop a guest that does not
  return.

## Prior art

xpute's bindings come from WebAssembly's component model, its requests from
io_uring, and its stream from a GPU's command buffer.

| Precedent | What is taken |
| --- | --- |
| A WIT world and its generated bindings | A protocol: its commands, each an export or an import, named from the guest's side, and the code both ends are generated with |
| A GPU's command buffer, as WebGPU's command encoder records one | The GPU stream: calls recorded during a turn and run by the host after it |
| An Android app's manifest | The manifest: the guest's own declaration, at build time, of the memory, stack, packet room and devices it needs |
| A linker script's output sections | The guest's ranges: laid end to end on pages without knowing what they hold |
| [evanw/buddy-malloc](https://github.com/evanw/buddy-malloc) | `kit`'s buddy allocator of pages, ported line for line |
| POSIX and Linux errno | Every result: a negative errno number |
| IEEE 754's round half to even, as WGSL rounds | The one rounding in every language |

## Codemap

The code, in the order its parts depend on each other.

- Each part holds its contract as JSON in its own `spec/`: the link's frame
  (`link/spec/`), the GPU stream's records (`gpu/link/spec/`), the errno table
  (`kit/spec/`), the manifest's schema (`link/spec/`), and beside each the
  golden records (`golden/`).
- `gen` is the data specs' generator, `xpute-gen`, and the writer every
  generator writes with, a program's manifest among the data specs;
  `link/gen`, `xpute-link-gen`, writes the link's bindings from WIT into both
  ends. `dev/tool` is
  xpute's own use of them: `cargo
  spec` checks xpute's generated files, and `cargo spec --write` writes them.
- `kit/rust`, `kit/ts` (`@xpute/kit`) and `cpp/xpute/kit` are the kit:
  `collection`, `codec`, `wire/xtp` and `wire/tlv`, `status` (the errno table
  and `bug!`), `math/scalar` and `math/rng`, and `alloc` (`buddy_tree`,
  `slab`).
- `link/rust` and `link/ts` (`@xpute/link`) are the link both sides take: its
  frames, its machine and its bindings, and in TypeScript the remote a UI
  reaches the link by. `guest/rust` is the guest end over it: its link, its
  clock and its waits. In C++, `cpp/xpute/link` is the link and
  `cpp/xpute/guest` the guest end, as `link/rust` and `guest/rust` are in
  Rust.
- `gpu/guest/rust` is the GPU's calls, and `gpu/native/host` the native GPU
  host. `cpp/xpute/gpu` is the encoder, its stream and its handles.
- `web/runtime` (`@xpute/web-runtime`) is a page's runtime: the worker's
  instantiation of the guest, the page's end of the worker, the console and
  the lifecycle; `web/host` (`@xpute/web-host`) is the host's servers there,
  the clocks and the stop.
- `gpu/web/host` (`@xpute/gpu-web-host`) is the GPU host on WebGPU (`GpuHost`,
  `WebGpu`), and `gpu/native/runtime/expo` (`@xpute/gpu-native-runtime`) on a
  device the `SurfaceView` and the display's frames, over its own Expo module.
- `native/runtime/expo` (`@xpute/native-runtime`) is the React Native module:
  the Kotlin and the Swift over the C ABI and JNI; the UI's end of the
  runtime's link, a `Remote` over the module (`open_remote`); the runtime's
  start with what the UI mounts on it (`start`); the guest's memory
  (`bytes`); and its stop as the link has it (`stop_of`).
- `dev/golden/rust`, `dev/golden/ts` (`@xpute/golden`) and `cpp/dev/test/` are
  what the tests take: the golden records' reader, and in C++ the tests'
  runner.
- `cpp/amalgamation.awk` makes the C++ library's two files.
