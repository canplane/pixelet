# xpute's parts, cut by what they are and what they vary on

**2026-11-15** — recorded with the user. Main was at `c4e70e9feb` when this was
written. It follows `20261113-xpute-takes-wit-and-wasi-and-the-native-runtime-s-shape.md`.
Every item says who decided it: **the user**, or **the agents**. An agent's
plan is never written as the user's decision.

## What was found

- **The host names the runtime.** On a device the host serves on the
  runtime's link (`xpute-native-runtime` `link.rs`), takes its generated
  bindings from the runtime's `abi`, wakes through its `event_loop`, `job` and
  `done`, and logs and reaches JNI through it; on a page `@xpute/web-host`
  takes `tell_stop` from `@xpute/web-runtime`. The user set on 2026-11-14 that
  nothing references the runtime. That rule was written nowhere but
  `rule.rs`'s order, and the commit that split the device's host from its
  runtime (`12e5875916`) rewrote that order to fit its split. `xpute/ARCHITECTURE.md`
  then wrote the result as the design: the runtime holds the host's `Side`.
  `ARCHITECTURE.md` already held that an import never points up a level.
- **The host was cut by platform.** It varies by language: the TypeScript
  host uses only `performance.now`, `Date.now` and the timers, which every JS
  environment has, and the Rust host only `std`; all else it took from the
  runtime is what the runtime should inject.
- **The GPU was given a runtime.** To mirror xpute's guest, host and runtime,
  the plugin got a runtime part it does not have: `xpute-gpu-native-runtime`
  holds only its host's bindings, and its Expo module took the runtime's name.
- **A name hid what a part depends on.** The two Expo modules, the only parts
  that depend on React Native, are named `native-runtime`, the name of the Rust
  runtime too.
- **A list was made a crate.** `xpute-gen-targets` and `xpute-gpu-gen-targets`
  each hold two or three paths, read only by `xpute-tool`.

## How the agents erred

What the user reproved in the agents' work, to be kept clear of:

- Reasoning up from the code there now, which agents wrote, and taking its
  splits and leftovers as the structure, instead of down from the field's
  precedents: the component model and its embedders, an operating system's
  loader and drivers, React Native's native modules.
- Matching symmetry of form: a part made to mirror another's shape, a
  platform's detour copied onto the other platform.
- Breaking the principle that was written: a lower layer provides the
  interface, an upper one implements and injects it.
- Proposing new packages and crates unasked, and splitting where no artifact
  or dependency set differs.
- Saying a plan before checking what each part's users are, then changing it at
  each objection.
- Offering a trial run in place of a plan.
- Taking an earlier design (the 2026-04-05 switch and devices) as a model to
  adopt, when the user pointed at how it was designed, top-down, in a different
  topology.
- Closing each small correction by asking whether to proceed.
- Answering for one cell, Rust on a device, instead of every language,
  platform and shell the parts run in.
- Taking each correction as a new absolute rule, its opposite end, instead of
  checking it against the whole: the log sent to the kit, then the host, then
  the runtime; messages declared none, then nothing printed at all.
- Searching again for what was already established in the session.

## What was decided

- **The entities** (**the user**). xpute's parts are the kit, the link, the
  guest, the host, the runtime and a shell's UI end; a plugin repeats the link,
  the guest, the host and the UI end under its own name. The kit, the link, the guest and the host vary by
  language; the runtime by platform, a page's worker or a device's thread; the
  UI end by shell, the page's inside the web runtime and React Native's an Expo
  module.
- **Dependencies point down** (**the user**). A lower layer provides an
  interface; an upper one implements it and injects it. The runtime injects
  into the host what the host needs of it, the link's `Side`, a way back to the
  guest's thread, where a report goes and the log; nothing names a runtime but
  its own UI end and the program's assembly. A part's generated bindings sit at
  its end: the guest's in the guest, the host's in the host, and the link keeps
  `xpute:link`'s own types.
- **An error, a bug and a log record** (**the user**). Inside xpute there are
  three. An error (`XputeError`: its errno, the place it was made and an
  optional message) goes back to the caller. A bug (its errno, its place, two
  numbers and an optional message) is reported by the stop before the part
  stops; a panic that `bug` did not raise, the standard library's or a
  dependency's, is a bug too, which the panic hook reports with the panic's own
  message. A log record (a level, a context and a message) goes to the
  environment. Each end has its `log`: the guest's calls `xpute:log`, in
  `wasi:logging`'s shape, the host serves it and hands the records, with its
  own, to the log its runtime injects, and the runtime alone writes to the
  platform's output. The error's message and the warning sink went on
  2026-09-26 (`abfd011ee5`); the message comes back, and the log takes the
  sink's place. The GPU hosts' slow-entry lines go with `GpuConfig`'s
  `slow_ms`, which carried the program's measuring limit into the plugin.
- **xpute lays out no memory** (**the user**). xpute's job is that the core
  runs unmodified as a module or natively. A module's memory is its own, grown
  as its allocator asks; a native guest's is its process's; a lent span is named
  by its address, an offset in a module and a pointer natively. So xpute holds
  no region, no size, no share and no heap placement, and no scheduler: whether
  the memory is fixed, its layout and its allocator are the program's, which
  picks in its code the allocator each allocation takes. pixelet's fixed size,
  `.fixed` and buddy/slab heap are pixelet's. The slab over the buddy, cut from
  the guests' region and share, becomes the kit's heap over a span, a
  `GlobalAlloc` in Rust and a `memory_resource` in C++, a tool any program may
  pick; the language's own allocator hook is the only injection, so xpute adds
  no allocator socket of its own. The share entered xpute's ABI by name on 2026-11-15
  (`a6cc3dfb64`), the C++ guest's global `new` was the heap in a module only
  since 2026-10-01 (`9f3e716861`), and the Rust guest's `link!` took the global
  allocator everywhere: agents' choices, no minute's. The GPU plugin's frame
  admission is the plugin's.
- **WIT assumes one thread** (**the user**). Where the guest and the UI run,
  a worker and a page's main thread or a guest thread and the JavaScript's, is
  the runtime's; a WIT function means the same in every topology. A method
  that answers later is an `async func`, as the component model holds `async`
  an invariant (a callee of a function without it that blocks traps): so the
  methods the UI serves are `async func` in pixelet's WIT, and the runtime
  refuses to start where a function without `async` is mounted on an end over
  a port, where until now it told the UI and answered 0 at once.
- **An address is a u64 on every platform** (**the user**, on the agents'
  proposal). A lent span's address (`lent.at`) and the GPU stream's address
  and fence words name the caller's memory as a u64, an offset in a module and
  a pointer natively. The other way, a pointer's width per platform as the
  component model's memory64 widens it, was weighed and dropped: a packet would
  have two layouts, the golden records two sets, the generator a pointer size
  in every offset and the TypeScript a width read at run time. A u64 everywhere
  costs four bytes an address in a module and follows the systems precedent for
  a structure that crosses pointer widths: Linux's uAPI carries a user pointer
  in an ioctl's struct as a `__u64`, Vulkan's device address and WebGPU's sizes
  and offsets are 64 bits on every platform. `lent` is xpute's own record, so
  no standard is departed from anew.
- **The UI is a handle** (**the user**). The channel to the UI is a general
  one, and the UI one remote party on it: its connection is a handle the
  runtime opens, which every message names and whose drop clears what it
  opened and owed. The device runtime's "UI generation", a number threaded
  through every C ABI call to drop what an old UI sent, was that handle in
  disguise and goes. The WIT between the guest and the UI is unchanged. One
  remote at a time for now; several need only the runtime to hold a set, a
  queue per handle and one server per named interface, brought in when needed.
  The link does not know a remote either (**the user**): `Remote` beside
  `Side` told the link that a line ran past its end, which is the runtime's
  business, as a link layer knows its ends and not the wire. One end, one
  machine, over a medium the runtime plugs in (the guest's boxes and crossing,
  a port to the UI); a waiting call is the medium's property; the runtime
  bridges the two ends; handing an object beside a call (`give`) is a property
  of a medium that can move objects.
- **A size in bytes is `_sz` in both trees** (**the user**). The naming skill
  drew the line by tree, `_sz` in xpute and `_bytes` in pixelet; a size in
  bytes is one thing, so it is `_sz` wherever it is, and `size` stays whole for
  a size in another unit. pixelet's `_bytes` names are renamed with the moves,
  leaving names that copy an outside API (`to_le_bytes`) or a verb (`write_bytes`).
- **The GPU is the user's own plugin** (**the user**). It is not general: its
  stream covers what pixelet draws with, not all of WebGPU. It moves to pixelet
  as `pxl-gpu` now, after the memory step that widens its stream's addresses,
  and the link generator stops carrying it: its embedded WIT and its package
  tables name xpute's own packages alone, and a plugin brings its WIT and its
  packages' names to the generator as a program does.
  The runtime and the GPU stand at one layer and neither names the other; the
  program's assembly mounts the GPU's host among its servers, and the shell
  makes the canvas or the view and gives it to the GPU.
- **A package's name is its folder** (**the user**, on the agents' proposal):
  the owner's prefix, then the path with `/` as `-`, the language's leaf left
  out; the base word first and its qualifier after; no folder that only groups.

  | folder | name |
  | --- | --- |
  | `kit/{rust,ts}` | `xpute-kit`, `@xpute/kit` |
  | `link/{rust,ts}`, `link/gen` | `xpute-link`, `@xpute/link`, `xpute-link-gen` |
  | `guest/rust` | `xpute-guest` |
  | `host/{rust,ts}` | `xpute-host`, `@xpute/host` |
  | `runtime/web`, `runtime/native` | `@xpute/runtime-web`, `xpute-runtime-native` |
  | `expo` | `@xpute/expo` |
  | `gpu/link/{rust,ts}`, `gpu/guest/rust` | `xpute-gpu-link`, `@xpute/gpu-link`, `xpute-gpu-guest` |
  | `gpu/host/{rust,ts}`, `gpu/expo` | `xpute-gpu-host`, `@xpute/gpu-host`, `@xpute/gpu-expo` |
  | `gen`, `tool`, `golden/{rust,ts}` | `xpute-gen`, `xpute-tool`, `xpute-golden`, `@xpute/golden` |
  | `rust/client/host` | `pxl-host` |

  `xpute-gpu-native-runtime`, `xpute-gen-targets` and `xpute-gpu-gen-targets`
  go. The C++ stays one tree under `cpp/`, one library built by one Makefile.

## Next

In this order (**the user**: the documents and the rules first):

1. The documents and the rules: this record, notes beneath the 2026-11-13
   minute's lines it changes, `xpute/ARCHITECTURE.md`'s contract, `AGENTS.md`,
   the code and naming skills and the review agent. The documents' maps and
   the parts' names in them (`xpute/ARCHITECTURE.md`'s codemap, the READMEs,
   the device-build skill) change with the moves in 5.
2. The injection: the host's end of the link and the host's bindings move to
   the host; the runtime injects what the host needs; a page's stop is
   injected as well; the GPU's host takes the NDK itself. With it, the error's
   optional message in the three languages, the bug's in the stop, and the log
   (`xpute:log`, each end's `log`, the runtime's injected log).
3. The memory: xpute lays out none. `lent.at` and the GPU stream's address
   and fence words are a u64, a module's offset or a native pointer, and the
   golden records are written again; the module owns and grows its memory, and the page's host
   and UI read it through a view made at each access; the device runtime's
   region, `xpute_mem`, `mem_sz` and the share go, and a device UI reads a lent
   span through a buffer over its pointer; the guests' heaps and `link!`'s
   global allocator leave xpute; pixelet declares its own memory policy in its
   code. Built apart in a worktree while step 2 is reviewed.
4. The UI's connection as a handle, and the link's one end over a medium
   (`Remote` goes; the runtime plugs the boxes or a port and bridges), and
   the UI's methods `async func`, a function without it refused over a port: `xpute_start` answers the UI's handle,
   `xpute_post`, `xpute_answer` and `xpute_take` name it, a new start drops the
   old handle through the link's drop, and the generation goes; after the
   memory step, which touches the same runtime.
5. The moves and the renames of the table, the three crates removed, and the
   GPU's page-side state heard by the program's assembly.
6. `rule.rs`'s order set to the layers above, with the moves, so main passes at
   each commit.
7. Every target's `cargo check`, the TypeScript check and bundle, the
   generator's check, the rules, the Expo prebuild and a build on iOS and
   Android; the review and the name agents; then the commit.
