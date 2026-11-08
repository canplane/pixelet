# xpute

xpute is a small embedded kernel for the compiled part of a program, as a
microcontroller's chip is for its firmware. The program's **core**, its compiled
code, owns nothing but its memory, and a **runtime** gives it its time, one turn
after another. The runtime implements only the devices that lend the core's
memory across turns, the GPU and the files. Everything else belongs to the
**program**, and it travels over one general channel whose messages xpute does
not interpret, as a UART carries bytes it does not read.

The same core runs as a WebAssembly module in a web page and as a library linked
into an iOS or Android app, while the program's JavaScript does the I/O and
draws the UI around it. xpute is both ends of that arrangement, the core's
library and the runtime, and `kit`, the generic code beneath them.

## Why a kernel this small

A program whose logic is compiled and whose surface is JavaScript usually
crosses between the two languages once for every piece of work: a binding for
each function, a copy for each argument, and a different binding on each
platform. Every crossing costs time, and together the crossings become the
program's real interface, scattered through glue code.

xpute replaces those crossings with one memory that both sides share and a pair
of rings inside it. Three things cross between the core and the runtime: the
memory, one call into the core that runs a turn, and the number that call
returns, which says when the core wants its next turn. Two calls go the other
way, and each is answered before it returns: the runtime's clock, and a question
the core needs answered within its turn. Everything else is a message written
into the shared memory.

That is the shape of firmware on a microcontroller. The chip gives its firmware
RAM, a timer and a few peripherals, and the firmware decides everything else.
The core is the firmware and xpute's runtime is the chip, and because the chip
is this small, the same core runs unchanged in a page, on a phone and in a test.

## The core, the runtime and the program

A program built on xpute has three parts.

- The **core** is the program's compiled code, linked with xpute's core library
  into one image. It runs only inside a turn, and everything it says to the
  world it writes into its memory.
- The **runtime** is xpute's TypeScript, `@xpute/runtime`, with Rust beside it
  on a device. It reserves the core's memory and lays out its front, rings each
  turn, reads what the turn left, and runs the GPU and the file device.
- The **program** is everything else the app decides: what its commands mean,
  what backs its files, when its turns should run, and its UI.

```text
  ┌──────────────────────────────────────────────────────────────┐
  │ the core: the program's code, built on xpute's core library. │
  │ It owns one memory, which holds the rings, and it runs only  │
  │ inside run(deadline).                                        │
  └───────────┬─────────────────────────────────┬────────────────┘
              ▲                                 │
              │ submission ring:                │ completion ring:
              │ what the core is sent           ▼ what the core says
  ┌───────────┴─────────────────────────────────┴────────────────┐
  │ the runtime: lays out the memory, rings each turn, and       │
  │ routes each frame by the major of its command                │
  └───────────┬─────────────────────────────────┬────────────────┘
              │ majors 0x00 to 0x0F             │ majors 0x10 and up
    xpute's own: the turns, the             the program: its handlers
    core's life, the GPU, the files         and its UI, which xpute
                                            does not read
```

Each frame on the rings carries a command and a packet, the command's data in
XTP, xpute's tree format. The high byte of a command's number is its major, and
xpute delivers a program's frame by that major and reads nothing of what it
says. xpute's own commands, under the majors below 0x10, tell the core about its turns and
its life and drive the two devices.

## How a program uses it

The smallest program ships in this tree and shows every piece: a core that
answers one command (`rust/example`), the runtime that drives it in a test
(`ts/example/sum.test.ts`), and a page that runs it in a worker
(`ts/example/web`).

### The machine

A core declares its **machine** in a `machine.json`: the sizes of memory a
runtime may give it, its stack, the capacities of its two rings, and which of
xpute's devices it asks for. Both ends compute the same layout from it, so the
runtime lays out the front of the memory exactly where the core expects it, and
the core checks that before it runs. The example's machine asks for two
megabytes, small rings and no devices:

```json
{
  "mem": {
    "bytes": [2097152],
    "region_bytes": 2097152,
    "min_heap_bytes": 65536
  },
  "stack": { "module_bytes": 65536, "thread_bytes": 1048576 },
  "rings": { "slot_bytes": 256, "sq_cap": 8, "cq_cap": 16, "turn_posts": 1 }
}
```

A machine that wants a GPU or files adds `gpu` or `fs`. Its schema is
`spec/machine.schema.json`.

### The protocol

What the program says over the rings is its own **protocol**, a JSON file of
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

The **generator** writes both ends' code from the machine and the protocol
files. `xpute-gen machine.json asker.json` writes the machine's constants for
both ends and, for the protocol, on the runtime the fields each export takes,
by its number, the packets' readers and a handler for each import, and on the
core a function for each import and a trait for the exports. The generated
files are committed, and `--check` fails where one is stale.

### The core

The core crate holds the generated code and invokes one macro, `core!`, which
defines the core in the form its target takes: a WebAssembly module on `wasm32`,
and a native library anywhere else. A module's build script links it from the
machine with one call, so that it imports its memory, puts its stack first and
places its data past the front. Off `wasm32` the macro expands to
`xpute-runtime`'s native form, so the crate depends on `xpute-runtime` for every
other target, and on `xpute-gpu` with its `runtime` feature there where the
machine asks for a GPU. The core's entry runs one turn. The example's turn hands
each waiting submission to the door, the core's end of the rings, and answers
its own commands through the generated `apply` and each of xpute's with 0:

```rust
xpute_core::core!(machine::MACHINE, 0, entry);

impl asker::Exports for Example {
    fn sum(&mut self, c: abi::Sum) -> Outcome {
        let answer = asker::ask().map_err(MarshalError::new)?;
        Ok(Some((c.a + c.b) as i32 + answer as i32))
    }
}

fn entry(_deadline_ms: f64, _boot: fn() -> BootInfo) -> f64 {
    xpute_core::machine::door().turn(|s| match s {
        Submission::Program(cmd, pkt) => asker::apply(&mut Example, cmd, &pkt),
        _ => Ok(Some(0)),
    })
}
```

What the entry returns says when the core wants its next turn: 0 for as soon as
possible, a number of milliseconds, or a negative number for none until
something arrives. The door's `turn` returns 0 while submissions are left, and
none otherwise.

### The manifest

On the JavaScript side the program declares its core once, in a **manifest**:
the generated machine, and a function that makes the core's configuration where
the core runs. The configuration holds what the program decides: where a page
loads the module from, what answers the program's imports, its scheduling policy
and what it submits with each turn, and, where the machine asks for them, what
backs the files and which GPU the core is granted.

```ts
export const manifest: Manifest = {
  machine: MACHINE,
  core: () => ({
    core: {
      module: new URL(CORE, import.meta.url),
      bytes: MACHINE.mem_bytes[0],
    },
    program: asker_imports({ ask: () => ANSWER }),
    policy: new Timeslice(4),
    submit: () => {},
  }),
};
```

`boot(manifest, ui)` brings the core up beside its caller, as the test does in
Node, and `ui` says what the UI hears of the core: the imports it takes, the
GPU's state and the core's stop. A page keeps its turns off the thread that
draws: it calls `start(manifest, ui, worker)`, the worker's module calls
`serve(manifest)`, and a message port between them carries frames. Either way
the UI receives a **link**, the same on every platform, through which it sends
commands, asks for turns, says whether it is shown, and lends the GPU its
canvases. A call waits on a turn, and the turns run in the worker, so across the
port the page requests `SUM` instead, and the reply comes with a later turn:

```ts
const link = await start(
  manifest,
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
The module builds that crate for the device as the app builds, and links it in.
The app's bundler takes each `x.native.ts` in place of `x.ts`, so the same
manifest boots the core beside the UI, on a thread of its own, with no worker.

A C++ core takes xpute's library as two files, as SQLite ships its own: it
includes `xpute.hpp` and compiles `xpute.cpp` once, as C++20 with no exceptions,
no RTTI and no floating-point contraction, the flags `cpp/Makefile` builds with.
`make -C cpp amalgamation` writes the two files under
`cpp/target/amalgamation/`, and xpute's own repository carries them at
`cpp/amalgamation/`.

### The packages

In TypeScript the parts are `@xpute/kit`, `@xpute/runtime`, `@xpute/gpu` and
`@xpute/native`, and in Rust they are the crates `xpute-kit`, `xpute-core`,
`xpute-runtime` and `xpute-gpu`. None is published to a registry. A program
beside this tree lists the packages in its pnpm workspace and depends on them as
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
- `spec/` is the contract itself: the header, the frame, the rings, the stream
  and the devices' commands, from which every language's numbers are generated.
- [XTP's README](./ts/kit/src/wire/xtp/README.md) describes the packet format
  that every frame carries.
- `rust/example` and `ts/example` are the example above, whole.

## Running it

xpute builds with Node at the version `ts/.tool-versions` names, with pnpm;
rustup, which installs the toolchain `rust/rust-toolchain.toml` names; a C++20
compiler, as `c++` or `$CXX`; GNU Make; and, for the C++ core built as a module,
the [WASI SDK](https://github.com/WebAssembly/wasi-sdk) at `/opt/wasi-sdk` or
`$WASI_SDK_PATH`.

```sh
make install       # ts/'s packages, with pnpm
make -j8 check     # formats, lints, types, the C++ compiled, spec/'s files
make -j8 build     # the cores the traces drive, and the example's
make -j8 test      # every tree's tests, the traces, the example
make -j8 test-gpu  # the GPU runtimes, on wgpu and in Chrome ($CHROME)
make fmt           # prettier and cargo fmt
make spec          # the files generated from spec/, written
```

`make check-ts`, `make test-cpp`, `make test-rust` and the like run one tree's
part. `./script/export.py --to <checkout>` copies this tree into xpute's own
repository.

## License

xpute is MIT. The buddy allocator under `alloc/`, in Rust and in C++, is a port
of Evan Wallace's buddy-malloc, and `THIRD_PARTY_NOTICES.md` carries the notice
its license asks for.
