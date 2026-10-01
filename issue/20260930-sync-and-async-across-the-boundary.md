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

On a device the same needs take another road. `pxl_gpu_open`,
`pxl_surface_bind`, `pxl_surface_unbind` and `pxl_image_hold`
(`crates/pxl/web/native/src/`) are C functions the app calls, each run on the
kernel's thread by `on_kernel` while the caller waits. They are synchronous
calls into the guest, and they are outside the contract: no message, no
turn, and on the page no counterpart.

The two platforms differ for a reason the contract does not see: who owns
the device. On the page the host opens WebGPU and the kernel records its
calls in a stream, so an open device is news the host sends
(`GPU_CREATE`, a message). On a device the kernel opens the GPU itself
through wgpu, so the app asks it to, and waits. An image goes the same two
ways: a body the host holds and a message naming it, or a C call that hands
the bytes over.

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
  written into it; the GPU's two roads are what its absence looks like,
  though there the roads part over ownership, which a rule of kinds does not
  join.
- **The device's C calls belong in the contract.** They are the host
  calling into the guest and waiting — `Doorbell.call`'s cell — made
  outside it. As commands called in a turn of no quota, the page and the
  device would share one road. `pxl_surface_bind` hands over a
  `CAMetalLayer`, a pointer into the host's objects rather than an offset
  into the memory, so it would cross as an opaque word the host alone reads;
  rule 1 speaks of addresses into the memory, and this is none.

Seen against WASI 0.3, the contract is a component model with a discipline:
the host may call the guest and wait or not, and the guest may call the host
only for the time, taking everything else as a completion. A WASI surface
over the rings, if one comes, is that shape written in WIT.

## Where it stands

**Recorded** 09-30. The device's C calls into the contract is queued in
[BACKLOG.md](./BACKLOG.md); the rule of a command's kind waits to be written
into the contract with it.
