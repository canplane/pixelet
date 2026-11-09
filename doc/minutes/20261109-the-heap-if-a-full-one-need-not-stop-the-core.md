# The heap, if a full one need not stop the core

**2026-11-09** — recorded with the user. A proposal, not a decision: whether to do it, and
when, is open.

## What was asked

The core's memory is fixed, 512 MiB on a handheld and 1 GiB on a desktop, and the user's stand
is that a heap that fills stops the core: nothing cleans up because it is full, and what is
held is let go by its owner. An entrance check on the heap was built on the way to closing the
memory and removed again. The user then asked what the check should be if one set aside the
thought that a full heap must stop the core.

## The proposal

No entrance check. Each owner has a budget in bytes and answers for it, from the real size of
what it holds and the allocator's real answer.

**A budget for each owner.** Of the tiles only the places and the divisions' lists are on the
heap; the terrain, the Earth and the divisions' cells have room of their own. `TileCache`
counts the bytes it holds beside its slots. A tile that would pass the budget evicts as a tile
with no slot does, the farthest held only as cache first. Where what is asked for alone passes
the budget, the new tile is refused and the tile over it answers there. Fetching and eviction
then match in bytes as they match in slots, and the budgets add up against the heap at compile
time, as `HEAP_FLOWS_BYTES` does.

**Fallible only where outside data comes in.** A body already is: `body::take` refuses and the
want waits. A decode takes its room once, at the exact lengths the packet states, and a failure
goes the same way, as a `Refusal`. With no growth by doubling a tile's peak falls from about
three times its packet to about one, and no estimate per byte is needed.

## What was weighed and not taken

- A watermark with an estimate per byte, checked at the entrance. It decides from a guess at
  what will be taken, and it was the one built and removed.
- Cleaning up inside the allocator, as an operating system's direct reclaim does. A caller may
  be allocating while it holds a borrow of the cache, which this code's global state does not
  make safe.
- Letting go of every cache when the heap refuses. It is one more mechanism that reacts to
  pressure, beside the eviction that already exists.

## Open

- Whether to do this at all.
- The budgets for the places and the divisions. They take the sizes of the baked tiles, which
  were not looked at.
- The objects, whose store has no bound while they are held whole on the device. They could
  take the same shape once they come as a server's tiles.
