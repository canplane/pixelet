---
name: code
description: How code in pixelet and xpute is written — fewest concepts, orthogonal, one hand's style throughout, as UNIX V6 and K&R are written. Use before writing or rewriting code, and when reviewing one's shape. Names are the naming skill's.
---

# Code

- Reduce first. A concept that is another in disguise (a reply is a frame, an
  abort is an import) is that other. Code is short because its concepts are few.
- Orthogonal. A concept says one thing; changing it reinterprets no other.
- One owner. A fact has one definition; every other form derives from it.
- Data first. Structures and tables, then short straight functions over them.
  Where a branch picks by kind, a table picks.
- Read in the order it runs. No step behind a wrapper or a hook.
- No optional seams: no hook that may be absent, no fallback for a value one
  place could give, no layer split out for a test.
- One hand. The same work in the same shape, the same names for the same
  things. Rewrite a part rather than patch a new manner beside the old.
- Keep a symmetry by role and contract, not by matching module lists. A local
  gain alone does not weaken one. A contract changed on purpose is named as
  such, with what keeps or replaces what it held.
- A comment holds what the code cannot say: the contract, the reason, the
  invariant. It reads on its own in whole sentences, never says how the code
  came to be and never cites a minute under `doc/`.
