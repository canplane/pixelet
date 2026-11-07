# Agent Instructions

- Skills and this file are in English.
- The kernel is a unikernel on a virtual machine: `pxl-kernel` and
  `xpute-core`, its library OS, are one image, the core, run inside; the
  runtime is its VMM, which implements its devices and gives it its time. The
  shell is the client outside the VM; the rings are the device between them.
- Under `xpute/`: runtime, core and their contract only. No pixelet domain
  (tiles, the world, sensors) and no pixelet name.
- xpute's predefined ABI takes majors 0x00 to 0x0F (`xpute/spec/abi/`); the
  app's ABI takes majors from `PROGRAM_MAJOR`, 0x10
  (`rust/client/kernel/shell.json`). No ports.
- pixelet's layers are the shell (`ts/client/src/shell/`), the services
  (`ts/client/src/service/`, `port/`, `sys/`, `origin/`) and the runtime
  (`ts/client/src/runtime/`); the kernel is `rust/client/kernel`.
- The app is one Expo app, `ts/client` (`@pxl/client`), for iOS, Android and the
  page; the page is bundled by Metro. What differs by platform is a file pair
  in `ts/client/src/service/`: `<x>.ts` the page's, `<x>.native.ts` the
  device's, the shapes both share in `service/types.ts`; the pair that opens
  the runtime is in `runtime/`. A piece of the shell the platforms draw
  differently is a file and its override beside it
  (`ts/client/src/shell/ui/README.md`).
- The runtime gives no app number a meaning. What answers the kernel's files goes
  in `ts/client/src/sys/`: their names, versions, kept keys and refusals in
  `sys/fs/`, the stop report beside. What the app's ABI carries on the client
  goes in `ts/client/src/port/`, a file a major (SHELL's snapshot in
  `snapshot.ts`, `shell.ts` the shell's way in): the commands for the shell's intents
  and its input, the snapshot, the surfaces it lends and each turn's grant,
  what the place's sensors tell the kernel, the text the kernel calls for, the
  profiler. The server the client reads from (its routes, its published
  tables, whether the device is online) is `ts/client/src/origin/`. The
  numbers both read, generated from `spec/` or written beside it (a state the
  client reports and the shell shows, `abi/state.ts`), are
  `ts/client/src/abi/`; the boot is `runtime/`. None of it goes into
  `@xpute/runtime` or `xpute-runtime`.
- The shell computes no coordinate or tile and is not the runtime. It reaches the
  kernel, `sys/` and `origin/` through `port/` alone, and `port/` reaches the
  kernel through the generated stubs alone.
- xpute is to be published for other programs. Don't propose removing a part
  of xpute's `kit`, or its C++ core, because nothing here calls it.

## Reading and changing

- Read the skill the task needs (`.agents/skills/`). What done means:
  `.agents/working.md`.
- The code is agents' work, not the user's intent: read it as suspect. Decide
  what a part must do from the user's decisions, then judge the code against
  that; a comment, a structure or a pattern in it, however often repeated, is
  no reason to keep it.
- Keep an established symmetry by role and contract, not by matching module
  lists. A local gain alone does not weaken one or cross a boundary. A
  contract changed on purpose is named as such, with what keeps or replaces
  the properties it held.
- A comment is understood on its own, in whole sentences, without reading the
  code under it, and does not say the code again.
