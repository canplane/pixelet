# What is drawn is an asset in a frame

**2026-10-24** — decided in the conversation; not built yet. What the code
already does is cited, and where it does not.

## The structure

- An **asset** is anything in the content library, as an engine's are: a
  definition (a cube's look, a decor's variants, a marker's design, a cloud,
  a character or a pet) and the shipped files a definition references (a
  glb, a png, the texture pack), as a prefab references a mesh. The catalog
  (`spec/`) is where assets are declared. An **asset id** is what today's
  `spec/spec/block/id.json` calls a block's id: its kind over its shape. A
  **marker** is any asset drawn flat to the camera from a design on the
  sheet: a pin, a place, a chat line's bubble, each its own design. What it is
  (how it is drawn, what it occupies, what it is at a coarser zoom) is its
  own metadata.
- A **frame** is a coordinate system, which may move: the world's grid, a
  character, the pointer, the wind's field, a preview's case.
- An **instance** is an asset in a frame at a pose, with what is its own
  alone: a clip's time, a joint palette. A **block** is an instance standing
  in a cell of the world's grid, and nothing else is: a cloud claims no
  cell, so it is no block.
- A **key** is the holder's: the name a holder calls an instance by — a world
  cell in the object store, a place's tile and index, a speaker. It is never
  part of what the asset is; an asset's painted or uploaded form is found by
  its content.
- **A holder treats every child alike.** It asks each the same questions —
  drawn at this pose, what it occupies, what it is at this zoom — and the
  answers are the child's. Whether a child is a cube, a decor or a marker is
  the child's to know, as a directory need not know whether an entry is a
  link, a regular file or a pipe.

So a character is a frame the walk moves, and its model and its chat line
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
  alike (`model.rs`).
- **The cumulus** is decor drawn on the wind's moving field and held as decor
  is not (`scene/environment/cloud.rs`).
- **The ghost** is a cube's look or a decor's model in the pointer's frame
  (`scene/indicator/ghost.rs`).
- **A preview's case** draws a character into its own canvas
  (`scene/character/preview.rs`).

## Where it does not

1. **Markers** (`scene/place.rs`, `billboard/badge.rs`).
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
     (pxl-render `gpu/pass/badges.rs` `labeled_pill`, `count_pill`), made a
     kind of its own in `Label`, `DrawSpec` and `Thing`.
2. **The chat line is in no frame.** Each tick the companion computes the
   point over its person's head and hands it over (`scene/character/
   companion.rs` `speak_from`). The person, the pet and the line each carry a
   world position the companion works out.
3. **A preview's case registers its model again** at one unit tall
   (`scene/character/preview.rs`): a scale, which is an instance's pose,
   made a second asset.
4. **The clouds are declared nowhere.** Their three models and their skin
   are written in the kernel (`scene/environment/cloud.rs` `VARIANTS`,
   `SKIN`), with no id and no catalog entry, though they are drawn as decor
   is. And the file table, `pxl-spec asset/ingested.spec.rs` `Asset`, holds
   the word asset for files alone.
5. **The kernel knows which assets exist.** The socket is already there:
   the kernel opens an asset on the `ASSET` root by a number
   (`io/load.rs`), and the host's backend reads the bytes that number is
   (`apps/client/src/host/io/asset.ts` `read_asset`). But the number is an
   index into the file table compiled into the kernel (`Asset`, 163 files),
   and what each file is for is compiled in beside it:
   - the characters and pets: `spec/client/kernel/model/variant.json`,
     paired to files in `scene/character/variant.rs`, apart from the block
     catalog;
   - the decors: `spec/spec/block/decor.json`;
   - the clouds: written in the kernel.

   The texture pack is one past the table's end (`asset.rs` `Load::Pack`,
   and `load === KEYS.length` on the host's side). The table's counts size
   the memory at build time (`door/layout.rs`).
6. **Control asks by kind** where it asks what a block occupies or hides:
   `control/interaction/collision.rs` (markers), `edit.rs` (`hide_in`), and
   `ghost.rs` (`Ghost::Box` or `Ghost::Decor`). Whether each is a question the
   child should answer is open.

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
2. **Frames.** A character is a frame, with its model and its line, a
   marker, instances in it. Whether the line shows is the frame's to say,
   not the world grid's chunks'.
3. **A preview's case** draws the one registered model, scaled by its
   instance.
4. **Control's questions by kind**, wherever one is better put as the child's
   answer.
5. **The catalog is an asset the host plugs in.**
   - The kernel reads the catalog through the socket at boot: every asset's
     id, its kind, and the ids of the files it references. It then asks for
     those files by id.
   - The texture pack is the file the cube kind's definition references.
     The characters, the pets, the clouds and the decors' models are
     references like any other.
   - The kernel keeps only each kind's behaviour: painting a cube, striking
     a decor, playing an actor's clips, a marker's designs. It knows no
     asset by name.
   - This reverses "A catalog is never sent, because both sides compile it"
     (ARCHITECTURE.md, "Catalogs and assets"), and the memory's bounds are
     then sized at boot, once the catalog is read.

## What stays

- The world's grid is the world's frame. The object store keeps its
  instances by position, and the query reads them unchanged.
- Each kind's way of being drawn is its own: the rays for the world's cubes,
  the GPU's decor for what the ground grows and what is placed.
