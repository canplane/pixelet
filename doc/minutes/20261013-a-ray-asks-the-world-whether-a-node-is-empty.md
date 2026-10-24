# A ray asks the world whether a node is empty

**2026-10-13** — decided in the conversation.

**Status (2026-10-24): the descent and the query hold; a ray is no longer
cast a camera-grid cell.** A ray is cast a pixel, started off its cell's
beam ([a cell casts a beam](./20261022-a-cell-casts-a-beam.md); pxl-spec-core
beam.rs, cast.rs `Until::Beam`), and asks the zoom its distance asks by the
camera grid as below. The query is pxl-spec-core world.rs `node`, which the
kernel asks through query/node.rs `World::node`.

What is drawn is what the eye's rays reach
([the ground is a grid](./20261012-the-ground-is-a-grid-then-overwrites.md),
"What is drawn"). This is how a ray reaches it, the same in space, on the
ground and under it.

## The descent

A ray is cast for each cell of the camera grid and descends the pyramid by
one rule of detail, the one that sizes every drawn cell: the zoom a ray asks
at a distance is the one whose block, drawn whole, spans at least one cell
of the camera grid there, its side at least the cell's angle times the
distance (`Tau::cell_at`). A cube is at least its side wide seen from any
direction, so how the ray meets its face does not enter (corrected the same
day; it first divided by the cosine to the face met, which asked an edge-on
face as coarse as the coarsest zoom). A node coarser than the zoom asked
there is not the world as drawn there, and is not asked what it draws. It
is asked only whether anything under it fills at the zoom the ray asks
there, the world as its level of detail defines it and nothing finer:

- **Nothing fills under it.** The ray goes on past the node whole.
- **Something may.** The node is split, and its children are taken in the
  order the ray meets them.

A node at that zoom is asked what it is, at that zoom, in the world as
composed:

- **Air or void.** The ray goes on past it.
- **Otherwise.** That block is where the ray stops, and that is what is
  drawn. Nothing else is.

A node is passed whole only where every cell under it at the zoom the ray
asks is air or void, so the ray stops on the same block it would by asking
every cell at that zoom: passing is the tree's, never a second answer. What
that zoom folds away is not there to miss. A source may answer more
cautiously, which only splits more; it never answers from detail finer than
the zoom asked.

What a node learns is handed to its children, never asked again from the
root. A branch the ray is done with is dropped. Nothing is materialized
whole to find out what is seen: no chunk is built, and no cave is dug ahead
of the ray.

**The traversal follows the ray.** A ray traverses the pyramid in encounter
order, skipping empty nodes whole: from a node it enters the child the ray
meets next, and past a node it climbs only as far as the ancestor that holds
the next one, never starting again from the root. Its work depends on the
nodes visited and their query costs, not on the pyramid's depth alone: the
depth bounds one path down, not a ray's whole walk (corrected the same day;
it first said O(log n)). Each source answers a node in time that does not
grow with the cells under it.

## The query between them

One function stands between the ray and storage: what is in a node, at the
node's own zoom. It is the world's one query, not a second beside it: asked
of a node it answers what is there at that zoom, and at the zoom the ray
asks that is the block, as `pxl-spec block/sample.rs` `at` did then. A block
is the smallest node, so a node and a block are one answer at two sizes, and
every reader — the ray, a chunk, a walk, an edit — calls that one function.
The ray knows where to ask and nothing of how the layers are held; the
layers know what is in a node and nothing of rays.

Under that function each source answers for the node from its own level of
detail at that zoom, as it already folds it, rather than by laying its
blocks out:

- **The ground**: its tile at that zoom, the cell's height and face as it
  folds them.
- **Caves**: the hash pyramid's node at that zoom, its value.
- **Separation**: the ways folded in 3D by volume, as the objects are, a
  tunnel's bore and a deck alike: the ways are 3D, so they fold as the
  objects do and not as the ground's 2D surface.
- **Objects**: the objects' pyramid record at that zoom, folded by their
  majority.

These are composed in the order the layers overwrite one another: an empty
cave node with a deck or a placed block in it is not air. Where the composed
answer is air or void, the ray goes on past the node. A source that does not
hold the node yet answers that it lacks it, never that it is air.

The same record answers the coarser question, whether anything under the
node fills, from what each source already keeps, with nothing new stored:

- **The ground**: its tile index's highest height under the node's bottom,
  or the node within where the ground may be taken (under its face block,
  over its bedrock, off the wall, as `at` takes it) with a cave through all
  of it. The index is gone: the bound is now the windows' (pxl-spec-core
  ground.rs `Held::relief`), and where the ground may be taken is
  `ground::slab`.
- **Caves**: the hash node hollow and drawing nothing again under it
  (`live`).
- **Separation**: no part of the ways' 3D fold at the node, as for the
  objects.
- **Objects**: no record of the objects' pyramid at the node.

## What goes

Every observation that stands in for the rays: the screen's patches and
their corner rays, the footprint's hull and band of heights, the walk that
cut chunks by what a footprint touches, and the goal, frontier and quality
pricing built on it. With them goes building a chunk whole to learn what
it shows: the ray finds what is seen without any chunk, so neither has to
come first.
