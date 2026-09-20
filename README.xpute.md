# xpute

xpute runs one program inside another. The **host** owns the machine — the clock, the memory, the I/O — and the **guest** computes and owns nothing. Everything the guest has, the host granted it, a slice at a time: the memory once at boot, a quota of time on every turn, credits for the I/O it asks for.

**xpute is both ends of that arrangement, and `core`, the generic code underneath them.** Both are written twice, once in Rust and once in TypeScript.

It was written for a Rust program compiled to one WebAssembly module and driven by TypeScript in a browser tab. Nothing in xpute knows what that program computes.

## The arrangement

```text
host · TypeScript                          guest · Rust
─────────────────                          ────────────

strobe   a frame, and only when one
         was asked for
quantum  a quota, from the frame it
         measured rather than assumed

commands ─────────── SQ ring ───────────▶  drained on the rising edge
doorbell ═════ interrupt(quota_ms) ═════▶  ┌ tick   phases in a fixed order
                                           │ tasks  futures that yield when
                                           │        the quota is spent
                                           │ queue  what I/O it wants run
completions ◀───────── CQ ring ──────────  └ posted before the falling edge
         ◀═══════════ wake_ms ═══════════  next frame · in n ms · never

credits  runs what that queue started,
         as far as the grant goes

         … nothing until wake_ms …         … nothing at all …
```

Each of the host's levers faces one of the guest's. The **strobe** decides whether a turn happens at all, the **tick** what runs inside it. The **quantum** sizes the turn; the **tasks** yield when it is spent. The **credits** run the I/O the **queue** asked for, since the guest runs none. The **doorbell** is the one call in, **`wake_ms`** the one answer out.

The strobe rings on an animation frame, and only when something asked for a turn: an input or an arrival on the host's side, or the guest's own last answer. A guest that wants nothing is rung for nothing.

The call itself carries nothing but the quota. Everything else crosses through one memory that both ends read where it lies.

```text
┌─ one linear memory ─ reserved once, never grown, read in place ──────────┐
│  sections    ranges laid end to end, placed at boot                      │
│  rings       the SQ and CQ above: four-word frames, payloads apart       │
│  change log  topic · key · revision, the read direction's own channel    │
│  heap        one door, with a slab over a buddy behind it                │
└──────────────────────────────────────────────────────────────────────────┘
```

A tick spends a quota of time on the guest's own work; the queue spends a grant of credits on work the host does for it. The host owns the connections the way an operating system owns its devices, and that is the whole of the division.

## `core`, and `runtime`

`core` is a kit and nothing more: an arena, a deque and a binary heap; a JSON codec; scalar, vec3, mat4 and an rng; an errno table; XTP and TLV; and, on the Rust side, a buddy and a slab. Link it and use it — it knows nothing of turns, hosts or memories.

It is the same kit on both sides, each with the few pieces only its own language needs. The maths runs over one libm in both, so a double is the same bits wherever it is made, and the golden records under `crates/xpute/core/golden/` hold the two implementations to each other wherever bytes cross. The TypeScript tests read those same record files rather than keeping a copy, because a second copy is how two implementations come to disagree without anything failing.

`runtime` is the arrangement above, and it is not symmetric — the two halves are the two ends of one thing, not two implementations of it. A module on one side with nothing facing it is not a gap to fill. What the two must agree on is the format: the ring's layout, the frame's four words, the memory's sections.

```text
crates/xpute/core     ·  packages/xpute/core       the same kit, twice
crates/xpute/runtime  ·  packages/xpute/runtime    the guest end · the host end
spec/xpute                                         numbers both are generated from
```

## What it leaves to whoever uses it

The line is drawn in the same place every time: xpute carries the mechanism, and the policy belongs to the program.

- **It does not know what the guest computes.** No domain types reach `core`, which is what keeps it portable to a second program.
- **It does not choose the memory map.** `mem/section` is told a base, a unit and a list of sizes, and answers where each range landed. What the ranges hold is the guest's.
- **It does not choose an allocator.** The heap is one door. `core` offers a buddy and a slab that fit behind it, and a guest is free to put something else there.
- **It is not a transport.** A ring is a producer, a consumer and a memory they share. What carries bytes to another machine is the host's business.
- **It is not real time.** A quota is how long a turn should take, not a deadline. Nothing preempts: a turn that runs long is charged for it in the next one rather than cut off.
- **It is not an RPC framework.** A message is four words — who asked, what is asked, what came back, where the packet is — and the numbers are the program's own.

[ARCHITECTURE.xpute.md](./ARCHITECTURE.xpute.md) is what each piece is and why it is shaped that way: the memory, the rings, the change log, the turn, the credits, the handles, the wire formats, the records.

## Running it

Deno 2.x and a Rust toolchain.

```sh
deno task check   # deno fmt --check, deno check, cargo fmt --check, clippy
deno task test    # deno test and cargo test, the golden records included
deno task lint    # deno lint
```

## Licence

xpute is MIT. One file under `alloc/` is a port of someone else's code rather than a dependency, and `THIRD_PARTY_NOTICES.md` carries the notice its licence asks for.
