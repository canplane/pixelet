# The arena is bounded by the camera grid

**2026-10-13** — decided in the conversation. The resident bound is not yet
shown in the code.

What the chunks draw is what the eye's rays observe
([a ray asks the world whether a node is empty](./20261013-a-ray-asks-the-world-whether-a-node-is-empty.md)),
so how much is drawn follows from the camera grid, not from a budget.

## What one observation makes

A screen holds at most `GRID_CELLS_MAX` cells of the camera grid
(`pxl-kernel param/view.rs`); call it N. Each cell casts one ray, and each ray
stops on at most one block. A chunk draws only the blocks its rays stopped on,
and a block stopped on is at most one box; the blocks about it are asked of the
world to decide its faces and their shading, and are not drawn. So one
observation makes at most N boxes in at most N chunks:

    boxes observed ≤ N,  chunks observed ≤ N.

This is a bound from the structure, not a figure measured or tuned. No pool is
sized by a budget of its own, and no policy picks a quality to fit the arena:
there is nothing for one to decide.

## What is resident

What is resident is not only what one observation makes. A chunk being built for
the current observation and a drawn chunk from an earlier one can both hold
slots, and a rebuild can hold a chunk's old slots beside its new ones. That N
bounds what is observed does not by itself bound what is resident. The resident
bound is derived from the slots' lifetimes:

- which chunks outside the current observation stay, and on what condition;
- how many slots one chunk holds at once, old and new, during a rebuild;
- when a chunk's slots are given back, so that what can pile up before they are
  is bounded, however the eye keeps moving and the zooms keep changing.

The resident bound is what those three give. It holds by those invariants, not
by how many observations the drawn chunks came from.

## What stays

What is drawn moves by the swaps that exist: a group of chunks swaps once every
chunk the current observation makes in it is built (`pxl-kernel chunk/table.rs`
`materialize`). A slot of fixed size per chunk stays: N chunks of slot size S is
O(N), only a larger constant. No new allocator and no new swap policy is made
for the bound. Only a concrete order of events in which what is resident grows
past a bound in N would call for a change, and then the smallest one.
