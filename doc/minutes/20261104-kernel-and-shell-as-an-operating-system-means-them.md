# xpute is the core's way out of JavaScript

**2026-11-04 to 2026-11-05** — recorded with the user, over one long session
in which two agent sessions worked xpute's guest and host side by side on
`main`. It began as "kernel and shell, as an operating system means them" and
as a reduction of xpute so a program writes no glue; it ended by reframing
what xpute is. The day's earlier positions that the reframing overturned are
kept below as such.

## Why xpute exists, in the user's words

- JavaScript is not to be the first-class citizen. Compute runs in
  WebAssembly; JavaScript keeps I/O and the UI. The core and the UI layers are
  apart, each in the language that suits it, and the leadership is the
  core's. The kernel and the shell of an operating system fit that split.
- No FFI for each piece of the ABI, and no dependence on FFI: a ring buffer
  sits between the two, which made it io_uring's shape. An asynchronous path,
  then a synchronous path behind it, then a path for the GPU.
- On the web the two share one thread, so they run cooperatively: hence the
  strobe. xpute is the runtime that governs this picture.
- Secondary: workers for small offloads are hard to govern from the core,
  since WebAssembly is isolated; like the GPU, a worker needs JavaScript's
  help, so xpute was to bear a worker's birth and death and its slab memory.
- Later: on React Native as on the web, the kernel and the shell need no
  change, since xpute was already between them.

## What the user decided

- **xpute is a tool for the core's first-class citizenship: an intentional
  asymmetry.** It is not to be general or symmetric. It is the glue that lets
  the web and native run the same code, governs the io_uring-shaped rings, and
  makes a program think from the core; the core talks to what lies outside it
  (the UI, the GPU, workers later) by its ABI.
- **Ports are not needed.** Generalizing workers or threads into ports was a
  trap. The precedent for what lies outside the core is the GPU: a worker is
  handled as the GPU is, a handle in the ABI's arguments. The worker's
  lifecycle ABI is opened when one is needed; GPU and worker share that both
  lie outside WebAssembly.
- **The rings stay**, as the user's choice and as generally useful.
- **Web and native are symmetric.** App logic is never written twice for the
  two targets.
- **Zero copy** between the core and its host, so the model stays one.
- **The GPU is a resource the machine asks for and the host grants**, as
  memory is: `machine.json`'s `gpu` (bytes, power, features), the host opening
  the device as asked, its readiness told to the guest as `GPU_READY` on
  xpute's own control commands. Not a device reached by a queue command.
- **The guest's life is xpute's control**: `SYS_ATTACH` (a host attached, the
  first time or again), `SYS_SUSPEND` and `SYS_RESUME` (calls), beside
  `SYS_ABORT`. pixelet's `SYS_BOOT` and `SYS_SLEEP` went; the platform facts
  they carried are pixelet's `SENSOR_PLATFORM` and `SETTING_DEVELOPMENT`.
- **The turn's edges are not commands.** Pseudo commands for them in the
  command table were weighed and not taken (see below).
- **Generated code is committed**, written by `cargo spec` and checked by it,
  as every generated output here is; a declaration lives in the guest crate it
  declares.
- **The GPU stream's size is the machine's; its head the stream says itself**
  (word 1, the head's length).
- **The native form is xpute's** (`native_guest!`, JNI under `xpute.Guest`
  and `xpute.Gpu`, `xpute.h`), so pixelet's native crate keeps only what is
  pixelet's.

## Later, after midnight: the layers and their names

The user set aside kernel, shell, host and guest as the app's layer names, then
took host and guest back, since nothing now gives a reason to ignore
WebAssembly's context. The user decided:

- **xpute and pixelet speak one language**, close to each other; the symmetry
  of a general runtime is not sought. **host and guest mean what they mean in
  WebAssembly** (wasmtime, wit-bindgen): the guest is the core that runs
  inside, the host what runs it. `xpute-guest` and `@xpute/host` keep their
  names.
- **Convention over invention.** Outside the guest there are three parts:
  - **host**: opens the guest, runs its turns, rings, memory and GPU; xpute's,
    which an app calls once (`boot`);
  - **ui**: the screens and components;
  - **services**: what answers the guest's app ABI, and the device's
    features (position, storage, files, text, the server).
  No separate device layer: as in React Native and Expo apps, device access
  lives in services. The UI-layer / data-layer split of Android's app
  architecture guide is the nearest precedent.
- **A platform's difference is a file's extension**, type-checked by one
  tsconfig per platform (`moduleSuffixes`). The client's `Bracket`, injected
  at boot, goes. As done: `x.ts`, the page's and the default, beside
  `x.native.ts` (the native tsconfig's `moduleSuffixes: [".native", ""]`),
  since a `.web` suffix also picked libraries' web typings under
  `node_modules` and broke the shell's types; stock Metro, Vite, vitest and
  tsc resolve it.
- **Predefined ABI and the app's own ABI are apart**: xpute's commands
  (`xpute/spec/abi/cmd.json`) and the app's (`shell.json` today). Whether a
  feature such as the position is defined in the app's ABI is the app's
  business; the counterparts do not change.
- **What another app can reuse is xpute's**, what only pixelet uses is
  pixelet's. So xpute ships the React Native native module (Kotlin, Swift,
  JNI, headers, the core's build for iOS and Android) an app installs, and the
  guest's form is picked by its target (`guest!`), so no per-app bracket crate
  is needed.
- **The web moves to Metro**, as Expo's convention has it, as its own step.
- **xpute is told in io_uring's terms, the platform's schedule its own.** The
  contract is io_uring's two rules: the position words are published and read
  with release and acquire, and a frame's buffer is the receiver's until its
  completion. The strobe is the web host's policy, the page's event loop; a
  native host runs the guest on its own thread.
- **One view, the guest's, as WebAssembly has it.** USB's out and in went with
  the bus. A protocol's commands are the guest's **exports** (the host calls
  them) and **imports** (the guest calls them), as a WIT world says.
- **The rings stay SQ and CQ, by io_uring's roles.** The side with the UI,
  where the outside's interrupts arrive, submits: the host writes SQ and the
  guest completes on CQ, as io_uring's application and kernel do; the guest's
  own requests ride CQ in ublk's way, and their results come back on SQ.
  Renaming the rings rx and tx by the guest's side was weighed and dropped. The
  host keeps laying the front, the way with less boilerplate.
- **The stack reads ui → core → host.** io_uring's SQ → CQ is the ui → core
  relation: the ui submits to the core as an application to its kernel, the
  core completes, and its rarer pre-emptive messages to the ui are upcalls; the
  app's ABI is this relation. The host is best read as the core's hardware,
  its HAL (the memory, the clock and the turns, the GPU, the storage): the core
  calls it, and it raises interrupts (attach, suspend, resume, the GPU's
  readiness, an open's answer); xpute's predefined ABI is this relation. host
  and guest stay the names. Both relations ride one ring pair, split by major,
  and the ui and the host share one JavaScript runtime, which is how it is
  carried, not the model. The virt-viewer picture weighed before it was set
  aside for this.
- **To a core's author the outside is one world**: whatever its destination,
  a message leaves the core; the world's commands are the core's imports and
  exports.
- **The order is the agents'**: xpute is fixed first and proven on its example,
  then pixelet is moved onto it at once, then the web onto Metro.

What the session found in the code while this was settled:

- The web and native apps split by history, not by design: the web app (Vite)
  came first, and the device app (2026-09-26, `99510ee24`) ran the web's shell
  under Metro with `@platform/` pointed at the device's side.
- `expo-horizon-location`'s web implementation does not cover what pixelet's
  page needs: its watch drops every error, the heading is unsupported on the
  web, and nothing ends an unanswered prompt. Whether React Native absorbs a
  sensor's fragmentation decides where the position goes.
- The GPU is today across three parts: the guest writes the stream, xpute
  grants and runs it, and pixelet binds the views and keeps the native turn's
  edges and the device's numbers. The edges and the surface format are
  xpute's to take.

## Overturned the same day

Taken in the morning and given up by evening, kept here so no record reads
them as standing:

- "A port is a scheduling unit", "1 and 255 are pixelet's names, to xpute
  peers", "the rings are RPC between peers and xpute is the bus", one set of
  verbs on every end. The reframing drops ports and the bus.
- Each protocol in its own file with a well-known port, and port 0 split into
  the host's control and a file device on port 1. The commands stay
  per-protocol files today; the reframing puts the core's ABI back in one
  table.
- The GPU as a device on port 2, reached by `GPU_QUEUE`.

## What was weighed and not taken

- **Pseudo commands for a turn's two edges**, named in port 0's table and
  refused on the wire. Practice keeps what crosses the wire and what a trace
  observes apart (KVM's `kvm_entry`/`kvm_exit` and exit reasons, strace's
  `--- SIGCHLD ---`); the conformance traces and the profiler already name
  turns; a turn renders as one span, `run(quota) = wake`.
- **Tauri** as the native host. Its IPC does not give the zero copy the user
  wants, and it does not run a core in a page.
- **A general worker or thread abstraction, M:N placement, an actor runtime.**
  None is wanted now; what is kept is that nothing in the contract fixes where
  the core runs.

## What the agents proposed, not decided

- xpute's essence as a **turn contract**: the core runs only in turns its
  host grants; everything it posts (frames, the GPU stream, memory a frame
  names) is taken after the turn returns and before the next, and the turn's
  edge is where that memory comes back. The test for a part's place: does
  removing xpute make every path invent it again?
- **Owner and leader are two axes**: the core owns the policy, the host owns
  the platform's resources.
- **The toc shrinks to a check word**: the front's addresses follow from the
  machine on both ends, so the directory repeats them; MAGIC and VERSION stay
  to refuse a guest built for another machine.
- **Platform symmetry is of the observable contract** (the ABI, the rings, a
  request's and its completion's order), not of threads or schedules; the
  strobe may then be the web host's policy rather than xpute's law.

## Open

- **Native need not be cooperative.** On React Native the core can run on its
  own thread and has no reason to wait for the host; io_uring itself does not
  schedule its user. Whether `run(quota) → wake` stays the contract on native,
  and what the turn's boundary implicitly guarantees today (memory handed back,
  order), which a concurrent native would have to make explicit (atomic
  publish and consume, an explicit release).
- **Removing ports** in the code, after the reframing: one ABI table for the
  core with xpute's reserved majors, a frame's tag without a port, the
  per-protocol ports and `x_out(host, port)` gone.
- **The files**: left to the agents, by whether another app reuses them. Taken:
  xpute's, as predefined ABI and the host's server, since roots, paths and
  handles mean nothing of pixelet's.
- **The toc**: left to the agents. Taken: it shrinks to a check word, MAGIC and
  VERSION, the addresses following from the machine on both ends.
- **`AGENTS.md`** still sets kernel and shell as the vocabulary and keeps
  pixelet's words out of `xpute/`; it is rewritten with the documents.
- **Zero copy's scope**: between the core and its host's memory, not the GPU
  upload or the network's buffers.
- The C++ guest's place: settled by the user. It stays because xpute is meant
  to be published next, for other programs, the same reason `kit` is kept.
- xpute's README and ARCHITECTURE, behind all of this.

## What was done (uncommitted on `main` at the end)

- **The machine** (`machine.json` in the guest crate, its schema
  `xpute/spec/machine.schema.json`): memory, stack, rings, the GPU's request.
  xpute lays the front out from it (`xpute_guest::machine`: stack, toc, rings,
  slots, GPU stream), writes the toc on the first turn, checks the budget at
  compile time, and gives the link arguments. `guest!` and `native_guest!` are
  a module's and a native library's whole form.
- **The generator**: `xpute-gen` and `cargo spec`; each protocol file gives
  messages (encoders, readers, enums) and bindings (`x_out(host)`, `XIn` and
  `x_in` on the host; `x::Client`, `Out` and `apply` on the guest).
- **xpute's own commands**: `NOP`, `SYS_ATTACH`, `SYS_SUSPEND`, `SYS_RESUME`,
  `GPU_READY`, `SYS_ABORT` (`spec/abi/cmd.json`); the files' calls in
  `spec/abi/fs.json` and the open flags in `spec/ipc/fs.json`. The guest takes
  them as `Submission::Life`, `GpuReady` and `Opened`.
- **The stream** says its own head; the GPU hosts read it from there.
- **pixelet**: `rust/client/kernel/machine.json` and `shell.json` replace
  `cmd.json`, `port.json`, `toc.json` and `mem.json`; the kernel applies the
  shell's commands through the generated trait, keeps its life in
  `door/life.rs`, and draws through `SCENE_DRAW` (which replaces
  `SURFACE_OPEN`'s role and `SCENE_PREVIEW_OPEN`/`CLOSE`); `TURN_GRANT`
  replaces `SURFACE_FRAME`. The client boots through `@xpute/host`'s `boot`
  and calls the generated stubs (the host session's part).
- **Native**: `native_guest!`, xpute's stop hook, JNI and headers; the device
  asked as `MACHINE.gpu` says (`Ask::of`).
- Checked by format, clippy (iOS, Android and wasm32 included), types, lint,
  the C++ compile and `cargo spec`; no test, GPU run or device build, by the
  user's rule for the day.
