# Agent Instructions

- How the tree is written — names, files, citations and the code's few fixed
  rules — is the `convention` skill's (`.agents/skills/convention/`).
- **xpute** is the floor a guest program runs on. Its words are host, guest and
  their contract, and they name no program. The host grants the guest memory and
  time, carries bytes, and dispatches the guest's calls by number; it decides
  nothing about what a number means. Its `kit` is a library any program may
  link: a part no program here calls is kept, and is never proposed for removal
  on that ground.
- **pixelet** fills those roles with its own parts. The kernel is its guest, and
  its host is `ts/client/src/host/` plus the client's bracket: `@xpute/host`
  mounted (on a device, `xpute-host` beside it) with the one spec it is handed,
  and nothing else. What the kernel's numbers mean outside it, and the policy
  over them, are pixelet's, outside the host: its commands, snapshot and boot in
  `ts/client/src/abi/`; what answers the kernel's opens and writes, with the
  names, versions, kept keys and refusals behind it, in
  `ts/client/src/namespace/`. None of it lowers into `@xpute/host` or
  `xpute-host`. The shell (`ts/client/src/shell/`) is the UI; it computes no
  coordinate or tile, and it is not the host. pixelet's words (kernel, shell,
  pixelet) appear nowhere under `xpute/`.
