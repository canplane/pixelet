# What is drawn is an asset in a frame

**2026-10-24** — decided in the conversation; not built yet. What the code
already does is cited, and where it does not.

## The structure

- An **asset** is anything in the content library, as an engine's are: a
  definition (a cube's look, a decor's variants, a marker's design, an
  actor, a cloud) and the shipped files a definition references (a glb, a
  png, the texture pack), as a prefab references a mesh. The catalog
  (`spec/`) is where assets are declared.
- An asset's **kind** is one of five: **cube**, **decor**, **marker**,
  **actor** and **sky**. An **asset id** is its kind over its shape, as
  today's `spec/spec/block/id.json` calls a block's id. These are the
  internal names; what the UI shows them as is the locale's (the drawer's
  "Character" is the actor kind).
  - A **marker** is any asset drawn flat to the camera from a design on the
    sheet: a pin, a place, a chat line's bubble, each its own design.
  - An **actor** is a person or a pet: a model with clips.
  - **Sky** is what hangs in the sky: the clouds, the Moon's picture.
- What an asset is (how it is drawn, what it occupies, what it is at a
  coarser zoom) is its own metadata.
- A **frame** is a coordinate system, which may move: the world's grid, a
  person, a pet, the pointer, the wind's field, a preview's case, the
  device's fix.
- An **instance** is an asset in a frame at a pose, with what is its own
  alone: a clip's time, a joint palette. A **block** is an instance standing
  in a cell of the world's grid, and nothing else is: a cloud claims no
  cell, so it is no block.
- A **key** is the holder's: the name a holder calls an instance by — a world
  cell in the object store, a place's tile and index, a speaker. It is never
  part of what the asset is; an asset's painted or uploaded form is found by
  its content.
- **A holder treats every child alike.** It asks each the same questions —
  drawn at this pose, what it occupies, what it is at this zoom, where a tap
  goes — and the answers are the child's. Whether a child is a cube, a decor
  or a marker is the child's to know, as a directory need not know whether
  an entry is a link, a regular file or a pipe.

So a person is a frame the walk moves, and their model and their chat line
are two assets in it: neither is anchored to the other.

Alike means one interface, not one behaviour. How a kind is drawn is its
own: a cube in the world's grid is found by the rays through the objects'
tree, the same cube in the pointer's frame is a box mesh, and a kind is still
drawn wholly on one side. Only the world grid's instances are the world —
what `world::node`, the walk, an edit and the fold read. An instance in any
other frame is drawn and nothing else, which is what a ghost or a chat line
wants.

## What already holds

- **The draw list.** Every draw is a handle, its group and a translation
  about the eye (pxl-render `frame/draw.rs` `draw_scaled`, `draw_as`,
  `draw_instances`).
- **Models** are registered once and played by the actors and stood by decor
  alike (`domain/query/model.rs`).
- **The cumulus** is drawn on the wind's moving field and held as decor is
  not (`domain/scene/environment/cloud.rs`).
- **The ghost** is a cube's look or a decor's model in the pointer's frame
  (`domain/scene/indicator/ghost.rs`).
- **A preview's case** draws an actor into its own canvas
  (`domain/scene/character/preview.rs`).
- **The socket.** The kernel opens an asset on the `ASSET` root by a number
  (`sys/io/loader.rs`), and the host's backend reads the bytes that number is
  (`ts/client/src/namespace/asset.ts` `read_asset`).

## Where it does not

1. **Markers** (`domain/scene/marker/`).
   - The collector names every kind it shows, `Label { Place, Marker, Count,
     Chat }`, and decides for each:
     - the pins' clustering by marker cell (`markers`, and `step` where a
       cell's markers become a `Count`);
     - whether the chat line shows, by the marker kind's `MARKER_SHOWN_FROM`
       and the chunk its speaker stands in (`block/marker.json`, `Label::at`);
     - where a tap goes (`tap`).
   - The sheet's cache is keyed by `Label`, which carries the holder's key:
     a pin's address (`EntryAt`, its cube at its zoom) and a cluster's cell.
     One design is so painted once per position.
   - A count is the pin's design with a number in it and its colors swapped
     (pxl-render `gpu/pass/marker.rs` `labeled_pill`, `count_pill`), made a
     kind of its own in `Label`, `DrawSpec` and `Thing`.

   > Since the marker work: no `Label` and no count kind remain. The
   > arrangement asks every source's `Instance` alike, by its holder's `Key`
   > (`domain/scene/marker/arrangement.rs`), and a design is found by what
   > it paints, painted once however many show it (`design.rs`). A count is
   > a pin's pill in its colors turned over (`pin.rs`, pxl-render
   > `gpu/sheet/shelf.rs` `Paint::Pill`), the clustering the pins' own rule
   > (`block/marker.json`, `marker_cell`), and the chat line shows by its
   > own rule (`chat.rs` `hangs`).
2. **The chat line is in no frame.** Each tick the companion computes the
   point over its person's head and hands it over (`domain/scene/character/
   companion.rs` `speak_from`). The person, the pet and the line each carry a
   world position the companion works out.
3. **An actor's instances are in one shared table**, `domain/scene/actor.rs`
   `Actors`, sized by its users' bounds (`ACTORS_MAX`, `CASES_MAX`), beside
   `domain/scene/character/`. Playing clips is the actor kind's behaviour, and a
   clip's time and palette are an instance's, held by its frame.
4. **A preview's case registers its model again** at one unit tall
   (`domain/scene/character/preview.rs`): a scale, which is an instance's pose,
   made a second asset.
5. **The clouds are declared nowhere.** Their three models and their skin
   are written in the kernel (`domain/scene/environment/cloud.rs` `VARIANTS`,
   `SKIN`), with no id and no catalog entry, and drawn with decor's draw
   groups and its by-size draw (`domain/scene/decor/cell.rs`, which only the clouds
   use). They are the sky kind's.
6. **The kernel knows which assets exist.** The number the socket takes is
   an index into the file table compiled into the kernel (`pxl-spec
   asset/ingested.spec.rs` `Asset`, 163 files, which also holds the word
   asset for files alone), and what each file is for is compiled in beside
   it:
   - the actors: `spec/client/kernel/domain/query/model/variant.json`, paired to files
     in `domain/scene/character/variant.rs`, apart from the block catalog, and
     named for its entries rather than its subject;
   - the decors: `spec/spec/block/decor.json`;
   - the clouds: written in the kernel.

   The texture pack is one past the table's end (`sys/load.rs` `Load::Pack`,
   and `load === KEYS.length` on the host's side). The table's counts size
   the memory at build time (`door/layout.rs`).
7. **Control asks by kind** where it asks what a block occupies or hides:
   `domain/control/interaction/collision.rs` (markers), `edit.rs` (`hide_in`), and
   `ghost.rs` (`Ghost::Box` or `Ghost::Decor`), and `turn.rs` turns each
   `Ghost` into a call of its own. Whether each is a question the child
   should answer is open.
8. **The top sums its owners' bounds by name** (`door/layout.rs`
   `MODELS_MAX`, `SINGLE_BLOCKS`). `SINGLE_BLOCKS` is 4 for five owners — the
   ghost, the puck, two debug line sets (`domain/scene.rs` `debug`, `coverage`) and
   the billboards — and the doubling in render's `gpu/block.rs` `block_slots`
   hides it.

   > Since fixed: `SINGLE_BLOCKS` is 5, one for each owner
   > (`door/layout.rs`). The top still sums its owners' bounds by name.

## What it asks, in order

1. **Markers.**
   - The sheet's cache is keyed by what is painted: the design, its words
     and its colors. A slot is freed when no instance uses it.
   - The collector lists instances by their own keys, and each asset gives
     its design.
   - The pins' clustering is the pins' own rule, declared with the marker
     kind in `block/marker.json` beside `MARKER_SCALE_STEPS`, whose cell it
     already is.
   - `Label::Count`, `DrawSpec::Count`, `count_pill` and `Thing::Count` go.
2. **Frames.** A person and a pet are each a frame, holding its actor's
   instance; the person's holds their line, a marker. Whether the line shows
   is the frame's to say, not the world grid's chunks'. The `Actors` table
   goes, and each frame declares its own bounds.
3. **A preview's case** draws the one registered model, scaled by its
   instance.
4. **Control's questions by kind**, wherever one is better put as the child's
   answer.
5. **The catalog is an asset the host plugs in.**
   - The kernel reads the catalog through the socket at boot: every asset's
     id, its kind, and the ids of the files it references. It then asks for
     those files by id.
   - `id.json`'s `KIND` takes the actor and the sky; `variant.json` becomes
     `actor.json` in the one catalog, and the clouds are declared there.
   - The texture pack is the file the cube kind's definition references.
     The actors, the clouds and the decors' models are references like any
     other.
   - The kernel keeps only each kind's behaviour: painting a cube, striking
     a decor, playing an actor's clips, a marker's designs. It knows no
     asset by name.
   - This reverses "A catalog is never sent, because both sides compile it"
     (ARCHITECTURE.md, "Catalogs and assets"), and the memory's bounds are
     then sized at boot, once the catalog is read.

## The kernel's folders

ARCHITECTURE.md's figure is the order: the top wires everything, the domain is
a DAG whose asks go down and whose answers come back up, and what all of it
stands on is below it. The folders show that order, and nothing else is a
sibling of it.

```text
kernel/src/
  turn.rs  door/  shell.rs   the top: wires everything; nothing below names it
  domain/                    the world and the eye, two heads over one trunk
    scene/                   what this frame draws
    control/                 what moves the eye and asks the query
    view/  query/  layer/    the trunk both heads stand on
  sys/                       what all of it stands on
    io/  sched/  mem/        and, at its root, the load table io and sched read
  profiler/                  beside: reads anything, and no decision reads it
```

- **The domain and the system** are named as modules are: a domain module
  by its whole word, a systemic one by its conventional abbreviation
  (the `convention` skill).
- **`chunk` goes.** It is one consumer's partition now (the markers'), while
  decor gathers over the eye's windows; the markers gather over the windows'
  squares too, and the z-range test that counted a place in one chunk of its
  column is not needed.
- **A leaf sits under its lowest reader,** so every edge points down: the
  billboards under `scene/`, decor and the models under `query/`, the settings
  under `view/`, the camera fixture under `control/camera/`, the commands'
  spec under `control/`, the directory's under `door/`, the host's under
  `sys/io/`, and each tuned number under the lowest rung that reads it (the
  ways' pool under `layer/terrain/`). The easing two heads share sits at the
  domain's root, or in xpute's kit, which it already imports.
- **The crate root's `Kernel` global** splits: the scene's state into
  `scene/`, the draw list beside the models, its lowest reader.
- **Open:** where each kind's behaviour (cube, decor, marker, actor, sky)
  sits once the catalog is plugged in, and the frames' folders under
  `scene/` (the world, a person, a pet, the pointer, the wind, the fix, a
  case).

## Open

- **The world's rule stays compiled.** The cube value space, the ground's
  scatter of decor and a decor's claimed box are compiled for the GPU too
  (`pxl-spec-core cube.spec.rs`, `decor.spec.rs`; ARCHITECTURE.md's "one
  source compiled for the CPU and, through rust-gpu, for the GPU"), and the
  bakes compile `cube.json`. What the socket carries is the kinds and the
  files they reference, unless that half becomes buffers uploaded at boot.
- **Bounds at boot.** `.fixed`'s block slots and render's draw groups are
  sized at build time today; whether they can be laid after the catalog is
  read is to be checked against `sys/mem/section.rs`.
- **The spec tree.** `spec/spec/block/` holds the one catalog; whether it
  becomes `spec/spec/asset/`, and what the file table is named then.
