---
name: collaboration
description: How sessions running side by side share one checkout — on main's tip, no worktrees, no holds, each building into its own target. Use when another session works in the same tree, and before holding a path, waiting for a peer, or building Rust.
---

# Collaboration

Sessions run side by side so the work finishes sooner. Work that waits on
another session finishes no sooner than one session doing it all in turn, so
none of it waits:

- All work in the one checkout, on main's tip: no worktrees, nothing beside
  the repository.
- No holds. A path is never reserved, and another session's announced work is
  no reason to stop yours.
- When two collide, the one whose session id (`$CLAUDE_CODE_SESSION_ID`) sorts
  first resolves it.

Sharing one checkout must not make the sessions queue anywhere else, so each
builds into a target of its own, named by its session id. A shared target would
make cargo's lock on it line every session's build up behind the others, and
hand each the binary another built last. How is the cargo skill's.
