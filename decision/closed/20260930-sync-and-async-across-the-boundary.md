# Sync and async, across the boundary

**2026-09-30** — Whether xpute must carry a synchronous path beside its
asynchronous one came up twice: once while the contract was being drawn, and
again while weighing WASI 0.3, whose component model has both. This reads the
code for every place one side waits on the other, sorts them by direction and
by kind, and says what the contract covers and what it leaves outside.

## Two directions, two kinds

A call either waits for its answer or takes it later, and it goes either from
the host into the guest or from the guest out to the host. That makes four
cells, and each is a different question.

| Direction                           | Takes it later                                           | Waits                                                          |
| ----------------------------------- | -------------------------------------------------------- | -------------------------------------------------------------- |
| Host into guest (the shell asks)    | a message, answered on a later ring (`Doorbell.send`)    | a message rung with no quota until its reply (`Doorbell.call`) |
| Guest out to host (the kernel asks) | a signal, a stream record, a fetch; the result next turn | the clock (`now`), and nothing else                            |

Both host-side cells are in the contract: rule 2 says a question the host
needs answered before it goes on is asked in a turn of no quota, and
`Doorbell.call` is that. The guest's waiting cell holds the clock alone
(rule 7). The guest never waits on anything else, and the code bears it out.

## What the code asks

**The host into the guest, waiting.** On the page, `Doorbell.call` carries
`WORLD_CREATE` at boot, `SURFACE_CREATE` and `SURFACE_DELETE`, and the
profiler's `PROFILER_READ` and `PROFILER_TAKE`. Everything else the shell
sends — input, sensors, fetch results, loads, a surface's size — is a
message it does not wait on.

On a device the shell sends the kernel the same: the same calls, the same
messages, from the same shell code. Beside them the app calls four C
functions — `pxl_gpu_open`, `pxl_surface_bind`, `pxl_surface_unbind` and
`pxl_image_hold` (`crates/pxl/web/native/src/`) — and none of them is a call
into the guest. They are the device's own services, linked into the library
the kernel is in, doing what the browser does for a page: opening the GPU
through wgpu where the page opens WebGPU, binding a view's Metal layer to a
surface where the page hands over a canvas, decoding an image where the
browser decodes one. What the kernel is told of them comes as a message, as
on the page: `GPU_CREATE` once the device is open (`platform/gpu.ts`) and
`LOAD_IMAGE` with the decoded image's handle, beside the `SURFACE_CREATE`
and its kin that made the surface. A layer bound to a surface it is not
told of: on a device the kernel draws itself (its GPU stream is 0), so the
GPU, the bound layers and the decoded images are a library its renderer
calls into in its own process, where on the page they are the host's,
reached through the stream. The first three calls run on the kernel's thread
because wgpu's objects live there, not because they enter the guest; the
image is decoded off it.

The boundary is crossed the same way on both. What differs lies on either
side of it — which API opens the device, and on a device the renderer
drawing through a GPU library of its own rather than asking the host to.

**The guest out to the host.** The clock is the only call that waits. What
else the kernel wants from outside goes through the host's stream
(`spec/pxl/web/kernel/abi/host.json`) — a body copied in (`BODY_READ`),
a value stored (`STORAGE_SET`), text laid out and rasterized
(`TEXT_LAYOUT`, `TEXT_RASTER`) — which the host runs between turns, writing
what a record asked for into memory the record named. The kernel reads it on
its next turn. Text layout is the strongest case for waiting, and it does
not wait.

**Read without a call.** The kernel writes the heap's reading into a fixed
row at every turn's fall and names it in its directory (`HEAP_ROW`), which
is a read with no call behind it, the way Linux's vDSO page is; only
`kernel.test.ts` reads it. The snapshot and the profiler are read in place,
but at an address a signal or a call has just named.

## What follows

- **No synchronous call from the guest besides the clock.** Nothing in the
  code needs one, and every answer the kernel wants arrives by the next
  turn. The guest never blocks, and the contract can go on saying so.
- **A read without a call needs nothing new.** A program's own directory
  key, with xpute's PROGRAM bit, is how the host learns where a row lies,
  and `HEAP_ROW` is one.
- **Which kind a command is, is the command's.** One answered from what is
  already resident, at a bounded cost, waiting on nothing outside, may be
  called; one that waits on anything outside is a message. A command has one
  kind and not both. This was proposed when the contract was drawn and not
  written into it, and nothing in the code breaks it.
- **The page and the device cross the boundary the same way.** Every command
  the kernel receives comes by the same road on both; the device's C calls
  are its host's own services, and the boundary has nothing to say of them.

Seen against WASI 0.3, the contract is a component model with a discipline:
the host may call the guest and wait or not, and the guest may call the host
only for the time, taking everything else as a completion. A WASI surface
over the rings, if one comes, is that shape written in WIT.

## Where it stands

**Recorded** 09-30; **corrected** 10-01: the device's C calls are its
host's own services, not calls into the guest, and the page and the device
cross the boundary the same way. The rule of a command's kind is in the
contract's rule 2 since 10-01.
