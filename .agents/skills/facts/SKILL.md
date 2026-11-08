---
name: facts
description: How a new state, rule, value or representation in pixelet is judged — whether it is an independent fact or one derived from a fact that already has an owner; one authoritative definition with representations derived from it; a copy, cache or generated file with its owner, derivation and invalidation said; and facts that only look alike kept apart. Use before adding state, a table, a constant, a message field or a second implementation, in any refactor, when clearing entropy, and when reviewing one.
---

# Facts

Minimize the facts the system keeps independently.

Before adding a state, a rule, a value or a representation, ask:

> Is this an independent fact, or one derived from a fact that already has an
> owner?

- A fact has one authoritative definition; every other form of it is derived
  from that one (generated, computed, or read through it).
- A value another fact determines is not owned again: the surface's size is
  `SURFACE_RESIZE`'s, so no command carries it a second time.
- Facts are not merged because they look alike: a turn and a drawn frame, a
  storage file and a GPU upload.

When a change finds one fact held in two places, it names the owner and makes
the other derive from it, or names it as open.

## Clearing entropy

Entropy is one fact kept in several shapes, and one question asked in several
places. Clearing it takes things away: less state, fewer types, each question
asked once. A new wrapper, a rename alone or a shorter function clears nothing.

Look for:

- A type per kind around one number: the number is the fact. A value says
  what it is by what it holds, so a variant carrying what its neighbor's number
  already says goes.
- A value and the tag that says how to read it: `{ tool, cube }`, when the
  number's kind says the tool.
- One either-or kept as two `Option`s: one field of an enum.
- The same `match` over the same thing in several functions: what a kind does
  moves onto one type, its behavior in the module it needs, the code asking it
  once.
- A boolean pair read off one value.
- The same value read in two components: passed from the one that decided.
- One thing decided outside the place that decides it.
- A kind that fails one way where its neighbors fail another: one reading, one
  rule.

Do:

- Find what is really stored, and keep that. Everything else is read from it.
- Follow the data across languages and generators: the UI, the core and the
  spec hold one shape, and the generator is changed, not its output.
- Name what is left for what it is (`naming`).
- Keep what truly differs: a tap on a cube and on a marker do different things.
  Keep what a person chose beside what it derives: a cube's size has its own
  field, a decor's comes of its id.
- Keep look and data out of what only says what a thing is: a cube's color and
  texture are read by its number.
- Drop a form kept in storage when what it holds fills back in. No migration
  unless the user asks for one.
