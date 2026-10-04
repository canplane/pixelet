# What is left in the April build, and where it belongs

**2026-09-09** — a salvage audit of `checkpoint/20260405-xpute` and
`archive/20260409-wasm-aborted`, prompted by finding that the API had
silently lost its Vite build (and with it `__IS_DEV__`) during the August
restructure. Those branches hold a substantial parked design and nobody had
written down what is still worth taking from it.

No code moved. This is the list, so the next person reads one page instead
of three branches.

**Why it was parked is not repeated here** — see
[20260708-concurrency-model.md](./20260708-concurrency-model.md), which
covers the SAB/COOP dilemma, browser workers being event-loop based, and
Figma reaching the same cooperative answer without SAB. The short version:
the browser has no interruption primitive, so SAB is necessary and not
sufficient, and the retreat was a correct read of the substrate. The current
build already _is_ that design one level down — `modules/world/sched` is
yield plus quota with no self-adjusting feedback.

## What the design was reaching for

Demote JavaScript to a second-class citizen: everything in WASM, WASM
talking to WASM by command ID, HTTP left as a dumb byte transport. The
endpoint layout follows from that mechanically — a single RPC root, like a
syscall boundary, and addressed URLs only where an intermediary must cache
by URL. Tiles get an address because CDN, browser and R2 cache by URL;
everything else is a call.

XTP falls out of the same premise. A transport that must not judge what a
request is needs a payload that is self-describing in structure but not in
meaning, valid wherever it lands in memory, and readable without building an
object graph. Its README says exactly that — a relocatable binary tree
packet with lazy reads, and _"not a schema system by itself"_.

Which is also why the tile format and the RPC payload format are one thing.
A tile is a cached RPC response, so publishing one to R2 costs nothing
extra, and the packet's magic means the reader validates bytes instead of
trusting a content type. The `.env` misconfiguration on this same day proved
it from the other side: the dev server returned HTML and the failure
surfaced as `magic mismatch`.

## Already here — do not go looking

The portable half came across in the August restructure. Today's
`xpute/core` is April's `xpute/spec/lib` plus its transport-neutral codecs:
`ds/*`, `kit/*`, `sys/*`, `io/scalar`, `io/json_codec`, `io/marshal/tlv`,
and `io/marshal/tree` (XTP itself).

`pxl/module/world/lod.ts` is an ancestor rather than a part. `LOD_STEP_BY_LEVEL`,
`LOD_MARGIN = 1.4` and the hysteresis in `calculate_lod_step` are all still
in `module/world/lod.ts`, which has since grown blend bands and per-threshold
margins.

## Worth taking later

**`control/post.ts` + `control/cmd.ts` — the command plane.** The command-ID
RPC the whole design was for, and its contract is the most valuable thing on
those branches:

> CommandPort does not own transport semantics (routing, reliability,
> request/response direction, tx_id allocation).
> CommandPort MUST NOT set/clear FrameFlag.RES — router/runtime owns
> directionality.
> preserves all non-payload fields by echoing the input frame and only
> replacing: `errno`, `body`.

A port that is pure codec and dispatch, knowing nothing about transport,
mounts on IPC, HTTP or a WASM boundary unchanged. Its args codec (TLV) is
already in `xpute/core`.

**`io/byte_envelope.ts`, `rpc/wire_header.ts`** — the framing, separable
from the rest.

One change is certain: `Command` is a string enum while `Vector` is a u16.
Across a WASM boundary the command wants to be numeric too, so what comes
back is the shape, not the file.

## Worth dropping

**`control/signal.ts` + `control/vec.ts` — the signal plane.** `Vector` is a
kernel interrupt table: critical (BOOT, SYNC, PANIC), system (AUTH, TILE),
and a `NET_MASK` band (LINK_UP, PEER_UNSTABLE, PEER_UNREACHABLE). The net
band needs many peers to mean anything. Two planes existed because signals
were fire-and-forget and posts were request/response; with one module
boundary, one plane is enough and the user-zone vectors are just commands.

**`ipc/frame.ts` as it stands.** 226 lines importing `ipc/topology` and
`rpc/node`, half of it `WorkerReady`/`WorkerInit`/`LinkInitSyn`/`LinkInitAck`
— a world where workers attach and detach. What survives is `FrameKind`,
`FrameFlag`, `errno`, `body`, `args`, and `byte_envelope`/`wire_header`
already hold it.

**`net/xport/{central,mesh}`, `net/crypto`, `ipc/topology`, `rpc/node`.**
These exist to paper over browser transports. Off the web there is nothing
to paper over — real sockets, QUIC, TLS — and a hand-rolled crypto layer is
a liability rather than an asset. The topology concepts are sound, but a
native host arrives with its own task runtime to map onto.

## Where these designs live

Not the browser, which is the finding rather than the disappointment. They
assume many instances, a real scheduler and interruptible execution: a
server-side WASM host (wasmtime, wasmer) or a native runtime, where the
abstractions stand on something instead of imitating it.

What travels regardless is the contract and the format — `CommandPort`'s
layering, and XTP, whose relocatable lazy-read shape is worth more against
linear memory than it is here.

## One caveat

It was parked, not finished. Taking it forward moves a design that was never
proven at scale, not something that worked. Expect to meet whatever stopped
it again, in whatever host it lands in next.
