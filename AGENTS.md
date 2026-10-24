# Agent Instructions

- Several sessions share this checkout on `main`, with no branches or
  worktrees. Commit only your own paths, and read each path's diff first:
  another session's hunk in the same file goes in with yours.
- Documents, comments and commits are in English (the `ko` locale aside), and
  say what and why, never who said it.
- How the tree is written — names, files, citations and the code's few
  fixed rules — is the `convention` skill's (`.agents/skills/convention/`).
- The boundary is xpute's, and its words name only it. xpute is the floor
  any guest runs on and owes pixelet nothing. The host grants the kernel
  memory and time, carries bytes, and dispatches the kernel's calls by
  number; it decides nothing about what a number means. pixelet's host is
  `apps/client/src/host/` plus a platform bracket. The world is the
  kernel's, its guest. What each number means outside the kernel, and the
  policy over it, is pixelet's, beside the host: its commands, snapshot
  and boot in `apps/client/src/abi/`; each root's backend, with the names,
  versions, kept keys and refusals behind it, in
  `apps/client/src/namespace/`. None of it lowers into `@xpute/host`. The shell
  (`apps/client/src/shell/`) is the UI; it computes no coordinate or tile,
  and it is not the host. Host, guest and their
  contract never name a pixelet part, and kernel, shell and pixelet appear
  nowhere under `xpute/`.
