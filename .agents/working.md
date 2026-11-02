# Working toward done

## Done

The work is done when the structure defends itself: a peer who joins and
works as peers do cannot break it or add entropy to it. That holds when:

- every fact has one owner, and every other form of it is generated from it,
  with a check that refuses a stale copy;
- the boundaries a machine can measure are held by one (`cargo check-rules`);
- the user's decisions are held by tests or invariants where they can be;
- nothing known to be wrong is left in the tree for a peer to copy.

## Without circling

Two fixes in a row that don't converge, or one revert, end an attempt: drop
it and start again from what must be true when the task is done.
