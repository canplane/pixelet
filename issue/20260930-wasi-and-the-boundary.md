# WASI and the boundary

**2026-09-30** — Building a guest for `wasm32` raised whether xpute should
lean on WASI: take its target, wrap its surface in WASI's, or reorganize
around it. This records what WASI is as of the day, where it meets xpute's
contract and where it cannot, and what was decided.

## WASI as of the day

WASI 0.2 ended its patch series at 0.2.12. WASI 0.3.0 shipped on 2026-06-11
and 0.3.1 on 2026-08-11, with 0.3.x every two months through 2027 and no
date for 1.0. 0.3 moves async into the component model itself — `async
func`, `stream<T>` and `future<T>` — and removes `wasi:io`; what is left is
`cli`, `http`, `sockets`, `filesystem`, `clocks` and `random`. Wasmtime 46
enables it by default; in a browser a component runs through jco's
transpilation, and whether jco carries 0.3's async was not confirmed.

Two facts about the component model decide the rest. A `stream<T>` read or
write names a buffer in the caller's memory and the values are copied into
or out of it; nothing is said of zero copy. And an async export may be
stackless: the runtime calls a callback per event until it returns 0, which
is close to xpute's turn.

## Where it meets the contract

The component model is shared-nothing: a component and its host never see
each other's memory, and what crosses is copied through the canonical ABI.
xpute's boundary is the opposite — the host reads the guest's memory where
it lies, and a turn crosses once. So:

- **WASI at the wasm boundary conflicts.** A guest whose imports and exports
  are `wasi:*` gives its host no right to its memory; the rings leave the
  boundary for the host's inside, and the boundary becomes calls and copies
  — per call, where xpute pays per turn. That cost lands on the end user's
  device, per frame.
- **A WASI-shaped API inside the guest does not.** The guest calls an API
  with WASI's vocabulary and xpute's runtime turns the calls into messages,
  the way a libc lays POSIX over io_uring. The wasm boundary stays the
  contract. What it buys is familiar vocabulary and interfaces written
  against a standard; what it does not buy is binary compatibility — a WASI
  runtime cannot run such a guest as it is.
- **An adapter component is the subsystem.** A WASI program composed with an
  adapter that exports `wasi:*` and speaks the contract below runs on an
  xpute host, the way WASI-Virt virtualizes a component. The adapter and
  its host still share memory, so it runs on xpute's own host and not on a
  standard runtime.

The last two share one precondition: an API above the backend must be
async-shaped, since xpute answers on a later turn. WASI 0.2 and earlier were
synchronous and could not share one API with xpute; 0.3 can.

## The target

Guests stay `wasm32-unknown-unknown`. Under `wasm32-wasip1` the standard
library reaches the host through WASI's imports — `fd_write` for a print,
`clock_time_get` for an instant, `random_get` for a hash seed — a second
channel beside the contract, and a second clock beside `now`; a browser
would need a shim to answer them. The WASI SDK builds C++ for the same kind
of guest: its linker and libc++ are what a C++ guest needs, and a guest that
calls nothing of the OS imports nothing of WASI.

## What was decided

- The contract stays the boundary. The page and the device, turns, and
  reading in place are what it is for, and a WASI boundary covers wasm
  alone and gives up the last two.
- WASI as a subsystem over the contract is queued in
  [BACKLOG.md](./BACKLOG.md), not now: it is what would let an existing
  program in, which is where an open-source xpute gains users, and it has
  no program to be built against yet.
- Aligning the vocabulary is the part worth having early, and it is the
  same question as the resource vocabulary
  ([20260930-resources-files-and-streams.md](./20260930-resources-files-and-streams.md)).

## Where it stands

**Decided** 09-30. Sources: the WASI roadmap (wasi.dev/roadmap), the WASI
0.3 release page (wasi.dev/releases/wasi-p3), and the component model's
`design/mvp/Concurrency.md`.
