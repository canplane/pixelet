# xpute

xpute is a foreign function interface for the compiled part of a program. The
program's **guest**, its compiled code, and xpute's **host**, which runs it,
meet in four things: a calling convention of one host function and three guest
exports, which pass frames laid in the guest's memory; bindings that a generator
writes for both ends from the program's interfaces in WIT, as WIT's tools do;
the entry, the only time the guest runs, when its host enters it; and the GPU
stream, a command buffer the guest records and the host runs, which moves large
data while that memory stays fixed. The guest owns nothing but its memory. xpute
declares the few interfaces every guest shares with its host, its clocks, its
files, its stop and its app's life, and the host implements the clocks, the stop
and the one device that lends that memory, the GPU. Everything else belongs to
the **program**, and it travels over the same link, whose frames xpute delivers
and does not interpret.

The same guest runs as a WebAssembly module in a web page and as a library
linked into an iOS or Android app, while the program does the I/O and its
JavaScript draws the UI around it. xpute is both ends of that arrangement, the
guest's library and the host, the **runtime** the host runs in on each platform,
and `kit`, the generic code beneath them.

## Why one interface

A program whose logic is compiled and whose surface is JavaScript usually
crosses between the two languages once for every piece of work: a binding for
each function, a copy for each argument, and a different binding on each
platform. Every crossing costs time, and together the crossings become the
program's real interface, scattered through glue code.

xpute replaces those crossings with one memory that both sides share and one
kind of message, a frame, laid in that memory: one method of an object, called
under the object's handle. Into the guest go three exports: `link_init`, which
lays its link and answers where the link lies, `link_boot`, its first entry,
and `link_in`, which enters it with a frame laid in its inbox, a request or an
answer. Out of the guest goes one function, `link_out`, the same on every
platform, which takes a frame laid in its outbox and returns at once what the
method came to: its result, or `EINPROGRESS` for a method that answers later,
in a frame of its own. Each method is declared in WIT and its bindings are
generated, so no binding is written by hand for any one piece of work.

The host gives the guest only its memory, its entries, the clocks, the stop and
the device that lends the memory, and the guest decides everything else.
Because that is all the host gives, the same guest runs unchanged in a page, on
a phone and in a test.

## The guest, the host and the program

A program built on xpute has three parts.

- The **guest** is the program's compiled code, linked with xpute's guest
  library into one image. It runs only inside an entry, and everything it says
  to the world it writes into its memory.
- The **host** serves the guest the clocks and the stop, `@xpute/web-host` on a
  page and `xpute-native-host` on a device, and the GPU, which is a plugin. It
  runs in a runtime, the platform's environment that runs the guest and enters
  it: `@xpute/web-runtime` in a page's worker, and on a device
  `xpute-native-runtime`, on a thread of its own, which the Expo module
  `@xpute/native-runtime` starts.
- The **program** is everything else the app decides: its interfaces and what
  their methods do, and its UI, which reaches the guest's link from another
  thread: over a port on a page (`Remote`), and through the native module on a
  device.

```text
  ┌────────────────────────────────────────────────────────────────┐
  │ the guest: the program's code, built on xpute's guest library. │
  │ It owns one memory, where every frame is laid, and it runs     │
  │ only inside an entry.                                          │
  └───────────┬─────────────────────────────────┬──────────────────┘
              ▲                                 │
              │ link_in:                        │ link_out:
              │ a frame for the guest           ▼ a frame it sends
  ┌───────────┴─────────────────────────────────┴──────────────────┐
  │ the host's link: enters the guest, and routes each frame by    │
  │ the handle in its command to the object served under it        │
  └───────────┬─────────────────────────────────┬──────────────────┘
              │ the host's own                  │ the program's
    the clocks, the stop,                   its objects, its files
    the GPU                                 among them, and its UI's,
                                            which xpute does not read
```

Each frame is four words, its tag, its command, its result and its packet's
length, and the packet past them, in the Component Model's Canonical ABI layout
(`link/rust/src/cabi.rs`, `link/ts/src/cabi.ts`, `cpp/xpute/link/cabi.hpp`).
The command carries the handle of the object called and the method's number,
and xpute delivers a program's frame by that handle and reads nothing of what
it says.

## How a program uses it

### The guest

A program declares its interfaces in WIT, as xpute declares its own under
`wit/` and `gpu/wit/`, and its guest's world says which the guest serves, its
exports, and which it calls, its imports. `xpute-link-gen` writes their
bindings for each side in Rust, TypeScript and C++, and for the host two lists
from the world: `IMPORTS`, the interfaces the guest calls, and `LATER`, the
methods of the interfaces the UI serves that answer later. A Rust guest
defines its three exports with one macro, over frames of a cap it chooses,
and mounts what it serves as it boots; the macro makes the guest's own bug or
panic its stop, told to the host before it stops, as a C++ guest's
`XPUTE_LINK` makes its bug. Here it serves xpute's `life`, which is called as
the app is hidden and shown and as a UI attaches:

```rust
use std::rc::Rc;
use xpute_guest::abi::life;
use xpute_kit::status::errno::Errno;

struct Program;

impl life::Guest for Program {
    fn hidden(&self) -> Result<(), Errno> { /* save */ Ok(()) }
    fn shown(&self) -> Result<(), Errno> { Ok(()) }
    fn attached(&self) -> Result<(), Errno> { /* tell the UI where all stands */ Ok(()) }
}

xpute_guest::link!(4096, || life::mount(Rc::new(Program)), || {}, || false);
```

The GPU is a plugin, declared in WIT as xpute's own packages are: the guest
imports `gpu`, which runs the records it lends and gives it a device, and
`vsync`, which answers at the display's next frame, and serves `surface`, which
the UI opens to draw on. `xpute-gpu-guest` writes WebGPU's calls as records of
a stream, and draws a frame only while fewer of its frames are unfinished on
the GPU than its frame latency (`xpute_gpu_guest::admit::frame`), so taking a
surface's texture never blocks.

A C++ guest takes xpute's library as two files, as SQLite ships its own: it
includes `xpute.hpp` and compiles `xpute.cpp` once, as C++20 with no exceptions,
no RTTI and no floating-point contraction, the flags `cpp/Makefile` builds with.
`make -C cpp amalgamation` writes the two files under
`cpp/target/amalgamation/`, and xpute's own repository carries them at
`cpp/amalgamation/`.

### On a page

The program builds its host as a list, and the runtime starts the guest from
it, here as on a device: the program's servers, each of which mounts on the
link what it serves, with `IMPORTS` and `LATER`. The servers come in no order,
since none depends on another, and the steps are the runtime's: it mounts each
server, then the stand-ins of what the UI serves, and enters the guest only
once every interface in `IMPORTS` is served, under its handle where it has
functions of its own and by an opener at some prefix where the guest opens its
resources by URI; which prefixes those are stays the program's. One that no
server mounts traps the link before the guest's first entry, naming each.

The worker instantiates the guest's module and starts it, and the runtime
tells the guest the UI attached. The GPU's server takes what the guest
asks of the device (`GpuAsk`, WebGPU's power preference and features) and the
program's settings for its host (`GpuConfig`), and the canvas the UI gives
beside the link under a surface's handle goes to it:

```ts
// The worker, given the page's `Instantiate`, m.
const link = await instantiate(m.module, m.init, MEM_SZ);
if (link === null) return;
const gpu = new WebGpu(link, ASK, CFG);
start(link, {
  servers: [
    serve_stop,
    serve_clock,
    serve_vsync,
    () => gpu.serve(),
  ],
  imports: IMPORTS,
  later: LATER,
  taker: {
    give: (h, canvas) => gpu.bind(h, canvas as OffscreenCanvas),
    take: (h) => gpu.unbind(h),
  },
}, m);

// The page.
const { worker, remote, port } = open_worker(WORKER_URL);
const ask = { kind: "instantiate", module: MODULE_URL, init: undefined };
worker.postMessage({ ...ask, port, named: remote.named, prefixes: remote.prefixes }, [port]);
tell_life(remote);
const surface = await open(remote, uri("xpute:gpu", "surface"));
remote.give(surface, canvas.transferControlToOffscreen());
```

The GPU's server posts the page what it has of a GPU as that changes
(`GpuReport`), as the device's GPU host tells its app.

### On a device

A native app installs `@xpute/native-runtime`, the React Native module, which
Expo autolinks, and names in its `package.json` under `xpute.guest.runtime` the
crate a device links: its guest, and the function that gives the runtime its
list, which the crate exports the runtime over. The runtime starts the guest
on a thread of its own, with the stack the list asks, and runs the loop for
the guest's life. The UI hears the guest's stop and its trap
(`stopped_on`, `trap_on`), those the runtime keeps told first to a UI that
starts later, and each start of a UI tells the guest it attached:

```rust
fn start(cache_dir: PathBuf) -> Boot {
    Boot {
        stack_sz: 8 << 20,
        servers: vec![
            Box::new(stop::serve),
            Box::new(clock::serve),
            Box::new(move || {
                native::serve(Ask::of(ASK), native::config(CFG), Some(cache_dir));
            }),
        ],
        imports: abi::IMPORTS,
        later: abi::LATER,
    }
}

xpute_native_runtime::native_abi!(start);
xpute_gpu_native_host::native_abi!();
```

The app builds that crate for the device as it builds, and the module links
it: a static library at `$PODS_CONFIGURATION_BUILD_DIR/Xpute/lib<crate>.a` on
an iPhone, and a shared one among the app's `jniLibs` on Android. The app's
bundler takes each `x.native.ts` in place of `x.ts`. `xpute-gpu-native-host`
opens the device when the guest asks, with the program's `GpuConfig`, its
frame latency among it, and keeps its compiled pipelines in `cache_dir`;
`@xpute/gpu-native-runtime`'s module gives the view a guest's surface draws
into (`SurfaceView`), drives the display's frames, and tells the UI what the
host has of a GPU as that changes, the state now first to a UI that starts
later (`state_on`). The host draws with a
fork of wgpu, which a program's workspace patches in as xpute's does
([wgpu-fork.md](wgpu-fork.md)).

### The packages

In TypeScript the parts are `@xpute/kit`, `@xpute/link`, `@xpute/web-runtime`,
`@xpute/web-host`, `@xpute/native-runtime`, `@xpute/gpu-link`,
`@xpute/gpu-web-host` and `@xpute/gpu-native-runtime`, and in Rust they are the
crates `xpute-kit`, `xpute-link`, `xpute-guest`, `xpute-native-runtime`,
`xpute-native-host`, `xpute-gpu-link`, `xpute-gpu-guest`,
`xpute-gpu-native-runtime` and `xpute-gpu-native-host`, with the generators
`xpute-gen` and `xpute-link-gen`.
Each carries what a registry asks. A program beside this tree lists the
packages in its pnpm workspace and depends on them as `workspace:*`, and it
names the crates by `path`. React Native and Expo are peers of the two native
modules alone, so a program that runs only on a page installs neither. Each
TypeScript package is its own source: its `package.json` maps a path under the
package's name to the file under `src/`, and the files use only syntax that
strips to JavaScript, so a bundler or Node's type stripping takes them with no
build.

## Where to read next

- [ARCHITECTURE.md](./ARCHITECTURE.md) follows the same claim through the
  link, its sides, how a failure ends, the device that lends the memory and
  the languages, and ends with xpute's boundaries, what it took from earlier
  work and a map of the code.
- Each part's `spec/` is the contract itself: the link's frame (`link/spec/`),
  the GPU stream's records (`gpu/link/spec/`) and the errno table
  (`kit/spec/`), from which every language's numbers are generated. The
  interfaces are WIT, under `link/wit/`, `wit/` and `gpu/wit/`.
- [XTP's README](./kit/ts/src/wire/xtp/README.md) describes XTP, the tree
  format of the data a program stores and serves, and of the native
  runtime's batches to a UI on the JavaScript thread.
- [CONTRIBUTING.md](./CONTRIBUTING.md) lists what xpute builds with and the
  commands that build and check it.

## License

xpute is MIT. The buddy allocator under `alloc/`, in Rust and in C++, is a port
of Evan Wallace's buddy-malloc, and `THIRD_PARTY_NOTICES.md` carries the notice
its license asks for.
