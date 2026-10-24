# Agent Instructions

- Several sessions share this checkout on `main`, with no branches or
  worktrees. Commit only your own paths, and read each path's diff first:
  another session's hunk in the same file goes in with yours.
- Documents, comments and commits are in English (the `ko` locale aside), and
  say what and why, never who said it.
- How the tree is written — names, files, citations and the code's few
  fixed rules — is the `convention` skill's (`.agents/skills/convention/`).
- **xpute** is the floor a guest program runs on. Its words are host, guest
  and their contract, and they name no program. The host grants the guest
  memory and time, carries bytes, and dispatches the guest's calls by
  number; it decides nothing about what a number means.
- **pixelet** fills those roles with its own parts. The kernel is its
  guest, and its host is `apps/client/src/host/` plus a platform bracket,
  holding only what `@xpute/host` is handed. What the kernel's numbers mean
  outside it, and the policy over them, are pixelet's: its commands,
  snapshot and boot in `apps/client/src/abi/`; each root's backend, with the
  names, versions, kept keys and refusals behind it, in
  `apps/client/src/namespace/`. None of it lowers into `@xpute/host`. The
  shell (`apps/client/src/shell/`) is the UI; it computes no coordinate or
  tile, and it is not the host. pixelet's words (kernel, shell, pixelet)
  appear nowhere under `xpute/`.
