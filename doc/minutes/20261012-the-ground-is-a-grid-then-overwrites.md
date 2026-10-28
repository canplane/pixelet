# The ground is a grid, then what overwrites it

**2026-10-12** — decided in the conversation; kept up to what the code holds.

## The structure

What is drawn is 1 × 1 × 1 blocks. What is in a block is one answer, laid in
this order, each layer written over the one before:

1. **The ground.** A tile's 512 × 512 cells, each a block shifted up to its
   cell's height at once: one pass over a fixed-stride grid. A cell shows its
   top face, and a side face as tall as the step down to each neighbour lower
   than it. A way at grade only recolors its cell.
2. **Caves.** A procedural field (`pxl-spec-core cave.rs` `Cave::hollow`),
   asked only under the ground's face.
3. **Separation.** The ways off the ground — decks, piers, the fill a ramp
   stands on, a tunnel's bore — written in 3D where they are, nowhere else. A
   bore writes its air and, in the blocks about it, a block of its own, so a
   cave written before it never opens into it.
4. **Objects.** What is placed, written in 3D where it is, air included: placed
   air takes away what the layers under it put there.

Where air meets ground under its face, whatever made the air, the ground shows
faces of its material by depth, soil or stone, bedrock under its floor
(`pxl-spec-core ground.rs` `by_depth`, `pxl-spec limit.rs`). Strata are
that rule of depth, not stored.

**One answer, every reader.** What is at a block is these layers read in this
order, by one function (`pxl-spec-core world.rs` `node`); the cast draws it, and
control — a walk, a floor, a ray, an edit, where decor grows — asks the same
function at the same zoom. Coarser than the leaves, the ground folds in 2D
(height and color over the cells folded) and each overwrite carries the fill its
store already folds (`pxl-spec-core overwrite.rs` `fold_word`).

**Invariant.** The ground has one representation, the grid; nothing that is not
an overwrite is 3D, and an overwrite exists only where its source holds
something.

## What is drawn

What is drawn is what the eye's rays reach. A ray is cast for each drawn pixel,
started off its cell's beam
([a cell casts a beam](./20261022-a-cell-casts-a-beam.md)), down the pyramid,
the same wherever the eye is. It passes through air and void and stops at the
first block that fills; what made the air — a tunnel, a cave, a room someone
built — makes no difference. The rays are cast on the GPU and each paints the
block it stops at (`pxl-render frame/shader/paint.wgsl` `paint`); no mesh and no
chunk is made. The rays read the world, so nothing waits on anything drawn. A
cave is the hash's, computed when its blocks are asked for; where it runs on, or
where it opens, is never worked out.

**Observation determines what is drawn.** World queries determine the block at a
position and zoom independently of observation. The eye's rays determine what is
observed. There is no chunk: what stands in the world is gathered over the eye's
windows' squares (`pxl-kernel domain/view/observation.rs` `squares`).

**Determinism.** Given the same world snapshot, observer, camera grid, and zoom,
observation and what is drawn are identical regardless of traversal order or
scheduling.

What goes is every rule about what made the air; what stays is the descent that
finds the first block. A ray is not walked block by block: it descends the
pyramid, each node's bound pruning where nothing can be, so a long tunnel or
cave costs the nodes it crosses. The meaning is one rule; finding that block
fast is still the descent's work, and the bounds are kept.

## What it replaces

Every cell composed into a list of z intervals — a "column" of "runs" — and
every reader scanning those lists: `pxl-spec block/stand.rs` `Stand`,
`stands_of`, `compose`; `block/sample.rs` `ground_runs` and the per-cell `Over`
and `Under`; `pxl-render mesh/run.rs`, `mesh/paint/column.rs`, and the run
masks, neighbour scans and coarse buckets of `mesh/chunk.rs`;
`pxl-kernel chunk/build/run.rs`; `query/block.rs` `Cell`, `Run` and `Column`,
and the interval walks of `query/support.rs`. The word goes with it.

What stays: the object store (already a sparse 3D map), the terrain's bound,
the windows' (`pxl-spec-core ground.rs` `Bounds`), and the bake. The mesh slots
go with the chunks' meshes.

## Why

Counted from the code (`mesh/chunk.rs` `run_faces`, `neighbor_relation`), with C
a chunk's cells, R the intervals a cell is composed into (four on plain ground:
stone, soil, grass, bedrock) and K the cells overwritten:

|                          | Every cell a list of intervals                                                        | A grid, then overwrites            |
| ------------------------ | ------------------------------------------------------------------------------------- | ---------------------------------- |
| Time                     | O(C·R²): each interval scans its own and eight neighbours' lists, some fourteen scans | O(C + K): a few comparisons a cell |
| Space                    | O(C·R): some 80–100 bytes a cell                                                      | O(C + K): some 3 bytes a cell      |
| Blocks stacked in a cell | R grows, the cost as R²                                                               | linear in K                        |

The complexity settles it. The list was never chosen: the first Rust mesher was
the TypeScript's `voxel_scan.ts` ported byte for byte (`34fa737f6`), and strata,
bedrock, caves, decks and objects were each added into it since. The
representation of the rare case — more than one surface in a cell — became every
cell's.
