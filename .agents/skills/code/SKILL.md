---
name: code
description: How code in pixelet and xpute is written — fewest concepts, orthogonal, one hand's style throughout, as UNIX V6 and K&R are written. Use before writing or rewriting code, and when reviewing one's shape.
---

# Code

- Reduce first. Before writing, ask why each concept the code would hold must
  exist; one that is another in disguise (a reply is a frame, an abort is an
  import) is that other. Code is short because its concepts are few, not
  because its lines are.
- Orthogonal. A concept says one thing, and changing it reinterprets no other.
- Data first. The structures and the tables come first; functions run over
  them, short and straight. Where a branch picks by kind, a table picks.
- Read in the order it runs. A step is not hidden behind a wrapper or a hook.
- No optional seams. What is there is there: no hook that may be absent, no
  fallback for a value one place could give, no layer split out for a test.
- One hand. A file, and a part across files, is written once in one manner:
  the same work in the same shape, the same names for the same things. Do not
  patch a new manner beside an old one; rewrite the part.
- A comment holds what lies between the lines: the contract, the reason, the
  invariant the code cannot say. Not what the code does, not how it came to
  be, and none written because a comment was due. A minute under `doc/` is
  what was said then, never a reason in the code.
