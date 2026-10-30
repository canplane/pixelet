# Agent Instructions

- Skills and this file are in English.
- `ARCHITECTURE.md` and `xpute/ARCHITECTURE.md` define the parts and their
  boundaries. At the host/guest boundary:
  - Under `xpute/`: host, guest and their contract only. No pixelet word
    (kernel, shell, pixelet).
  - pixelet's host is `ts/client/src/host/` and the client's bracket, and gives
    no number a meaning. Commands, snapshot and boot go in `ts/client/src/abi/`;
    the answers to the kernel's opens and writes (names, versions, kept keys,
    refusals) in `ts/client/src/namespace/`. None of it goes into
    `@xpute/host` or `xpute-host`.
  - The shell (`ts/client/src/shell/`) is the UI. It computes no coordinate or
    tile, and is not the host.
  - Don't propose removing a part of xpute's `kit` because nothing here calls
    it.
- Performance analysis or optimization, anywhere: apply the `performance`
  skill (`.agents/skills/performance/SKILL.md`).
