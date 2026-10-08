# Terrain stays one tile

**2026-10-08**

A terrain tile keeps its ground, elevation, decks and bores, and index
together, one object a node (`crates/spec/src/bake/format.rs` `TerrainTile`). It
is not torn into a pyramid per channel, and it is not made sparse, a child
kept only where it says more than its parent. Places and divisions are
sparse (`crates/server/geo-bake/src/pyramid.rs` `prune`); terrain is not.

## The question

A leaf's elevation almost never equals its parent's, so a terrain child is
almost never its parent again, and the pyramid goes to every leaf. Torn
apart, an all-forest ground could stop where it is uniform and decks where
there are none, with only elevation going on down. The question was whether
that is worth what it costs.

## What was weighed

- **Each channel its own pyramid, with its own residency.** The kernel would
  hold ground, elevation and decks at different zooms for one node, so a
  coarse deck could stand over leaf ground (or a coarse grade line fill
  across finer relief), a chunk's apron would read its height from one zoom
  and its look from another, and a chunk would wait on three exact answers.
  Set aside for that alone.
- **One tree, four objects a node, the edge assembling today's tile.** Past
  the I/O edge nothing changes, but nothing is gained: the node set is the
  leaf set as now, so nothing is sparser.
- **Three pyramids, ground and decks stopping early, the edge filling them
  in** — a ground object absent is its stopping ancestor's repeated, a deck
  object absent is empty, elevation with the extent and occupancy at every
  leaf. Exact, and the kernel past the edge unchanged. Rejected on the
  costs:
  - **Requests.** Every tile goes through the Worker, which fails closed at
    100k a day on the free plan (AGENTS.md "Scope"). A node becomes up to
    three asks, and a channel is learned to have stopped only by a 404,
    which is an ask too.
  - **A lost object reads as right.** A missing tile today falls back
    visibly. A ground or deck object lost to an upload reads as "stopped":
    the ancestor's ground, or no bridge, which looks plausible.
  - **Versions as a tuple.** Ground is painted around the decks solved on
    one elevation; the three would be published as one tuple, and a mismatch
    draws no error.
  - **A wait across tiles.** A node's ground could not be filled until the
    ancestor it stopped at arrived, and tiles arrive in any order.
  - **What it saves is small.** A uniform ground lane and an empty decks lane
    already pack to almost nothing under zstd inside today's tile, and
    elevation still reaches every leaf, so neither objects nor requests go
    down.
- **Channels kept apart in the build only, packed into one tile to publish.**
  Its one gain, refolding a channel alone, is already had: the fold is per
  channel inside one function (`crates/spec/src/bake/fold.rs`
  `fold_terrain`), and a bake folds again only what its shards change,
  tile by tile (`pyramid.rs` `fold`). Set aside as state with no use.

## What holds

The leaf is baked as it was; the pyramid over it is one tile a node, folded
by `fold_terrain`, every zoom down to the leaves; a shard baked beside the
others folds again only what it changes
(`crates/server/geo-bake/src/tile/shard.rs`).

Asked again if re-uploading the whole terrain for a change that only
repaints becomes the measured bottleneck.
