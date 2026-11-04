# Kernel and shell, as an operating system means them

**2026-11-04** — recorded with the user. Main was at `e82319435`. The user asked
why agents mix host and guest, kernel and shell. The comments beside the
brackets named the shell for what the host does (the memory given, the
directory read, `run` rung); port 255, `SHELL`, carried the shell and the
drivers alike, so one word meant the UI in `shell/` and the whole client on the
rings; and xpute had grown a second layer beside its base, with its own words
for the same acts.

## What the user decided

- **Kernel and shell mean what they mean in an operating system**, read no
  further than that: the kernel is the core, the shell the UI. In friendlier
  words, core and UI.
- **A port is a scheduling unit.** What runs on one loop shares one port; the
  shell and what the platform's sensors, GPU and visibility tell run on the
  platform's event loop, so they share 255. Splitting them by role would give
  two numbers to what talks directly anyway.
- **1 and 255 are pixelet's names**, not xpute's. To xpute's host every port
  but 0 is the guest's; 0 is the host's control plane.
- **The core's way in and out is its ABI alone.** A sensor or an actuator means
  nothing to it; how raw the ABI is, a sensor's own terms or a cut at the
  level the core needs, is how the core is written.
- **`driver/` mixed two things, so it goes.** What answers the kernel's
  hostcalls and what the shell's port carries are apart.
- **The base comes first: it stands right, its names fit, and its ends are
  symmetric.** An easier surface is made in the base itself where that keeps
  the rings and no bindgen; where it cannot be, a layer goes over the base, as
  libc's functions go over the system calls: the same words, the same
  meanings, the same formats. A layer whose words mean something else, or
  whose formats split from the base's, is not one.
- **The rings are RPC between peers, and xpute is the bus.** As a PCIe fabric
  has a controller and a device on each port, the host is the controller and
  each port holds a guest; to xpute 1 and 255 are peers, and only the guests
  rank themselves by role. Every end speaks the same verbs: `send` (taken on
  the other's turn), `call` (answered before it returns), `on` (a handler
  bound), `receive` (what came, handed on). `Door` and `Doorbell` stay: they
  name the substrate's two ends, where host and guest do rank.
- **No shortcut beside the rings.** As Unix keeps one way, the hostcall stream
  goes: a hostcall is a frame for the host's port, 0, on the completion ring,
  and a question answered before it returns is a call to the port that answers
  it, laid unpublished in the ring's next slot. The user left it to judgment
  whether it went; it went.
- **The package puts itself together.** A program says what only it can — its
  guest, its hostcalls' answers, its ports, its policy, when turns may run,
  what to do when the guest stops — and the order the host is assembled in is
  xpute's. The user's sketch of it was `xpute.boot(topology)`, an example and
  not a name; the session widened the base's `mount` rather than stand a new
  entry beside it.
- **xpute's program layer is taken out.** It was built on 2026-11-01 without a
  record of the user's, beside the base rather than over it: `call` meant
  "now" on the host's `Port` and "after the turn" on the guest's `Turn`;
  `Port` took packets only in forms of its own and threw on an errno, so
  pixelet, which lays its memory out itself, could not use it and reached past
  it to the doorbell.

## What was weighed and not taken

- `CLIENT` for 255: it reads as a server's client.
- `USERLAND` for 255: an operating system's line between kernel and user land
  is privilege, and here it runs between the host, 0, and every guest, the
  kernel among them.
- The shell as everything around the kernel, its devices too (functional core,
  imperative shell): no operating system reads a shell that way.
- `PLATFORM`, `PLACE`, `OUTSIDE`, `LOOP` and `CORE` as port names: with 255 the
  UI, `KERNEL` and `SHELL` stand.

## What was done

- `driver/` taken apart (`ce6ce0b37`). `ts/client/src/sys/` answers the
  kernel's hostcalls, as `cfg.sys` hands them to `@xpute/host`: `sys/fs/` the
  files, `sys/text.ts`, `sys/stop.ts`. `ts/client/src/origin/` is the server
  the client reads from. `ts/client/src/port/` carries the shell's port: a file
  for each major but SHELL, whose snapshot is `snapshot.ts`, with `shell.ts`
  the shell's way in and `SYS_BOOT` with the boot in `abi/boot.ts`.
- The comments that gave the shell the host's work, in the brackets,
  `mem.json` and the kernel's profiler, say the host, the page or the client.
- The program layer out of xpute: `program!` and `XPUTE_PROGRAM`,
  `spec/program.json`, `@xpute/host`'s `open` and `Port`, the C++ example, the
  example's table, xpute-tool's `cmd` binary and the program branches of its
  emitters, `LinkConfig::DEFAULT` and `Files::try_open`. The Rust example and
  its host test are again a guest that lays its range out on the base, as
  before `e3be4b371`.
- What every program wrote around the strobe is the base's: `resume` and
  `pause` are no-ops where they already hold, and the mount halts the strobe once the
  guest stops and tells the program (the mount's `stopped`). The client keeps only
  its two facts, the world's view open and the page seen.
- The architecture's figure keeps the shell to the UI; its port is the shell's
  event loop, which the sensors, the GPU and the visibility are heard on too.
- The verbs, on both ends: the guest's `Sys::call` is `send`, `call_now`
  `call`, `try_call` `try_send`; `Door::raise_to` is `send`, `raise`
  `send_with`, `drain` `receive`; the host's `Doorbell.listen` is `on`,
  `Sys.call_now` `call`, and `Host.now` folds into `Host.call`, whose answer
  none reads is logged on a failure as the doorbell logs one. pixelet's port
  speaks through `HOST.send` and `HOST.call` alone, but for the frame each
  turn's submit writes.
- The hostcall stream out of xpute: `SYS_OPENAT`, `SYS_READ`, `SYS_WRITE`,
  `SYS_CLOSE` and `SYS_ABORT` are xpute's own commands, posted for port 0
  (`Door::openat` and the rest; the guest's `Sys` is gone), and the host's
  `Sys` is port 0's handler. A call is `Door::call(port, &event)`, answered by
  the port's handler through `Doorbell.answer`; the toc's `SYS` key is
  retired, `spec/ipc/sys.json` keeps the open flags, and the C++ guest, the
  conformance guests (`CALL` and `ANSWER` where `HOSTCALL` was, `call.tsv`)
  and the example follow. A report that would not fit a slot keeps its path's
  end, or none.
- pixelet: the kernel posts its files through the door and its report as
  `SYS_ABORT`; its text is `TEXT_LAYOUT` and `TEXT_RASTER`, a major of its
  own, called on the shell's port and answered in `ts/client/src/port/text.ts`
  (was `sys/text.ts`). The door's reserve counts the files' frames a turn
  (`host::FILE_FRAMES`) and a call's slot; the 64 KiB stream left the fixed
  ranges. The command table's sections say the shell where they said the
  host. The built page in headless Chrome booted, read its tiles and
  assets through port 0 and drew the world.
- `mount` takes the whole of it: `toc_in_turn` (it runs the turn that writes
  the toc), `ports` (each port's handler, never 0), `stopped`; `Host` gives
  `wake`, `resume` and `pause` beside `send` and `call`, and no longer the
  strobe. A kernel that stops in its boot shows the stop code, as one stopped
  later does.
- Names, after a review of them: the turn switch is `pause` and `resume`,
  since "stop" is a guest's end everywhere here; what reads a call's answer is
  `read` (was `take`); the kernel's handle on its host `Host` (was `Hosted`);
  conformance's question `INC` (was `ANSWER`); the door's kept buffers
  `writes`; `ABORT_PATH_BYTES`.
  `@xpute/host` has an entry (`src/mod.ts`), which pixelet imports alone; its
  deep paths stay for xpute's own packages. The web and native openers no
  longer run the first turn; the client's boot declares its port and its stop
  in the mount, and nothing in it reaches the strobe or binds a port after.
- Kept, as the base's: what the same days moved down into it and pixelet uses,
  `stop`, `Opened`, `Door::raise_to` with `Event`, `OwnedRange`, and the
  command table's generator (`spec::cmd::table_outputs`), which writes
  pixelet's table.

## Open

- The base's verbs beyond `send`, `call`, `on` and `receive`: the doorbell's
  own (`ring`, `cmd`, `writer`, `fits`) and the guest's `send_with`.
- The client still opens its guest through the bracket and keeps `HOST` and
  `PACING` for the port's modules (`host/mount.ts`), and the web bracket plays
  the GPU stream after each turn and the GPU's wake asks `HOST.wake`: the
  GPU host is the platform's, which `mount` does not yet take.
- The kernel's sensor ABI stays raw: it takes the platform's permissions, its
  gating and its error codes.
- Checked by format, lint, type, `cargo spec`, xpute's whole suite (its
  conformance traces on every guest and the example among it), the kernel's
  and the client's tests, and the page in headless Chrome; a device build was
  not made.
