# Decor stands at every zoom, as the caves do

**2026-10-30** — recorded with the user. Main was at `2d1d872ec`. What the
user decided is said as theirs; everything under "Proposed" is a design, not
built.

## What was asked

Asked how the indicators and the actors take the level of detail decor does,
the user put the rule as a thing's own size, not the zoom a window is drawn at:
decor is taken out wholesale where its window is coarser, when it should depend
on the room it takes. The puck's zone, made of ground cells, hides where the
ground is drawn coarser than they are; an actor follows the same dynamics by
its size; the ghost needs none, since nothing coarse is picked. Decor folding
should be what a cube folding is; no smaller mesh is loaded for it, as there is
none, though one would be used if there were. The user then asked whether the
procedural decor could be known from afar as the caves are, symmetrically with
them, and whether that takes a hash interface; they took the design below,
which shares the caves' descent and makes no interface ahead of a need for one.

## What the code does

- **Decor is generated in the leaves' windows alone.** Each kind has a bit a
  window zoom (`domain/scene/decor/drawn.rs` `shown`), and only the leaf
  cell's is ever set (`shown_by`); a coarser window generates none, whatever
  the decor's size. The reason given was in a commit, not a record (folded
  into `9ee92320f`): the ground's decor is one set, "never thinned or posed
  again at a coarser window". No record attributes it to the user.
- **The GPU already takes a zoom a window.** `decor_mark_cs` lists a decor
  for an entry where its bit for the entry's window zoom is set (pxl-shader
  `decor.rs` `shown_at`), at a leaf's cells or coarser.
- **The process is already a hierarchy by zoom.** A patch at zoom m bears the
  candidates born there, and a candidate born at m stands at every zoom from
  m finer (pxl-spec-core `scatter.rs`); `each_candidate` reads those born at
  a zoom or coarser without going down to the leaves. Each patch is seeded by
  `node_seed`, the one seed every procedural draw takes, as the caves are
  (`cave.rs`).
- **The thinning is already asked at a zoom** (`retained(decor, zoom, c)`),
  and a deleter must come before what it deletes (`before`: born coarser,
  then the lower seed). A finer zoom adds only candidates born finer, which
  come after every one already there, so what is retained at a zoom is
  retained at every finer one: the coarse set is a subset of the fine, and
  going finer only adds.
- **Posing is already asked at a cell zoom** (`generated(q, decor, cell, c,
  boxed)`): the footing down the candidate's column at `cell`, and the box
  in steps of it.
- **Decor is a block** (folded into `9ee92320f`: "a decor is the block it
  claims"): struck as the box it claims and stood on as a cube is
  (`domain/query/decor/strike.rs`). But it is not one of the parts the world
  composes at a node (pxl-spec-core `world.rs` `node`: the ground's grid,
  the caves, the ways, what is placed), and a fold strikes none of it: a
  coarse cell keeps nothing of it.
- **The caves fold to themselves.** A cube's caves are its parent's, flipped
  in at most one of its eight, so the majority of any cube's children is the
  cube: a coarse cave cube is the exact summary of what is under it, handed
  down a descent as a `Cave` (`Cave::child`).
- **The others.** An actor is seen where its reach spans a camera cell and is
  drawn whole there, with no impostor (`domain/scene/actor.rs`); the puck's
  zone is seen where it spans one, whatever the ground is drawn at
  (`indicator/location_puck.rs`); the ghost has no distance rule.

## Proposed

### One rule for what is drawn into the world

1. **Whether it stands** is its own size against the cells it stands among:
   a thing stands where the ground under it is drawn in cells no larger than
   the room it takes, and folds into the cell where they are larger.
2. **How it is drawn** is its footprint on the screen, in camera cells: a
   model from `DECOR_MODEL_CELLS`, an impostor from `DECOR_IMPOSTOR_CELLS`,
   nothing below (pxl-spec-core `scatter::drawn::tier`).

| Thing      | 1, whether it stands                                    | 2, how it is drawn               |
| ---------- | ------------------------------------------------------- | -------------------------------- |
| decor      | its claimed brick against the window's cell; else folds | model, impostor, nothing         |
| cumulus    | — (its own frame, the wind's lattice)                   | as decor, as now                 |
| actor      | its reach against the window's cell                     | as decor; its impostor is open   |
| puck zone  | hidden where the ground is drawn coarser than a leaf    | —                                |
| ghost      | — (nothing coarse is picked)                            | —                                |

### Decor as a part of the node, as the caves are

The caves and decor are both procedural and both seeded by `node_seed`; what
they share is the descent, not a hash interface:

- **Handed down.** Decor gets a state the node over it hands it, as
  `Handed` hands the caves theirs: the candidates born coarser that lie in
  the node, filtered to each child, and the node's zoom's own births added.
  A patch is a power of two of cells, aligned to a node, so the handing
  follows the tree.
- **Composed.** The world's node asks decor as its fifth part, beside the
  ground's grid, the caves, the ways and what is placed: what stands in a
  node, at any zoom.
- **Each its own fold.** The caves fold exactly to themselves; decor folds to
  what is retained at the zoom, a subset that only grows finer. Where a decor
  is smaller than the node's cell, it is folded as a cube is: the room it
  claims adds to the cell's kept eighths (`answer::cover`), and its look to
  the cell's majority, by what the asset says it is at a coarser zoom (its
  metadata, 20261024). Where it is as large or larger, it stands as itself on
  that zoom's ground and is drawn by rule 2.
- **No trait yet.** The world's node names each part, and the code runs on
  the GPU, where words, not traits, are the form. The contract is said here:
  a procedural part is seeded by `node_seed`, handed down by a `child`, and
  says its own fold. A trait comes when something reads the parts
  generically, as the frames' record says of their queries.

### What changes

1. `shown_by` sets a kind's bit at every window zoom whose cell is no larger
   than its brick, not the leaf's alone; each window generates the
   candidates born at its zoom or coarser, retained at that zoom, posed on
   that zoom's ground.

   > Built the same day: a variant generates at a window only where it stands
   > as itself there (pxl-spec-core `scatter::stands_at`, in `generated`), a
   > kind is listed at each window one of its variants does (`shown_by`), and
   > a record is drawn where the rays ask its own window's zoom
   > (`scatter::drawn_at`, by the zoom its record names). The GPU's passes
   > already took a window's zoom. A brick at any window meets no more
   > patches than the leaves' bound (`brick_patches_any_window`). What is
   > smaller than a coarser window's cell is not folded yet (2), and the cost
   > of the windows that now generate is not measured.
2. Decor joins the node's descent as a handed state, and its fold into the
   cell's kept eighths and look where it is smaller than the cell.

   > Built 2026-10-31, with the user, folded once a window generates rather
   > than handed down: a candidate list fits no word a descent hands, and
   > asking decor again at every node a ray meets is its whole generation
   > again. Where an entry generates whole, a candidate whose variant is
   > smaller than the window's cell (pxl-spec-core `scatter::folds_at`)
   > generates nothing and adds the room it claims (`fold_share`) and the
   > look it is folded to into its column's word, over the ground's top, the
   > same in any order (`window::fold_into`, pxl-shader decor.rs `decor_cs`);
   > the node reads
   > it beside the ground in the column's top cell (`ground::Cell::folded`,
   > `world::node`), its share added as a cube's, its look the decor's
   > `fold` (decor.json; the tree's `forest`) or the ground's under it. A
   > kind's folded windows are handed with its shown ones, none with the
   > decor setting off (pxl-kernel domain/scene/decor/drawn.rs `shown`); an
   > entry's head names its slot, since the fold lies in the slot's columns;
   > the cast forgets the entries folded into the frame after, decor being
   > generated after it (pxl-render gpu/ray/cast.rs `folding`). What is
   > folded is what is retained at the window's zoom, a subset, as said
   > above; a share under a unit (`window::FOLD_UNIT`) is none, and a top
   > cell kept whole takes none it can show. The CPU reads a brick back once
   > its fill lands, before decor generates on it, so its node holds no
   > folded decor: the one answer the GPU and the CPU give apart, where only
   > a coarse cell differs, which no walker or pick asks. Measured the same
   > day, orbiting 30 s over a park in Wonju, the fold built and undone:
   > decor's `generate` span p95 1.2 and 1.4 ms with it, 1.3 without; the
   > frames' GPU was not compared, the views having come out apart. On
   > screen the fold is faint: a retained tree is a quarter of the leaves'
   > density a zoom coarser, and three eighths of a z23 cell.
3. The puck's zone hides where the ground is drawn coarser than a leaf; the
   actor takes rule 2.

### To check before building

- A coarser window's brick spans more leaf cells: whether `BRICK_PATCHES`
  and `BRICK_RECORDS`, worked out for a leaf brick, bound it.
- What the fold's look is for a decor: a color its asset declares, or its
  skin's mean as a distant cell's is (pxl-render `mesh/paint/palette.rs`).
- Whether a decor standing as itself at a coarse window is struck, where a
  folded cell is not (ARCHITECTURE.md, the layers); the ghost picks nothing
  coarse either way.
- The actor between the two floors: the model kept, or an impostor of its
  pose.
- The cost a screen: the windows are the camera grid's, so the cells
  generated a frame stay about the same however far the eye is; measured,
  not assumed (`performance`).
