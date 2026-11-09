// pxl-render/frame/shader/part.rs

//! Every shader's number, and the WGSL parts it is assembled from, in order. The crate
//! compiles this file for the numbers. build.rs includes it to assemble each shader
//! (record.rs `src`), validate it, and read off it which stages read each binding (record.rs
//! `pipeline`), so a pipeline's groups take their visibility from its shader and from nowhere
//! else. Each shader's parts follow constant.wgsl. A shader is one module, made once, and each
//! pipeline over it names the entry points it runs: a render pipeline names them in its
//! record, and a compute pipeline by its number (record.rs `compute`).
//!
//! A shader called a port by hand below is WGSL written after a rule of pxl-engine's Rust, and
//! a test on a device holds what it writes to that rule, word for word over a real world's
//! buffers (tests/), with two exceptions. The test of the light's slide and propagation builds
//! no world (tests/light_reads.rs), and it holds the slide only to setting to none every tile
//! the rule cannot keep, so the slide may also set to none a tile it could keep. No test reads
//! the cells' starts that `SHADER_CELL` writes: they are held only through the samples that
//! start from them, which must stop where the Rust's walk from the root stops
//! (tests/sample.rs).

/// The names a shader's vertex entry points take. A pipeline names its vertex entry by its
/// index here (record.rs `Header`), and build.rs reads which of them reads each binding.
pub const VERTEX_ENTRY: [&str; 2] = ["vs", "vs_depth"];
/// The name of a shader's fragment entry point, which every pipeline with a fragment stage
/// runs (record.rs `Header`).
pub const FRAGMENT_ENTRY: &str = "fs";

pub const SHADER_BOX: u32 = 0;
pub const SHADER_DECOR: u32 = 1;
pub const SHADER_SKINNED: u32 = 2;
pub const SHADER_LOCATION_ZONE: u32 = 3;
pub const SHADER_LOCATION_BEACON: u32 = 4;
pub const SHADER_MARKER: u32 = 5;
pub const SHADER_SKY: u32 = 6;
pub const SHADER_DEBUG: u32 = 7;
pub const SHADER_MIP: u32 = 8;
pub const SHADER_AO_DOWNSAMPLE: u32 = 9;
pub const SHADER_AO_SAMPLE: u32 = 10;
pub const SHADER_AO_BLUR: u32 = 11;
pub const SHADER_ATMOS_TRANSMITTANCE: u32 = 12;
pub const SHADER_ATMOS_MULTISCATTER: u32 = 13;
pub const SHADER_ATMOS_SKY_VIEW: u32 = 14;
/// Measures the eye's exposure and how far each light reaches the air, one texel a thread
/// (atmos_exposure.wgsl). It has one compute entry point.
pub const SHADER_ATMOS_EXPOSURE: u32 = 15;
pub const SHADER_STAR: u32 = 16;
pub const SHADER_CLOUD: u32 = 17;
pub const SHADER_CLOUD_FIELD: u32 = 18;
pub const SHADER_ATLAS: u32 = 19;
pub const SHADER_IMPOSTOR: u32 = 20;
pub const SHADER_IMPOSTOR_BAKE: u32 = 21;
pub const SHADER_ATMOS_IRRADIANCE: u32 = 22;
pub const SHADER_MARKER_REGION: u32 = 23;
/// Draws the eye's rays at their pixels (pxl-engine ray.rs).
pub const SHADER_SCREEN_RAY: u32 = 24;
/// Ends each drawn pixel's ray on the visible voxel the raster kept at it, or on the sky. Every
/// other pixel is a hole, and gets a bit that marks it to be walked again (cast.wgsl). It is a
/// port by hand with one compute entry point.
pub const SHADER_CAST: u32 = 25;
/// Generates the decor of the ground in the eye's clip levels, one record a thread: each decor
/// stands on its column's surface and takes its pose (decor_pose.wgsl). It has one compute
/// entry point.
pub const SHADER_GENERATION: u32 = 26;
/// The three passes after `SHADER_GENERATION` (pxl-engine `generation::drawn`). The count
/// sizes each record that generation and placed decor leave for the screen, and counts it in a
/// draw region (decor_cnt.wgsl). The args lay out the regions one after another with their
/// indirect args (decor_args.wgsl). The compact writes each drawn record into the decor's
/// instances (decor_compact.wgsl).
pub const SHADER_GENERATION_CNT: u32 = 27;
pub const SHADER_GENERATION_ARGS: u32 = 28;
pub const SHADER_GENERATION_COMPACT: u32 = 29;
/// Lists the eye's slots whose decor generates again this frame, for the dispatches of
/// `SHADER_GENERATION_THIN` and `SHADER_GENERATION` (decor_mark.wgsl). It has one compute
/// entry point.
pub const SHADER_GENERATION_MARK: u32 = 30;
/// Fills a light's share of the air's volume, one column of froxels a workgroup
/// (atmos_froxel.wgsl). It has one compute entry point.
pub const SHADER_ATMOS_FROXEL: u32 = 31;
/// Walks the eye's beam rays over the node cache alone (queue_walk.wgsl over corner.wgsl and
/// beam.eye.wgsl). A ray that meets a node no slot keeps is parked. `SHADER_QUEUE_ASK_CORNERS`
/// then asks the world for that node, and `SHADER_QUEUE_LAST_CORNERS` ends the ray.
pub const SHADER_QUEUE_CORNERS: u32 = 32;
/// Finds where the rays of each cell of the camera grid start, from the beam rays about the
/// cell (cell.wgsl). It is a port by hand with one compute entry point.
pub const SHADER_CELL: u32 = 33;
/// `SHADER_QUEUE_LAST` is the last walk of the holes, which asks the world where the node cache
/// keeps no node (queue_last.wgsl). `SHADER_FORGET` makes the node cache forget what the
/// world's changes have made stale (pxl-engine `node_cache`, forget.wgsl). Both are ports by
/// hand.
pub const SHADER_QUEUE_LAST: u32 = 34;
pub const SHADER_FORGET: u32 = 35;
/// The last walk of the eye's samples, which asks the world where the node cache keeps no node
/// (queue_last.wgsl over sample.wgsl and sample.eye.wgsl).
pub const SHADER_QUEUE_LAST_SAMPLES: u32 = 36;
/// `SHADER_LIGHT` casts the world's depth from the light into its clipmap (pxl-engine
/// `light`): each slot takes the visible voxel, one tile a workgroup (light.wgsl).
/// `SHADER_LIGHT_MAP` lays that depth into the shadow map (light_map.wgsl, pass/shadow.rs).
/// `SHADER_QUEUE_ASK` asks the world, once a node, for each node a walk of the holes was
/// parked at (queue_ask.wgsl).
pub const SHADER_LIGHT: u32 = 37;
pub const SHADER_LIGHT_MAP: u32 = 38;
pub const SHADER_QUEUE_ASK: u32 = 39;
/// Brings the occlusion's half-resolution map up to each pixel and multiplies it into the
/// world's color, drawn as the claimed tiles' quads (ao_apply.wgsl).
pub const SHADER_AO_APPLY: u32 = 40;
/// `SHADER_SAMPLE_BOX` writes the eye's samples into the record of voxels as stops (pxl-engine
/// `record`, sample_box.wgsl). It is a port by hand with two compute entry points.
/// `SHADER_RASTER` draws the record's voxels as boxes, the eye's and the light's, and keeps at
/// each pixel the voxel whose face its ray enters first (raster.wgsl). The cast reads what
/// each pixel keeps (`SHADER_CAST`).
pub const SHADER_SAMPLE_BOX: u32 = 41;
pub const SHADER_RASTER: u32 = 42;
/// Asks the world what lies around each face of the record's voxels, once a face (pxl-engine
/// `raster::ARGS_FACES`, `record::R_FACES`), and around the face each missed pixel's hit
/// entered (face.wgsl). It is a port by hand with one compute entry point.
pub const SHADER_FACE: u32 = 43;
/// The three shaders that list the tiles of the light's box the shadow map's receivers stand
/// on (pxl-engine `light::LIST_RECEIVERS`), then the tiles they read. A receiver stands on the
/// tiles its box covers. `SHADER_LIGHT_MARK` stands the pixels whose rays the holes' queue
/// walked; the record's draw stands the boxes of the voxels the raster drew
/// (`SHADER_RECORD_DRAW`). `SHADER_LIGHT_MARK_DRAWS` stands the decor, and what the core draws
/// that reads the map. `SHADER_LIGHT_PROPAGATE` lists the tiles those receivers read
/// (`light::LIST_READS`, `light::reads_at`). All three are ports by hand (light_mark.wgsl,
/// light_mark_draws.wgsl, light_propagate.wgsl).
pub const SHADER_LIGHT_MARK: u32 = 44;
pub const SHADER_LIGHT_MARK_DRAWS: u32 = 45;
pub const SHADER_LIGHT_PROPAGATE: u32 = 46;
/// Thins the candidates of each slot `SHADER_GENERATION_MARK` listed, one clip level cell a
/// thread, and lays out those it keeps for `SHADER_GENERATION` to generate (decor_thin.wgsl).
/// It has one compute entry point.
pub const SHADER_GENERATION_THIN: u32 = 47;
/// Brings the world's toned color up to the canvas's pixels and encodes it, the one encode a
/// frame takes (screen_finish.wgsl).
pub const SHADER_SCREEN_FINISH: u32 = 48;
/// Gathers the world's color over frames (screen_resolve.wgsl).
pub const SHADER_SCREEN_RESOLVE: u32 = 49;
/// The gate of a frame not drawn whole: it clears each claimed tile to no color at the far
/// plane and raises its stencil (screen_mask.wgsl).
pub const SHADER_SCREEN_MASK: u32 = 50;
/// Shows on the canvas the finished picture kept at the canvas's size (screen_show.wgsl).
pub const SHADER_SCREEN_SHOW: u32 = 51;
/// Claims the screen's tiles whose pixels read a tile of the light's map that changed
/// (screen_propagate.wgsl). It has one compute entry point.
pub const SHADER_SCREEN_PROPAGATE: u32 = 52;
/// Copies the claimed tiles of the history the resolve wrote this frame back into the history
/// the frame before wrote (screen_write_back.wgsl).
pub const SHADER_SCREEN_WRITE_BACK: u32 = 53;
/// The gate of the samples' raster: it clears each claimed tile to no sample at the far plane
/// (raster_mask.wgsl).
pub const SHADER_RASTER_MASK: u32 = 54;
/// Writes the frame's number into the word of each tile the frame's one mark covers, or of
/// every tile when the frame is drawn whole (screen_mark.wgsl). It has one compute entry
/// point.
pub const SHADER_SCREEN_MARK: u32 = 55;
/// The gate of the light's map: it lays the world's depth in the map's claimed tiles and
/// raises their stencil (light_mask.wgsl).
pub const SHADER_LIGHT_MASK: u32 = 56;
/// Sets the least and the most depth of each square of the screen, in the tiles the world's
/// pass drew (screen_bound.wgsl). It has one compute entry point.
pub const SHADER_SCREEN_BOUND: u32 = 57;
/// Draws a character where it was and where it is, and each fragment marks its screen tile
/// (skinned_place.wgsl, tile_mark.wgsl).
pub const SHADER_SKINNED_MARK: u32 = 58;
/// Carries the words of the light's box's tiles over once the box slid, the eye's zooms moved
/// or the world under the box changed (light_slide.wgsl). A tile the box slid onto, a tile
/// moved into another band (pxl-engine `Light::keeps`), and a tile whose rays may meet a
/// changed column are set to none, so they are cast again. It is a port by hand.
pub const SHADER_LIGHT_SLIDE: u32 = 59;
/// The four shaders that draw decor, an impostor, the location indicator's zone and its beacon
/// where each was and where it is. Each fragment marks its screen tile (tile_mark.wgsl).
pub const SHADER_DECOR_MARK: u32 = 60;
pub const SHADER_IMPOSTOR_MARK: u32 = 61;
pub const SHADER_LOCATION_ZONE_MARK: u32 = 62;
pub const SHADER_LOCATION_BEACON_MARK: u32 = 63;
/// Writes the indirect args of the draw of one quad a tile over the box the frame's marks
/// cover (rect_args.wgsl). It has one compute entry point.
pub const SHADER_RECT_ARGS: u32 = 64;
/// `SHADER_QUEUE_CORNERS`, `SHADER_CELL`, `SHADER_QUEUE_LAST_SAMPLES` and `SHADER_SAMPLE_BOX`
/// for the light's map: the walks of its beam rays, its cells, the last walk of its samples,
/// and its samples' stops into the record (beam.light.wgsl, light_cell.wgsl,
/// sample.light.wgsl, sample_box.light.wgsl).
pub const SHADER_QUEUE_LIGHT_CORNERS: u32 = 65;
pub const SHADER_LIGHT_CELL: u32 = 66;
pub const SHADER_QUEUE_LAST_LIGHT_SAMPLES: u32 = 67;
pub const SHADER_LIGHT_SAMPLE_BOX: u32 = 68;
/// Walks the eye's samples over the node cache alone (queue_walk.wgsl over sample.wgsl and
/// sample.eye.wgsl). A sample that meets a node no slot keeps is parked.
/// `SHADER_QUEUE_ASK_SAMPLES` then asks the world for that node, and
/// `SHADER_QUEUE_LAST_SAMPLES` ends the sample's walk.
pub const SHADER_QUEUE_SAMPLES: u32 = 69;
/// Walks the holes over the node cache alone (queue_walk.wgsl), the eye's pixels and the
/// light's slots alike. A hole that meets a node no slot keeps is parked. `SHADER_QUEUE_ASK`
/// then asks the world for that node, and `SHADER_QUEUE_LAST` ends the hole's ray.
pub const SHADER_QUEUE: u32 = 70;
/// The last walk of the eye's beam rays, which asks the world where the node cache keeps no
/// node (queue_last.wgsl over corner.wgsl and beam.eye.wgsl).
pub const SHADER_QUEUE_LAST_CORNERS: u32 = 71;
/// `SHADER_QUEUE_SAMPLES` for the light's map.
pub const SHADER_QUEUE_LIGHT_SAMPLES: u32 = 72;
/// `SHADER_QUEUE_LAST_CORNERS` for the light's map.
pub const SHADER_QUEUE_LAST_LIGHT_CORNERS: u32 = 73;
/// Lists the claimed tiles, so that `SHADER_CAST` takes only their pixels.
pub const SHADER_CAST_TILES: u32 = 74;
/// Finds the box of the screen's tiles claimed in a frame drawn through the gate, for
/// `SHADER_DECOR_CULL`.
pub const SHADER_DAMAGE_RECT: u32 = 75;
/// Lists the generated decor's instances that meet the claimed tiles, the only ones a frame
/// drawn through the gate draws.
pub const SHADER_DECOR_CULL: u32 = 76;
/// Goes over every slot's drawable entries in the node cache (record_draw.wgsl). Each entry in
/// view is written as a box for the raster, and also stood as a receiver of the light when the
/// world's receivers are stood this frame. Each entry on the light's box is written as a box
/// for the light's raster. It is a port by hand.
pub const SHADER_RECORD_DRAW: u32 = 77;
/// Writes the holes' stops into the node cache's drawable entries, and puts at each hole's
/// pixel what lies about the face it entered (record_hits.wgsl). It is a port by hand.
pub const SHADER_RECORD_HITS: u32 = 78;
/// `SHADER_RECORD_HITS` for the light's stops (record_hits.light.wgsl).
pub const SHADER_LIGHT_RECORD_HITS: u32 = 79;
/// Writes the dispatch of the drawn receivers' pass (light_mark_draws_open.wgsl). It is a
/// module apart from that pass, so the pass, which takes its dispatch from the buffer this one
/// writes, does not bind that buffer writable.
pub const SHADER_LIGHT_MARK_DRAWS_OPEN: u32 = 80;
/// Asks the world, once a node, for each node a walk of the samples was parked at, the eye's
/// and the light's (queue_ask.wgsl).
pub const SHADER_QUEUE_ASK_SAMPLES: u32 = 81;
pub const SHADER_QUEUE_ASK_LIGHT_SAMPLES: u32 = 82;
/// Asks the world, once a node, for each node a walk of the beam rays was parked at, the eye's
/// and the light's (queue_ask.wgsl).
pub const SHADER_QUEUE_ASK_CORNERS: u32 = 83;
pub const SHADER_QUEUE_ASK_LIGHT_CORNERS: u32 = 84;
/// The camera grid's cells over the canvas (camera_grid.wgsl).
pub const SHADER_CAMERA_GRID: u32 = 85;
pub const SHADERS: u32 = 86;

/// The parts of the air as the frame's shaders read it: its medium, its tables and the sky,
/// the eye in it, the march through it, its light and the view through it. A shader's own
/// parts go before and after them.
macro_rules! air {
    [$($before:literal),*; $($after:literal),*] => {
        &[$($before,)* "atmos_medium", "atmos_bind", "atmos_lut", "atmos_sky", "atmos_eye", "atmos_march", "atmos_light", "atmos_view", $($after),*]
    };
}

/// The parts every lit draw reads before its own: the frame, the air and the volume before
/// each point, the ground's light, the shading, the light's shadow and the finish.
macro_rules! lit {
    [$($own:literal),*] => {
        air!["frame", "encode", "effect"; "atmos_air", "atmos_ground", "shade", "shadow", "finish", $($own),*]
    };
}

pub const PARTS: [&[&str]; SHADERS as usize] = [
    // SHADER_BOX
    lit!["paint", "box"],
    // SHADER_DECOR
    lit![
        "place",
        "instance",
        "decor_place",
        "lit_vertex",
        "lit_skin",
        "decor"
    ],
    // SHADER_SKINNED
    lit![
        "place",
        "skinned_draw.slot",
        "skinned_place",
        "lit_vertex",
        "lit_skin",
        "skinned_mesh"
    ],
    // SHADER_LOCATION_ZONE
    &[
        "frame",
        "encode",
        "finish",
        "location",
        "location_zone_place",
        "location_zone",
    ],
    // SHADER_LOCATION_BEACON
    lit![
        "place",
        "location.slot",
        "location_beacon_place",
        "lit_vertex",
        "location_beacon"
    ],
    // SHADER_MARKER
    &["frame", "encode", "marker", "marker_sprite"],
    // SHADER_SKY
    air!["frame", "encode", "finish"; "atmos_air", "atmos_ground", "sky_word", "sky"],
    // SHADER_DEBUG
    &["frame", "encode", "debug"],
    // SHADER_MIP
    &["mip"],
    // SHADER_AO_DOWNSAMPLE
    &["frame", "ao_view", "ao_pass", "ao_downsample"],
    // SHADER_AO_SAMPLE
    &["frame", "ao_view", "ao_pass", "ao_sample"],
    // SHADER_AO_BLUR
    &["ao_view", "ao_pass", "ao_blur"],
    // SHADER_ATMOS_TRANSMITTANCE
    &["atmos_medium", "atmos_pass", "atmos_transmittance"],
    // SHADER_ATMOS_MULTISCATTER
    &[
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_pass",
        "atmos_multiscatter",
    ],
    // SHADER_ATMOS_SKY_VIEW
    &[
        "frame",
        "encode",
        "finish",
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_ground",
        "atmos_eye.unit",
        "atmos_light",
        "atmos_pass",
        "atmos_march",
        "atmos_skyview",
    ],
    // SHADER_ATMOS_EXPOSURE
    &[
        "frame",
        "encode",
        "finish",
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_sky",
        "atmos_eye.unit",
        "atmos_march",
        "atmos_light",
        "atmos_view",
        "atmos_ground",
        "atmos_exposure",
    ],
    // SHADER_STAR
    &[
        "frame",
        "encode",
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "star",
    ],
    // SHADER_CLOUD
    air!["frame", "encode", "finish", "cloud_hash", "noise"; "atmos_air", "atmos_ground", "cloud_tile", "cloud_field", "cloud"],
    // SHADER_CLOUD_FIELD
    &[
        "cloud_hash",
        "noise",
        "cloud_tile",
        "atmos_pass",
        "cloud_bake",
    ],
    // SHADER_ATLAS
    &["atlas"],
    // SHADER_IMPOSTOR
    lit!["instance", "impostor_place", "lit_vertex", "impostor"],
    // SHADER_IMPOSTOR_BAKE
    &["impostor_bake"],
    // SHADER_ATMOS_IRRADIANCE
    &[
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_eye.unit",
        "atmos_pass",
        "atmos_march",
        "atmos_irradiance",
    ],
    // SHADER_MARKER_REGION
    &["frame", "encode", "marker", "marker_region"],
    // SHADER_SCREEN_RAY
    lit![
        "lattice",
        "ray",
        "eye",
        "paint",
        "px",
        "tile_word",
        "screen_tile",
        "tile_quad",
        "debug_look",
        "screen_ray",
        "screen_shade"
    ],
    // SHADER_CAST
    &[
        "walk_word",
        "raster_box",
        "eye_bind",
        "grid",
        "walk_node",
        "side",
        "record",
        "visible",
        "walked",
        "cast",
    ],
    // SHADER_GENERATION
    &[
        "walk_word",
        "world_word",
        "decor_word",
        "eye_bind",
        "clipmap",
        "seed",
        "clip",
        "brick",
        "bound_mip",
        "palette.none",
        "world_rule",
        "answer",
        "world",
        "decor_record",
        "decor_rect",
        "decor_row",
        "decor_pose",
    ],
    // SHADER_GENERATION_CNT
    &[
        "walk_word",
        "world_word",
        "decor_word",
        "eye_bind",
        "walk_node",
        "side",
        "resume",
        "clipmap",
        "seed",
        "clip",
        "decor_record",
        "decor_run",
        "decor_cnt",
    ],
    // SHADER_GENERATION_ARGS
    &["decor_word", "decor_instance", "decor_args"],
    // SHADER_GENERATION_COMPACT
    &[
        "walk_word",
        "world_word",
        "decor_word",
        "clipmap",
        "seed",
        "decor_record",
        "decor_row",
        "decor_run",
        "decor_instance",
        "decor_compact",
    ],
    // SHADER_GENERATION_MARK
    &[
        "walk_word",
        "world_word",
        "decor_word",
        "clipmap",
        "seed",
        "decor_record",
        "decor_rect",
        "decor_mark",
    ],
    // SHADER_ATMOS_FROXEL
    &[
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_march",
        "atmos_froxel",
    ],
    // SHADER_QUEUE_CORNERS
    &[
        "walk_word",
        "eye_bind",
        "walk_node",
        "side",
        "resume",
        "answer",
        "cached",
        "walk",
        "beam.eye",
        "parked",
        "corner_run.eye",
        "corner",
        "item_list",
        "queue_walk",
    ],
    // SHADER_CELL
    &["walk_word", "eye_bind", "grid", "window", "cell"],
    // SHADER_QUEUE_LAST
    &[
        "walk_word",
        "world_word",
        "eye_bind",
        "grid",
        "walk_node",
        "side",
        "resume",
        "clipmap",
        "seed",
        "clip",
        "brick",
        "bound_mip",
        "palette",
        "world_rule",
        "answer",
        "world",
        "world_split",
        "ask",
        "walk",
        "hit",
        "miss_word",
        "walked",
        "light_aim",
        "light_resume",
        "parked",
        "hole_run",
        "queue",
        "queue_last",
    ],
    // SHADER_FORGET
    &["walk_word", "world_word", "seed", "forget"],
    // SHADER_QUEUE_LAST_SAMPLES
    &[
        "walk_word",
        "world_word",
        "eye_bind",
        "grid",
        "walk_node",
        "side",
        "resume",
        "clipmap",
        "seed",
        "clip",
        "brick",
        "bound_mip",
        "palette",
        "world_rule",
        "answer",
        "world",
        "world_split",
        "ask",
        "walk",
        "sample.eye",
        "sample_put",
        "parked",
        "sample_run.eye",
        "sample",
        "item_list",
        "queue_last",
    ],
    // SHADER_LIGHT
    &[
        "walk_word",
        "raster_box",
        "eye_bind",
        "walk_node",
        "side",
        "light_aim",
        "light_list",
        "record",
        "visible",
        "light",
    ],
    // SHADER_LIGHT_MAP
    &["light_word", "light_lay", "light_map"],
    // SHADER_QUEUE_ASK
    &[
        "walk_word",
        "world_word",
        "world_rule",
        "clipmap",
        "seed",
        "clip",
        "palette",
        "brick",
        "bound_mip",
        "eye_bind",
        "walk_node",
        "answer",
        "world",
        "world_split",
        "ask",
        "light_aim",
        "hole_run",
        "queue_ask",
    ],
    // SHADER_AO_APPLY
    &[
        "tile_word",
        "screen_tile",
        "tile_quad",
        "effect",
        "ao_view",
        "ao_apply",
    ],
    // SHADER_SAMPLE_BOX
    &[
        "walk_word",
        "raster_box",
        "eye_bind",
        "grid",
        "walk_node",
        "side",
        "resume",
        "sample.eye",
        "record",
        "record_put",
        "sample_box.eye",
        "sample_box",
    ],
    // SHADER_RASTER
    &[
        "walk_word",
        "raster_box",
        "eye_bind",
        "walk_node",
        "side",
        "record",
        "record_box",
        "px",
        "raster",
    ],
    // SHADER_FACE
    &[
        "walk_word",
        "world_word",
        "eye_bind",
        "walk_node",
        "clipmap",
        "seed",
        "clip",
        "brick",
        "bound_mip",
        "palette.none",
        "world_rule",
        "answer",
        "world",
        "world_split",
        "face",
    ],
    // SHADER_LIGHT_MARK
    &[
        "walk_word",
        "eye_bind",
        "grid",
        "walk_node",
        "side",
        "light_aim",
        "light_box",
        "light_list",
        "light_stand",
        "list",
        "light_receiver_tile",
        "walked",
        "light_mark",
    ],
    // SHADER_LIGHT_MARK_DRAWS
    &[
        "walk_word",
        "world_word",
        "decor_word",
        "eye_bind",
        "walk_node",
        "side",
        "seed",
        "light_aim",
        "light_box",
        "light_list",
        "light_stand",
        "list",
        "light_receiver_tile",
        "decor_ball",
        "light_mark_draws",
    ],
    // SHADER_LIGHT_PROPAGATE
    &[
        "walk_word",
        "decor_word",
        "eye_bind",
        "walk_node",
        "side",
        "light_aim",
        "light_box",
        "light_list",
        "light_stand",
        "light_enlist",
        "light_propagate",
    ],
    // SHADER_GENERATION_THIN
    &[
        "walk_word",
        "world_word",
        "decor_word",
        "seed",
        "decor_record",
        "decor_row",
        "decor_scatter",
        "decor_thin",
    ],
    // SHADER_SCREEN_FINISH
    &[
        "frame",
        "encode",
        "finish",
        "tile_word",
        "screen_tile",
        "tile_quad",
        "screen_finish",
    ],
    // SHADER_SCREEN_RESOLVE
    &[
        "frame",
        "tile_word",
        "screen_tile",
        "tile_quad",
        "screen_resolve",
    ],
    // SHADER_SCREEN_MASK
    &["tile_word", "screen_tile", "tile_quad", "screen_mask"],
    // SHADER_SCREEN_SHOW
    &["screen_show"],
    // SHADER_SCREEN_PROPAGATE
    &["tile_word", "screen_propagate"],
    // SHADER_SCREEN_WRITE_BACK
    &["tile_word", "screen_tile", "tile_quad", "screen_write_back"],
    // SHADER_RASTER_MASK
    &["tile_word", "screen_tile", "tile_quad", "raster_mask"],
    // SHADER_SCREEN_MARK
    &["screen_mark"],
    // SHADER_LIGHT_MASK
    &[
        "light_word",
        "tile_word",
        "screen_tile",
        "light_lay",
        "light_mask",
    ],
    // SHADER_SCREEN_BOUND
    &["tile_word", "screen_tile", "screen_bound"],
    // SHADER_SKINNED_MARK
    &[
        "frame",
        "place",
        "skinned_draw.mark",
        "skinned_place",
        "tile_mark",
    ],
    // SHADER_LIGHT_SLIDE
    &[
        "walk_word",
        "eye_bind",
        "walk_node",
        "side",
        "light_aim",
        "light_box",
        "light_list",
        "light_stand",
        "light_slide",
    ],
    // SHADER_DECOR_MARK
    &["frame", "place", "instance", "decor_place", "tile_mark"],
    // SHADER_IMPOSTOR_MARK
    &["frame", "instance", "impostor_place", "tile_mark"],
    // SHADER_LOCATION_ZONE_MARK
    &["frame", "location", "location_zone_place", "tile_mark"],
    // SHADER_LOCATION_BEACON_MARK
    &[
        "frame",
        "place",
        "location",
        "location_beacon_place",
        "tile_mark",
    ],
    // SHADER_RECT_ARGS
    &["tile_word", "rect_args"],
    // SHADER_QUEUE_LIGHT_CORNERS
    &[
        "walk_word",
        "eye_bind",
        "walk_node",
        "side",
        "resume",
        "answer",
        "cached",
        "walk",
        "light_aim",
        "light_list",
        "beam.light",
        "parked",
        "corner_run.light",
        "corner",
        "item_list",
        "queue_walk",
    ],
    // SHADER_LIGHT_CELL
    &[
        "walk_word",
        "eye_bind",
        "walk_node",
        "light_aim",
        "light_list",
        "window",
        "light_cell",
    ],
    // SHADER_QUEUE_LAST_LIGHT_SAMPLES
    &[
        "walk_word",
        "world_word",
        "eye_bind",
        "walk_node",
        "side",
        "resume",
        "clipmap",
        "seed",
        "clip",
        "brick",
        "bound_mip",
        "palette",
        "world_rule",
        "answer",
        "world",
        "world_split",
        "ask",
        "walk",
        "light_aim",
        "light_resume",
        "light_list",
        "sample.light",
        "sample_put",
        "parked",
        "sample_run.light",
        "sample",
        "item_list",
        "queue_last",
    ],
    // SHADER_LIGHT_SAMPLE_BOX
    &[
        "walk_word",
        "raster_box",
        "eye_bind",
        "walk_node",
        "side",
        "resume",
        "light_aim",
        "light_resume",
        "light_list",
        "sample.light",
        "record",
        "record_put",
        "sample_box.light",
        "sample_box",
    ],
    // SHADER_QUEUE_SAMPLES
    &[
        "walk_word",
        "eye_bind",
        "grid",
        "walk_node",
        "side",
        "resume",
        "answer",
        "cached",
        "walk",
        "sample.eye",
        "sample_put",
        "parked",
        "sample_run.eye",
        "sample",
        "item_list",
        "queue_walk",
    ],
    // SHADER_QUEUE
    &[
        "walk_word",
        "eye_bind",
        "grid",
        "walk_node",
        "side",
        "resume",
        "answer",
        "cached",
        "walk",
        "hit",
        "miss_word",
        "walked",
        "light_aim",
        "light_resume",
        "parked",
        "hole_run",
        "queue",
        "queue_walk",
    ],
    // SHADER_QUEUE_LAST_CORNERS
    &[
        "walk_word",
        "world_word",
        "eye_bind",
        "walk_node",
        "side",
        "resume",
        "clipmap",
        "seed",
        "clip",
        "brick",
        "bound_mip",
        "palette",
        "world_rule",
        "answer",
        "world",
        "world_split",
        "ask",
        "walk",
        "beam.eye",
        "parked",
        "corner_run.eye",
        "corner",
        "item_list",
        "queue_last",
    ],
    // SHADER_QUEUE_LIGHT_SAMPLES
    &[
        "walk_word",
        "eye_bind",
        "walk_node",
        "side",
        "resume",
        "answer",
        "cached",
        "walk",
        "light_aim",
        "light_resume",
        "light_list",
        "sample.light",
        "sample_put",
        "parked",
        "sample_run.light",
        "sample",
        "item_list",
        "queue_walk",
    ],
    // SHADER_QUEUE_LAST_LIGHT_CORNERS
    &[
        "walk_word",
        "world_word",
        "eye_bind",
        "walk_node",
        "side",
        "resume",
        "clipmap",
        "seed",
        "clip",
        "brick",
        "bound_mip",
        "palette",
        "world_rule",
        "answer",
        "world",
        "world_split",
        "ask",
        "light_aim",
        "light_list",
        "walk",
        "beam.light",
        "parked",
        "corner_run.light",
        "corner",
        "item_list",
        "queue_last",
    ],
    // SHADER_CAST_TILES
    &["tile_word", "cast_tile"],
    // SHADER_DAMAGE_RECT
    &["tile_word", "damage_rect"],
    // SHADER_DECOR_CULL
    &[
        "frame",
        "decor_word",
        "tile_word",
        "decor_ball",
        "decor_cull",
    ],
    // SHADER_RECORD_DRAW
    &[
        "walk_word",
        "world_word",
        "raster_box",
        "seed",
        "eye_bind",
        "walk_node",
        "side",
        "record",
        "record_box",
        "light_aim",
        "light_box",
        "light_list",
        "light_stand",
        "list",
        "light_receiver_tile",
        "record_draw",
    ],
    // SHADER_RECORD_HITS
    &[
        "walk_word",
        "raster_box",
        "eye_bind",
        "walk_node",
        "hit",
        "record",
        "record_put",
        "record_hits.eye",
        "hit_read",
        "record_hits",
    ],
    // SHADER_LIGHT_RECORD_HITS
    &[
        "walk_word",
        "raster_box",
        "eye_bind",
        "walk_node",
        "side",
        "light_aim",
        "hit",
        "record",
        "record_put",
        "record_hits.light",
        "hit_read",
        "record_hits",
    ],
    // SHADER_LIGHT_MARK_DRAWS_OPEN
    &[
        "walk_word",
        "world_word",
        "decor_word",
        "eye_bind",
        "walk_node",
        "side",
        "seed",
        "light_aim",
        "light_box",
        "light_list",
        "light_stand",
        "list",
        "light_receiver_tile",
        "decor_ball",
        "light_mark_draws",
        "light_mark_draws_open",
    ],
    // SHADER_QUEUE_ASK_SAMPLES
    &[
        "walk_word",
        "world_word",
        "world_rule",
        "clipmap",
        "seed",
        "clip",
        "palette",
        "brick",
        "bound_mip",
        "eye_bind",
        "walk_node",
        "answer",
        "world",
        "world_split",
        "ask",
        "light_aim",
        "sample_run.eye",
        "queue_ask",
    ],
    // SHADER_QUEUE_ASK_LIGHT_SAMPLES
    &[
        "walk_word",
        "world_word",
        "world_rule",
        "clipmap",
        "seed",
        "clip",
        "palette",
        "brick",
        "bound_mip",
        "eye_bind",
        "walk_node",
        "answer",
        "world",
        "world_split",
        "ask",
        "light_aim",
        "sample_run.light",
        "queue_ask",
    ],
    // SHADER_QUEUE_ASK_CORNERS
    &[
        "walk_word",
        "world_word",
        "world_rule",
        "clipmap",
        "seed",
        "clip",
        "palette",
        "brick",
        "bound_mip",
        "eye_bind",
        "walk_node",
        "answer",
        "world",
        "world_split",
        "ask",
        "light_aim",
        "corner_run.eye",
        "queue_ask",
    ],
    // SHADER_QUEUE_ASK_LIGHT_CORNERS
    &[
        "walk_word",
        "world_word",
        "world_rule",
        "clipmap",
        "seed",
        "clip",
        "palette",
        "brick",
        "bound_mip",
        "eye_bind",
        "walk_node",
        "answer",
        "world",
        "world_split",
        "ask",
        "light_aim",
        "corner_run.light",
        "queue_ask",
    ],
    // SHADER_CAMERA_GRID
    &["encode", "sky_word", "debug_look", "camera_grid"],
];
