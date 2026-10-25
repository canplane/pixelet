# xpute

xpute runs one program inside another across three edges and no others: one memory they share, one call in, and one number out. The outer program, the **host**, owns the platform: the clock, the memory, whatever reaches outside. The inner one, the **guest**, owns none of it on its own account. Everything it has, the host granted it: the memory once at boot, and a quota of time on every turn. What the guest builds over those grants — its allocator, its scheduler, its tables — is its own: the host/guest split follows the exokernel/library-OS division, and xpute does not enforce those grants.

**xpute is both ends of that arrangement, and `kit`, the generic code underneath them.** `kit` is written in every language xpute has, and each end in the languages it is built in.

It was written for a program driven by TypeScript: compiled to one WebAssembly module in a browser tab, or linked into a native app. Nothing in xpute knows what that program computes.

## The arrangement

```text
┌─ host ── TypeScript ───┐                        ┌─ guest ──────────────────┐
│ owns the clock, the    │                        │ computes, and owns       │
│ memory, the I/O        │ ═ interrupt(quota) ═▶  │ nothing                  │
│                        │                        │                          │
│                        │  ◀═══ wake_ms ═══════  │                          │
└───────────┬────────────┘                        └────────────┬─────────────┘
            │                                                  │
            └──────────────────────────┬───────────────────────┘
                                       ▼
           ┌─ the core memory ───────────────────────────────────┐
           │ everything else crosses here, and nothing else does │
           └─────────────────────────────────────────────────────┘
```

Three edges, and one call the other way: the guest reads the host's clock (`now`), because it spends its quota against that clock in the middle of a turn, when the host cannot run. There is no second entry point, no other callback, and nothing either side holds a pointer to on the other.

`interrupt` is the whole of the guest's life. It runs when the host rings, for as long as the quota it was handed, and when the call returns it is not running at all. Its answer, `wake_ms`, is when it would like the next turn. A guest that asks for nothing is rung for nothing.

Everything else is written into the memory while a turn runs. The host's commands wait for the guest in one queue, and what the guest answers or raises waits for the host in another, each a message of four words. What the guest cannot do itself — a file, a call to a GPU — it writes as records into a stream, and the host runs every record after the turn returns and before it rings again. Every file call goes to the program's one `SysHost.open`: xpute does not route by root, and the program dispatches by root to whatever it bound there.

[ARCHITECTURE.md](./ARCHITECTURE.md) is what each piece is and why it is shaped that way: the memory, the messages and the packets they carry, the turn, the handles, I/O and the GPU's calls, the contract either end must keep, and the records that hold the languages to it.

## `kit`, `guest` and `host`

`kit` is that and nothing more — collections, codecs, scalar math, an errno table, the wire formats. Link it and use it: it knows nothing of turns, hosts or memories.

Golden records under `spec/golden/` hold the languages to each other wherever bytes cross.

`guest` and `host` are the arrangement above, named by the end each is. They are the two ends of one thing, not two implementations of it: a module on one side with nothing facing it is not a gap to fill, and what the two owe each other is not matching module lists but matching formats. The guest end exists in more than one language, and those are one end, so they match as `kit` does.

```text
rust/kit  ·  ts/kit  ·  cpp/kit                the same kit, in each language
rust/guest  ·  cpp/guest  ·  ts/host           the guest end, in each language · the host end
rust/gpu  ·  ts/gpu  ·  cpp/gpu                WebGPU's calls, which a guest writes and a host runs
rust/conformance  ·  cpp/conformance           a guest in each, and the contract's traces
ts/conformance                                 the host that runs the traces against them
ts/script                                      what the repository runs over its trees, never shipped
spec                                           numbers all of them are generated from
```

## What it leaves to whoever uses it

The line is drawn in the same place every time: xpute carries the mechanism, and the policy belongs to the program.

- **It does not know what the guest computes.** No domain types reach `kit`, which is what keeps it portable to a second program.
- **It does not choose the memory map.** It is told a base, a unit and a list of sizes, and answers where each range landed. What the ranges hold is the guest's.
- **It does not choose an allocator.** The heap is one door, and what answers behind it is the guest's choice.
- **It is not a transport.** Nothing here leaves the machine; what carries bytes to another one is the host's business.
- **It is not real time.** A quota is how long a turn should take, not a deadline. Nothing preempts: a turn that runs long is charged for it in the next one rather than cut off.
- **It is not an RPC framework.** A message is four words, and the numbers in them are the program's own.

## Running it

Deno 2.x, a Rust toolchain with the `wasm32-unknown-unknown` target, a C++20 compiler as `c++` or `$CXX`, GNU Make, and the [WASI SDK](https://github.com/WebAssembly/wasi-sdk) at `/opt/wasi-sdk` or `$WASI_SDK_PATH`, for the C++ guest built as a WebAssembly module.

```sh
deno task check   # deno fmt --check, deno lint, deno check, cargo fmt --check, clippy, the C++ compiled
deno task test    # deno test, cargo test and the C++ tests, the golden records included,
                  # and the traces against every guest, native and WebAssembly, once build has made them
deno task build   # the guest the traces drive, in Rust and C++, native and WebAssembly; test builds it first
deno task test:gpu  # the GPU hosts against golden/gpu: the native one on wgpu, the web one in Chrome (installed, or $CHROME)
deno task lint    # deno lint
```

Each runs every tree's own task in parallel; `check:ts`, `check:cpp`, `check:rust` and the like run one tree's, as `deno task --cwd ts`, `make -C cpp` and `cargo` in `rust/` do.

## License

xpute is MIT. The buddy allocator under `alloc/`, in Rust and in C++, is a port of someone else's code rather than a dependency, and `THIRD_PARTY_NOTICES.md` carries the notice its license asks for.
