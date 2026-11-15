# xpute

xpute is a foreign function interface for the compiled part of a program. The
program's **guest**, its compiled code, and xpute's **host**, which runs it,
meet in three things: a calling convention of one host function and three guest
exports, which pass frames laid in the guest's memory; bindings that a generator
writes for both ends from the program's interfaces in WIT, as WIT's tools do;
and the entry, the only time the guest runs, when its host enters it. The guest
owns nothing but its memory. xpute declares the few interfaces every guest
shares with its host, its clocks, its files, its stop, its log and its app's
life, and the host implements the clocks, the stop and the log. Everything else
belongs to the **program**, and it travels over the same link, whose frames
xpute delivers and does not interpret. A program may bring plugins of its own
beside the runtime, a GPU for one.

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

The host gives the guest only its memory, its entries, the clocks, the stop and
the log, and the guest decides everything else.
Because that is all the host gives, the same guest runs unchanged in a page, on
a phone and in a test.

## The guest, the host and the program

A program built on xpute has three parts.

- The **guest** is the program's compiled code, linked with xpute's guest
  library into one image. It runs only inside an entry, and everything it says
  to the world it writes into its memory.
- The **host** serves the guest the clocks and the stop, `@xpute/host` in
  TypeScript and `xpute-host` in Rust, and a plugin the program brings
  serves beside it. It runs in a runtime, the platform's environment that runs the guest and enters
  it: `@xpute/runtime-web` in a page's worker, and on a device
  `xpute-runtime-native`, on a thread of its own, which the Expo module
  `@xpute/expo` starts.
- The **program** is everything else the app decides: its interfaces and what
  their methods do, and its UI, which calls and serves the guest by the same
  bindings from another thread, carried to the guest's end by the runtime: over
  a page's MessagePort, and the native module on a device.

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
    the log                                 among them, its plugins'
                                            and its UI's, which xpute
                                            does not read
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
`wit/`, and its guest's world says which the guest serves, its
exports, and which it calls, its imports. `xpute-link-gen`, one command a
side ([link/gen](./link/gen/README.md)), writes their bindings in Rust,
TypeScript and C++, and for the host two lists from the world: `IMPORTS`, the
interfaces the guest calls, and `CALLS`, the methods they hold that answer at
once, xpute's and a plugin's among them, a function without `async`. A
method the UI serves from another thread is answered later, so it is an
`async func`, and the runtime refuses to start the guest where one of `CALLS`
is mounted on the UI's end. A Rust guest
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

A plugin is a package of the program's own in WIT, whose bindings are written
as the program's are, over xpute's, and which the program's package may take
types of and its world import or export. Its guest's end, its hosts and its UI
ends are the program's, under the plugin's name.

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
program's is. A C++ guest takes what it lays and lends, the link's boxes among
it, from its default memory resource
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
link what it serves, with `IMPORTS` and `CALLS`. The servers come in no order,
since none depends on another, and the steps are the runtime's: it mounts each
server, then the stand-ins of what the UI serves, and enters the guest only
once every interface in `IMPORTS` is served, under its handle where it has
functions of its own and by an opener at some prefix where the guest opens its
resources by URI; which prefixes those are stays the program's. One that no
server mounts, or one of `CALLS` the UI serves, traps the link before the
guest's first entry, naming each.

The worker instantiates the guest's module and runs it, and the runtime tells
the guest the UI attached. A module that does not load traps the guest before
its first entry, as a device's runtime traps on its own failure. A plugin's
server is one of the list, and a thing the UI gives beside its calls under a
handle, a canvas for one, which a page's port can move, goes to the
program's `taker`, here a plugin's:

```ts
// The worker, given the page's `Instantiate`, m.
const link = await instantiate(m.module, m.init);
if (link === null) return;
const view = new View(link.side); // a plugin of the program's
run(link, {
  servers: [
    serve_stop,
    serve_clock,
    () => view.serve(),
  ],
  imports: IMPORTS,
  calls: CALLS,
  taker: {
    give: (h, canvas) => view.bind(h, canvas as OffscreenCanvas),
    take: (h) => view.unbind(h),
  },
}, m);
```

The UI meets the runtime as a device's does (`open_ui`, `stopped_on`,
`trap_on`, `start`), with two differences of the platform. It names the
worker's module as it opens its end, and the guest's module, how
to fetch it and any fields of the program's own its worker reads beside them
as it starts the guest. And the start tells the guest the page's lifecycle
from the document's visibility, which a worker cannot see, as a device's
module tells its runtime the app's from the platform. What the worker posts
the page is a message by its kind, the runtime's, a plugin's and the
program's alike, which the UI hears by its kind as it comes (`message_on`).
Nothing
is kept for a late hearer, since the worker dies with its page and no UI comes
late to it; a device's runtime outlives its UI and keeps the guest's stop and
its trap:

```ts
const ui = open_ui(WORKER_URL);
stopped_on((stop) => { /* show it */ });
trap_on((err) => { /* show it */ });
message_on<ViewReport>("view", (m) => { /* show it */ });
// Mount on `ui` what the UI serves, before `start`.
start(MODULE_URL, undefined);
const view = await open(sender(ui), uri("app:view", "view"));
give(view, canvas.transferControlToOffscreen());
```

### On a device

A native app installs `@xpute/expo`, the React Native module, which
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
            Box::new(move || view::serve(Some(cache_dir))), // a plugin's
        ],
        imports: abi::IMPORTS,
        calls: abi::CALLS,
    }
}

xpute_runtime_native::native_abi!(start);
```

The UI opens its end of the runtime (`open_ui`), mounts on it what
it serves, and then starts the runtime (`start`). The first start of a
process starts the guest and serves it what is mounted on the UI's end by
then, its named handles and prefixes; each start attaches the UI under a new
handle, and one after the first drops the last UI's, as a reload's, the new
UI served as the first was, and each start tells the guest a UI attached. The UI hears the guest's stop and its trap
(`stopped_on`, `trap_on`), and each, as the UI begins to hear it, tells at
once the last the runtime keeps, so a UI that starts after the guest's end
hears it too:

```ts
const ui = open_ui();
stopped_on((stop) => { /* show it */ });
trap_on((err) => { /* show it */ });
// Mount on `ui` what the UI serves, before the first `start`.
start();
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

Where an import names `x.ts`, the app's bundler takes `x.native.ts` on a
device and `x.web.ts` on a page. A plugin's UI end on a device is a module of
its own beside `@xpute/expo`, which links into the crate the runtime's module
builds.

### The packages

In TypeScript the parts are `@xpute/kit`, `@xpute/link`, `@xpute/host`,
`@xpute/runtime-web` and `@xpute/expo`, and in Rust they are the crates
`xpute-kit`, `xpute-link`, `xpute-guest`, `xpute-host` and
`xpute-runtime-native`, with the generators `xpute-gen` and `xpute-link-gen`.
Each carries what a registry asks. A program beside this tree lists the
packages in its pnpm workspace and depends on them as `workspace:*`, and it
names the crates by `path`. React Native and Expo are peers of the native
module alone, so a program that runs only on a page installs neither. Each
TypeScript package is its own source: its `package.json` maps a path under the
package's name to the file under `src/`, and the files use only syntax that
strips to JavaScript, so a bundler or Node's type stripping takes them with no
build.

## Where to read next

- [ARCHITECTURE.md](./ARCHITECTURE.md) follows the same claim through the
  link, its sides, how a failure ends and the languages, and ends with xpute's boundaries, what it took from earlier
  work and a map of the code.
- Each part's `spec/` is the contract itself: the link's frame (`link/spec/`)
  and the errno table (`kit/spec/`), from which every language's numbers are
  generated. The interfaces are WIT, under `link/wit/` and `wit/`.
- [XTP's README](./kit/ts/src/wire/xtp/README.md) describes XTP, the tree
  format of the data a program stores and serves, and of the native
  runtime's batches to a UI on the JavaScript thread.
- [CONTRIBUTING.md](./CONTRIBUTING.md) lists what xpute builds with and the
  commands that build and check it.

## License

xpute is MIT. The buddy allocator under `alloc/`, in Rust and in C++, is a port
of Evan Wallace's buddy-malloc, and `THIRD_PARTY_NOTICES.md` carries the notice
its license asks for.
