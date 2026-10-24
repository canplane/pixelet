# Agent Instructions

- Several sessions share this checkout on `main`, with no branches or
  worktrees. Commit only your own paths, and read each path's diff first:
  another session's hunk in the same file goes in with yours.
- Documents, comments and commits are in English (the `ko` locale aside), and
  say what and why, never who said it.
- A structure is named by one word that maps to one path or component, never
  a loose phrase. Borrow a known term only where the code has its defining
  properties, else use the project's own; an outside term is a borrowed
  structure, said once, never an identity.
- xpute's words are host, guest and their contract; kernel, shell and pixelet
  appear nowhere under `xpute/`. pixelet's host is `apps/client/src/host/`
  plus a platform bracket, and the kernel is its guest.
