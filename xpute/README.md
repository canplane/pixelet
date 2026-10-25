# xpute

xpute runs one program inside another across three edges and no others: one memory they share, one call in, and one number out. The outer program, the **host**, owns the platform: the clock, the memory, whatever reaches outside. The inner one, the **guest**, owns none of it on its own account. Everything it has, the host granted it: the memory once at boot, and a quota of time on every turn. What the guest builds over those grants — its allocator, its scheduler, its tables — is its own: the host/guest split follows the exokernel/library-OS division, and xpute does not enforce those grants.

**xpute is both ends of that arrangement, and `kit`, the generic code underneath them.** `kit` is written in every language xpute has, and each end in the languages it is built in.

It was written for a program driven by TypeScript: compiled to one WebAssembly module in a browser tab, or linked into a native app. Nothing in xpute knows what that program computes.

## The arrangement

```text
┌─ host ── TypeScript ───┐                        ┌─ guest ──────────────────┐
│ owns the clock, the    │                        │ computes, and owns       │
│ memory, the I/O        │ ═══ intr(quota) ════▶  │ nothing                  │
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

`intr` is the whole of the guest's life, the entry the floor names as a C runtime names `main`. It runs when the host rings, for as long as the quota it was handed, and when the call returns it is not running at all. Its answer, `wake_ms`, is when it would like the next turn. A guest that asks for nothing is rung for nothing.

Everything else is written into the memory while a turn runs. The host's commands wait for the guest in one queue, and what the guest answers or raises waits for the host in another, each a frame of four words. What the guest cannot do itself — a file, a call to a GPU — it writes as records into a stream, and the host runs every record after the turn returns and before it rings again. Every file call goes to whatever the program bound below the root it names, in the host's namespace (`ipc/ns`): xpute carries the call, and what a file is stays the binding's.

[ARCHITECTURE.md](./ARCHITECTURE.md) is what each piece is and why it is shaped that way: the memory, the frames and the packets they carry, the turn, the handles, I/O and the GPU's calls, the contract either end must keep, and the records that hold the languages to it.

## `kit`, `guest` and `host`

`kit` is that and nothing more — collections, codecs, scalar math, an errno table, the wire formats. Link it and use it: it knows nothing of turns, hosts or memories.

Golden records under `spec/golden/` hold the languages to each other wherever bytes cross.

`guest` and `host` are the arrangement above, named by the end each is. They are the two ends of one thing, not two implementations of it: a module on one side with nothing facing it is not a gap to fill, and what the two owe each other is not matching module lists but matching formats. The guest end exists in more than one language, and those are one end, so they match as `kit` does. The host end exists in TypeScript and in Rust too, and those are one role but not one module list: each holds what its platform needs. TypeScript's reserves a module's memory, rings the doorbell, answers the system calls and paces the quantum, mounted on the guest in one call (`mount`); Rust's hosts a guest linked into its own native program, reserving the memory there, running the guest on a thread of its own, and keeping the guest's blocks in its memory.

```text
rust/kit  ·  ts/kit  ·  cpp/xpute/kit          the same kit, in each language
rust/guest  ·  cpp/xpute/guest  ·  ts/host     the guest end, in each language · the host end in TypeScript
rust/host                                      the host end in Rust: a guest's memory read by offset, and the host of a guest linked into its own native program
rust/gpu  ·  ts/gpu  ·  cpp/xpute/gpu          WebGPU's calls, which a guest writes and a host runs; ts/gpu/script runs the web host's test in Chrome
rust/conformance  ·  cpp/conformance           a guest in each, which the contract's traces drive; ts/host runs them (conformance.test.ts)
script                                         the repository's own scripts, reading none of the trees' code; never shipped
spec                                           numbers all of them are generated from
```

## Embedding a guest

A guest built as a **WebAssembly module** takes its whole form from `xpute_guest::wasm`: one call in its `build.rs`, which takes `xpute-guest` as a build dependency too, and one `wasm_abi!` in the guest, which defines its allocator over the heap its plan names and exports its entry, `intr`. It reads what the module itself says through the same module, and the host hands it its memory and rings `intr` (`rust/conformance` is one, whole):

```rust
// build.rs
fn main() {
    // memory imported, the stack first, its size the guest's
    xpute_guest::wasm::link(&xpute_guest::wasm::LinkPlan { stack_bytes: 1 << 20, global_base: None, initial_bytes: None });
}

// lib.rs: the heap from the page past the linker's data to the memory's end
xpute_guest::wasm_abi!(
    xpute_guest::wasm::Plan { heap_at: |data_end| data_end.next_multiple_of(PAGE_BYTES), stack_bytes: 1 << 20 },
    my_guest::intr // fn(quota_ms: f64, boot: fn() -> BootInfo) -> f64
);
```

A guest **linked into a native program** is booted there by `rust/host`'s `native`: the program declares the allocator and writes one `native_abi!` with its numbers and the guest's entry, which boots the guest on the first call, installs the plan's clock on the guest's thread as the first `intr` runs, and exports `xpute_intr` and `xpute_mem` under xpute's names: a process holds one guest, so no program's prefix is needed (`rust/host/examples/native.rs`, which `make check` compiles and `make test` runs as a doc test):

```rust
#[global_allocator]
static ALLOC: native::Alloc = native::Alloc;

xpute_host::native_abi!(
    native::Plan {
        memory_bytes: 16 << 20,
        data_end: 0,              // no front of the program's own
        heap_at: 16 * PAGE_BYTES, // a whole number of the guest's pages
        stack_bytes: 1 << 20,
        thread_name: "demo",
        clock: native::now,
    },
    my_guest::intr
);
```

A **C++ program** takes the library as SQLite ships its own, in two files: `xpute.hpp`, every header, and `xpute.cpp`, every source over it. They are at `cpp/amalgamation/` in xpute's own repository, and `make -C cpp amalgamation` makes them from the tree under `cpp/target/amalgamation/`. The program includes the one and compiles the other once, as C++20 with `-ffp-contract=off`, the flags `cpp/Makefile` builds with; the conformance guest is built that way, so the traces hold the two files:

```cpp
#include "xpute.hpp"
```

Or it takes the tree itself: `-I cpp`, each header by its path from there, which begins `xpute/` as every name the library declares begins `xpute::`, and every source under `cpp/xpute/` but the tests' (`*.test.cpp`, `kit/test.cpp`, `kit/golden.cpp`):

```cpp
#include "xpute/guest/ipc/door.hpp"
```

## What it leaves to whoever uses it

The line is drawn in the same place every time: xpute carries the mechanism, and the policy belongs to the program.

- **It does not know what the guest computes.** No domain types reach `kit`, which is what keeps it portable to a second program.
- **It does not choose the memory map.** It is told a base, a unit and a list of sizes, and answers where each range landed. What the ranges hold is the guest's.
- **It does not choose an allocator.** The heap is one door, and what answers behind it is the guest's choice.
- **It is not a transport.** Nothing here leaves the machine; what carries bytes to another one is the host's business.
- **It is not real time.** A quota is how long a turn should take, not a deadline. Nothing preempts: a turn that runs long is charged for it in the next one rather than cut off.
- **It is not an RPC framework.** A frame is four words, and the numbers in them are the program's own.

## Running it

Deno 2.x, a Rust toolchain with the `wasm32-unknown-unknown` target, a C++20 compiler as `c++` or `$CXX`, GNU Make, and the [WASI SDK](https://github.com/WebAssembly/wasi-sdk) at `/opt/wasi-sdk` or `$WASI_SDK_PATH`, for the C++ guest built as a WebAssembly module.

```sh
make -j8 check     # deno fmt --check, deno lint, deno check, cargo fmt --check, clippy, the C++ compiled
make -j8 test      # deno test, cargo test and the C++ tests, the golden records included,
                   # and the traces against every guest, native and WebAssembly, once build has made them
make -j8 build     # the guest the traces drive, in Rust and C++, native and WebAssembly; test builds it first
make -j8 test-gpu  # the GPU hosts against golden/gpu: the native one on wgpu, the web one in Chrome (installed, or $CHROME)
make fmt           # deno fmt, cargo fmt
make lint          # deno lint
```

Each runs every tree's own target, in parallel with `-j`; `check-ts`, `check-cpp`, `check-rust` and the like run one tree's, as `deno task --cwd ts`, `make -C cpp` and `cargo` in `rust/` do. The copy to xpute's own repository is `./script/export.py --to <its checkout>` (Python 3.11 or newer).

## License

xpute is MIT. The buddy allocator under `alloc/`, in Rust and in C++, is a port of someone else's code rather than a dependency, and `THIRD_PARTY_NOTICES.md` carries the notice its license asks for.
