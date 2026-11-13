# xpute

xpute is a foreign function interface for the compiled part of a program. The
program's **guest**, its compiled code, and xpute's **host**, which runs it,
meet in four things: a calling convention over three host functions, which pass
frames laid in the guest's memory; bindings that a generator writes for both
ends from the program's protocols, as WIT's tools do; the turn, the one call
that gives the guest its time; and the GPU stream, a command buffer the guest
records and the host runs, which moves large data while that memory stays fixed.
The guest owns nothing but its memory, and the host implements only the device
that lends it across turns, the GPU. Everything else belongs to the
**program**, and it travels over one general channel whose messages xpute
delivers and does not interpret.

The same guest runs as a WebAssembly module in a web page and as a library
linked into an iOS or Android app, while the program's JavaScript does the I/O
and draws the UI around it. xpute is both ends of that arrangement, the guest's
library and the host, and `kit`, the generic code beneath them.

## Why one interface

A program whose logic is compiled and whose surface is JavaScript usually
crosses between the two languages once for every piece of work: a binding for
each function, a copy for each argument, and a different binding on each
platform. Every crossing costs time, and together the crossings become the
program's real interface, scattered through glue code.

xpute replaces those crossings with one memory that both sides share and one
kind of message, a frame, laid in that memory. Into the guest goes one call that
runs a turn, and the number it returns says when the guest wants its next turn.
Out of the guest go four functions, the same on every platform: one that hands
it the next frame waiting for it, one that takes a frame it sends, a call
answered before it returns, and the host's clock. What a frame says is the
program's, so no binding is written for any one piece of work.

The host gives the guest only its memory, its turns, a clock and the device
that lends the memory, and the guest decides everything else. Because
that is all the host gives, the same guest runs unchanged in a page, on a
phone and in a test.

## The guest, the host and the program

A program built on xpute has three parts.

- The **guest** is the program's compiled code, linked with xpute's guest
  library into one image. It runs only inside a turn, and everything it says to
  the world it writes into its memory.
- The **host** is xpute's TypeScript, `@xpute/runtime`. It reserves the
  guest's memory and lays out its front, runs each turn and reads what the turn
  sent. The GPU is a plugin, `@xpute/gpu`.
- The **program** is everything else the app decides: what its commands mean,
  when its turns should run, and its UI.

```text
  ┌────────────────────────────────────────────────────────────────┐
  │ the guest: the program's code, built on xpute's guest library. │
  │ It owns one memory, where every frame is laid, and it runs     │
  │ only inside run(deadline).                                     │
  └───────────┬─────────────────────────────────┬──────────────────┘
              ▲                                 │
              │ receive:                        │ send:
              │ what the guest is sent          ▼ what the guest says
  ┌───────────┴─────────────────────────────────┴──────────────────┐
  │ the host: lays out the memory, runs each turn, and routes      │
  │ each frame by the major of its command                         │
  └───────────┬─────────────────────────────────┬──────────────────┘
              │ majors 0x00 to 0x0F             │ majors 0x10 and up
    xpute's own                             the program: its handlers
                                            and its UI, which xpute
                                            does not read
```

Each frame carries a command and a packet, the command's data in
XTP, xpute's tree format. The high byte of a command's number is its major, and
xpute delivers a program's frame by that major and reads nothing of what it
says.

## How a program uses it

### On a device, and in C++

A native app installs `@xpute/native`, the React Native module, which Expo
autolinks, and names its guest crate in its `package.json` under `xpute.guest`.
The app builds that crate for the device as it builds, and the module links it:
a static library at `$PODS_CONFIGURATION_BUILD_DIR/Xpute/lib<crate>.a` on an
iPhone, and a shared one among the app's `jniLibs` on Android.
The app's bundler takes each `x.native.ts` in place of `x.ts`.

A C++ guest takes xpute's library as two files, as SQLite ships its own: it
includes `xpute.hpp` and compiles `xpute.cpp` once, as C++20 with no exceptions,
no RTTI and no floating-point contraction, the flags `cpp/Makefile` builds with.
`make -C cpp amalgamation` writes the two files under
`cpp/target/amalgamation/`, and xpute's own repository carries them at
`cpp/amalgamation/`.

### The GPU

The GPU is a plugin: `xpute-gpu` on the guest's side, with its wgpu host behind
the `runtime` feature, and `@xpute/gpu` on the host's. A guest that draws gives
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

The generator writes `GPU_ASK` beside `MANIFEST`, and the crate defines its
guest through the plugin's macro, which also defines `admit_frame`, whether the
GPU takes a frame now. The GPU's commands reach the program as any other,
numbered by minors under a major the program chooses from its own, so its turn
asks the plugin's generated `apply` with that major, and its UI names the same
major (`gpu_exports`, `gpu_grant`):

```rust
xpute_gpu::core!(MANIFEST, FIXED_BYTES, entry, CONFIG, GPU_ASK);

impl xpute_gpu::abi::Exports for Program {
    fn gpu_ready(&mut self) -> Outcome { /* a device answered */ }
    fn gpu_surface_open(&mut self) -> Outcome { /* a surface lent */ }
    // ... _resize, _close
}

Event::Program(cmd, pkt) if cmd >> 8 == GPU_MAJOR => {
    xpute_gpu::abi::apply(&mut program, GPU_MAJOR, cmd, &pkt)
}
```

On a page, `after_turn` runs the stream as each turn returns. The worker grants
the GPU what the host holds, with the program's settings for the page's host
(`GpuConfig`), and a port of the app's carries the GPU's state up and the
canvases down. A page's host stages its buffer writes in mapped chunks of
`upload_chunk_bytes`, as a device's does. `slow_ms` has the hosts say a record
of the stream, the stream's run, a completion's landing or the guest's turn
slower than that many milliseconds; on a page the guest's host takes it in its
config as well, and null, `None` or none times none:

```ts
// The config, where the guest runs.
{ after_turn: () => gpu.after_turn(), /* ... */ }

// The worker.
const gpu = new WebGpu();
serve(MANIFEST, core).then((runtime) =>
  gpu_grant(gpu, runtime, MANIFEST, GPU_MAJOR, {
    upload_chunk_bytes: 1 << 20,
    slow_ms: null,
  }),
);
gpu_serve(port, gpu);

// The page.
const binder = gpu_connect(port, (state) => show(state));
binder.bind(handle, canvas);
```

On a device the host runs the stream itself, and `@xpute/gpu`'s module opens
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
`@xpute/native`, and in Rust they are the crates `xpute-kit`, `xpute-core`
and `xpute-gpu`, with the generators `xpute-gen` and `xpute-link-gen`. Each carries
what a registry asks. A program beside this tree lists the packages in its pnpm workspace and depends on them as
`workspace:*`, and it names the crates by `path`. `@xpute/native` is an optional
peer of `@xpute/runtime` and `@xpute/gpu`, so a program that runs only on a page
installs neither React Native nor Expo. Each TypeScript package is its
own source: its `package.json` maps a path under the package's name to the file
under `src/`, and the files use only syntax that strips to JavaScript, so a
bundler or Node's type stripping takes them with no build.

## Where to read next

- [ARCHITECTURE.md](./ARCHITECTURE.md) follows the same claim through each part,
  the memory, the turn, the devices and the languages, and ends
  with the invariants every end keeps and a map of the code.
- `spec/` is the contract itself: the header, the frame and the stream,
  from which every language's numbers are generated.
- [XTP's README](./ts/kit/src/wire/xtp/README.md) describes the packet format
  that every frame carries.

## Running it

xpute builds with Node at the version `ts/.tool-versions` names, with pnpm;
rustup, which installs the toolchain `rust/rust-toolchain.toml` names; a C++20
compiler, as `c++` or `$CXX`; GNU Make; and, for the C++ guest built as a
module, the [WASI SDK](https://github.com/WebAssembly/wasi-sdk) at
`/opt/wasi-sdk` or `$WASI_SDK_PATH`.

```sh
(cd ts && pnpm install)             # ts/'s packages
(cd rust && cargo spec --write)     # the files generated from spec/, written
(cd rust && cargo clippy --workspace --all-targets -- -D warnings)
(cd rust && cargo test --workspace)
(cd rust && cargo test-gpu)         # the GPU host on wgpu
(cd ts && pnpm check && pnpm test)
make -C cpp -j8 check test          # the C++ compiled, its tests, its guests
(cd ts && pnpm test:gpu:web)        # the GPU host in Chrome ($CHROME)
```

## License

xpute is MIT. The buddy allocator under `alloc/`, in Rust and in C++, is a port
of Evan Wallace's buddy-malloc, and `THIRD_PARTY_NOTICES.md` carries the notice
its license asks for.
