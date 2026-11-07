# xpute

xpute makes a program's core its first-class citizen. The core is compiled
code, the same code as a WebAssembly module in a page and as a library linked
into a native app; it leads, and JavaScript does the I/O and the UI around it.
No foreign function call is made for each piece of the boundary: the two share
one memory, and a pair of rings in it carries everything else, in io_uring's
shape. xpute is that arrangement, both ends of it, and `kit`, the generic code
underneath. It is not an actor bus.

The **core** is a **unikernel** on a virtual machine: the program and xpute's core library,
its library OS, built as one image, as MirageOS and Unikraft build theirs, and
run inside. The runtime is its **VMM**: it implements the core's devices and
gives it its time. Nothing in xpute knows what the core computes.

## The core and its world

A core's author sees two things: the core and its **world**. The core sends
to its world and receives from it, and never names a destination. A world is
its commands, as a WIT world is: its **imports**, which the core calls, and
its **exports**, which the core handles.

```text
┌─ the core ─────────────┐                 ┌─ its world ────────────┐
│ decides; owns no       │ ─── imports ──▶ │ answers what the core  │
│ device                 │                 │ calls                  │
│                        │ ◀── exports ─── │ sends what the core    │
│                        │                 │ handles                │
└────────────────────────┘                 └────────────────────────┘
```

## The UI, the core and the runtime

A platform's author sees the world as two parts: the UI outside the VM,
and the VMM under it.

- **The UI and the core.** The program's **UI**, its screens and
  components, stands outside the VM, as an application outside a VM reaches a
  service inside it over virtio-vsock: a unikernel has no userspace. The ring
  pair is the device between them, by io_uring's roles: the UI submits
  its commands to the core, and the core completes them; what the core
  tells the UI unasked, more rarely, is one of its **imports**. The app's ABI
  is this relation.
- **The core and the runtime.** The runtime implements the devices the core's
  machine asks for (the GPU, the file device), by virtio's convention, and
  gives the core its memory, its clock and its turns, as a VMM gives a
  vCPU's time. The core calls its devices, and the runtime raises interrupts.
  xpute's predefined ABI is this relation.

```text
  UI outside the VM     screens, components
   │ ▲
   ▼ │   the app's ABI: commands submitted and completed; imports
  core     the unikernel    the program and its library OS; owns no device
   │ ▲
   ▼ │   xpute's ABI: calls to its devices; interrupts up
  runtime  its VMM          memory, clock and turns, GPU, files
```

Physically the runtime sits where the core's memory is reached. Natively it
shares the UI's JavaScript thread, with Rust pieces beside it: the core's
thread, its memory and the GPU's run. On a page the core and the runtime run
in a worker and the UI on the main thread, joined by a MessagePort. Both
relations ride the same pair of rings, told apart by a command's major: the
runtime carries the UI's submissions on the SQ and hands the core's
completions and imports back. The UI meets the runtime directly only for the
surfaces (a view lent to the GPU) and the lifecycle (boot, visibility),
through its link (`Link`), the same on every platform.

## Two ABIs, one number space

A command is `major << 8 | minor` (`kit` `abi/cmd`), and who defines it decides
its major. xpute's **predefined ABI** takes majors 0x00 to 0x0F (`spec/abi/`,
a protocol a device: `sys.json`, `gpu.json`, `fs.json`).
Its imports are the core's calls to its devices: the file device's calls
(`FS_OPENAT`, `FS_READ`, `FS_WRITE`, `FS_CLOSE`, `spec/abi/fs.json`) and
`SYS_ABORT`, the report the core posts as it stops. Its exports are the
runtime's interrupts:

- `SYS_ATTACH`, a runtime attached, the first time or again;
- `SYS_SUSPEND` and `SYS_RESUME`, calls around a hidden runtime;
- `SYS_TURN`, a turn the scheduling policy grants, as against one rung for a
  command alone, and the wall clock; what else the policy says of the turn,
  such as a display's pacing, is the program's to send;
- `GPU_READY`, the GPU the machine asked for answered;
- `GPU_SURFACE_OPEN`, `GPU_SURFACE_RESIZE` and `GPU_SURFACE_CLOSE`, a surface
  the UI lends the GPU, by the handle the first answers, the one `bind` lends
  a canvas under;
- `FS_CHANGED`, a file changed under the core, which it reads again.

`NOP` is the rings' own test. xpute ships both ends of the file device.

The **app's ABI** takes majors from `PROGRAM_MAJOR`, 0x10
(`spec/ipc/frame.json`), in protocol files of the app's own, each command an
export or an import. The generator refuses an app's command under
`PROGRAM_MAJOR`, and a number two protocols both give. There are no ports: the
runtime hands a frame to what answers its command's major.

## The rings

The rings are io_uring's, by its roles. The **SQ** is written by the runtime, the
side where the outside's interrupts arrive: the UI's submissions and the
runtime's own interrupts. The **CQ** is written by the core: its completions and
imports, what it tells the UI and asks its devices, in ublk's manner, whose results
come back on the SQ. Five rules make the contract:

1. Each ring has one writer.
2. A position is published with release and read with acquire.
3. A buffer a frame names is the receiver's until it moves its head past that
   frame.
4. A completion finds its request by id.
5. A call never nests, and its answer makes no call.

When each side runs is the platform's. On the web the core runs on the page's
event loop, in turns the **scheduler** gives at the moments its policy picks,
paced to the display's frames; natively it runs on its own thread.

## The machine

A core crate declares its **machine** (`machine.json`, its schema
`spec/machine.schema.json`): the memory sizes a runtime may give, the stack, the
rings, and the devices it asks for, `gpu` and `fs`. Which devices the core
needs is the core's to decide, as a VM's configuration names its virtio
devices: the runtime attaches what the machine asks for, a boot that backs a
device the machine does not ask for, or none it does, is refused, and an
unattached device's command is answered `ENODEV`. From the machine the runtime
lays the **front** before the first turn: a header of words
(`spec/ipc/head.json`) naming the rings and the GPU stream, and each ring's own
header. The core computes the same front from the same machine and checks the
header against it, stopping with `ENOEXEC` where a word differs. The GPU
stream says its own head.

## Embedding a core

The smallest program shows the whole of it: a core that answers one command
(`rust/example`) and the runtime that drives it (`ts/example/sum.test.ts`), which
`make test` runs. Its machine is `rust/example/machine.json`, and its one
protocol `rust/example/asker.json`: the runtime sends `SUM`, and the core calls
`ASK` as it adds, answered before the call returns.

The **generator** writes both ends' code from the machine and the protocol
files in the core crate: `MACHINE` for each end, and for each protocol a
stub per export and a handler per import on the runtime, and a function per
import and a trait for the exports on the core. A program runs it as
`xpute-gen <machine.json> <protocol.json>...` (`--check` to fail where a file
is stale); xpute's own files are written by `cargo spec`. The generated code
is committed.

The **core crate is the artifact**, the unikernel's one image. It builds as
`rlib`, `cdylib` and `staticlib`, links by its machine in `build.rs`, and
writes one `xpute_core::core!`, which picks the WebAssembly module or the
native library by its target:

```toml
[lib]
crate-type = ["rlib", "cdylib", "staticlib"]

[dependencies]
xpute-core = "…"
xpute-gpu = "…"

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
xpute-runtime = "…"
xpute-gpu = { version = "…", features = ["runtime"] }
```

```rust
// build.rs
fn main() {
    xpute_core::wasm::link(&machine::MACHINE.link());
}

// lib.rs; `gpu:` only where the machine asks for a GPU
const GPU: xpute_gpu::Numbers = xpute_gpu::Numbers { frame_latency: 2, upload_chunk_bytes: 1 << 20 };
xpute_core::core!(MACHINE, FIXED_BYTES, my_core::run, gpu: GPU);
```

The entry, `run`, is a turn: called with a deadline of time, it applies what
waits on the SQ, posts what it has on the CQ, and returns when it wants the
next turn. On wasm32 the form is a module that exports `run`; anywhere else it
is the native library, with its memory laid out from the machine, its own
thread, which runs the core while it has work and sleeps until a kick, the
scheduler kicking at the moment its policy gives for the time the core asked
for, its allocator and xpute's stop hook, exported as
`xpute_run`, `xpute_kick`, `xpute_mem`, `xpute_call_on`,
`xpute_notify_on`, `xpute_answer` and `xpute_turns`
(`rust/runtime/include/xpute.h`), its GPU runtime's calls as
`xpute_gpu_*` (`rust/gpu/include/xpute_gpu.h`), and on Android the same calls
under the Kotlin classes `xpute.Core` and `xpute.Gpu`.

On the JavaScript side an app calls `@xpute/runtime`'s `boot(manifest, ui)` once,
or `start` (below). The platform is picked by file: `x.ts` is the page's,
`x.native.ts` the device's, as a native bundler and `tsconfig.native.json`
(`moduleSuffixes`) resolve them. A page passes its module as `core`; a device
passes none, since the core is linked in. A native app installs
**`@xpute/native`**, the React Native module (Kotlin, Swift, JNI, the headers
and the core's build for iOS and Android), which Expo autolinks; the app's
`package.json` names its core crate (`xpute.core`). The rest of the config is
what the program decides: what backs the files (`fs`) and the GPU it grants
(`gpu`, `@xpute/gpu`'s `WebGpu` or `NativeGpu`), each where the machine asks
for it, what hears its own commands (`program`), its scheduling policy and
what it submits before each turn.

An app with a UI declares its core once in a **manifest**, as xpute's April
build did: the machine, and `core`, which makes `boot`'s config where the core
runs, with what else of the core's memory an import names (`mem_of`); code
beside the core reads its memory as `mem`. The UI is four things: the imports it takes
(`cmds`, `SYS_ABORT` among them where it shows the stop) and what it does with
each (`on`), the GPU's state (`gpu`), the stop (`stop`). `start(manifest, ui,
worker)` puts the core where the platform runs it and resolves to the UI's
**link** (`Link`), the same calls on every platform. On a device it boots the
core beside the UI. On a page it starts `worker`, whose module calls
`serve(manifest)`; between them a port carries frames, up and down, a reply
being a frame and an import the UI takes another.

```ts
// core.ts, read where the core runs and by the UI
export const manifest = { machine: MACHINE, core: () => ({ core, gpu, program, policy }) };

// core.worker.ts, a page's
serve(manifest);

// the UI
const ui = { cmds: [Command.SHOW], on: shown, gpu: showGpu, stop: stopped };
const link = await start(manifest, ui, () =>
  new Worker(new URL("./core.worker.ts", import.meta.url), { type: "module" }));
```

A **C++ core** takes the library in two files, as SQLite ships its own: it
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
  what answers it is the core's choice.
- **It is not a transport.** Nothing here leaves the machine; what carries bytes
  to another one is the runtime's business.
- **It is not real time.** A deadline is how long a turn should take. Nothing
  preempts: a turn that runs long is charged for it in the next one.
- **It does not give the app's commands a meaning.** A message is four words
  and a packet, and the app's numbers are the app's own.

## `kit`, `core` and `runtime`

`kit` is collections, codecs, scalar math, an errno table and the wire formats.
Link it and use it: it knows nothing of turns, runtimes or memories. Golden
records under `spec/golden/` hold the languages to each other wherever bytes
cross. `core` and `runtime` are the two ends of the arrangement, which owe each
other matching formats, and whose module lists differ.

```text
rust/kit  ·  ts/kit  ·  cpp/xpute/kit       the same kit, in each language
rust/core   ·  cpp/xpute/core               the core end, in each language
ts/runtime                                  the runtime end in TypeScript
rust/runtime                                the runtime's Rust: a core linked in
ts/native                                   the React Native module
rust/gpu  ·  ts/gpu  ·  cpp/xpute/gpu       WebGPU's calls, written and run
rust/conformance  ·  cpp/conformance        a core each, for the traces
rust/example  ·  ts/example                 the smallest core and its runtime
rust/tool                                   the generator, and the tools
script                                      the repository's scripts
spec                                        the numbers, generated from
```

In TypeScript they are `@xpute/kit`, `@xpute/runtime`, `@xpute/gpu` and
`@xpute/native` (`ts/kit`, `ts/runtime`, `ts/gpu`, `ts/native`), each its own
source: its `package.json` maps a path under its name to the file under
`src/` (`@xpute/runtime/runtime.ts` is `ts/runtime/src/runtime.ts`), and `@xpute/runtime`'s
entry is `ts/runtime/src/mod.ts`; the files strip to JavaScript as they are, so a
bundler or Node's type stripping takes them with no build. In Rust they are
the crates `xpute-kit`, `xpute-core`, `xpute-runtime` and `xpute-gpu`. None is
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
`$WASI_SDK_PATH`, for the C++ core built as a module.

```sh
make install       # ts/'s packages, pnpm's
make -j8 check     # formats, lints and types checked, the C++ compiled, spec's files current
make -j8 test      # every tree's tests, then the traces against every core
make -j8 build     # the cores the traces drive; test builds them first
make -j8 test-gpu  # the GPU runtimes, on wgpu and in Chrome ($CHROME)
make fmt           # prettier, cargo fmt
make spec          # the files generated from spec, written
```

`check-ts`, `test-cpp`, `test-rust` and the like run one tree's. The copy to
xpute's own repository is `./script/export.py --to <its checkout>`.

## License

xpute is MIT. The buddy allocator under `alloc/`, in Rust and in C++, is ported
from someone else's code, and `THIRD_PARTY_NOTICES.md` carries the notice its
license asks for.
