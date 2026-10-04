# The view is its own crate

**2026-09-17** — worked out in a conversation and split the same day.

## The line

The Rust was three crates and a module: `pxl-shared`, `pxl-core` with a
`render/` inside it, and `pxl-wasm`. `render/` read the kernel wherever it
needed something — `kernel()`, the settings store, the companion's skins,
the preview's cases, the decor store, the environment, the billboard sheet,
`request_world_tick` — and the kernel read `render/` back, so neither could
be built or reasoned about without the other.

The picture it was split to:

```
xpute-core ← pxl-shared ← pxl-render ← pxl-core ← pxl-wasm
             the model     the view     the kernel
```

- **The model is `pxl-shared`**, the server's and the client's: the monorepo
  shares one model between them, so nothing that only draws belongs in it and
  nothing a server would read is left outside it.
- **The view is `pxl-render`: how the model looks when drawn.** The voxel
  volumes and the meshes a chunk bakes into, the terrain's elevation, the LOD
  ladder, glTF, the billboards' sheet, the markers' style, the draw list, the
  shaders and pipelines, and the device that draws them. glTF is here because
  a model's file is what it draws.
- **The kernel is `pxl-core`: what runs.** The stores, the fetches, the
  schedulers and what they hold, the turn; what is drawn and when. It calls the
  view; the view never names it.

The server does not have to depend on the view, and a check of the view
cannot pass on the kernel's behalf.

## Precedents

- **Veloren** keeps its model in `common`, shared by `client` and `server`,
  and its terrain meshing and animation in `voxygen`, the crate that draws —
  the split of model from view taken here.
- **Godot's RenderingServer** knows no scene node: the scene tree creates
  resources by handle and pushes what changed. That is the direction taken
  here — the kernel pushes, the view does not pull.
- **Bevy** goes the other way, the render world extracting from the main one
  at a fixed point. The hand-off point is what was borrowed: one call a frame.

## What crossed and how

- **A frame's inputs are handed over** (`gpu::renderer::FrameInput`): the draw
  list's lanes, the environment's lane, each decor group's model block and
  skin, the actors' skins, the picker's cases, the pack sheet as a function
  asked until it is drawn, the billboard atlas, and whether the device is a
  phone's. What comes back (`FrameOutput`) is whether a surface waits another
  turn to settle and what the frame cost. The kernel's side is
  `module/world/frame/stage.rs`, which also writes the stage lane the view
  reads, and turns the cost into the ledger's row.
- **A turn asked for by the host's own act** — a device answered, a canvas
  handed over or resized — is asked by `pxl-wasm` after the call, not by the
  view.
- **Layouts moved to whoever reads them against a shader:** a model vertex's
  words, the stage lane and its edge constants.
- **The light is composed by the view** (`environment.rs`): the theme's floats,
  the environment lane's layout and the composition of the two lights, the
  fog and the turned sky. The kernel keeps the clock, the theme, where the sun
  and the moon are and the easing toward the theme, and hands the view a
  `Light` each tick.
- **An object entry's layout moved to the model** (`bake::constant`): the band,
  the packed position and its tiers are the object store's record format, and
  the view paints a column's entries as lanes (`layer::object::adapter::Entries`)
  rather than reading the store.
- **Left in the kernel:** the chunk ranges a fetcher and the scheduler hold,
  the raycast over the view's volumes, and a model's registration into the
  scheduler's blocks (`module/world/model_asset.rs`).

## The trees, by kind

The kernel's tree was the TypeScript app's (`app/`, `lib/env`, `module/{camera,
device, map, world}`), and `module/world/` held a scheduler, a ledger, the
fetches, the turn and every layer side by side. Both crates were sorted by what
a thing is instead, and a crate's root holds its directories and their barrels.

- **pxl-core:** `sys` (the kernel's state, the host, the memory), `sched` (the
  turn, the tick, priorities, the ledger), `world` (the change log, the frame,
  the chunks and their scheduler, the resident pages), `layer` (what stands in
  the world: terrain, objects, decor, POI, characters, billboards, markers,
  the environment), `io` (fetches, their scheduler, loads, a model's
  registration), `control` (what moves the world: the camera, the sensors,
  navigation) and `shell` (what the shell reads and asks: the snapshot, the
  config, the settings, the map's controls).
- **pxl-render:** `mesh` (voxels to a mesh, and the paint of terrain, objects
  and the palette into voxels), `asset` (glTF, the billboards' atlas), `look`
  (LOD, the light, color, the markers' style), `frame` (the draw list, the
  stage, the shaders) and `gpu` (the device, and a pass for each of badges,
  decor, the puck and actors).

A file inside a directory that already names its concept lost the prefix it
carried (`chunk_node` is `world/chunk/node`, `gltf_cache` is `io/load`,
`ledger_spec` is `sched/ledger/spec`, `poi_layer` is `layer/poi/placement`,
`decor_layer` is `layer/decor/build`). A decor kind is one file under
`layer/decor/kind/` — its seed and rules and its bundle together, beside the
spec `kind.json` generates — where it was a `_kind` file and a `_decor` file.

## What is not done

- `pxl-shared` still holds what only the offline bake reads (its raw rows and
  the feature model). Separating that waits until a server crate reads the
  model.

Nothing here has been run; `cargo check`, clippy on wasm32 and natively on
every target, and the spec check passed.

## The crates' names

The same day, two crates were named for what they are. `pxl-shared` is
`pxl-spec`: everything in it — the space and its coordinates, the cell
catalogs, the command and message numbers, the bake's formats, the assets'
tables — is a definition every side follows, with its reference
implementation, and "shared" named who uses it, which is the TypeScript
monorepo's convention rather than a Rust crate's. Its inner `spec/` level went,
so a path is `pxl_spec::cell` rather than `pxl_spec::spec::cell`. `pxl-core` is
`pxl-kernel`, which is what its own docs called it, and `core` in `xpute-core`
means a generic kit, not the center of pixelet.

The TypeScript package keeps `@pxl/shared` for now. Once the server and the
bake tools are Rust, what the shell still reads of it (the command numbers,
the catalogs, the assets' paths) is generated into the web app from the JSON
the way the other `*-spec.ts` files are, and the package goes.

## The spec's source and the shell's tree

The JSON every language is generated from lay beside whichever code first
read it — the kernel's and the view's modules, and the TypeScript packages the
server's move will retire. It is one tree at the root now, `spec/`: pxl-spec's
at the top (`cmd.json`, `cell/`, `bake/`), and `kernel/`, `render/` and
`xpute/` below, each at the path of the module it generates. `script/spec.ts`
is the only reader.

The web app's tree was the React app it grew from (`app/`, `bootstrap/`,
`lib/env`, `module/`, `feature/`, `page/`, `shared/`, `wasm/`). It is sorted
by what the shell does instead: `kernel` (the driver of the kernel and its
module), `device` (the host's WebGPU), `config`, `locale`, `app` (the entry,
the router, the pages, the providers, the store, the style), `ui` (the
screens), `catalog` (the data the screens show), `dev` (the ledger's tools)
and `kit` (the shared parts). A file whose directory names its concept lost
the prefix, as in the crates.
