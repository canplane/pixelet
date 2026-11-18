# xpute

xpute is a foreign function interface for the compiled part of a program. The
program's **guest**, its compiled code, and xpute's **host**, which implements
its imports, meet in three things: a calling convention of one host function and
three guest exports, which pass frames laid in the guest's memory; bindings that
a generator writes for both ends from the program's interfaces in WIT, as WIT's
tools do; and the entry, the only time the guest runs, when its host enters it.
The guest owns nothing but its memory. xpute declares the few interfaces every
guest shares with its host, its clocks, its files, its stop, its log and its
app's life, and the host implements the clocks, the stop and the log. Everything
else belongs to the **program**, and it travels over the same link, whose frames
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
memory layout or policy, no scheduler and no isolation, and it draws no UI: a
shell's UI end, one of its parts, only carries the program's UI to the guest.

## Why one interface

A program whose logic is compiled and whose surface is JavaScript usually
crosses between the two languages once for every piece of work: a binding for
each function, a copy for each argument, and a different binding on each
platform. Every crossing costs time, and together the crossings become the
program's real interface, scattered through glue code.

xpute replaces those crossings with one memory that both sides share and one
kind of message, a frame, laid in that memory: one method of an object, called
under the object's handle. Arguments are laid once in a frame's packet; buffers
can instead be lent where they lie and accessed in place by a host in the same
process. A frame crosses into the guest by one export and out of it by one
function, the same on every platform, and each crossing returns at once what the
method came to: its result, or word that the answer will come later in a frame
of its own. Each method is declared in WIT and its bindings are generated, so no
binding is written by hand for any one piece of work.
[ARCHITECTURE.md](./ARCHITECTURE.md) lays out the frame and the crossings.

The host gives the guest only its entries, the clocks, the stop and the log, so
the same guest runs unchanged on the web, on a phone and in a test.

## Beside WASI and the component model

xpute takes its words and its layouts from WIT and the component model: the
guest and its host, a world's imports and exports, interfaces declared in WIT
with both ends' bindings generated, an object called under a handle, and a
packet's values in the Canonical ABI's layout. It is not WASI, and a guest is
not a component. WASI answers how a program runs on any host that keeps a
standard; xpute answers how one program's compiled part runs unchanged in a
browser and inside a phone's app. Between the guest and its host they part in
five places.

| | WASI and the component model | xpute |
| --- | --- | --- |
| The guest | a component, which a browser does not run: a tool transpiles it into JavaScript glue over core modules | a core module as a browser loads it, or a native library linked into the app |
| The boundary | an import or an export for each function | one function out and three exports, and frames between them, so a native app's C ABI carries the same boundary |
| A value | copied into the other side's memory at each call | laid once in the frame's packet, and a buffer lent where it lies |
| What the host gives | a standard set of interfaces, files, sockets and HTTP among them, which a browser has to imitate | its entries, the clocks, the stop and the log; the rest is the program's own WIT |
| Isolation | components share no memory | the host reads the guest's memory, and nothing is isolated: both are one program's |

A guest written for xpute runs on no WASI host and composes with no component,
and what a host must keep for it is the program's own to declare. What of WIT the link takes and what it reads
its own way, and how xpute's packages stand to WASI's, is in
[link/gen](./link/gen/README.md).

## The guest, the host and the program

A program built on xpute has three parts.

- The **guest** is the program's compiled code, linked with xpute's guest
  library into one image. It runs only inside an entry, and everything it says
  to the world it writes into its memory.
- The **host** implements the guest's clocks, stop and log, `@xpute/host` in
  TypeScript and `xpute-host` in Rust, and a plugin the program brings
  implements its own beside it. It runs in a runtime, the platform's environment
  that runs the guest and enters it: `@xpute/runtime-web` in the web worker,
  and on iOS and Android `xpute-runtime-native`, on a thread of its own, which
  the Expo module `@xpute/expo` starts.
- The **program** is everything else the app decides: its interfaces and what
  their methods do, and its UI, which calls the guest and implements its imports
  by the same bindings from another thread, carried to the guest's end by the
  runtime: over the web's MessagePort, and the native module on iOS and Android.

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
  │ the handle in its command to the object under it               │
  └───────────┬─────────────────────────────────┬──────────────────┘
              │ the host's own                  │ the program's
    the clocks, the stop,                   its objects, its files
    the log                                 among them, its plugins'
                                            and its UI's, which xpute
                                            does not read
```

## How a program takes it

A program takes xpute by writing four things, each over the part whose README
says how.

- **Its interfaces**, in WIT, as xpute declares its own under `wit/`. Its
  guest's world says which the guest implements, its exports, and which it
  calls, its imports. `xpute-link-gen` ([link/gen](./link/gen/README.md)) writes
  their bindings in Rust, TypeScript and C++, and for the host the lists
  `IMPORTS` and `CALLS`.
- **Its guest**, whose three exports one macro defines from a frame's cap, a
  boot that mounts what the guest implements, and what starts and ends each
  entry: `link!` in Rust ([guest/rust](./guest/rust/README.md)), and
  `XPUTE_LINK` in C++, which takes xpute's library as two files
  ([cpp](./cpp/README.md)).
- **Its host**, as a list of mounts, each mounting implementations of some of
  the guest's imports, with `IMPORTS` and `CALLS`, from which the runtime starts
  the guest by the steps [ARCHITECTURE.md](./ARCHITECTURE.md) gives in "The
  sides": the web worker runs it on `@xpute/runtime-web`
  ([runtime/web](./runtime/web/README.md)), and a native app links it, with the
  guest, into one Rust crate over `xpute-runtime-native`
  ([runtime/native](./runtime/native/README.md)).
- **Its UI**, which opens its end of the runtime, mounts on it what it
  implements, starts the guest and then calls it by the same bindings: on the
  web through `@xpute/runtime-web`, and natively through `@xpute/expo`
  ([expo](./expo/README.md)).

A plugin is a package of the program's own in WIT, whose bindings are written
as the program's are, over xpute's, and which the program's package may take
types of and its world import or export. Its guest's end, its hosts and its UI
ends are the program's, under the plugin's name.

The packages and crates are named in [ARCHITECTURE.md](./ARCHITECTURE.md)'s
codemap, those a program takes carrying what a registry asks, and
[USAGE.md](./USAGE.md) shows a program depending on them. React
Native and Expo are peers of the native module alone, so a program that runs
only on the web installs neither. Each TypeScript package is its own source:
its `package.json` maps a path under the package's name to the file under
`src/`, and the files use only syntax that strips to JavaScript, so a bundler
or Node's type stripping takes them with no build.

## Where to read next

- [USAGE.md](./USAGE.md) builds a program on xpute step by step, from its WIT
  to a call that crosses.
- [ARCHITECTURE.md](./ARCHITECTURE.md) follows the same claim through the
  link, its sides, how a failure ends and the languages, to a map of the code.
- Each part's `spec/` is the contract itself: the link's frame (`link/spec/`)
  and the errno table (`kit/spec/`), from which every language's numbers are
  generated. The interfaces are WIT, under `link/wit/` and `wit/`.
- [XTP's README](./kit/ts/src/wire/xtp/README.md) describes the format of
  XTP, the kit's tree packet.
- [CONTRIBUTING.md](./CONTRIBUTING.md) lists what xpute builds with and the
  commands that build and check it.

## License

xpute is MIT, and every crate and package carries the license beside its
manifest, as the C++ amalgamation's two files carry it at their head. The buddy
allocator under `alloc/`, in Rust and in C++, is a port of Evan Wallace's
buddy-malloc, and `THIRD_PARTY_NOTICES.md` carries the notice its license asks
for.
