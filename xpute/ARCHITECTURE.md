# Architecture — xpute

[xpute](./README.md) claims that a program's compiled **guest** and the **host**
that implements its imports need share nothing but the guest's memory and one
kind of message laid in it. This document follows that claim through the parts
that make it hold, in the order they depend on each other, across xpute's three
parts: the guest, the host and the link between them, which WIT draws; the
runtime, the platform's environment that runs them; and the kit beneath both.
The **link** carries every call as a frame. The **sides** run the link alike on
every platform, each end over a transport its runtime plugs in, and a runtime
carries a UI on another thread to the guest's end. A failure is its call's to
judge, and a bug ends in a stop, after which the link answers what was owed, so
that no side waits on an answer that will not come. And the three languages hold
the same facts: a spec or WIT owns each one and a generator writes it into each,
and frozen records hold the rest of their code to each other. The layouts are
written once in `spec/` and the interfaces in WIT, and this document points
there.

---

## The link

A frame is four words and a packet (`link/spec/frame.json5`): TAG, the request;
CMD, the handle of the object called and the method's place in its interface,
with a RES bit that marks an answer; RESULT, the answer's status alone, 0 or a
negative errno; and PKT, the packet's length. The guest has three exports:
`link_init`, which lays its link and answers where the link lies; `link_boot`,
its first entry, which mounts its implementations; and `link_in`, which enters
it with a frame the host laid in its inbox, a request or an answer. Out of the
guest goes one function, `link_out`, the same on every platform, which takes a
frame the guest laid in its outbox. Each crossing returns at once what the
method came to.

Whatever is called is an object under a handle. A singleton interface is a
**named handle**, the hash of its package and name, called with no open. An
object of which there are many, a file or a surface, is opened by a **URI**, a
tuple of tokens, through the root's `open`, which the implementing side answers
from the opener mounted at the longest matching prefix. The implementing side is
polymorphic behind the handle, so the caller never branches on what it calls.

A packet holds the args, or an answer's value, in the Component Model's
Canonical ABI layout, with four departures. A pointer is an offset into the
packet, which is the one copy. A buffer the caller lends, WIT's `lent`
(`link/wit/types.wit`), stays where it lies and is the callee's until the
answer; its address is a u64 on every platform, an offset into a module's
memory or a pointer natively, so a packet has one layout everywhere. A packet
that does not hold what its type says is refused, EBADMSG,
where the ABI traps. And RESULT is status alone, the value in the packet. What
of WIT the link takes, and what it reads its own way, is the generator's to
list ([link/gen](./link/gen/README.md)).

A `func` answers at once, and is called. An `async func` answers later, and is
sent: the crossing answers EINPROGRESS, and the callee answers later by a
frame marked RES with the request's tag. The guest is entered one entry at a
time, and an answer is never run inside the `send` that asked for it. The
guest runs only when it is entered, by a request, an answer, or the answer of
what it armed; when it runs is the program's.

## The sides

An end of the link is a **`Side`**, the same machine in Rust, TypeScript and
C++: it lays frames in its own box, crosses by its own function, answers by what
is mounted on it and hears the answers to what it sent. The guest holds one over
its outbox, and the host one over the guest's inbox. TypeScript names apart the
part every end runs alike, `End`: the handle table, the requests pending by tag,
the dispatch to an object by its handle, the later answers, the cancels and the
drops; a `Side` is an `End` over the guest's boxes.

What runs beside the guest is split the same way everywhere, and every
dependency points down: a part declares the interface it needs, and a part above
implements it and injects it. The **host** is the guest's counterpart: the
host's end of the link, the bindings mounted there, and xpute's own
implementations, the clocks, the stop and the log. It varies by language, as the
guest does, and names no runtime. The **runtime** is the platform's environment,
the web worker or the native guest thread. It instantiates or links the guest,
makes the host's `Side`, injects into the host what the host needs of it (where
a stop is reported and where a log record is written, and natively the way back
to the guest's thread and the timers there) and runs the loop that enters the
guest. Nothing names a runtime but its own UI end and the
program's assembly. A program may bring plugins of its own, a GPU for one, each
beside the runtime as its own guest's end and host, and neither names the other.
The program starts its guest with a `Boot`, which lists its mounts, the host's
among them, each a `Mount` that mounts the implementations of some of the
guest's imports, in no order, since none depends on another, and its runtime
runs the steps in their order, on both platforms alike: it runs each mount,
stands in for the UI, enters the guest only once every interface the guest's
world imports is mounted, and natively runs the loop.

An end of the link is one machine whatever carries its frames. What carries
them is the **transport**, which the runtime plugs in: the guest's boxes and
the crossing into it, in the guest's memory, which both ends share.

The UI calls the guest's exports and implements some of its imports by the same
WIT and the same generated bindings as any end, which call by a `Sender` and
implement by a `Callee`, and never know which thread they run on. A UI on
another thread than the guest's, a page's main thread to the guest's worker or
the native JavaScript thread to the guest's thread, is across a second boundary,
copied, which the runtime alone carries, by its shell's convention: messages
over the web's MessagePort, the module's functions and events over the native C
ABI. The runtime holds both halves of that crossing. On the guest's thread it
stands in for the UI on the guest's end: a stand-in under each named handle the
UI implements and an opener at each prefix it implements, each method sent
across and answered later by its tag. On the UI's thread, its UI end is an `End`
over the runtime's own transport, the web's port or the native module, so the
bindings call and implement there as on any end. One thread cannot wait on the
other, so every method the UI implements is an `async func`, and the runtime
refuses to start the guest where a function without `async` (the bindings'
`CALLS`) is one the UI implements, as it refuses an import with no
implementation. The guest's open of an object the UI implements is answered at
once on the guest's end, pipelined, so that end issues its handle, and the
runtime tells the UI end the handle and the prefix that matched, whose opener
the UI end runs. The guest's cancel of a method and its drop of an object cross
as the runtime's own messages. Handing an object beside a call, a canvas, is a
property of a crossing that can move one, the web's port.

The UI's connection is the runtime's, and an object like any other: the native
runtime attaches a UI under a handle of its own, everything the UI sends and
takes names that handle, and the UI leaves when the handle is dropped, which
drops on the guest's end what it opened and answers ECANCELED what it owed. The
guest is told only that a UI attached (`xpute:life` `attached`); the WIT
between it and the UI does not change. The native runtime outlives its UI, so a
reload opens a new handle on the same runtime and drops the old one, and the
runtime keeps the guest's end, its last stop and its trap, which a new UI reads
as it begins to hear them. The web worker dies with its page, so a reloaded
page meets a new runtime.

## How a failure ends

A failure is one error, `XputeError`, carrying an errno
(`kit/spec/errno.json5`). What happened is its errno's kind, the gRPC and
Abseil status code. The kind says nothing of whether the failure is fatal:
that is the call's. A caller that holds a failure impossible makes it a
**bug** (`bug` and `or_bug`), reported with its errno, its place, two numbers
and an optional message before it stops.

Between xpute's parts a failure moves as an `XputeError` with its optional
message, an end as a stop or a trap report, the stop with the bug's optional
message or that of a panic the language raised, and what is worth telling and
is neither as a log record, a level, a context and a message (`wit/log`, in
`wasi:logging`'s shape). Each end has its `log`: the guest's calls the import,
and the host implements it and hands the records, with its own, to the log its
runtime injects. The runtime alone writes them, every one, to the platform's
output: its error stream, Android's log at the level's priority, or the
browser's console by the level's method. The native UI end hands it the UI's
records to write the same way (`log`); the web worker tells the page each
record it writes, which the page does not write again.

A guest's bug is reported through `stop`, and the guest traps. A host's own
failure is a stop on both platforms too, reported the same way, and then the
link traps, so no answer the host owed leaves the guest waiting. It is handed
off one of two ways: synchronously, caught at `link_out` while the host answers
the guest, or asynchronously, by the platform's listener for what the worker
leaves uncaught, or by Rust's panic hook. A trapped guest is entered no more,
and a frame to it answers EPIPE, a request already pending on it too, once. The
app hears the trap with what stopped it, alike on both platforms, the imports
with no implementation among them where the runtime refused to start the guest.

## The languages, held to each other

xpute is written in TypeScript, Rust and C++. What the golden records hold is
the same in each: the wire formats (TLV and XTP), scalar math and the seeded
streams, the deque and the heap, the JSON texts a reader accepts (TypeScript's
being its runtime's `JSON.parse`), and the link's frame and packet layout. The
errno table is the same by its spec, and the link's `Side` is one machine in
all three. The rest of the kit is
each language's own: TypeScript has no allocator, and Rust no text encoding or
hash codec. The guest exists in Rust and in C++ from the same WIT; the hosts are
TypeScript's on the web and Rust's natively. Where the C++ differs, the
language is the reason: RAII, templates and results, with no exceptions and no
RTTI.

A fact that crosses languages has one owner, a spec or WIT, and a generator
writes it into every language that holds it: `xpute-gen` the data specs, and
`xpute-link-gen` the link's bindings, each README saying how. What crosses
between two languages is a frame, its packet the Canonical ABI's; XTP, the kit's
tree packet, is the format of data a program stores and serves.

What holds the languages to each other is data. Under a part's
`spec/golden/` are records each language reads bit for bit, so a difference
means one side moved. A record is frozen: it is written again only when a
contract changes on purpose, the packets' by a Rust test behind
`XPUTE_GOLDEN_WRITE`. Every rounding in the tree is one rule, half to even as
WGSL rounds (`math/scalar`), so the guest, the UI and a shader round alike;
clippy refuses Rust's `round`, which rounds half away from zero.

A GPU's floats are held only as far as WGSL holds them. Its division may be
2.5 ulps off, its square root and transcendentals have only bounds, a GPU may
be further off than that, and a compiler may fuse or reorder, so no order
written in a shader holds. So a rule the CPU and a GPU both decide is decided
in integers, or in floats the CPU rounded once and handed to the GPU in a
word; and a rule the GPU alone decides is one function every pass calls.

---

## Boundaries

- **The kit knows nothing of the link, hosts or memories.** It never depends
  on either end.
- **No method of the program's has a meaning in xpute.** A side delivers a
  frame by its handle and reads nothing of what it says.
- **The guest's memory is the guest's.** xpute lays nothing in it and sizes
  nothing: a module's memory is its own, which it grows as its allocator asks,
  and a native guest's is its process's. A span the guest
  lends is read where it lies (`lent`, above), in place by a host in the same
  process and by the native UI through a buffer over that span alone. Whether
  the memory is fixed, what lies where in it and which allocator each
  allocation takes are the program's, in either language.
- **The policy is the program's.** When the guest runs and how much I/O is out
  are decided outside xpute; it has no turn, no budget and no backpressure.
- **The host implements only what every guest shares**: the clocks, the stop and
  the log. Any other I/O is the program's, over its own interfaces or a
  plugin it brings.
- **xpute does not isolate.** The contract describes one host and one guest;
  a guest that does not return is not stopped.

## Prior art

| Precedent | What is taken |
| --- | --- |
| WIT and its generated bindings (wit-bindgen, jco) | Every interface on the link declared in WIT, and both ends' bindings generated from it |
| The component model's Canonical ABI and handle table | The packet's layout, `lent` from a borrow, `drop`, a later answer as `task.return`, and handles reused with a generation |
| Binder's handle 0, 9P's walk and open | The root: one fixed `open` that turns a URI into a handle |
| WASI's `wasi:clocks` | The clocks, as a subset |
| gRPC's and Abseil's status codes | An errno's kind |
| Linux's uAPI, which carries a user pointer in an ioctl's struct as a `__u64`; Vulkan's `VkDeviceAddress`; WebGPU's `GPUSize64` | An address the link carries, a u64 whatever the platform's pointer, so one layout and one golden record serve every platform |
| [evanw/buddy-malloc](https://github.com/evanw/buddy-malloc) | `kit`'s buddy allocator, ported line for line |
| POSIX and Linux errno | Every failure: a negative errno |
| IEEE 754's round half to even, as WGSL rounds | The one rounding in every language |

## Codemap

- `link/spec/` and `kit/spec/` hold the frame and the errno table, each with
  golden records, the link's of its frames and packets; `link/wit/` and
  `wit/` the interfaces.
- `gen` (`xpute-gen`) and `link/gen` (`xpute-link-gen`) are the generators,
  the second with the command a program writes its bindings with;
  `gen-targets` is the data specs `xpute-gen` writes of xpute's parts, and
  `tool` the `cargo spec` that checks and writes xpute's own.
- `kit/rust` (`xpute-kit`), `kit/ts` (`@xpute/kit`) and `cpp/xpute/kit` are
  the kit.
- `link/rust` (`xpute-link`), `link/ts` (`@xpute/link`) and `cpp/xpute/link`
  are the link; `guest/rust` (`xpute-guest`) and `cpp/xpute/guest` the
  guest's end.
- `host/rust` (`xpute-host`) and `host/ts` (`@xpute/host`) are the host,
  one in each language; `runtime/web` (`@xpute/runtime-web`) is the web
  runtime, its page half (`page.ts`) the web UI end, and `runtime/native`
  (`xpute-runtime-native`) the native runtime; and `expo` (`@xpute/expo`) is
  the native UI end, the Expo module that reaches the native runtime, with
  the runtime's C header (`ios/CXpute/xpute.h`) and its JNI class
  (`android/.../xpute/Runtime.kt`). The guest in Rust and C++, both runtimes
  and `expo` each have a README beside them that says how a program takes
  them.
- `golden/rust` (`xpute-golden`), `golden/ts` (`@xpute/golden`) and
  `cpp/dev/test/` are what the tests take,
  `cpp/amalgamation.awk` makes the C++ library's two files, and
  `cpp/dev/guest.cpp` is the smallest guest, which the build links over them
  as a module.
