# Agent Instructions

- Several sessions share this checkout on `main`, with no branches or
  worktrees. Commit only your own paths, and read each path's diff first:
  another session's hunk in the same file goes in with yours.
- Documents, comments and commits are in English (the `ko` locale aside), and
  say what and why, never who said it.
- Write by [CONVENTION.md](./CONVENTION.md): names, files, citations and the
  code's few fixed rules.
- The boundary is xpute's, and its words name only it. The host grants the
  kernel memory and time, carries bytes, and binds each root to where its
  bytes are; it decides nothing about what they mean. pixelet's host is
  `apps/client/src/host/` plus a platform bracket. Reading the world's data
  is the kernel's, its guest, and the UI is the shell's
  (`apps/client/src/shell/`), which is not the host. Host, guest and their
  contract never name a pixelet part, and kernel, shell and pixelet appear
  nowhere under `xpute/`.
