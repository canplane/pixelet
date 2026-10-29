# Agent Instructions

- Each skill has one role, stated in its description. A new rule goes into the
  skill whose role it is, or into a new skill, never into one it is merely
  related to.
- The skills are in `.agents/skills/`: `cargo` (running cargo), `naming`
  (choosing and spelling a name), `write-documents` (documents people read),
  `deploy` (checking once and sending), and `bake-tiles`, `publish-bucket` and
  `device-build` (tiles, the bucket, devices). Read the skill for a step before
  taking it.
- The agents are in `.agents/agents/`: `review` reviews a change. It is
  read-only and is handed the code, not its author's
  conclusions.
- `ARCHITECTURE.md` (and `xpute/ARCHITECTURE.md` for xpute) defines the parts
  and their boundaries. At the host/guest boundary:
  - xpute speaks only of host, guest and their contract, and names no program.
    pixelet's words (kernel, shell, pixelet) never appear under `xpute/`.
  - pixelet's host is `ts/client/src/host/` plus the client's bracket, nothing
    else, and no host decides what a number means. That meaning, and the policy
    over it, is pixelet's and lives outside the host: commands, snapshot and
    boot in `ts/client/src/abi/`; the answers to the kernel's opens and writes
    (names, versions, kept keys, refusals) in `ts/client/src/namespace/`. None
    of it moves into `@xpute/host` or `xpute-host`.
  - The shell (`ts/client/src/shell/`) is the UI. It computes no coordinate or
    tile, and it is not the host.
  - xpute's `kit` is a library any program may link: a part no program here
    calls is kept, and is never proposed for removal on that ground.
