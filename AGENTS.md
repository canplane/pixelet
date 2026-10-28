# Agent Instructions

- A skill holds one role, its description says which, and a rule goes to the
  skill whose role it is, or to a new one — never to one it merely sits near.
- The skills are in `.agents/skills/`, each its role: `cargo` running cargo,
  `collaboration` sessions sharing the checkout, `convention` how the tree is
  written, `reshape` how a change to structure or speed is found, proposed and
  proven, `write-documents` the documents people read, `deploy` checking once
  and sending, `bake-tiles`, `publish-bucket` and `device-build` the tiles, the
  bucket and the devices.
  Read the one whose role a step is before taking it.
- The agents are in `.agents/agents/`: `review` reviews a change before it is
  committed, read-only and handed the code, not its author's conclusions.
- What the parts are and where their boundaries run is `ARCHITECTURE.md`'s,
  xpute's `xpute/ARCHITECTURE.md`'s. Over the host/guest boundary these hold:
  - xpute's words are host, guest and their contract, and name no program;
    pixelet's words (kernel, shell, pixelet) appear nowhere under `xpute/`.
  - pixelet's host is `ts/client/src/host/` plus the client's bracket, and
    nothing else, and no host decides what a number means. What the kernel's
    numbers mean outside it, and the policy over them, are pixelet's, outside
    the host: its commands, snapshot and boot in `ts/client/src/abi/`; what
    answers the kernel's opens and writes, with the names, versions, kept keys
    and refusals behind it, in `ts/client/src/namespace/`. None of it lowers
    into `@xpute/host` or `xpute-host`.
  - The shell (`ts/client/src/shell/`) is the UI: it computes no coordinate or
    tile, and it is not the host.
  - xpute's `kit` is a library any program may link: a part no program here
    calls is kept, and is never proposed for removal on that ground.
