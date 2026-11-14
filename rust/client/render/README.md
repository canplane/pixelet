# pxl-render

The renderer turns the core's frame into calls on the GPU. It finds the world
with rays, draws what the rays found with a raster, and remembers it, so the
work of a frame follows what changed on the screen. Everything it computes about
the world is the same rule the CPU computes in pxl-engine, written again in WGSL
and held to it by tests. This README is how those parts work; why the system is
shaped this way is in [ARCHITECTURE.md](../../../ARCHITECTURE.md).

## The cast

The cast (`ray/cast.rs`) runs once a frame over the camera grid's angular
cells. Beam rays at each cell's corners walk the part of the descent the cell's
pixels share, and the cell starts where the nearest of them stopped (`cell_cs`).
Samples, a few to a cell's side, walk on from there, and the node each stops on
is drawn as a box (`sample_hit_cs`). The beam rays and the samples are items of
one queue (`queue_cs`), walked by the same code as the holes later.

The raster (`ray/raster.rs`, `raster.wgsl`) draws every box of the record
into a visibility buffer. A face keeps a pixel only where the pixel's own ray
enters the box across that face, so the depth test keeps the nearest box the ray
enters, at the point where it enters. The eye draws only fresh voxels; the light
draws its stale ones behind the fresh.
`cast_cs` then reads each pixel: a fresh voxel gives the pixel its ray; a pixel
no sample's cell met anything in is sky; any other pixel is a hole, listed for
the queue, and walked to its first hit (`queue_walk.wgsl`, `queue_ask_cs`,
`queue_last_cs`). A walk that meets a node the GPU does not hold parks, and the
node is asked for; one that may ask no more in its frame ends there as a lack,
and shows the deepest node held: the node it stands at as a cell of its own
zoom, or the nearest over it that the world holds.

A ray leaves four words: the asset with its face, zoom and fill eighths; the
distance; the asset ids a coarse cell folds; and which edges and corners
of the face meet the air. The screen pass paints them with the one function
every lit face uses (`screen_ray.wgsl` `px_ray`, `paint.wgsl`).

Two debug views read the cast. In the ray steps view (pxl-engine `eye::STEPS`)
the third word holds, in place of the asset ids, the steps a ray took on
its way from the eye, which the screen pass paints on a log ramp: its beam ray's
descent from the root and its own walk from the cell's start for a hole, and for
a pixel that took a voxel or the sky without a walk, those of the sample before
it on the grid's lattice (`cast.wgsl` `taken_word`). Under the camera gizmo the screen pass tints each face by the
solid angle it shows the eye against a grid cell's (`screen_shade.wgsl`
`cell_share`), and the camera grid is drawn over the canvas
(`camera_grid.wgsl`).

## The node cache and the record

What the rays asked of each node is kept between frames in the node cache, a
table keyed by the node (pxl-engine `node_cache.rs`). An entry is forgotten
where what its node read changed: a box of damage, a cut, or a tile that landed
under it (`forget_cs`). Where the bucket an entry needs is full, the cache gives
up one that has not been drawn for two frames (pxl-engine `record.rs`). A few
rare events forget every entry at once: what was placed changed by more than the
stream has room to send, or by revisions the GPU never held, or by a write that
names no box, and the casts outrunning the frames a key's tag tells apart
(`ray/cast.rs`). That is correct, and costs only a cast of everything again.
The entries a ray stopped on are the record, and the raster draws every fresh
one in view each frame (`rec_draw_cs`). A frame whose rays reached little
still draws everything found before that is still fresh; a still frame over a
still world draws nothing at all.

## The decor the GPU generates

Decor the ground grows is stored nowhere: the GPU generates it from the same
clip levels the rays read, and placed decor arrives beside it as records in
the same layout (`gpu/generator.rs`). Its passes redo only the entries whose
brick, damage or distance changed, and nothing generated is read back.

## Time: the scale and the rate

Two knobs give the GPU room. The scale draws fewer pixels, and buys back only
what is done a pixel at a time: the pixels' cast and holes, the visibility
buffer, the world's passes, the occlusion. The beam rays, cells, samples and
boxes run on angular cells and do not shrink, and the finish and the markers
always draw at the display's pixels, so text stays sharp. Each GPU span says
which kind it is (span.json5 `GPU_SPAN_PER_PX`). The rate spaces the frames, a refresh at a
time, down to 30 Hz (`RATE_MIN_HZ`), and gives every pass more time.

`auto` steers both as congestion control steers a connection. While the GPU's
time runs over the frame's budget, a knob steps down, the rate first; a knob
steps up, the scale first, where the step above would fit, one step for each
settling window. Without timestamps only the scale moves, and a step up is a
probe whose failure doubles the wait before the next.

## The shadow map

The sun is a second observer. The same passes run for it over the parts each
observer gives (`beam.eye.wgsl` and `beam.light.wgsl`, `sample_run.eye.wgsl`
and `sample_run.light.wgsl`), into a map of the world's depth along the light
in a square box about the eye (`pass/shadow.rs`). Each texel is fixed in the
world and held in a slot by its index modulo the map's side, and its ray starts
at a fixed point, summed in integers (`light_aim.wgsl` `texel_start`).

Each tile of the box asks one zoom, its band (pxl-engine `Light::band`): the
zoom the eye's rays ask at the point of the tile's column nearest the eye,
measured from the box's middle. A band depends only on where the tile lies in
the box. When the eye moves, the box slides a tile at a time; a tile that slid
on, or into another band, is cast anew and the rest keep what they cast
(`light_slide_cs`, `Light::keeps`).

The effects on a drawn surface, its lines, the occlusion of its corners and of
the screen, the shadows and the rim, all end at one distance from the eye, the
**effects' reach**, and fade alike before it (pxl-spec `EFFECT_REACH`,
`effect.wgsl` `effect_share`). It lies where the camera grid's cells a zoom
coarser than the ground's leaf begin, and the box reaches at least that far.

The light casts only what a receiver reads. Each voxel the eye sees, each pixel
it walked, each placed decor and each drawn character marks the tiles it stands
on within the effects' reach (`rec_draw_cs`, `light_mark_walked_cs`,
`light_mark_draws_cs`). A tile is read if it lies sunward of one stood on,
within the tallest decor's height (`light_propagate_cs`), and only the tiles
read anew are cast. Where nothing within the effects' reach is seen, the light
does no work past the receivers.

Decor and characters move, so they are drawn into the map each frame as depth
only. A receiver compares nine taps, each moved by its own plane's slope, and
casters are pushed by the same formula, so no plane shadows itself
(`shadow.wgsl` `shadow_visibility`).

## The frame

Each frame the core hands a draw list (`gpu/draw.rs`) of what it draws beside the
world: the cumulus, the characters, the location indicator, an edit's ghost, the
markers and the debug lines. A draw holds its position as a `Q3` and a remainder
in `f32`, and the shader takes the wrapping integer difference from the eye
(`frame.wgsl` `from_eye`). The renderer (`renderer.rs` `render_frame`) draws
every open surface:

```text
ahead       impostor atlases, the atmosphere's tables, the air's volume
cast        the screen's rays; decor generated; the shadow map
world pass  generated decor → rays at their depth → generated impostors →
            the occlusion multiplied in → sky and stars at the far plane →
            characters, cumulus, impostors → high clouds → indicator, ghosts
occlusion   N8AO at half resolution, off the depth the world pass laid (ao.rs)
finish      the world's color encoded onto the canvas once
over        markers → debug lines → camera grid → character surfaces
```

The calls are written through xpute's encoder into one stream (`gpu/api.rs`),
which the page runs on WebGPU and the device on wgpu. A frame where only the
draws the core bounds moved is drawn again in the screen's tiles alone
(`gpu/tile.rs`): what moved marks the tiles it covered and covers, the light's
change is carried to the screen's tiles that read it (`pass/propagate.rs`), and a
tile stays in the damage until its history has gathered the jitter's frames
(`pass/resolve.rs`). The CPU decides from what it handed whether a frame is drawn
whole, through tiles, or not at all, so nothing is read back to decide it.

## What it holds

The GPU's memory has no sum the renderer keeps to, only a bound on each part.
The terrain's bricks and palette are fixed at build time, and the ways' pool
grows to a cap the build checks. What is sized by the screen is capped by the
area drawn (`gpu/surface.rs`). The objects' tree, placed decor, the models and
the images grow with what they hold, each buffer checked only against what
every device can bind (`frame/shader/floor.rs` `fits`). Past that check the core
stops. A device that runs out of memory first fails on its own terms: no error
scope is kept, so a buffer it could not make is an error buffer, and a device it
loses comes back through `on_gpu_lost`, which makes everything again.

## Making the renderer

The renderer is made before its first frame, so that making it never holds the
GPU process's main thread, which every frame the browser presents waits on
(`gpu/pipeline.rs` `PipelineCache`). Shader modules are made a few a frame, within
`MODULE_BYTES` of the stream; then every pipeline is asked for by WebGPU's async
call, which compiles off that thread and reports through a ticket's word; and no
frame is drawn until every word says its pipeline is made. The snapshot carries
how far it has come (`renderer_progress`), which the UI's bar shows. A pipeline
asked for once the renderer draws is a bug (`EBUSY`). wgpu has no async call,
and the device's host answers the same op at once.

## The shaders, held to the engine

The shaders are WGSL by hand, assembled from parts (`frame/shader/part.rs`), and
the layouts and numbers they read are generated from pxl-engine and pxl-spec by
`build.rs`.

- Every compute pass over the world (the cast, the node cache, the light's and
  decor's) is held word for word to pxl-engine's Rust by a test over a real
  world's buffers (`tests/`), but for the two that `frame/shader/part.rs`
  names: the light's slide, held looser, and the cells' starts, held only
  through the samples that start from them.
- Every walk ends at pxl-engine's `cast::STEPS_CAP` steps.
- A value both sides would divide by or take the root of is rounded once on the
  CPU and handed in a word, and both sides multiply by it. The eye's ray at each
  pixel and a beam's width, which only the GPU walks, are worked out on the GPU
  alone, each in one function (`walk_node.wgsl` `eye_aim`, `spacing_width`). A
  cell's window, where its rays start, is worked out on each side
  (`cell.wgsl` `cell_window`, pxl-engine `Grid::cell_window`): a GPU's root may
  round it one less, which pxl-engine `param.rs` `RADIUS_SLACK` covers.
- The build validates every shader with naga, and checks each pipeline's groups,
  render and compute, against the bindings its shaders read, both ways
  (`frame/shader/record.rs` `lays_out`); no struct of a shader holds a struct
  (`build/check.rs` `refuse_faults`).
- A test compiles each shader with Tint, the compiler a browser runs, and makes
  every pipeline the renderer makes (`ts/tool/shader/shader.test.ts`).

The passes over the screen's tiles, the air's, the clouds' and the location
indicator's are not held to the engine; naga and Tint check them as they check
every shader.

## The sky, the air and the clouds

The sky is drawn at the far plane, so only pixels no ray met run it
(`sky.wgsl`); where such a ray falls to the land, it shows flat land shaded from
the earth layer. The air between the eye and what is drawn is Hillaire's
aerial-perspective volume (`atmos_froxel.wgsl` `froxel_cs`), read by each drawn
point at its direction and depth (`atmos_air.wgsl`). Neither the sky view nor
the volume reads the view's direction, so turning the eye keeps both, and both
are drawn again only where the light's height or the eye's altitude has moved by
half the step over which the sky would change by a display level. The exposure
is measured by its own pass (`exposure_cs`).

The clouds are three layers on one wind (pxl-content `cloud.rs`): the cumulus,
models drawn like decor, and above them altocumulus and cirrus, planes read
where a ray meets them (`cloud.wgsl` `sheet_of`).

## Markers

Markers are one instanced quad over one texture packed shelf by shelf, the atlas
(`frame/shelf.rs`, `marker.wgsl`). A sprite is found by what it paints, so equal
designs share one place, and it is drawn on the GPU only when what it shows
changes: its shapes as signed distances, its text as lines the platform drew.
