# Boundaries, commands, and what comes back from April

**2026-09-13** — the move to a Rust core in WASM drawing through WebGPU was
scheduled today, piece by piece
([20260912-why-the-destination-is-wasm.md](./20260912-why-the-destination-is-wasm.md),
BACKLOG P0). Rust, WebGPU and a command structure arrive together, and
switching all three in one cut is the complexity to avoid. The question
this records: April's `checkpoint/20260405-xpute` built a full IPC/RPC
layer — frames, ports, a transaction controller, a central switch — for a
core of several WASM instances. What of it belongs in a core that is one
WASM instance on the main thread, and where does message passing belong at
all?

## What April built

Read from the branch, not from memory:

| Layer         | File                                   | What it does                                                                                                                                                                                                                                             |
| ------------- | -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Frame         | `spec/ipc/frame.ts`                    | `tx_id` (u32: worker id in the high 8 bits, a sequence in the low 24), `errno`, `flags` (`RES`, `ACKREQ`, `CACHE`), `args` as TLV, `body` as bytes. Two kinds: SIGNAL (`vec` u16, `sel` u64 selector, `rid`) and POST (`cmd` string)                     |
| Port          | `control/post.ts`, `control/signal.ts` | Codec and dispatch only: a handler `(args, body) → body`; the reply echoes the request and replaces `errno` and `body`; never touches `RES` or `tx_id`                                                                                                   |
| TxController  | `core/ipc/tx.ts`                       | One per worker: allocates `tx_id`, holds the `ACKREQ` table, a bounded mailbox (64 per transaction, 32k total), timeout as EOF, `wait_next` for fan-out streams                                                                                          |
| Switch        | `core/ipc/switch.ts`                   | Central, on the main thread: `vec`/`cmd` → (worker, port), takeover by port priority, `seal()` sends each worker its table. A response is routed by the high bits of `tx_id` with no lookup. A failed request gets a synthesized NACK carrying the errno |
| Worker, guest | `core/ipc/worker.ts`, `guest.ts`       | A worker loads devices, advertises what it handles (`LINK_INIT_SYN`), replies only when `ACKREQ` is set. The guest is a virtual worker (255) for the app and the network                                                                                 |
| Virtual link  | `core/ipc/worker/virtual.ts`           | An in-memory link with a worker link's interface, so the same code runs with or without a thread                                                                                                                                                         |

[20260909-april-checkpoint-salvage.md](./20260909-april-checkpoint-salvage.md)
already judged the command plane worth taking and the signal plane, the
selector and the transports worth dropping. This document takes that
further, now that the host is decided.

## What the references say

Figma is the closest precedent — a native core compiled to WASM, a web UI
around it, WebGPU since 2025 — so it was checked first.

**Confirmed:**

- The document representation and the canvas are C++; the UI around the
  canvas is TypeScript and React
  ([Evan Wallace](https://madebyevan.com/figma/),
  [Figma is powered by WebAssembly](https://www.figma.com/blog/webassembly-cut-figmas-load-time-by-3x/)).
- The two are joined by a "bindings layer", through which Figma is also
  moving parts of the C++ into TypeScript
  ([The Pragmatic Engineer, Building Figma Slides](https://newsletter.pragmaticengineer.com/p/building-figma-slides-with-noah-finer)).
  The WebGPU renderer uses Emscripten's WebGPU bindings, with hand-written
  ones where those were too slow
  ([Figma rendering: powered by WebGPU](https://www.figma.com/blog/figma-rendering-powered-by-webgpu/)).
- WASM in a browser runs synchronously on the same thread as JS; crossing
  the boundary costs mostly in converting data (strings through a decoder),
  and structured data crosses as an address into the memory
  ([Evan Martin, Notes on WebAssembly](https://neugierig.org/software/blog/2022/06/wasm-notes.html)).
- Message passing appears where isolation does: between a plugin's sandbox
  and its UI iframe, and to the Code Layers toolchain in a worker
  ([How we built the Figma plugin system](https://www.figma.com/blog/how-we-built-the-figma-plugin-system/);
  the worker case is in
  [20260911-single-thread-core-and-when-to-spawn-a-worker.md](./20260911-single-thread-core-and-when-to-spawn-a-worker.md)).

**Not found:** what the bindings layer is inside — whether the UI is
notified of a change or asks, whether a mutation is a call or a queued
command. Four Figma posts and a former engineer's notes
([Notes From Figma II](https://andrewkchan.dev/posts/figma2.html)) do not
say. Photoshop on the web is on record only as using embind
([web.dev](https://web.dev/articles/ps-on-the-web)). Nothing below claims
Figma does it the way this document proposes.

**The reading:** the UI and the core talk through **synchronous bindings on
one thread**, not through frames, transactions or a switch. IPC appears at
real isolation boundaries only. April put IPC inside the core as well —
the same difference 20260911 recorded as the reason April was set down.

## Three boundaries, three answers

What a boundary needs depends on what can go wrong across it.

| Boundary                   | Carries                                                                         | Transactions and lifetimes                                                                                                                                                                                               |
| -------------------------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **UI ↔ core**, main thread | A command is a synchronous call. State is read where it lives, in linear memory | **None.** Nothing is waited for, nothing is lost, so there is no timeout, retry or debounce. A reader that looks once a tick already coalesces                                                                           |
| **Main ↔ bake worker**     | A fixed frame header, and the body as a slab handed over by transfer            | **The scheduler's, not a new one.** A Demand already has a priority, cancellation and in-flight state, so `tx_id` names a Demand slot. A worker does not drop messages; a dead one is restarted and its Demands requeued |
| **Client ↔ server**, later | Packets                                                                         | **Here, and only here.** Loss, reordering and a peer that does not answer are real, so acknowledgement, deadlines and retry belong to this boundary. Designed when the server work starts                                |

The worry that bringing April's `TxController` back means owning lifecycles
and debouncing everywhere is right. On the first two rows a transaction
controller would be that work with nothing to protect against; only the
third needs one, and it does not exist yet.

## What comes back

**The command's shape, on every boundary.** A numeric `cmd` (u16 — April's
string was the part certain to change), args as an XTP packet, a reply of
`errno` plus body, and a dispatch table that is codec and dispatch only.
The same command is a function-table entry on the main thread, a frame to
the worker, a packet to the server. That is
[20260908-command-channel-for-state-mutation.md](./20260908-command-channel-for-state-mutation.md)'s
"one dispatch, swappable backend", with the backend now including a worker.

**The frame, for the worker only.** Kept from April: `tx_id` with the
worker id in its high bits, so a reply is routed without a lookup and a
switch can be inserted later without changing the header; `ACKREQ` as
behaviour (set, exactly one reply; unset, none); `errno` replies and a
synthesized failure reply. Changed: a fixed C-layout header instead of TLV
fields; args as XTP; the body as a slab transfer — April's own TODO read
"body will be replaced with SAB pointer handle", and SharedArrayBuffer is
ruled out, so the handle is a transferred slab.

**The virtual link.** A worker boundary with an in-memory twin keeps the
bake testable without a thread.

## What stays behind

- **`TxController`**: per-request promises and `setTimeout` deadlines,
  bounded mailboxes. The worker's lifecycle is the Demand's; the server's
  is designed with the server.
- **`Switch`, `seal`, the `LINK_INIT` handshake, the guest.** They earn
  their place with several workers or a network behind the switch. With
  one worker the main thread is the only router, and the header already
  leaves room to insert one.
- **`NodeSelector`**: a u64 held as BigInt, the very cost the move is for.
- **`rid`** as base64, the `CACHE` flag, `net/xport`, dynamic topology
  loading — as the salvage document already concluded.
- **Allocation per message**: April built every reply with an object
  spread. The frame's work is meant to allocate nothing; a header is
  written into a slab.

## Where reliability and transactions are decided

Reliable delivery, acknowledgement, deadlines, retry and a transaction
table are one question, and it is answered per boundary by what the
boundary can lose — not once, globally, as a layer every message passes
through. That was April's shape, and it is the one this document does not
take.

- **Inside a page (UI ↔ core, main ↔ worker) there is nothing to decide.**
  A call on one thread cannot be lost. `postMessage` between a page and its
  worker is not lost and not reordered; the one failure is the worker
  dying, which the worker's `error` event reports. So no acknowledgement,
  no timeout, no retry. What a request needs beyond correlating its reply
  — priority, cancellation, being re-run after a crash — is the
  scheduler's: the Demand already holds it, and `tx_id` only names the
  slot.
- **Across the network (client ↔ server) is where it is decided**, and
  when the server work starts, with the command channel
  ([20260908-command-channel-for-state-mutation.md](./20260908-command-channel-for-state-mutation.md)):
  what a lost or late command means for world-shared state, whether a
  command is idempotent by id, what the client does while a command is
  unconfirmed, and whether the transport's own guarantees (a WebSocket is
  ordered and reliable until it drops) leave only reconnection and replay
  to design. April's `TxController` is a candidate for that design, not a
  default.

Nothing is to be added to the in-page boundaries on the grounds that the
network will need it.

## Libraries considered

Suggested as a way to offload request/response handling, and checked on
2026-09-13 by reading the published packages:

|              | comlink 4.4.2                                                                   | postmsg-rpc 2.4.0                                 |
| ------------ | ------------------------------------------------------------------------------- | ------------------------------------------------- |
| Size         | 4.5 KB minified, 2 KB gzipped, no dependencies                                  | small, depends on `shortid`                       |
| Upkeep       | GoogleChromeLabs, pushed 2026-09-05                                             | last release 2018, last push 2021                 |
| Model        | a `Proxy` over the remote object, a Promise per call                            | a function name and arguments, a Promise per call |
| Timeout      | none — a dead worker leaves its Promises pending                                | none                                              |
| Cancellation | none                                                                            | `cancel()` removes the listener; the work goes on |
| Message      | a JS object by structured clone; buffers moved only when marked                 | a JS object, oriented to `window.postMessage`     |
| Per call     | a random id string, a Promise, object spreads, a path array per property access | an id string, a Promise, a closure, a listener    |

Neither does reliable transfer — inside a page there is none to do. What
both do is correlate a reply with its request by id, which in comlink is
about ten lines. Not taken for the bake worker, for four reasons:

- They do not take lifecycles off our hands: with no timeout or
  cancellation, a hung Promise is ours to guard, while the Demand already
  owns priority, cancellation and requeue.
- Completion arrives as a microtask beside the tick, a second path to keep
  in step with the one that reads Demand state.
- The message is a JS object graph. When the worker's inside becomes Rust
  (the Rust mesher in BACKLOG P0), the protocol would have to be imitated there or removed — the
  one-cut rewiring this schedule avoids.
- Each call allocates, on a path that runs dozens of times a second during
  a zoom-out.

The boundary is written directly instead: a `tx_id → Demand` slot table
and a header read in `onmessage` on the main side, a `cmd → handler` table
on the worker side, and a requeue on the worker's `error` event — on the
order of fifty lines. comlink remains a reasonable choice for a worker
that stays JS, runs one-off tool-shaped jobs, and has no part in a frame.

## Open

- **The read direction** (BACKLOG P0-1). The first row suggests the UI
  pulls: a revision counter per thing it watches, read once a tick, and a
  read of the lanes when it moved. That is a hypothesis to design, not a
  conclusion from Figma, which does not say.
- **Command numbering**: one table across the three boundaries, or one per
  boundary with the shared commands aligned. Decide with the second
  consumer.
- **Whether a mutation from the UI is applied at once or at the next
  tick.** Synchronous is simplest; a tick boundary would make a command
  log — undo, the server's replay — fall out of the same queue. Decide with
  the command channel.
