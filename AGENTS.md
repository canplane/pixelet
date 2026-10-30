# Agent Instructions

- Skills and this file are in English.
- `ARCHITECTURE.md` and `xpute/ARCHITECTURE.md` define the parts and their
  boundaries. At the host/guest boundary:
  - Under `xpute/`: host, guest and their contract only. No pixelet word
    (kernel, shell, pixelet).
  - pixelet's host is `ts/client/src/host/` and the client's bracket, and gives
    no number a meaning. The kernel's two ABIs stand apart. What the shell holds
    (the commands for the UI's intents and its input, the snapshot, the surfaces
    it lends, the profiler) goes in `ts/client/src/port/`. What answers the
    kernel's devices goes in `ts/client/src/driver/`: its opens and writes
    (names, versions, kept keys, refusals) in `driver/namespace/`, its text,
    sensors and GPU beside. The numbers both read, generated from `spec/` or
    written beside it (a driver's state the shell shows, `abi/state.ts`), and
    the boot are `ts/client/src/abi/`. None of it goes into `@xpute/host` or
    `xpute-host`.
  - The shell (`ts/client/src/shell/`) is the UI. It computes no coordinate or
    tile, is not the host, and reaches the kernel and the drivers through
    `port/` alone.
  - Don't propose removing a part of xpute's `kit` because nothing here calls
    it.

## Reading and changing

- Read the architecture sections and the READMEs beside the code for the
  roles and boundaries the change touches, and the skills its task takes:
  performance analysis or optimization, anywhere, takes `performance`
  (`.agents/skills/performance/SKILL.md`).
- `ARCHITECTURE.md`, `xpute/ARCHITECTURE.md` and the READMEs beside the code
  state the parts' contracts; a skill gives a task's procedure; the code shows
  the implementation, its debt and its work in progress among it. A pattern in
  the code, however often repeated, is not by itself a rule, and an asymmetry
  is not by itself debt.
- The records under `doc/` recover intent, scope and limits. A record that
  attributes a decision to the user explicitly makes it an adopted decision;
  check its scope, and whether a later adopted decision replaced it. A
  proposal, an attempt or a measurement is no decision, however recent. The
  user's explicit word now applies within its scope before the documents
  catch up.
- Keep an established symmetry by role and contract, not by matching module
  lists. A local gain alone does not weaken one or cross a boundary. A
  contract changed on purpose is named as such, with what keeps or replaces
  the properties it held.
- Where the code and a stated contract disagree, tell debt, an outdated
  document and an unfinished transition apart by the records and the history,
  and copy no unresolved pattern as precedent. Ask only when the doubt changes
  what the task should do; go on with what it does not touch.
