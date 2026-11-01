---
name: facts
description: How a new state, rule, value or representation in pixelet is judged — whether it is an independent fact or one derived from a fact that already has an owner; one authoritative definition with representations derived from it; a copy, cache or generated file with its owner, derivation and invalidation said; and facts that only look alike kept apart. Use before adding state, a table, a constant, a message field or a second implementation, in any refactor, and when reviewing one.
---

# Facts

Minimize the facts the system keeps independently.

Before adding a state, a rule, a value or a representation, ask:

> Is this an independent fact, or one derived from a fact that already has an
> owner?

- A fact has one authoritative definition; every other form of it is derived
  from that one (generated, computed, or read through it). Two definitions of
  one fact, in two languages or two parts, are two owners: keep one.
- A value another fact determines is not owned again: the surface's size is
  `SURFACE_RESIZE`'s, so no command carries it a second time.
- A physical copy, a cache or a generated file is fine where its owner, how it
  is derived and when it is invalidated are said: the GPU's copy of the
  world's data, read from the CPU's; a `.spec.ts` written from its JSON.
- Facts are not merged because they look alike: a turn and a drawn frame, a
  storage file and a GPU upload.

When a change finds one fact held in two places, it names the owner and makes
the other derive from it, or names it as open.
