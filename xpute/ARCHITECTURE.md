# Architecture — xpute

[xpute](./README.md) claims that a program's compiled **guest** and the
**host** that serves it need share nothing but the guest's memory and one kind
of message laid in it. This document follows that claim through the parts that
make it hold, in the order they depend on each other, across xpute's three
parts: the guest, the host and the link between them, which WIT draws; the
runtime, the platform's environment that runs them; and the kit beneath both.
The **link** carries every call as a frame. The **sides** run the link alike on
every platform, each end over a medium its runtime plugs in, a UI's too. A failure is its call's to
judge, and a bug ends in a stop, after which the link answers what was owed, so
that no side waits on an answer that will not come. The GPU is the one device
whose work holds the guest's memory past an answer, so it is the one device the
host serves. And the three languages hold the same facts: a spec or WIT owns
each one and a generator writes it into each, and frozen records hold the rest
of their code to each other. The document ends with
what xpute does not know, what it took from earlier work and a map of the code.
The layouts are written once in `spec/` and the interfaces in WIT, and this
document points there.

---

## The link

A frame is four words and a packet (`link/spec/frame.json5`): TAG, the
request; CMD, the handle of the object called and the method's place in its
interface, with a RES bit that marks an answer; RESULT, the answer's status
alone, 0 or a negative errno; and PKT, the packet's length. The guest lays a
frame in its outbox and crosses by `link_out`; the host lays one in the
guest's inbox and enters it by `link_in`. Each crossing returns at once what
the method came to.

Whatever is called is an object under a handle. A singleton interface is a
**named handle**, the hash of its package and name, called with no open. An
object of which there are many, a file or a surface, is opened by a **URI**, a
tuple of tokens, through the root's `open`, which the serving side answers
from the opener mounted at the longest matching prefix. The serving side is
polymorphic behind the handle, so the caller never branches on what it calls.

A packet holds the args, or an answer's value, in the Component Model's
Canonical ABI layout, with four departures. A pointer is an offset into the
packet, which is the one copy. A buffer the caller lends, WIT's `lent`
(`link/wit/types.wit`), stays where it lies and is the callee's until the
answer; its address is a u64 on every platform, an offset into a module's
memory or a pointer natively, so a packet has one layout everywhere. A packet
that does not hold what its type says is refused, EBADMSG,
where the ABI traps. And RESULT is status alone, the value in the packet.

A `func` answers at once, and is called. An `async func` answers later, and is
sent: the crossing answers EINPROGRESS, and the callee answers later by a
frame marked RES with the request's tag. The guest is entered one entry at a
time, and an answer is never run inside the `send` that asked for it. The
guest runs only when it is entered, by a request, an answer, or the answer of
what it armed; when it runs is the program's.

## The sides

An end of the link is a **`Side`**, the same machine in Rust, TypeScript and
C++: it lays frames in its own box, crosses by its own function, serves what
is mounted on it and hears the answers to what it sent. The guest holds one
over its outbox, and the host one over the guest's inbox.

What runs beside the guest is split the same way everywhere, and every
dependency points down: a part declares the interface it needs, and a part
above implements it and injects it. The **host** is the guest's counterpart: the
host's end of the link, the bindings of what is served there, and xpute's own
servers, the clocks and the stop. It varies by language, as the guest does,
and names no runtime. The **runtime** is the platform's environment, a page's
worker or a device's guest thread. It instantiates or links the guest, makes
the host's `Side`, injects into the host what the host needs of it (the way
back to the guest's thread, where a report goes, the log) and runs the loop
that enters the guest. Nothing names a runtime but its own UI end and the
program's assembly. A plugin stands beside the runtime, as its own guest's end
and host, and neither names the other. The program starts its guest with a `Boot`, which lists
its servers, the host's among them, each a `Server` that mounts some of the
guest's imports, and its runtime runs the steps in their order, on both
platforms alike: it mounts each server, serves the UI's side, enters the guest
only once every interface the guest's world imports is mounted, and on a
device runs the loop.

An end of the link is one machine whatever carries its frames, as a link
layer knows its two ends and not the wire. What carries them is the
**medium**, which the runtime plugs in: the guest's boxes and the crossing into
it, or a port to a UI on another thread (a page's MessagePort, a device's C
ABI). A call that waits for its answer needs a medium that can wait, so an end
over a port has none. The runtime bridges an end over a port to the guest's
end, as a bridge joins two links. A method the UI serves that WIT marks
`async func`, which the bindings list as `LATER`, it answers later by its tag;
any other is told to it and answered at once.

The UI is one party on the link, over a port, and its connection is an object like
any other: it attaches by opening a handle, everything it sends and takes names
that handle, and it leaves when the handle is dropped, which drops what it
opened and answers ECANCELED what it owed, by the link's own drop. The guest is
told only that a remote attached (`xpute:life` `attached`); the WIT between it
and the UI does not change. A device's runtime outlives its UI, so a reload
opens a new handle on the same runtime and drops the old one, and the runtime
keeps the guest's end, its last stop and its trap, which a new UI reads as it
begins to hear them. A page's worker dies with its page, so a reloaded page
meets a new runtime.

## How a failure ends

A failure is one error, `XputeError`, carrying an errno
(`kit/spec/errno.json5`). What happened is its errno's kind, the gRPC and
Abseil status code. The kind says nothing of whether the failure is fatal:
that is the call's. A caller that holds a failure impossible makes it a
**bug** (`bug` and `or_bug`), reported with its errno, file and line before it
stops.

Between xpute's parts a failure moves as an `XputeError` with its optional
message, an end as a stop or a trap report, the stop with the bug's optional
message (a panic the language raises, the panic's own), and what is worth
telling and is neither as a log record, a level, a context and a message
(`wit/log`, in `wasi:logging`'s shape). Each end has its `log`: the guest's
calls the import, and the host serves it and hands the records, with its own,
to the log its runtime injects. The runtime alone writes them to the
platform's output, its error stream, Android's log or a page's console, and
alone decides by level which it writes; no other part drops a record.

A guest's bug is reported through `stop`, and the guest traps. A host's own
failure is a stop on both platforms too, reported the same way, and then the
link traps, so no answer the host owed leaves the guest waiting. It is handed
off one of two ways: synchronously, caught at `link_out` while the host
answers the guest, or asynchronously, by the platform's listener for what the
worker leaves uncaught, or by Rust's panic hook. A trapped guest is entered no
more, and a frame to it answers EPIPE, a request already pending on it too,
once. The app hears the trap with what stopped it, alike on both platforms,
the imports no server mounts among them where the runtime refused to start
the guest.

## The device that lends the memory

A frame is complete in itself: once its answer is in, the caller may write
over everything it lent. Some work reaches into the guest's memory long after
it was asked for: a buffer the GPU uploads from, a word the GPU writes when a
request finishes. Such work needs a rule that says who owns which bytes and
when they come back. So the host implements exactly the device whose work
lends the guest's memory in this way, the GPU, and the program implements
everything else over the link.

The GPU is a plugin, declared in WIT as xpute's own packages are
(`gpu/wit/`), and a program that never draws links none of it. A renderer
makes thousands of calls a frame, and none needs a reply. For those the guest
writes a **stream** of records (`gpu/link/spec/api.json5`) in WebGPU's own
vocabulary and lends it to the host's `gpu.run`; until the answer, the records
and the memory they name are the host's. The host runs them on WebGPU on a
page and on wgpu on a device. The device is answered by `get-device`, and a
completion the stream asked for lands as a **fence**, a word only the host
writes, set to the guest's **ticket** with release, so a late answer to a
dropped request never matches. Both hosts admit a frame by one rule: a guest
draws only while fewer of its frames are unfinished than its frame latency,
which the program sets in its host's config and the host writes beside its
count in the device's block, so taking a surface's texture never blocks. What a surface shows is lent by the
UI, beside the link, under the handle of a surface the guest serves.

## The languages, held to each other

xpute is written in TypeScript, Rust and C++. The kit is symmetric: each
language implements the same collections, codecs, wire formats, errno table
and scalar math. So is the link: the same frame, codec and `Side`. The guest
exists in Rust and in C++ from the same WIT; the hosts are TypeScript's on a
page and Rust's on a device. Where the C++ differs, the language is the
reason: RAII, templates and results, with no exceptions and no RTTI.

A fact that crosses languages has one owner, a spec or WIT, and a generator
writes it into every language that holds it: `xpute-gen` the data specs, and
`xpute-link-gen` the link's bindings, each README saying how. XTP, the kit's
tree packet, is the format of data a program stores and serves, and of a
device's batches to its UI; the link's packets are the Canonical ABI's.

What holds the languages to each other is data. Under a part's
`spec/golden/` are records each language reads bit for bit, so a difference
means one side moved. A record is frozen: its writer, a Rust test behind
`XPUTE_GOLDEN_WRITE`, writes it again only when a contract changes on
purpose. Every
rounding in the tree is one rule, half to even as WGSL rounds (`math/scalar`),
so the guest, the UI and a shader round alike; a call that rounds otherwise is
refused by the tools.

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
  and a native guest's is its process's. A span the guest lends is named by
  its address (`lent`), a u64 on every platform: an offset into the module's
  memory in a module, a pointer natively, which a host in the same process
  reads in place and a device's UI through a buffer over that span alone. The
  GPU's stream carries its addresses so too, each in two words. Whether the memory is fixed, what lies where in it and
  which allocator each allocation takes are the program's, in either language.
- **The policy is the program's.** When the guest runs and how much I/O is out
  are decided outside xpute; it has no turn, no budget and no backpressure.
- **The host serves only what every guest shares**: the clocks, the stop and
  the GPU. Any other I/O is the program's, over its own interfaces.
- **xpute does not isolate.** The contract describes one host and one guest;
  a guest that does not return is not stopped.

## Prior art

| Precedent | What is taken |
| --- | --- |
| WIT and its generated bindings (wit-bindgen, jco) | Every interface on the link declared in WIT, and both ends' bindings generated from it |
| The component model's Canonical ABI and handle table | The packet's layout, `lent` from a borrow, `drop`, a later answer as `task.return`, and handles reused with a generation |
| Binder's handle 0, 9P's walk and open | The root: one fixed `open` that turns a URI into a handle |
| WASI's `wasi:clocks` | The clocks, as a subset |
| A GPU's command buffer, as WebGPU's encoder records one | The GPU stream |
| gRPC's and Abseil's status codes | An errno's kind |
| Linux's uAPI, which carries a user pointer in an ioctl's struct as a `__u64`; Vulkan's `VkDeviceAddress`; WebGPU's `GPUSize64` | An address the link carries, a u64 whatever the platform's pointer, so one layout and one golden record serve every platform |
| [evanw/buddy-malloc](https://github.com/evanw/buddy-malloc) | `kit`'s buddy allocator, ported line for line |
| POSIX and Linux errno | Every failure: a negative errno |
| IEEE 754's round half to even, as WGSL rounds | The one rounding in every language |

## Codemap

- `link/spec/`, `gpu/link/spec/` and `kit/spec/` hold the frame, the GPU
  stream and the errno table, each with golden records, the link's of its
  frames and packets; `link/wit/`, `wit/` and `gpu/wit/` the interfaces.
- `gen` (`xpute-gen`) and `link/gen` (`xpute-link-gen`) are the generators,
  the second with the command a program writes its bindings with; each
  part's `gen-targets` is the data specs `xpute-gen` writes of it, and
  `dev/tool` the `cargo spec` that checks and writes xpute's own.
- `kit/rust`, `kit/ts` (`@xpute/kit`) and `cpp/xpute/kit` are the kit.
- `link/rust` (`xpute-link`), `link/ts` (`@xpute/link`) and `cpp/xpute/link`
  are the link; `guest/rust` (`xpute-guest`) and `cpp/xpute/guest` the
  guest's end.
- `web/runtime` (`@xpute/web-runtime`) and `web/host` (`@xpute/web-host`)
  are a page's runtime and host; `native/runtime/rust`
  (`xpute-native-runtime`) and `native/host` (`xpute-native-host`) a
  device's, and `native/runtime/expo` (`@xpute/native-runtime`) the Expo
  module the UI reaches them by.
- `gpu/link` and `cpp/xpute/gpu/link` are the GPU's link, and `gpu/guest/rust`
  and `cpp/xpute/gpu/guest` its encoder; `gpu/web/host` and `gpu/native/host`
  its hosts on WebGPU and wgpu, the device's with its bindings, served on the
  side `xpute-native-runtime` makes, and `gpu/native/runtime/expo` a
  device's surface view.
- `dev/golden` and `cpp/dev/test/` are what the tests take,
  `cpp/amalgamation.awk` makes the C++ library's two files, and
  `cpp/dev/guest.cpp` is the smallest guest, which the build links over them
  as a module.
