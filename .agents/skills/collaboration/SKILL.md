---
name: collaboration
description: How sessions running side by side work — optimistically, each on main's tip, never waiting on another. Use when another session works in the same tree, and before holding a path, waiting for a peer's landing, or asking a peer to confirm.
---

# Collaboration

Sessions run side by side so the work finishes sooner. Work that waits on
another session finishes no sooner than one session doing it all in turn,
so none of it waits:

- No holds. A path is never reserved, and another session's announced work
  is no reason to stop yours.
- Each works on main's tip, lands small commits as soon as they pass, and
  rebases over whatever landed meanwhile. Whoever lands second resolves the
  conflict.
- Say what landed after it lands, so the others rebase knowingly. Do not
  ask before you start, and do not wait for an answer to go on.
- A review or a confirmation round runs beside the work, not in front of
  it, unless the owner asks for it first.
