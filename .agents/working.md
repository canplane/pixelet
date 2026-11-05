# Working toward done

## Done

The work is done when the structure defends itself: a peer who joins and
works as peers do cannot break it or add entropy to it. That holds when:

- every fact has one owner, and every other form of it is generated from it,
  with a check that refuses a stale copy;
- the boundaries a machine can measure are held by one (`cargo check-rules`);
- the user's decisions are held by tests or invariants where they can be.
