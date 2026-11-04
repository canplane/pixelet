# xpute

xpute makes a program's core its first-class citizen. The core is compiled
code, the same code as a WebAssembly module in a page and as a library linked
into a native app; it leads, and JavaScript does the I/O and the UI around it.
No foreign function call is made for each piece of the boundary: the two share
one memory, and a pair of rings in it carries everything else, in io_uring's
shape. xpute is that arrangement, both ends of it, and `kit`, the generic code
underneath. It is not a general runtime, not an actor bus and not an operating
system.

**Host** and **guest** mean what they mean in WebAssembly. The core is the
guest, run inside; the host is what runs it. Nothing in xpute knows what the
core computes.

## The core and its world

A core's author sees two things: the core and its **world**. The core sends to
its world and receives from it, and never names a destination. A world is its
commands, as a WIT world is: its **imports**, which the core calls, and its
**exports**, which the core handles.

```text
┌─ the core ─────────────┐                 ┌─ its world ────────────┐
│ decides; owns no       │ ─── imports ──▶ │ answers what the core  │
│ device                 │                 │ calls                  │
│                        │ ◀── exports ─── │ sends what the core    │
│                        │                 │ handles                │
└────────────────────────┘                 └────────────────────────┘
```

## The ui, the core and the host

A platform's author sees the world as two parts, and the stack reads ui, core,
host: an application over a kernel over its hardware.

- **The ui and the core** stand as io_uring's application and kernel. The ui
  submits its commands to the core, and the core completes them; what the core
  sends the ui unasked, more rarely, is an **upcall**. The app's ABI is this
  relation.
- **The core and the host** stand as a kernel and its HAL. The host is the
  core's hardware: the memory, the clock and the turns, the GPU, the storage
  (the file device). The core calls its HAL, and the HAL raises interrupts.
  xpute's predefined ABI is this relation.

```text
  ui     the application    screens, components
   │ ▲
   ▼ │   the app's ABI: commands submitted and completed; upcalls
  core   the kernel         decides; owns no device
   │ ▲
   ▼ │   xpute's ABI: calls down; interrupts up
  host   its HAL            memory, clock and turns, GPU, files
```

Physically the ui and the host share one JavaScript runtime, and natively the
host has Rust pieces beside it: the guest's thread, its memory and the GPU's
run. Both relations ride the same pair of rings, told apart by a command's
major: the host carries the ui's submissions on the SQ and hands the core's
completions and upcalls back. The ui meets the host directly only for the
surfaces (a view lent to the GPU) and the lifecycle (boot, visibility),
through the host's TypeScript API.

## Two ABIs, one number space

A command is `major << 8 | minor` (`kit` `abi/cmd`), and who defines it decides
its major. xpute's **predefined ABI** takes majors 0x00 to 0x0F (`spec/abi/`).
Its imports are the core's calls to its HAL: the file device's calls
(`FS_OPENAT`, `FS_READ`, `FS_WRITE`, `FS_CLOSE`, `spec/abi/fs.json`) and
`SYS_ABORT`, the report the core posts as it stops. Its exports are the HAL's
interrupts:

- `SYS_ATTACH`, a host attached, the first time or again;
- `SYS_SUSPEND` and `SYS_RESUME`, calls around a hidden host;
- `GPU_READY`, the GPU the machine asked for answered;
- `FS_OPENED`, an open answered.

`NOP` is the rings' own test. xpute ships both ends of the file device.

The **app's ABI** takes majors from `PROGRAM_MAJOR`, 0x10
(`spec/ipc/frame.json`), in protocol files of the app's own, each command an
export or an import. The generator refuses an app's command under
`PROGRAM_MAJOR`, and a number two protocols both give. There are no ports: the
host hands a frame to what answers its command's major.

## The rings

The rings are io_uring's, by its roles. The **SQ** is written by the host, the
side where the outside's interrupts arrive: the ui's submissions and the HAL's
interrupts. The **CQ** is written by the core: its completions and upcalls,
and its own calls to the HAL, in ublk's manner, whose results come back on the
SQ. Five rules make the contract:

1. Each ring has one writer.
2. A position is published with release and read with acquire.
3. A buffer a frame names is the receiver's until it moves its head past that
   frame.
4. A completion finds its request by id.
5. A call never nests, and its answer makes no call.

When each side runs is the platform's. On the web the core runs on the page's
event loop, in turns the **strobe** gives, the web host's policy, paced to the
display's frames; natively it runs on its own thread.

## The machine

A guest crate declares its **machine** (`machine.json`, its schema
`spec/machine.schema.json`): the memory sizes a host may give, the stack, the
rings, and the GPU it requests. The GPU is a resource, as the memory is: the
machine requests it, and the host grants it. From the machine the host lays
the **front** before the first turn: a header of words (`spec/ipc/head.json`)
naming the rings and the GPU stream, and each ring's own header. The guest
computes the same front from the same machine and checks the header against
it, stopping with `ENOEXEC` where a word differs. The GPU stream says its own
head.

## Embedding a guest

The smallest program shows the whole of it: a guest that answers one command
(`rust/example`) and the host that drives it (`ts/example/sum.test.ts`), which
`make test` runs. Its machine is `rust/example/machine.json`, and its one
protocol `rust/example/asker.json`: the host sends `SUM`, and the guest calls
`ASK` as it adds, answered before the call returns.

The **generator** writes both ends' code from the machine and the protocol
files in the guest crate: `MACHINE` for each end, and for each protocol a
stub per export and a handler per import on the host, and a function per
import and a trait for the exports on the guest. A program runs it as
`xpute-gen <machine.json> <protocol.json>...` (`--check` to fail where a file
is stale); xpute's own files are written by `cargo spec`. The generated code
is committed.

The **core crate is the artifact**. It builds as `rlib`, `cdylib` and
`staticlib`, links by its machine in `build.rs`, and writes one
`xpute_guest::guest!`, which picks the WebAssembly module or the native
library by its target:

```toml
[lib]
crate-type = ["rlib", "cdylib", "staticlib"]

[dependencies]
xpute-guest = "…"
xpute-gpu = "…"

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
xpute-host = "…"
xpute-gpu = { version = "…", features = ["host"] }
```

```rust
// build.rs
fn main() {
    xpute_guest::wasm::link(&machine::MACHINE.link());
}

// lib.rs; `gpu:` only where the machine requests a GPU
const GPU: xpute_gpu::Numbers = xpute_gpu::Numbers { frame_latency: 2, upload_chunk_bytes: 1 << 20 };
xpute_guest::guest!(MACHINE, FIXED_BYTES, my_core::run, gpu: GPU);
```

The entry, `run`, is a turn: called with a quota of time, it applies what
waits on the SQ, posts what it has on the CQ, and returns when it wants the
next turn. On wasm32 the form is a module that exports `run`; anywhere else it
is the native library, with its memory laid out from the machine, its thread,
its allocator and xpute's stop hook, exported as `xpute_run`, `xpute_mem` and
`xpute_hostcall_on` (`rust/host/include/xpute.h`), its GPU host's calls as
`xpute_gpu_*` (`rust/gpu/include/xpute_gpu.h`), and on Android the same calls
under the Kotlin classes `xpute.Guest` and `xpute.Gpu`.

On the JavaScript side an app calls `@xpute/host`'s `boot(machine, { guest?,
... })` once. The platform is picked by file: `x.ts` is the page's,
`x.native.ts` the device's, as a native bundler and `tsconfig.native.json`
(`moduleSuffixes`) resolve them. A page passes its module as `guest`; a device
passes none, since the core is linked in. A native app installs
**`@xpute/native`**, the React Native module (Kotlin, Swift, JNI, the headers
and the core's build for iOS and Android), which Expo autolinks; the app's
`package.json` names its core crate (`xpute.core`). The rest of the config is
what the program decides: what backs the files (`fs`), what hears its own
commands (`program`), the GPU it grants (`gpu`, `@xpute/gpu`'s `WebGpu` or
`NativeGpu`), its scheduling policy and what it submits before each turn.

A **C++ guest** takes the library in two files, as SQLite ships its own: it
includes `xpute.hpp` and compiles `xpute.cpp` once, as C++20 with
`-ffp-contract=off`, the flags `cpp/Makefile` builds with. They are at
`cpp/amalgamation/` in xpute's own repository; in this tree
`make -C cpp amalgamation` writes them to `cpp/target/amalgamation/`.

## What it leaves to whoever uses it

xpute carries the mechanism, and the policy belongs to the program.

- **It does not know what the core computes.** No domain type reaches `kit` or
  either end.
- **It does not choose the memory past the front.** The machine says the sizes;
  what the core's ranges hold, and where, is the core's.
- **It does not choose an allocator.** The heap is reached through one call, and
  what answers it is the guest's choice.
- **It is not a transport.** Nothing here leaves the machine; what carries bytes
  to another one is the host's business.
- **It is not real time.** A quota is how long a turn should take. Nothing
  preempts: a turn that runs long is charged for it in the next one.
- **It does not give the app's commands a meaning.** A message is four words
  and a packet, and the app's numbers are the app's own.

## `kit`, `guest` and `host`

`kit` is collections, codecs, scalar math, an errno table and the wire formats.
Link it and use it: it knows nothing of turns, hosts or memories. Golden
records under `spec/golden/` hold the languages to each other wherever bytes
cross. `guest` and `host` are the two ends of the arrangement, which owe each
other matching formats, and whose module lists differ.

```text
rust/kit  ·  ts/kit  ·  cpp/xpute/kit       the same kit, in each language
rust/guest  ·  cpp/xpute/guest              the guest end, in each language
ts/host                                     the host end in TypeScript
rust/host                                   the host's Rust: a guest linked in
ts/native                                   the React Native module
rust/gpu  ·  ts/gpu  ·  cpp/xpute/gpu       WebGPU's calls, written and run
rust/conformance  ·  cpp/conformance        a guest each, for the traces
rust/example  ·  ts/example                 the smallest guest and its host
rust/tool                                   the generator, and the tools
script                                      the repository's scripts
spec                                        the numbers, generated from
```

In TypeScript they are `@xpute/kit`, `@xpute/host`, `@xpute/gpu` and
`@xpute/native` (`ts/kit`, `ts/host`, `ts/gpu`, `ts/native`), each its own
source: its `package.json` maps a path under its name to the file under
`src/` (`@xpute/host/host.ts` is `ts/host/src/host.ts`), and `@xpute/host`'s
entry is `ts/host/src/mod.ts`; the files strip to JavaScript as they are, so a
bundler or Node's type stripping takes them with no build. In Rust they are
the crates `xpute-kit`, `xpute-guest`, `xpute-host` and `xpute-gpu`. None is
published yet: a program beside this tree lists the packages in its pnpm
workspace and depends on them as `workspace:*`, and names the crates by
`path`.

[ARCHITECTURE.md](./ARCHITECTURE.md) is what each piece is and why it is shaped
that way, and the contract either end must keep.

## Running it

Node at the version `ts/.tool-versions` pins with pnpm (`make install` first),
rustup (it installs what `rust/rust-toolchain.toml` names), a C++20 compiler
as `c++` or `$CXX`, GNU Make, and the
[WASI SDK](https://github.com/WebAssembly/wasi-sdk) at `/opt/wasi-sdk` or
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
