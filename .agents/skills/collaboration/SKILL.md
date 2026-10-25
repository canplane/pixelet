---
name: collaboration
description: How sessions running side by side share one checkout — on main's tip, no worktrees, no holds, each committing its own paths and pushing main as it is, each building into its own target. Use when another session works in the same tree, and before holding a path, waiting for a peer, committing or pushing main, or building Rust.
---

# Collaboration

Sessions run side by side so the work finishes sooner. Work that waits on
another session finishes no sooner than one session doing it all in turn, so
none of it waits:

- All work in the one checkout, on main's tip: no worktrees, nothing beside
  the repository.
- No holds. A path is never reserved, and another session's announced work is
  no reason to stop yours.
- The index is shared: commit your own paths, `git commit -- <paths>`, never
  the staged set, which may hold another session's.
- Push main as it is, other sessions' commits included, and nothing is run
  before such a push: a check is for sending a line out (deploy skill), not
  for syncing.
- When two collide, the one whose session id (`$CLAUDE_CODE_SESSION_ID`) sorts
  first resolves it.

Sharing one checkout must not make the sessions queue anywhere else, so each
builds into a target of its own, named by its session id. A shared target would
make cargo's lock on it line every session's build up behind the others, and
hand each the binary another built last. How is the cargo skill's.
