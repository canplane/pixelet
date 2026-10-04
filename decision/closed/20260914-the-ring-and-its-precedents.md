# The ring and its precedents

**2026-09-14** — recorded as reference, while the `pxl_*` exports were
being folded into the rings
([what stays in JavaScript](./20260914-what-stays-in-javascript.md)). The
question was whether the model — a TypeScript shell and a WebAssembly
kernel that talk through submission and completion rings in shared memory,
packets in XTP, and a tick that hands over the turn — is common, and where
to look when a design question about it comes up.

What is below is from general knowledge of those systems, not from reading
their source for this document; a claim that turns a decision here should
be checked against the system itself first.

## The shape, stated once

- A message is four words — tag, command and flags, result, packet — and
  the packet lives in the ring's own arena.
- The producer writes and moves the tail; the consumer reads and moves the
  head. One writer per word, no lock.
- The doorbell carries no message: it hands over the turn
  (`interrupt(vector)`), and the receiver reads what waits.
- A full ring refuses (EAGAIN); nothing is dropped, and the producer
  waits.
- A command is a number, major and minor; a reply is marked (`RES`), and
  only what asks for one gets one (`ACKREQ`).

## Where the same shape is ordinary

**Between a kernel and its devices.** This is where the model is standard.

- **io_uring** (Linux): a submission queue and a completion queue shared
  between the process and the kernel, entries of fixed size, buffers
  registered once and named by index, a system call (or none, with polling)
  to say "look". The closest precedent for the rings here, including the
  split between a fixed entry and a separately held payload.
- **virtio virtqueues** and **NVMe queues**: descriptor rings in memory
  both sides map, a doorbell write naming the new tail, completions in a
  ring of their own.
- **GPU command buffers**: commands recorded into a buffer first and
  submitted once. WebGPU's `GPUCommandEncoder` → `queue.submit` is the
  shape the renderer already follows on the other side of the kernel.

**Inside game engines.** A render thread fed by a command queue the game
thread fills, a frame at a time; a job system whose workers pull from
queues. The tick as the interrupt is the engine loop's own idea.

**In the browser, for audio.** A single-producer, single-consumer ring over
a `SharedArrayBuffer` between the main thread and an `AudioWorklet` is the
known way to move samples without allocating or posting messages each
block. Different payload, same discipline — though there the ring crosses
threads, and here it does not.

## Where the opposite was tried, and left

**React Native's original bridge**: JavaScript and native code exchanged
messages serialized as JSON, batched and asynchronous. Serialization cost
and the asynchrony became the bottleneck, and the new architecture replaced
it with JSI — synchronous calls into native objects. The lesson taken here
is not "messages are slow" but what made those slow: text serialization,
copies, and a queue across threads for every call. The ring here is binary
(XTP), written in place in one linear memory, with the turn handed over on
one thread. Measured on V8: a ring call with no packet costs what the
export it replaced did (92 ns against 91 ns); what a query pays past that
is the TypeScript XTP encoder's per-leaf objects, not the ring
([what stays in JavaScript](./20260914-what-stays-in-javascript.md)).

## Where WebAssembly applications usually stand

**Bindings.** The large C++-to-WebAssembly applications on record join
their UI to their core through bindings — direct calls, generated or
hand-written. Figma describes a bindings layer between its TypeScript UI
and its C++ document and canvas; what that layer is inside is not public
([boundaries, commands, and what comes back from April](./20260913-boundaries-commands-and-what-comes-back-from-april.md)).
Photoshop on the web is on record as using Emscripten's embind. The
WebAssembly component model's canonical ABI is also a call interface, with
resources as handles.

**What is uncommon here** is the combination: a UI shell and a WebAssembly
kernel joined only by rings and a cooperative tick, with the bindings kept
to the turn-handing primitives. No public example of exactly that was found.
So when a question about this boundary comes up — backpressure, ordering,
who owns a buffer, when a reply is due, how a query stays cheap — the
precedents to read are io_uring, virtio and the engines' command queues,
not the web's binding generators.

## The questions those precedents have already answered

Kept here because each will come up again:

- **A synchronous answer on an asynchronous queue.** io_uring leaves some
  operations as plain system calls, and a caller can wait on a completion
  in the same call. Here: a query is a submission with `ACKREQ`, rung for
  in the same turn (`CoreBuild.call`).
- **Registered buffers instead of copies.** io_uring registers buffers once
  and names them by index; virtio passes descriptors, not data. Here: bulk
  data stays in lanes at addresses the kernel hands out; the packet carries
  the address, not the bytes.
- **Backpressure over loss.** A full NVMe or io_uring queue makes the
  producer wait. Here: EAGAIN, and the kernel holds what a turn made until
  the completion ring has room.
- **Batching.** A GPU command buffer is filled, then submitted. Here: a
  table taught once is one packet (the terrain's materials, every decor
  kind), where a call per entry was a thousand crossings.
- **Mechanism and policy.** The OS keeps the queue and the doorbell
  generic and lets the driver decide what a command means. Here:
  `xpute-core` is the ring, the arena and the numbering scheme; what the
  commands are is `pxl`'s
  ([mechanism and policy, and what xpute is](./20260914-mechanism-and-policy-and-what-xpute-is.md)).
