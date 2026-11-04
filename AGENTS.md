# Agent Instructions

- Skills and this file are in English.
- `ARCHITECTURE.md` and `xpute/ARCHITECTURE.md` define the parts and their
  boundaries. At the host/guest boundary:
  - Under `xpute/`: host, guest and their contract only. No pixelet word
    (kernel, shell, pixelet).
  - Kernel and shell mean what they mean in an operating system: the kernel is
    the core, the shell the UI. A port is a scheduling unit, not a role.
  - pixelet's host is `ts/client/src/host/` and the client's bracket, and gives
    no number a meaning. What answers the kernel's hostcalls goes in
    `ts/client/src/sys/`: its files (names, versions, kept keys, refusals) in
    `sys/fs/`, its text and its stop report beside. What the shell's port
    carries goes in `ts/client/src/port/`, a file a major: the commands for the
    UI's intents and its input, the snapshot, the surfaces it lends and each
    turn's frame, what the place's sensors, GPU and visibility tell the kernel,
    the profiler. The server the client reads from (its routes, its published
    tables, whether the device is online) is `ts/client/src/origin/`. The
    numbers both read, generated from `spec/` or written beside it (a state the
    client reports and the shell shows, `abi/state.ts`), and the boot are
    `ts/client/src/abi/`. None of it goes into `@xpute/host` or `xpute-host`.
  - The shell (`ts/client/src/shell/`) is the UI. It computes no coordinate or
    tile, is not the host, and reaches the kernel, `sys/` and `origin/` through
    `port/` alone.
  - Don't propose removing a part of xpute's `kit` because nothing here calls
    it.

## Reading and changing

- Read the architecture sections and the READMEs beside the code for the
  roles and boundaries the change touches, and the skill the task needs
  (`.agents/skills/`). What done means: `.agents/working.md`.
- `ARCHITECTURE.md`, `xpute/ARCHITECTURE.md` and the READMEs beside the code
  state the parts' contracts; a skill gives a task's procedure. The code is
  agents' work, not the user's intent: read it as suspect. Decide what a part
  must do from the contracts and the user's decisions, then judge the code
  against that; a comment, a structure or a pattern in it, however often
  repeated, is no reason to keep it, and an asymmetry is not by itself debt.
- The records under `doc/` recover intent, scope and limits. A record that
  attributes a decision to the user explicitly makes it an adopted decision;
  check its scope. A proposal, an attempt or a measurement is no decision,
  however recent. The user's explicit word now applies within its scope before
  the documents catch up.
- Keep an established symmetry by role and contract, not by matching module
  lists. A local gain alone does not weaken one or cross a boundary. A
  contract changed on purpose is named as such, with what keeps or replaces
  the properties it held.
- Where the code and a stated contract disagree, tell debt, an outdated
  document and an unfinished transition apart by the records and the history,
  and copy no unresolved pattern as precedent. Ask only when the doubt changes
  what the task should do; go on with what it does not touch.
