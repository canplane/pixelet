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
files, its stop, its log and its app's life, and the host implements the clocks,
the stop, the log and the one device that lends that memory, the GPU. Everything else belongs to
the **program**, and it travels over the same link, whose frames xpute delivers
and does not interpret.

The same guest runs as a WebAssembly module in a web page and as a library
linked into an iOS or Android app, while the program does the I/O and its
JavaScript draws the UI around it. xpute is three parts that run there, and
tools that only build them:

- **the guest, the host and the link between them**, the side WIT draws: what
  crosses the boundary, and the protocol both ends run. It assumes one thread,
  knows nothing of where each end runs, and varies by language;
- **the runtime**, the execution environment: where and how the guest runs on
  a platform, what carries frames to it and to the UI, and what it gives its
  host. It varies by platform, with an adapter for each shell;
- **the kit**, the reusable code beneath both: collections, codecs, math, the
  errors, the wire format and allocators. It varies by language and names
  nothing above it.

The generators, xpute's build tool and the golden records' readers run only
when a program is built or tested, and no image links them. xpute holds no
memory layout or policy, no scheduler, no isolation and no UI.

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

The host gives the guest only its memory, its entries, the clocks, the stop, the
log and the device that lends the memory, and the guest decides everything else.
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
exports, and which it calls, its imports. `xpute-link-gen`, one command a
side ([link/gen](./link/gen/README.md)), writes their bindings in Rust,
TypeScript and C++, and for the host two lists from the world: `IMPORTS`, the
interfaces the guest calls, and `LATER`, the methods of the program's own
named interfaces among them that answer later, which tells the runtime which
of the UI's methods answer later. A Rust guest
defines its three exports with one macro, `link!`, given four things. The
first is a frame's cap in bytes, a multiple of 8 that holds a frame's words
(`HDR_SZ`) and the largest packet the program lays or takes. The second is
its boot, which mounts what it serves. The last two are the program's start
and end of every entry: `enter` runs as each entry begins, and `end` as it
ends, answering whether it left work, in which case the entry runs it again.
The macro makes the guest's own bug or panic its stop, told to the host
before it stops, as a C++ guest's `XPUTE_LINK` makes its bug. Here it serves
xpute's `life`, which is called as the app is hidden and shown and as a UI
attaches, and has no work of its own to start or end an entry with:

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
the GPU than its frame latency (`xpute_gpu_guest::admit::frame`), which the
program sets in its host's `GpuConfig`, so taking a surface's texture never
blocks.

A C++ guest takes xpute's library as two files, as SQLite ships its own: it
includes `xpute.hpp` and compiles `xpute.cpp` once, as C++20 with no exceptions,
no RTTI and no floating-point contraction, the flags `cpp/Makefile` builds with.
Its bindings include `xpute.hpp` as well, so the two files and the bindings
are all it compiles. `make -C cpp amalgamation` writes the two files under
`cpp/target/amalgamation/`, and xpute's own repository carries them at
`cpp/amalgamation/`. A module is compiled with the WASI SDK's clang for
`wasm32-wasip1` and linked as a Rust guest's is, with no start files and no
entry, and its stack first, of the size the program chooses:
`-nostartfiles -Wl,--no-entry -Wl,--stack-first -Wl,-z,stack-size=<bytes>`.
It then exports its memory and the link's three exports, imports `link_out`
from `env` and nothing of WASI, and runs its static initializers as its first
export starts.
`make -C cpp module GUEST="<sources>" MODULE=<file>` links one so.

xpute lays out none of the guest's memory and picks none of its allocators.
A module's memory is its own, grown as its allocator asks, and a host on a
page reads it through a view made at each access, so a growth detaches
nothing. A native guest's memory is its process's. A span the guest lends is
named by its address, a u64 on every platform: an offset into the module's
memory, or a native pointer, which the host and the UI read in place. A Rust
guest's global allocator is the one its program declares, as any Rust
program's is. A C++ guest takes what it lays and lends, the link's boxes and a
GPU stream's records, from its default memory resource
(`std::pmr::get_default_resource`), which its program sets, and its global
`operator new` is the program's as well.

A program that wants its memory fixed and laid out by itself declares so in
its own code. On a page it links its module with `--initial-memory` and
`--max-memory` equal, so the memory starts at its size and never grows; on a
device it reserves a block of that size of its process. Its fixed ranges lie
first, on the page past the module's data or at the block's base, and the
kit's heap (`xpute_kit::alloc::heap::HeapMalloc`, a slab of size classes over
a buddy allocator of pages, taken under one lock) runs over the rest, which
its terminal crate declares its global allocator: the module's crate on a
page, the device's library on a device. The C++ kit has the same heap as a
`std::pmr::memory_resource` (`xpute/kit/alloc/heap.hpp`).

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

The worker instantiates the guest's module and runs it, and the runtime tells
the guest the UI attached. A module that does not load traps the guest before
its first entry, as a device's runtime traps on its own failure. The GPU's server takes what the
guest asks of the device (`GpuAsk`, WebGPU's power preference and features)
and the program's settings for its host (`GpuConfig`), and the canvas the UI
gives beside the link under a surface's handle goes to it:

```ts
// The worker, given the page's `Instantiate`, m.
const link = await instantiate(m.module, m.init);
if (link === null) return;
const gpu = new WebGpu(link, ASK, CFG);
run(link, {
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
```

The UI meets the runtime as a device's does (`open_remote`, `stopped_on`,
`trap_on`, `start`), with two differences of the platform. It names the
worker's module as it opens its end of the link, and the guest's module, how
to fetch it and any fields of the program's own its worker reads beside them
as it starts the guest. And the start tells the guest the page's lifecycle
from the document's visibility, which a worker cannot see, as a device's
module tells its runtime the app's from the platform. What the worker posts
the page is a message by its kind, the runtime's, the GPU's server's and the
program's alike, which the UI hears by its kind as it comes (`message_on`),
the GPU's state among them (`GpuReport`, from `@xpute/gpu-web-host`). Nothing
is kept for a late hearer, since the worker dies with its page and no UI comes
late to it; a device's runtime outlives its UI and keeps the guest's stop and
its trap:

```ts
const remote = open_remote(WORKER_URL);
stopped_on((stop) => { /* show it */ });
trap_on((err) => { /* show it */ });
message_on<GpuReport>("gpu", (m) => { /* show m.state */ });
// Mount on `remote` what the UI serves, before `start`.
start(remote, MODULE_URL, undefined);
const surface = await open(remote, uri("xpute:gpu", "surface"));
remote.give(surface, canvas.transferControlToOffscreen());
```

### On a device

A native app installs `@xpute/native-runtime`, the React Native module, which
Expo autolinks, and names in its `package.json` under `xpute.guest.runtime` the
crate a device links: its guest, and the function that gives the runtime its
list, which the crate exports the runtime over. It starts the guest on a
thread of its own, with the stack the list asks, and runs the
loop for the guest's life:

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

The UI opens its end of the runtime's link (`open_remote`), mounts on it what
it serves, and then starts the runtime (`start`). The first start of a
process starts the guest and serves it what is mounted on the remote by then,
its named handles and prefixes; each start after it is a new UI on the same
runtime, as a reload's, served as the first was, and each start tells the
guest a UI attached. The UI hears the guest's stop and its trap
(`stopped_on`, `trap_on`), and each, as the UI begins to hear it, tells at
once the last the runtime keeps, so a UI that starts after the guest's end
hears it too:

```ts
const remote = open_remote();
stopped_on((stop) => { /* show it */ });
trap_on((err) => { /* show it */ });
// Mount on `remote` what the UI serves, before the first `start`.
start(remote);
```

The app builds that crate for the device as it builds, and the module links
it: a static library at `$PODS_CONFIGURATION_BUILD_DIR/Xpute/lib<crate>.a` on
an iPhone, and a shared one among the app's `jniLibs` on Android. The crate is
Rust whichever language its guest is in, as the runtime it holds is, so a C++
guest reaches a device by the same rule. The crate's build script compiles the
guest's sources and the amalgamation's `xpute.cpp` for the device, with the
flags a module is compiled with, into a static library the crate links, as
the `cc` crate does. In a library, `XPUTE_LINK` defines the three exports as
the C symbols the runtime calls, and the runtime defines `link_out`, which
the guest calls:

```rust
// build.rs of the crate `xpute.guest.runtime` names.
fn main() {
    cc::Build::new()
        .cpp(true)
        .std("c++20")
        .flag("-fno-exceptions")
        .flag("-fno-rtti")
        .flag("-ffp-contract=off")
        .include("xpute") // xpute.hpp
        .file("xpute/xpute.cpp")
        .file("guest/main.cpp")
        .compile("guest");
}
```

The app's bundler takes each `x.native.ts` in place of `x.ts`.
`xpute-gpu-native-host` opens the device when the guest asks, with the
program's `GpuConfig`, its frame latency among it, and keeps its compiled
pipelines in `cache_dir`; `@xpute/gpu-native-runtime`'s module gives the view
a guest's surface draws into (`SurfaceView`), drives the display's frames, and
tells the UI what the host has of a GPU as that changes, the state now first
to a UI that starts later (`state_on`). The host draws with a fork of wgpu,
which a program's workspace patches in as xpute's does
([wgpu-fork.md](wgpu-fork.md)).

### The packages

In TypeScript the parts are `@xpute/kit`, `@xpute/link`, `@xpute/web-runtime`,
`@xpute/web-host`, `@xpute/native-runtime`, `@xpute/gpu-link`,
`@xpute/gpu-web-host` and `@xpute/gpu-native-runtime`, and in Rust they are the
crates `xpute-kit`, `xpute-link`, `xpute-guest`, `xpute-native-runtime`,
`xpute-native-host`, `xpute-gpu-link`, `xpute-gpu-guest` and
`xpute-gpu-native-host`, with the generators
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
