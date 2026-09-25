# xpute

xpute runs one program inside another. The **host** owns the machine — the clock, the memory, the I/O — and the **guest** computes and owns nothing. Everything the guest has, the host granted it, a slice at a time: the memory once at boot, a quota of time on every turn, credits for the I/O it asks for. That is what an operating system has with a process, without the hardware that enforces it.

**xpute is both ends of that arrangement, and `core`, the generic code underneath them.** All of it is written twice, once in Rust and once in TypeScript.

It was written for a Rust program compiled to one WebAssembly module and driven by TypeScript in a browser tab. Nothing in xpute knows what that program computes.

## The arrangement

```text
┌─ host ── TypeScript ───┐                        ┌─ guest ── Rust ──────────┐
│ owns the clock, the    │                        │ computes, and owns       │
│ memory, the I/O        │ ═ interrupt(quota) ═▶  │ nothing                  │
│                        │                        │                          │
│                        │  ◀═══ wake_ms ═══════  │                          │
└───────────┬────────────┘                        └────────────┬─────────────┘
            │                                                  │
            └──────────────────────────┬───────────────────────┘
                                       ▼
           ┌─ one linear memory ─────────────────────────────────┐
           │ everything else crosses here, and nothing else does │
           └─────────────────────────────────────────────────────┘
```

Three edges, and no others: there is no second entry point, no callback, and nothing either side holds a pointer to on the other.

`interrupt` is the whole of the guest's life. It runs when the host rings, for as long as the quota it was handed, and when the call returns it is not running at all — its answer is when it would like the next one. A guest that asks for nothing is rung for nothing.

Two budgets run and they never touch: time for the guest's own work, and credits for the work the host does on its behalf.

[ARCHITECTURE.xpute.md](./ARCHITECTURE.xpute.md) is what each piece is and why it is shaped that way: the memory, the rings, the change log, the turn, the credits, the handles, the wire formats, the records.

## `core` and `runtime`

`core` is a kit and nothing more — collections, codecs, maths over one libm, an errno table, the wire formats. Link it and use it: it knows nothing of turns, hosts or memories.

Golden records under `crates/xpute/core/golden/` hold the two languages to each other wherever bytes cross.

`runtime` is the arrangement above, and it is not symmetric — the two halves are the two ends of one thing, not two implementations of it. A module on one side with nothing facing it is not a gap to fill; what the two owe each other is not matching module lists but matching formats.

```text
crates/xpute/core     ·  packages/xpute/core       the same kit, twice
crates/xpute/runtime  ·  packages/xpute/runtime    the guest end · the host end
spec/xpute                                         numbers both are generated from
```

## What it leaves to whoever uses it

The line is drawn in the same place every time: xpute carries the mechanism, and the policy belongs to the program.

- **It does not know what the guest computes.** No domain types reach `core`, which is what keeps it portable to a second program.
- **It does not choose the memory map.** It is told a base, a unit and a list of sizes, and answers where each range landed. What the ranges hold is the guest's.
- **It does not choose an allocator.** The heap is one door, and what answers behind it is the guest's choice.
- **It is not a transport.** Nothing here leaves the machine; what carries bytes to another one is the host's business.
- **It is not real time.** A quota is how long a turn should take, not a deadline. Nothing preempts: a turn that runs long is charged for it in the next one rather than cut off.
- **It is not an RPC framework.** A message is four words, and the numbers in them are the program's own.

## Running it

Deno 2.x and a Rust toolchain.

```sh
deno task check   # deno fmt --check, deno check, cargo fmt --check, clippy
deno task test    # deno test and cargo test, the golden records included
deno task lint    # deno lint
```

## License

xpute is MIT. One file under `alloc/` is a port of someone else's code rather than a dependency, and `THIRD_PARTY_NOTICES.md` carries the notice its license asks for.
