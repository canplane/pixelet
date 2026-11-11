# xpute

xpute is a foreign function interface for the compiled part of a program. The
program's **core**, its compiled code, and xpute's **runtime**, which runs it,
meet in four things: a calling convention over three host functions, which pass
frames laid in the core's memory; bindings that a generator writes for both ends
from the program's protocols, as WIT's tools do; the turn, the one call that
gives the core its time; and two ways to move large data while that memory
stays fixed, a read into a buffer the caller names, as WASI's `fd_read` reads,
and the GPU stream, a command buffer the core records and the runtime runs. The
core owns nothing but its memory, and the runtime implements only the devices
that lend it across turns, the GPU and the files. Everything else belongs to the
**program**, and it travels over one general channel whose messages xpute
delivers and does not interpret.

The same core runs as a WebAssembly module in a web page and as a library linked
into an iOS or Android app, while the program's JavaScript does the I/O and
draws the UI around it. xpute is both ends of that arrangement, the core's
library and the runtime, and `kit`, the generic code beneath them.

## Why one interface

A program whose logic is compiled and whose surface is JavaScript usually
crosses between the two languages once for every piece of work: a binding for
each function, a copy for each argument, and a different binding on each
platform. Every crossing costs time, and together the crossings become the
program's real interface, scattered through glue code.

xpute replaces those crossings with one memory that both sides share and one
kind of message, a frame, laid in that memory. Into the core goes one call that
runs a turn, and the number it returns says when the core wants its next turn.
Out of the core go four functions, the same on every platform: one that hands
it the next frame waiting for it, one that takes a frame it sends, a call
answered before it returns, and the runtime's clock. What a frame says is the
program's, so no binding is written for any one piece of work.

The runtime gives the core only its memory, its turns, a clock and the two
devices that lend the memory, and the core decides everything else. Because
that is all the runtime gives, the same core runs unchanged in a page, on a
phone and in a test.

## The core, the runtime and the program

A program built on xpute has three parts.

- The **core** is the program's compiled code, linked with xpute's core library
  into one image. It runs only inside a turn, and everything it says to the
  world it writes into its memory.
- The **runtime** is xpute's TypeScript, `@xpute/runtime`, with Rust beside it
  on a device. It reserves the core's memory and lays out its front, runs each
  turn, reads what the turn sent, and runs the file device. The GPU is a plugin,
  `@xpute/gpu`.
- The **program** is everything else the app decides: what its commands mean,
  what backs its files, when its turns should run, and its UI.

```text
  ┌──────────────────────────────────────────────────────────────┐
  │ the core: the program's code, built on xpute's core library. │
  │ It owns one memory, where every frame is laid, and it runs   │
  │ only inside run(deadline).                                   │
  └───────────┬─────────────────────────────────┬────────────────┘
              ▲                                 │
              │ receive:                        │ send:
              │ what the core is sent           ▼ what the core says
  ┌───────────┴─────────────────────────────────┴────────────────┐
  │ the runtime: lays out the memory, runs each turn, and        │
  │ routes each frame by the major of its command                │
  └───────────┬─────────────────────────────────┬────────────────┘
              │ majors 0x00 to 0x0F             │ majors 0x10 and up
    xpute's own: the turns, the             the program: its handlers
    core's life, the GPU, the files         and its UI, which xpute
                                            does not read
```

Each frame carries a command and a packet, the command's data in
XTP, xpute's tree format. The high byte of a command's number is its major, and
xpute delivers a program's frame by that major and reads nothing of what it
says. xpute's own commands, under the majors below 0x10, tell the core about its turns and
its life and drive the two devices.

## How a program uses it

The smallest program ships in this tree and shows every piece: a core that
answers one command (`rust/dev/example`), the runtime that drives it in a test
(`ts/dev/example/sum.test.ts`), and a page that runs it in a worker
(`ts/dev/example/web`).

### The manifest

A core declares what it needs in its **manifest**, a `manifest.json`, as an
Android app declares its own: the sizes of memory a runtime may give it, its
stack, the most a frame's packet takes, and which of xpute's devices it asks
for. Both ends compute the same front from it, so the runtime lays out the front
of the memory exactly where the core expects it, and the core checks that before
it runs. The example's manifest asks for two megabytes, small packets and no
devices:

```json
{
  "mem": {
    "bytes": [2097152],
    "region_bytes": 2097152,
    "min_heap_bytes": 65536
  },
  "stack": { "module_bytes": 65536, "thread_bytes": 1048576 },
  "pkt": { "bytes": 256 }
}
```

A manifest that wants a GPU or files adds `gpu` or `fs`. Its schema is
`spec/manifest.schema.json`.

### The protocol

What the program says in its frames is its own **protocol**, a JSON file of
commands. Each command is an export, which the core handles, or an import, which
the core sends, as a WIT world names them from the core's side. A command's
number is its major and its minor, `major << 8 | minor`, and a program's majors
begin at 0x10. The example has one command of each kind: the runtime sends
`SUM`, and the core calls `ASK` as it adds. Both are marked `call`, so each is
answered before it returns.

```json
{
  "name": "asker",
  "export": [
    {
      "name": "SUM",
      "val": 4097,
      "call": true,
      "pkt": [
        { "name": "a", "type": "u32" },
        { "name": "b", "type": "u32" }
      ]
    }
  ],
  "import": [{ "name": "ASK", "val": 4098, "call": true }]
}
```

The **generator** writes both ends' code from the manifest and the protocol
files. `xpute-gen manifest.json asker.json` writes the manifest's constants for
both ends and, for the protocol, on the runtime the fields each export takes,
by its number, the packets' readers and a handler for each import, and on the
core a function for each import and a trait for the exports. The generated
files are committed, and `--check` fails where one is stale. It installs with
`cargo install --path rust/gen`.

### The core

The core crate holds the generated code and invokes one macro, `core!`, which
defines the core in the form its target takes: a WebAssembly module on `wasm32`,
and a native library anywhere else. A module's build script links it from the
manifest with one call, so that it imports its memory, puts its stack first and
places its data past the front. Off `wasm32` the macro expands to
`xpute-runtime`'s native form, so the crate depends on `xpute-runtime` for every
other target, and on `xpute-gpu` with its `runtime` feature there where the
manifest asks for a GPU. The core's entry runs one turn. The example's turn hands
each waiting submission to the door, the core's end of the channel, and answers
its own commands through the generated `apply` and each of xpute's with 0:

```rust
xpute_core::core!(manifest::MANIFEST, 0, entry);

impl asker::Exports for Example {
    fn sum(&mut self, c: abi::Sum) -> Outcome {
        let answer = asker::ask().map_err(MarshalError::new)?;
        Ok(Some((c.a + c.b) as i32 + answer as i32))
    }
}

fn entry(_deadline_ms: f64, _boot: fn() -> BootInfo) -> f64 {
    xpute_core::io::door::door().receive(|s| match s {
        Event::Program(cmd, pkt) => asker::apply(&mut Example, cmd, &pkt),
        _ => Ok(Some(0)),
    });
    NO_WAKE
}
```

What the entry returns says when the core wants its next turn: 0 for as soon as
possible, a number of milliseconds, or a negative number for none until
something arrives. The door's `receive` takes every frame that waits, so a turn
with nothing more to do returns none (`NO_WAKE`).

### The config

On the JavaScript side the program boots its core from two things, as
`WebAssembly.instantiate` takes a module and its imports: the generated
`MANIFEST`, and a function that makes the core's **config** where the core
runs. The config holds what the program decides: where a page loads the module
from, what answers the program's imports, its scheduling policy and what it
submits with each turn, and, where the manifest asks for them, what backs the
files. A core that draws takes `@xpute/gpu` beside this, and is defined by
`xpute_gpu::core!`.

```ts
export const core = (): Config => ({
  core: {
    module: new URL(CORE, import.meta.url),
    bytes: MANIFEST.mem_bytes[0],
  },
  program: asker_imports({ ask: () => ANSWER }),
  policy: new Timeslice(4),
  submit: () => {},
});
```

`boot(MANIFEST, core, ui)` brings the core up beside its caller, as the test
does in Node, and `ui` says what the UI hears of the core: the protocols whose
imports it takes and the core's stop. A page keeps its turns off the thread
that draws: it calls `start(MANIFEST, ui, worker)`, the worker's module calls
`serve(MANIFEST, core)`, and a message port between them carries frames. Either
way the UI receives a **link**, the same on every platform, through which it
sends commands, asks for turns, and says whether it is shown and reads it back.
The UI never waits on the core: it requests `SUM`, and the reply comes with a
later turn:

```ts
const link = await start(
  MANIFEST,
  ui,
  () =>
    new Worker(new URL("./core.worker.ts", import.meta.url), {
      type: "module",
    }),
);
link.resume();
const sum = await link.request(Command.SUM, 2, 3);
```

### On a device, and in C++

A native app installs `@xpute/native`, the React Native module, which Expo
autolinks, and names its core crate in its `package.json` under `xpute.core`.
The app builds that crate for the device as it builds, and the module links it:
a static library at `$PODS_CONFIGURATION_BUILD_DIR/Xpute/lib<crate>.a` on an
iPhone, and a shared one among the app's `jniLibs` on Android.
The app's bundler takes each `x.native.ts` in place of `x.ts`, and there
`start(MANIFEST, core, ui)` boots the core beside the UI, on a thread of its
own, with no worker.

A C++ core takes xpute's library as two files, as SQLite ships its own: it
includes `xpute.hpp` and compiles `xpute.cpp` once, as C++20 with no exceptions,
no RTTI and no floating-point contraction, the flags `cpp/Makefile` builds with.
`make -C cpp amalgamation` writes the two files under
`cpp/target/amalgamation/`, and xpute's own repository carries them at
`cpp/amalgamation/`.

### The GPU

The GPU is a plugin: `xpute-gpu` on the core's side, with its wgpu runtime behind
the `runtime` feature, and `@xpute/gpu` on the runtime's. A core that draws gives
its manifest a stream and says what it asks of the device:

```json
{
  "stream": { "bytes": 4194304 },
  "gpu": {
    "power": "high-performance",
    "features": { "required": [], "optional": [] }
  }
}
```

The generator writes `GPU_ASK` beside `MANIFEST`, and the crate defines its core
through the plugin's macro, which also defines `admit_frame`, whether the GPU
takes a frame now. The GPU's commands reach the program as any other, numbered
by minors under a major the program chooses from its own, so its turn asks the
plugin first and its UI names the same major (`gpu_exports`, `gpu_grant`):

```rust
xpute_gpu::core!(MANIFEST, FIXED_BYTES, entry, CONFIG, GPU_ASK);

match xpute_gpu::event::decode(GPU_MAJOR, cmd, pkt)? {
    Some(Event::Ready) => {}      // a device answered
    Some(Event::Surface(s)) => {} // a surface opened, sized or closed
    None => {}                         // the program's own command
}
```

On a page, `after_turn` runs the stream as each turn returns. The worker grants
the GPU what the runtime holds, with the program's settings for the page's
runtime (`GpuConfig`, the page's counterpart of `CoreConfig`'s), and a port of
the app's carries the GPU's state up and the canvases down. A page's runtime
stages its buffer writes in mapped chunks of `upload_chunk_bytes`, as a device's
does. `slow_ms` has the runtimes say a record of the stream, the stream's run,
a completion's landing or the core's turn slower than that many milliseconds;
on a page the core's
runtime takes it in its config as well, and null, `None` or none times none:

```ts
// The config, where the core runs.
{ after_turn: () => gpu.after_turn(), /* ... */ }

// The worker.
const gpu = new WebGpu();
serve(MANIFEST, core).then((runtime) =>
  gpu_grant(gpu, runtime, MANIFEST.gpu, GPU_MAJOR, {
    upload_chunk_bytes: 1 << 20,
    slow_ms: null,
  }),
);
gpu_serve(port, gpu);

// The page.
const binder = gpu_connect(port, (state) => show(state));
binder.bind(handle, canvas);
```

On a device the runtime runs the stream itself, and `@xpute/gpu`'s module opens
the device, opens it again after a loss as a page asks again, and holds the
view a surface draws into (`SurfaceView`):

```ts
const gpu = new NativeGpu();
gpu.watch((state) => show(state));
gpu.grant({
  ready: () => gpu_exports(link, GPU_MAJOR).gpu_ready(),
  shown: link.shown, // no ask is spent while the app is hidden
});
```

### The packages

In TypeScript the parts are `@xpute/kit`, `@xpute/runtime`, `@xpute/gpu` and
`@xpute/native`, and in Rust they are the crates `xpute-kit`, `xpute-core`,
`xpute-runtime` and `xpute-gpu`, with the generator `xpute-gen`. Each carries
what a registry asks. A program beside this tree lists the packages in its pnpm workspace and depends on them as
`workspace:*`, and it names the crates by `path`. `@xpute/native` is an optional
peer of `@xpute/runtime` and `@xpute/gpu`, so a program that runs only on a page
installs neither React Native nor Expo. Each TypeScript package is its
own source: its `package.json` maps a path under the package's name to the file
under `src/`, and the files use only syntax that strips to JavaScript, so a
bundler or Node's type stripping takes them with no build.

## Where to read next

- [ARCHITECTURE.md](./ARCHITECTURE.md) follows the same claim through each part,
  the memory, the turn, the channel, the two devices and the languages, and ends
  with the invariants every end keeps and a map of the code.
- `spec/` is the contract itself: the header, the frame, the stream
  and the devices' commands, from which every language's numbers are generated.
- [XTP's README](./ts/kit/src/wire/xtp/README.md) describes the packet format
  that every frame carries.
- `rust/dev/example` and `ts/dev/example` are the example above, whole.

## Running it

xpute builds with Node at the version `ts/.tool-versions` names, with pnpm;
rustup, which installs the toolchain `rust/rust-toolchain.toml` names; a C++20
compiler, as `c++` or `$CXX`; GNU Make; and, for the C++ core built as a module,
the [WASI SDK](https://github.com/WebAssembly/wasi-sdk) at `/opt/wasi-sdk` or
`$WASI_SDK_PATH`.

```sh
(cd ts && pnpm install)             # ts/'s packages
(cd rust && cargo spec --write)     # the files generated from spec/, written
(cd rust && cargo clippy --workspace --all-targets -- -D warnings)
(cd rust && cargo test --workspace)
(cd rust && cargo test-gpu)         # the GPU runtime on wgpu
(cd ts && pnpm check && pnpm test)
make -C cpp -j8 check test          # the C++ compiled, its tests, its cores
(cd rust && cargo core && cargo core-wasm)
(cd ts && pnpm test:conformance)    # the traces, on every core
(cd ts && pnpm test:gpu:web)        # the GPU runtime in Chrome ($CHROME)
```

## License

xpute is MIT. The buddy allocator under `alloc/`, in Rust and in C++, is a port
of Evan Wallace's buddy-malloc, and `THIRD_PARTY_NOTICES.md` carries the notice
its license asks for.
