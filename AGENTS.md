# Agent Instructions

- Several sessions share this checkout on `main`, with no branches or
  worktrees. Commit only your own paths, and read each path's diff first:
  another session's hunk in the same file goes in with yours.
- Documents, comments and commits are in English (the `ko` locale aside), and
  say what and why, never who said it.
- How the tree is written — names, files, citations and the code's few
  fixed rules — is the `convention` skill's (`.agents/skills/convention/`).
- The boundary is xpute's, and its words name only it. The host grants the
  kernel memory and time, carries bytes, and binds each root to where its
  bytes are; it decides nothing about what they mean. pixelet's host is
  `apps/client/src/host/` plus a platform bracket. The world is the
  kernel's, its guest. What the kernel's numbers mean outside it is the
  client's beside the host: its commands, snapshot and boot in
  `apps/client/src/abi/`; each root's backend, with the names, versions
  and keys behind it, in `apps/client/src/namespace/`. The shell
  (`apps/client/src/shell/`) is the UI; it computes no coordinate or tile,
  and it is not the host. Host, guest and their
  contract never name a pixelet part, and kernel, shell and pixelet appear
  nowhere under `xpute/`.
