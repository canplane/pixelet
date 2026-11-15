# xpute

xpute is a foreign function interface for the compiled part of a program. The
program's **guest**, its compiled code, and xpute's **host**, which serves it,
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
under the object's handle. A frame crosses into the guest by one export and out
of it by one function, the same on every platform, and each crossing returns at
once what the method came to: its result, or word that the answer will come
later in a frame of its own. Each method is declared in WIT and its bindings are
generated, so no binding is written by hand for any one piece of work.
[ARCHITECTURE.md](./ARCHITECTURE.md) lays out the frame and the crossings.

The host gives the guest only its entries, the clocks, the stop and the log.
The guest's memory is its own, and the guest decides everything else. Because
that is all the host gives, the same guest runs unchanged in a page, on a
phone and in a test.

## The guest, the host and the program

A program built on xpute has three parts.

- The **guest** is the program's compiled code, linked with xpute's guest
  library into one image. It runs only inside an entry, and everything it says
  to the world it writes into its memory.
- The **host** serves the guest the clocks, the stop and the log,
  `@xpute/host` in TypeScript and `xpute-host` in Rust, and a plugin the
  program brings serves beside it. It runs in a runtime, the platform's
  environment that runs the guest and enters it: `@xpute/runtime-web` in a
  page's worker, and on a device `xpute-runtime-native`, on a thread of its
  own, which the Expo module `@xpute/expo` starts.
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

## How a program takes it

A program takes xpute by writing four things, each over the part whose README
says how.

- **Its interfaces**, in WIT, as xpute declares its own under `wit/`. Its
  guest's world says which the guest serves, its exports, and which it calls,
  its imports. `xpute-link-gen` ([link/gen](./link/gen/README.md)) writes
  their bindings in Rust, TypeScript and C++, and for the host the lists
  `IMPORTS` and `CALLS`.
- **Its guest**, whose three exports one macro defines from a frame's cap, a
  boot that mounts what the guest serves, and what starts and ends each entry:
  `link!` in Rust ([guest/rust](./guest/rust/README.md)), and `XPUTE_LINK` in
  C++, which takes xpute's library as two files ([cpp](./cpp/README.md)).
- **Its host**, as a list of servers, each mounting what it serves, with
  `IMPORTS` and `CALLS`, from which the runtime starts the guest by the steps
  [ARCHITECTURE.md](./ARCHITECTURE.md) gives in "The sides": a page's worker
  runs it on `@xpute/runtime-web` ([runtime/web](./runtime/web/README.md)),
  and a device links it, with the guest, into one Rust crate over
  `xpute-runtime-native` ([runtime/native](./runtime/native/README.md)).
- **Its UI**, which opens its end of the runtime, mounts on it what it serves,
  starts the guest and then calls it by the same bindings: on a page through
  `@xpute/runtime-web`, and on a device through `@xpute/expo`
  ([expo](./expo/README.md)).

A plugin is a package of the program's own in WIT, whose bindings are written
as the program's are, over xpute's, and which the program's package may take
types of and its world import or export. Its guest's end, its hosts and its UI
ends are the program's, under the plugin's name.

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
  link, its sides, how a failure ends and the languages, and ends with
  xpute's boundaries, what it took from earlier work and a map of the code.
- Each part's `spec/` is the contract itself: the link's frame (`link/spec/`)
  and the errno table (`kit/spec/`), from which every language's numbers are
  generated. The interfaces are WIT, under `link/wit/` and `wit/`.
- [XTP's README](./kit/ts/src/wire/xtp/README.md) describes the format of
  XTP, the kit's tree packet.
- [CONTRIBUTING.md](./CONTRIBUTING.md) lists what xpute builds with and the
  commands that build and check it.

## License

xpute is MIT. The buddy allocator under `alloc/`, in Rust and in C++, is a port
of Evan Wallace's buddy-malloc, and `THIRD_PARTY_NOTICES.md` carries the notice
its license asks for.
