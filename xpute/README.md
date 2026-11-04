# xpute

xpute runs one program inside another across three edges and no others: one
memory they share, one call in, and one number out. The outer program, the
**host**, owns the platform: the clock, the memory, whatever reaches outside.
The inner one, the **guest**, owns none of it on its own account. Everything it
has, the host granted it: the memory once at boot, and a quota of time on each
call in, which is a **turn**. What the guest builds over those grants — its
allocator, its scheduler, its tables — is its own, as a library operating
system's is over an exokernel, and xpute does not enforce the grants.

**xpute is both ends of that arrangement, and `kit`, the generic code underneath
them.** `kit` is written in every language xpute has, and each end in the
languages it is built in.

It was written for a program driven by TypeScript: compiled to one WebAssembly
module in a browser tab, or linked into a native app. Nothing in xpute knows
what that program computes.

## The arrangement

```text
┌─ host ── TypeScript ───┐                        ┌─ guest ──────────────────┐
│ owns the clock, the    │                        │ computes, and owns       │
│ memory, the I/O        │ ═══ run(quota) ═════▶  │ nothing                  │
│                        │                        │                          │
│                        │  ◀═══ wake_ms ═══════  │                          │
│                        │  ◀──── now() ────────  │                          │
│                        │  ◀── hostcall(at) ───  │                          │
└───────────┬────────────┘                        └────────────┬─────────────┘
            │                                                  │
            └──────────────────────────┬───────────────────────┘
                                       ▼
           ┌─ the core memory ───────────────────────────────────┐
           │ everything else crosses here, and nothing else does │
           └─────────────────────────────────────────────────────┘
```

Three edges, and two calls the other way, each answered before it returns: the
guest reads the host's clock (`now`), because it spends its quota against that
clock in the middle of a turn, when the host cannot run; and it calls a port
(`hostcall`), a frame it laid on its ring answered now rather than a turn
later: a question what answers holds already, where an answer a turn later
would cost the guest that turn. There is no second entry point, no other
callback, and nothing either side holds a pointer to on the other.

`run` is the whole of the guest's life. xpute names it, as a C runtime names
`main`, and the guest implements it. It runs when the host rings, for as long as
the quota it was handed, and when the call returns the guest is not running at
all. Its answer, `wake_ms`, is when it would like the next turn. A guest that
asks for nothing is rung for nothing.

What the guest cannot do itself — a file, a call to a GPU — it asks for in what
its turn leaves, and the host takes every such request before it rings again.
The host is a port like the rest, 0: what the guest asks of it are frames on the
same ring as everything else the guest sends.

A file is asked for as POSIX's `openat` and io_uring's submission and completion
name it: opened below a root the host bound, read and written by descriptor,
each answered with a result or Linux's errno, its open's answer (`SYS_OPENED`) a
turn later. WASI's calls look alike — an open, a read, an errno, a directory the
host grants — but each returns before the guest goes on. Here nothing the guest
asks returns within its turn but a call to a port answered from what it holds: a
guest waits a turn for what it asked, and never blocks.

[ARCHITECTURE.md](./ARCHITECTURE.md) is what each piece is and why it is shaped
that way, and the contract either end must keep.

## `kit`, `guest` and `host`

`kit` is that and nothing more — collections, codecs, scalar math, an errno
table, the wire formats. Link it and use it: it knows nothing of turns, hosts or
memories. Golden records under `spec/golden/` hold the languages to each other
wherever bytes cross.

`guest` and `host` are the arrangement above, named by the end each is: the two
ends of one thing, which owe each other matching formats, and whose module lists
differ. The guest ends in each language are one end, and match as `kit` does.

```text
rust/kit  ·  ts/kit  ·  cpp/xpute/kit       the same kit, in each language
rust/guest  ·  cpp/xpute/guest              the guest end, in each language
ts/host                                     the host end in TypeScript, mounted on any guest its platform opened
rust/host                                   the host end in Rust: a guest's memory by offset, and a guest linked in
rust/gpu  ·  ts/gpu  ·  cpp/xpute/gpu       WebGPU's calls, which a guest writes and a host runs
rust/conformance  ·  cpp/conformance        a guest in each, which the contract's traces drive
rust/example  ·  ts/example                 the smallest guest, and the host that drives it
rust/tool                                   the repository's own Rust programs, run by cargo
script                                      the repository's own scripts (Python)
spec                                        the numbers every language's are generated from
```

A program takes these as packages, by name. In TypeScript they are
`@xpute/kit`, `@xpute/host` and `@xpute/gpu` (`ts/kit`, `ts/host`, `ts/gpu`),
each its own source: its `package.json` maps a path under its name to the file
under `src/` (`@xpute/host/host.ts` is `ts/host/src/host.ts`), and `@xpute/host`
alone is its entry, what a program takes (`ts/host/src/mod.ts`); the files
strip to JavaScript as they are, so a bundler or Node's type stripping takes
them with no build. In Rust they are the crates `xpute-kit`, `xpute-guest`,
`xpute-host` and `xpute-gpu`. None is published yet: a program beside this tree
lists the packages in its pnpm workspace and depends on them as `workspace:*`,
and names the crates by `path`.

## Embedding a guest

The smallest program shows the whole of it: a guest that answers one command
(`rust/example`) and the host that drives it (`ts/example/sum.test.ts`), which
`make test` runs. The guest lays out its own range, a page for the toc, then the
two rings and their slots, and answers the toc's offset from a boot call; each
turn it applies what waits, and calls the port that sent the command for a
number it holds, answered before the call returns (`Door::call`). The host
opens the module over a memory of its own (`@xpute/host` `open_module`) and
mounts on it in one call (`mount`),
given that offset, its answers to the guest's hostcalls (`SysHost`, each part
optional), what each of its ports hears (`ports`) and the scheduling policy that
says when a turn runs and how much it gets (`sched/policy.ts`: a fixed quota
where there is no display, frame pacing on a page); then it sends the command
and reads the reply. `mount` takes all the program decides and nothing of the
order it is put together in: a guest that writes its toc in its first turn
says so (`toc_in_turn`) and the mount runs that turn, and what is told when the
guest stops is given with the rest (`stopped`). A program takes it all from
`@xpute/host`'s entry (`src/mod.ts`).

A guest stops on a broken invariant (`bug!`) or a panic: what it installs
(`xpute_guest::stop::install`, `cpp/xpute/guest/stop.hpp`) posts the report
(`SYS_ABORT`) as the turn's one frame, which the host hears. An event a guest's
table generates is sent to the port it names (`ipc/door` `Door::send`).
A program's command table is generated for both ends by xpute-tool
(`spec::cmd::table_outputs`), handed the table and where its code goes.

A guest built as a **WebAssembly module** takes its form from
`xpute_guest::wasm`: a call in its `build.rs`, and one `wasm_abi!`, which
installs its allocator and exports `run`, the entry it calls:

```rust
// build.rs, with xpute-guest as a build dependency
fn main() {
    xpute_guest::wasm::link(&xpute_guest::wasm::LinkConfig { stack_bytes: 1 << 20, global_base: None, init_bytes: None });
}

// lib.rs
xpute_guest::wasm_abi!(
    xpute_guest::wasm::Config { heap_at: |data_end| data_end.next_multiple_of(PAGE_BYTES), stack_bytes: 1 << 20 },
    my_guest::entry // fn(quota_ms: f64, boot: fn() -> BootInfo) -> f64
);
```

A guest **linked into a native program**, on a unix platform, is booted by one
`xpute_host::native_abi!`, beside `native::Alloc` as the program's allocator,
and exports `xpute_run`, `xpute_mem` and `xpute_hostcall_on`
(`rust/host/examples/native.rs` is one, and `make test` runs it).

A **C++ program** takes the library in two files, as SQLite ships its own: it
includes `xpute.hpp` and compiles `xpute.cpp` once, as C++20 with
`-ffp-contract=off`, the flags `cpp/Makefile` builds with. They are at
`cpp/amalgamation/` in xpute's own repository; in this tree
`make -C cpp amalgamation` writes them to `cpp/target/amalgamation/`.

## What it leaves to whoever uses it

The line is drawn in the same place every time: xpute carries the mechanism, and
the policy belongs to the program.

- **It does not know what the guest computes.** No domain type reaches `kit` or
  either end.
- **It does not choose the memory map.** It is told a base, a unit and a list of
  sizes, and answers where each range landed. What the ranges hold is the
  guest's.
- **It does not choose an allocator.** The heap is reached through one call, and
  what answers it is the guest's choice.
- **It is not a transport.** Nothing here leaves the machine; what carries bytes
  to another one is the host's business.
- **It is not real time.** A quota is how long a turn should take. Nothing
  preempts: a turn that runs long is charged for it in the next one.
- **It is not an RPC framework.** A message is four words and a packet, and the
  numbers in them are the program's own.

## Running it

Node at the version `ts/.tool-versions` pins with pnpm (`make install`
first), rustup (it installs what
`rust/rust-toolchain.toml` names), a C++20
compiler as `c++` or `$CXX`, GNU Make, and the [WASI
SDK](https://github.com/WebAssembly/wasi-sdk) at `/opt/wasi-sdk` or
`$WASI_SDK_PATH`, for the C++ guest built as a module.

```sh
make install       # ts/'s packages, pnpm's
make -j8 check     # formats, lints and types checked, the C++ compiled, spec's files current
make -j8 test      # every tree's tests, then the traces against every guest
make -j8 build     # the guests the traces drive; test builds them first
make -j8 test-gpu  # the GPU hosts, on wgpu and in Chrome ($CHROME)
make fmt           # prettier, cargo fmt
make spec          # the files generated from spec, written
```

`check-ts`, `test-cpp`, `test-rust` and the like run one tree's. The copy to
xpute's own repository is `./script/export.py --to <its checkout>`.

## License

xpute is MIT. The buddy allocator under `alloc/`, in Rust and in C++, is ported
from someone else's code, and `THIRD_PARTY_NOTICES.md` carries the notice its
license asks for.
