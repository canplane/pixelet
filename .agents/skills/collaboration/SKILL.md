---
name: collaboration
description: How sessions running side by side work — optimistically, all in one checkout on main's tip, never waiting on another, each building into its own target. Use when another session works in the same tree, and before holding a path, waiting for a peer's landing, asking a peer to confirm, or building Rust.
---

# Collaboration

Sessions run side by side so the work finishes sooner. Work that waits on
another session finishes no sooner than one session doing it all in turn,
so none of it waits:

- All work in the one checkout, on main's tip; no worktrees.
- No holds. A path is never reserved, and another session's announced work
  is no reason to stop yours.
- Land small commits as you go, and rebase over whatever landed
  meanwhile. Whoever lands second resolves the conflict.
- Say what landed after it lands, so the others rebase knowingly. Do not
  ask before you start, and do not wait for an answer to go on.
- A review or a confirmation round runs beside the work, not in front of
  it, unless the owner asks for it first.

Sharing one checkout must not make the sessions queue anywhere else, so
each builds into a target of its own, named by its session id. A shared
target would make cargo's lock on it line every session's build up behind
the others, and hand each the binary another built last. How is the cargo
skill's.
