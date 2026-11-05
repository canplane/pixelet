// pxl-render/frame/shader/part.rs

//! Every shader by its number, and its parts in order: compiled into the
//! crate for its numbers, and read by build.rs, which assembles each
//! (record.rs `src`), validates it, and reads off it which stages read
//! each binding (record.rs `pipeline`), so a pipeline's groups take their
//! visibility from its shader and from nowhere else. A shader's parts follow
//! constant.wgsl. A shader is one module, made once, and each pipeline over
//! it names the entry points it runs: a render pipeline in its record, a
//! compute pipeline by its number (record.rs `compute`).

/// A shader's vertex entry points, a pipeline naming one by its index (record.rs `Head`), so
/// build.rs reads which of them reads each binding. The shell holds the same list against
/// the same numbers.
pub const VERTEX_ENTRY: [&str; 2] = ["vs", "vs_depth"];
/// A shader's fragment entry point, which a pipeline with a fragment stage runs (record.rs
/// `Head`).
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
/// The eye's exposure and how far each light reaches the air, a texel a
/// thread (atmos_exposure.wgsl): its one compute entry.
pub const SHADER_ATMOS_EXPOSURE: u32 = 15;
pub const SHADER_STAR: u32 = 16;
pub const SHADER_CLOUD: u32 = 17;
pub const SHADER_CLOUD_FIELD: u32 = 18;
pub const SHADER_ATLAS: u32 = 19;
pub const SHADER_IMPOSTOR: u32 = 20;
pub const SHADER_IMPOSTOR_BAKE: u32 = 21;
pub const SHADER_ATMOS_IRRADIANCE: u32 = 22;
pub const SHADER_MARKER_REGION: u32 = 23;
/// The eye's rays drawn at their pixels (pxl-engine ray.rs).
pub const SHADER_SCREEN_RAY: u32 = 24;
/// Each drawn pixel's ray off the visible sample or the sky, the rest marked and listed
/// for their walk again, by hand (cast.wgsl): its one compute entry.
pub const SHADER_CAST: u32 = 25;
/// Each newly held brick's levels over its columns, a level a dispatch,
/// pxl-engine's `fill::level` and `clipmap::join` by hand (bound.wgsl): its
/// one compute entry.
pub const SHADER_BOUND: u32 = 26;
/// Each newly held brick's columns' ground tops off its samples,
/// pxl-engine's `fill::heights` by hand (height.wgsl): its one compute entry.
pub const SHADER_HEIGHT: u32 = 27;
/// The decor the ground generates in the eye's clip levels, a record a thread, on its
/// column's surface and posed (decor_pose.wgsl): its one compute entry.
pub const SHADER_GENERATION: u32 = 28;
/// What `SHADER_GENERATION` and placed decor leave, sized for the screen and
/// counted a draw region each; the regions laid out with their indirect
/// args (decor_args.wgsl); each drawn compacted into the decor's instances:
/// pxl-engine `generation::drawn`.
pub const SHADER_GENERATION_CNT: u32 = 29;
pub const SHADER_GENERATION_ARGS: u32 = 30;
pub const SHADER_GENERATION_COMPACT: u32 = 31;
/// Each newly held terrain frame's ways, its columns' spans off its lines
/// (pxl-engine fill.rs `spans`), by hand (way.wgsl): its one compute entry.
pub const SHADER_WAY: u32 = 32;
/// Which eye slots' decor generates again this frame, listed for
/// `SHADER_GENERATION_THIN`'s and `SHADER_GENERATION`'s dispatches: its one
/// compute entry.
pub const SHADER_GENERATION_MARK: u32 = 33;
/// The eye's clip levels folded over their bricks, pxl-engine's
/// `clipmap::fold` by hand (fold.wgsl): its one compute entry.
pub const SHADER_FOLD: u32 = 34;
/// A light's share of the air's volume, a column of froxels a workgroup
/// (atmos_froxel.wgsl): its one compute entry.
pub const SHADER_ATMOS_FROXEL: u32 = 35;
/// Each handed brick's ways as a tree of cells in one pool (pxl-engine
/// `way::tree`), by hand: a re-filled brick's old child groups given back
/// (way_free.wgsl), its tree built off the pool's stack (way_tree.wgsl), the
/// child groups its build gave back pushed after (way_return.wgsl): a compute
/// entry each.
pub const SHADER_WAY_FREE: u32 = 36;
pub const SHADER_WAY_TREE: u32 = 37;
pub const SHADER_WAY_RETURN: u32 = 38;
/// The camera grid's beam rays `SHADER_BEAM_HIT` left, a thread each, walked asking the
/// world, by hand (beam.wgsl, beam.eye.wgsl): its one compute entry.
pub const SHADER_BEAM: u32 = 39;
/// Where each of the camera grid's cells' rays start off its beam rays, by hand
/// (cell.wgsl): its one compute entry.
pub const SHADER_CELL: u32 = 40;
/// The pixels the cast's reading walk missed, walked again writing the
/// node cache, every one or those listed (miss.wgsl), and the node cache
/// forgotten (pxl-engine `node_cache`), by hand (forget.wgsl): compute
/// entries.
pub const SHADER_MISS: u32 = 41;
pub const SHADER_FORGET: u32 = 42;
/// The samples, a thread each, resumed from their cells' starts and walked
/// to the cells the pixels about them take (pxl-engine `beam::Grid::samples`):
/// a compute entry.
pub const SHADER_SAMPLE: u32 = 43;
/// The world's depth from the light into its clipmap (pxl-engine `light`):
/// every slot taking the visible sample, a workgroup a tile (light.wgsl), a
/// compute entry; that depth laid into the map (light_map.wgsl,
/// gpu/shadow.rs); and the slots that took none walked apart (pxl-engine
/// `light::misses_at`, light_miss.wgsl), a compute entry of its own.
pub const SHADER_LIGHT: u32 = 44;
pub const SHADER_LIGHT_MAP: u32 = 45;
pub const SHADER_LIGHT_MISS: u32 = 46;
/// The occlusion's half-resolution map brought to the pixel and multiplied
/// into the world's color, as the tiles' quads (ao_apply.wgsl).
pub const SHADER_AO_APPLY: u32 = 47;
/// The camera grid's cells over the screen (camera_grid.wgsl).
pub const SHADER_CAMERA_GRID: u32 = 48;
/// The samples' hits handed to the rasterizer (pxl-engine `raster`):
/// their boxes written, by hand (boxes.wgsl, sample_box.wgsl), a compute
/// entry; and the boxes drawn, the nearest
/// face a pixel's ray enters kept with its sample (raster.wgsl), which the
/// cast's pixels read (`SHADER_CAST`).
pub const SHADER_SAMPLE_BOX: u32 = 49;
pub const SHADER_RASTER: u32 = 50;
/// What lies about each box's face, asked once a box (pxl-engine
/// `raster::ARGS_FACES`), and each missed pixel's hit's, by hand (face.wgsl): its one
/// compute entry.
pub const SHADER_FACE: u32 = 51;
/// The tiles of the light's box its receivers stand on in a cast
/// (pxl-engine `light::stands_at`): where the eye's rays meet the world, and
/// where the decor and the actors drawn stand; then those they read
/// (`light::reads_at`), by hand (light_mark.wgsl, light_mark_draws.wgsl,
/// light_propagate.wgsl): compute entries.
pub const SHADER_LIGHT_MARK: u32 = 52;
pub const SHADER_LIGHT_MARK_DRAWS: u32 = 53;
pub const SHADER_LIGHT_PROPAGATE: u32 = 54;
/// Which candidates of a slot `SHADER_GENERATION_MARK` listed the thinning
/// retains, a clip level cell a thread, for `SHADER_GENERATION` to generate: its
/// one compute entry.
pub const SHADER_GENERATION_THIN: u32 = 55;
/// The world's toned color encoded onto the canvas once (screen_finish.wgsl).
pub const SHADER_SCREEN_FINISH: u32 = 56;
/// The world's color gathered over frames (screen_resolve.wgsl).
pub const SHADER_SCREEN_RESOLVE: u32 = 57;
/// The claimed tiles set back to nothing at the far plane, their stencil
/// raised (screen_gate.wgsl).
pub const SHADER_SCREEN_GATE: u32 = 58;
/// The finished picture kept at the canvas's size shown on the canvas
/// (screen_show.wgsl).
pub const SHADER_SCREEN_SHOW: u32 = 59;
/// The light's map's tiles that changed propagated to the screen's
/// (screen_propagate.wgsl): its one compute entry.
pub const SHADER_SCREEN_PROPAGATE: u32 = 60;
/// The claimed tiles of the history the resolve wrote copied back into the
/// last (screen_write_back.wgsl).
pub const SHADER_SCREEN_WRITE_BACK: u32 = 61;
/// The claimed tiles of the samples' raster set back to no sample at the far
/// plane (raster_gate.wgsl).
pub const SHADER_RASTER_GATE: u32 = 62;
/// The tiles the frame's one mark covers, or every one, taking its number
/// (screen_mark.wgsl): its one compute entry.
pub const SHADER_SCREEN_MARK: u32 = 63;
/// The world's depth laid in the claimed tiles of the light's map, their
/// stencil raised (light_gate.wgsl).
pub const SHADER_LIGHT_GATE: u32 = 64;
/// Each square of the screen's least and most depth, set in the tiles drawn
/// (screen_bound.wgsl): its one compute entry.
pub const SHADER_SCREEN_BOUND: u32 = 65;
/// An actor where it was and where it is, its fragments marking their screen
/// tiles (skinned_place.wgsl, tile_mark.wgsl).
pub const SHADER_SKINNED_MARK: u32 = 66;
/// The tiles of the light's box the box slid onto, or slid into another
/// band (pxl-engine `Light::keeps`), by hand (light_slide.wgsl): a compute
/// entry.
pub const SHADER_LIGHT_SLIDE: u32 = 67;
/// Decor, an impostor, the location indicator's zone and its beacon where each
/// was and where it is, their fragments marking their tiles (tile_mark.wgsl).
pub const SHADER_DECOR_MARK: u32 = 68;
pub const SHADER_IMPOSTOR_MARK: u32 = 69;
pub const SHADER_LOCATION_ZONE_MARK: u32 = 70;
pub const SHADER_LOCATION_BEACON_MARK: u32 = 71;
/// The missed pixels' hits whose faces are asked once a square of them,
/// the one of each square that asks listed (pxl-engine `beam::HIT_WORDS`), and
/// the faces asked put at their pixels, by hand (hit_lead.wgsl,
/// hit_faces.wgsl): compute entries.
pub const SHADER_HIT_LEAD: u32 = 72;
pub const SHADER_HIT_FACES: u32 = 73;
/// The indirect args of a quad a tile of the box the frame's marks cover
/// (box_args.wgsl): its one compute entry.
pub const SHADER_BOX_ARGS: u32 = 74;
/// The shadow map's beam rays, cells, samples and samples' boxes: the
/// passes of `SHADER_BEAM`, `SHADER_CELL`, `SHADER_SAMPLE` and
/// `SHADER_SAMPLE_BOX` for the light's map (beam.light.wgsl,
/// light_cell.wgsl, sample.light.wgsl, light_sample_box.wgsl).
pub const SHADER_LIGHT_BEAM: u32 = 75;
pub const SHADER_LIGHT_CELL: u32 = 76;
pub const SHADER_LIGHT_SAMPLE: u32 = 77;
pub const SHADER_LIGHT_SAMPLE_BOX: u32 = 78;
/// The samples' walk over the node cache alone, by hand: what it does not
/// end `SHADER_SAMPLE` walks again.
pub const SHADER_SAMPLE_HIT: u32 = 79;
/// The missed pixels' walk over the node cache alone, by hand: what it does
/// not end `COMPUTE_MISS_LIST` walks again.
pub const SHADER_MISS_HIT: u32 = 80;
/// The beam rays' walk over the node cache alone, by hand: what it does not
/// end `SHADER_BEAM` walks again.
pub const SHADER_BEAM_HIT: u32 = 81;
/// The light's missed slots' walk over the node cache alone, by hand: what it does
/// not end `SHADER_LIGHT_MISS` walks again.
pub const SHADER_LIGHT_MISS_HIT: u32 = 82;
/// `SHADER_SAMPLE_HIT` for the light's map: what it does not end `SHADER_LIGHT_SAMPLE` walks
/// again.
pub const SHADER_LIGHT_SAMPLE_HIT: u32 = 83;
/// `SHADER_BEAM_HIT` for the light's map: what it does not end `SHADER_LIGHT_BEAM` walks again.
pub const SHADER_LIGHT_BEAM_HIT: u32 = 84;
/// The tiles claimed, listed for `SHADER_CAST`.
pub const SHADER_CAST_TILES: u32 = 85;
/// The box of the screen's claimed tiles, a frame through the gate's (`SHADER_DECOR_GATE`).
pub const SHADER_CLAIMED_BOX: u32 = 86;
/// Decor generated a frame through the gate draws: the instances that meet its claimed tiles.
pub const SHADER_DECOR_GATE: u32 = 87;
pub const SHADERS: u32 = 88;

/// The air as the frame's shaders read it: its medium, its tables and the
/// sky, the eye in it, the march through it, its light and the view
/// through it, between a shader's own parts before and after.
macro_rules! air {
    [$($before:literal),*; $($after:literal),*] => {
        &[$($before,)* "atmos_medium", "atmos_bind", "atmos_lut", "atmos_sky", "atmos_eye", "atmos_march", "atmos_light", "atmos_view", $($after),*]
    };
}

/// What a lit draw reads before its own parts: the frame, the air and the
/// volume before each point, the ground's light, the shading, the light's
/// shadow and the finish.
macro_rules! lit {
    [$($own:literal),*] => {
        air!["frame", "encode", "effect"; "atmos_air", "atmos_ground", "shade", "shadow", "finish", $($own),*]
    };
}

pub const PARTS: [&[&str]; SHADERS as usize] = [
    // SHADER_BOX
    lit!["paint", "box"],
    // SHADER_DECOR
    lit!["place", "instance", "decor_place", "decor"],
    // SHADER_SKINNED
    lit!["place", "skinned_place", "skinned_mesh"],
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
        "location",
        "location_beacon_place",
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
    lit!["instance", "impostor_place", "impostor"],
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
        "debug_look",
        "px",
        "tile_word",
        "screen_tiles",
        "screen_ray",
        "zoom",
        "screen_shade"
    ],
    // SHADER_CAST
    &[
        "walk_word",
        "eye_bind",
        "grid",
        "zoom",
        "walk_node",
        "side",
        "visible",
        "list",
        "cast",
    ],
    // SHADER_BOUND
    &[
        "world_word",
        "way_word",
        "clip",
        "world_rule",
        "way_rule",
        "handed",
        "brick",
        "level",
        "spans",
        "bound",
    ],
    // SHADER_HEIGHT
    &[
        "world_word",
        "way_word",
        "fill_word",
        "clip",
        "handed",
        "brick",
        "i64",
        "ground_height",
        "height",
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
        "level",
        "palette.none",
        "world_rule",
        "world",
        "decor_record",
        "decor_box",
        "decor_row",
        "decor_pose",
    ],
    // SHADER_GENERATION_CNT
    &[
        "walk_word",
        "world_word",
        "decor_word",
        "eye_bind",
        "zoom",
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
    &["decor_word", "decor_instances", "decor_args"],
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
        "decor_instances",
        "decor_compact",
    ],
    // SHADER_WAY
    &[
        "walk_word",
        "world_word",
        "way_word",
        "clipmap",
        "seed",
        "clip",
        "world_rule",
        "way_rule",
        "handed",
        "i64",
        "ground_height",
        "way",
    ],
    // SHADER_GENERATION_MARK
    &[
        "walk_word",
        "world_word",
        "decor_word",
        "clipmap",
        "seed",
        "decor_record",
        "decor_box",
        "decor_mark",
    ],
    // SHADER_FOLD
    &[
        "walk_word",
        "world_word",
        "seed",
        "clip",
        "world_rule",
        "brick",
        "level",
        "fold",
    ],
    // SHADER_ATMOS_FROXEL
    &[
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_march",
        "atmos_froxel",
    ],
    // SHADER_WAY_FREE
    &["world_word", "way_word", "handed", "way_stack", "way_free"],
    // SHADER_WAY_TREE
    &[
        "walk_word",
        "world_word",
        "way_word",
        "clipmap",
        "seed",
        "clip",
        "world_rule",
        "way_rule",
        "handed",
        "level",
        "way_stack",
        "way_tree",
    ],
    // SHADER_WAY_RETURN
    &["way_word", "handed", "way_stack", "way_return"],
    // SHADER_BEAM
    &[
        "walk_word",
        "world_word",
        "eye_bind",
        "zoom",
        "walk_node",
        "side",
        "resume",
        "clipmap",
        "seed",
        "clip",
        "brick",
        "level",
        "palette",
        "world_rule",
        "world",
        "world_split",
        "ask",
        "beam_walk",
        "beam.eye",
        "list",
        "beam",
    ],
    // SHADER_CELL
    &["walk_word", "eye_bind", "grid", "window", "cell"],
    // SHADER_MISS
    &[
        "walk_word",
        "world_word",
        "eye_bind",
        "grid",
        "zoom",
        "walk_node",
        "side",
        "resume",
        "clipmap",
        "seed",
        "clip",
        "brick",
        "level",
        "palette",
        "world_rule",
        "world",
        "world_split",
        "ask",
        "hit",
        "miss_word",
        "list",
        "miss",
    ],
    // SHADER_FORGET
    &["walk_word", "world_word", "seed", "forget"],
    // SHADER_SAMPLE
    &[
        "walk_word",
        "world_word",
        "eye_bind",
        "grid",
        "zoom",
        "walk_node",
        "side",
        "resume",
        "clipmap",
        "seed",
        "clip",
        "brick",
        "level",
        "palette",
        "world_rule",
        "world",
        "world_split",
        "ask",
        "sample.eye",
        "sample_put",
        "list",
        "sample",
    ],
    // SHADER_LIGHT
    &[
        "walk_word",
        "eye_bind",
        "zoom",
        "walk_node",
        "side",
        "light_aim",
        "light_list",
        "visible",
        "list",
        "light",
    ],
    // SHADER_LIGHT_MAP
    &["light_word", "light_lay", "light_map"],
    // SHADER_LIGHT_MISS
    &[
        "walk_word",
        "world_word",
        "world_rule",
        "clipmap",
        "seed",
        "clip",
        "palette",
        "brick",
        "level",
        "eye_bind",
        "zoom",
        "walk_node",
        "side",
        "resume",
        "world",
        "world_split",
        "ask",
        "light_aim",
        "light_resume",
        "list",
        "light_miss",
    ],
    // SHADER_AO_APPLY
    &["tile_word", "screen_tiles", "effect", "ao_view", "ao_apply"],
    // SHADER_CAMERA_GRID
    &["frame", "encode", "sky_word", "debug_look", "camera_grid"],
    // SHADER_SAMPLE_BOX
    &[
        "walk_word",
        "raster_box",
        "eye_bind",
        "grid",
        "zoom",
        "walk_node",
        "side",
        "resume",
        "sample.eye",
        "boxes",
        "sample_box",
    ],
    // SHADER_RASTER
    &["raster_box", "px", "raster"],
    // SHADER_FACE
    &[
        "walk_word",
        "world_word",
        "eye_bind",
        "zoom",
        "walk_node",
        "clipmap",
        "seed",
        "clip",
        "brick",
        "level",
        "palette.none",
        "world_rule",
        "world",
        "world_split",
        "face",
    ],
    // SHADER_LIGHT_MARK
    &[
        "walk_word",
        "eye_bind",
        "grid",
        "zoom",
        "walk_node",
        "side",
        "light_aim",
        "light_box",
        "light_list",
        "light_stand",
        "light_mark",
    ],
    // SHADER_LIGHT_MARK_DRAWS
    &[
        "walk_word",
        "world_word",
        "decor_word",
        "eye_bind",
        "zoom",
        "walk_node",
        "side",
        "seed",
        "light_aim",
        "light_box",
        "light_list",
        "light_stand",
        "decor_ball",
        "light_mark_draws",
    ],
    // SHADER_LIGHT_PROPAGATE
    &[
        "walk_word",
        "decor_word",
        "eye_bind",
        "zoom",
        "walk_node",
        "side",
        "light_aim",
        "light_box",
        "light_list",
        "light_stand",
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
        "screen_tiles",
        "screen_finish",
    ],
    // SHADER_SCREEN_RESOLVE
    &["frame", "tile_word", "screen_tiles", "screen_resolve"],
    // SHADER_SCREEN_GATE
    &["tile_word", "screen_tiles", "screen_gate"],
    // SHADER_SCREEN_SHOW
    &["screen_show"],
    // SHADER_SCREEN_PROPAGATE
    &["tile_word", "screen_propagate"],
    // SHADER_SCREEN_WRITE_BACK
    &["tile_word", "screen_tiles", "screen_write_back"],
    // SHADER_RASTER_GATE
    &["tile_word", "screen_tiles", "raster_gate"],
    // SHADER_SCREEN_MARK
    &["screen_mark"],
    // SHADER_LIGHT_GATE
    &[
        "light_word",
        "tile_word",
        "screen_tiles",
        "light_lay",
        "light_gate",
    ],
    // SHADER_SCREEN_BOUND
    &["tile_word", "screen_tiles", "screen_bound"],
    // SHADER_SKINNED_MARK
    &["frame", "place", "skinned_place", "tile_mark"],
    // SHADER_LIGHT_SLIDE
    &[
        "walk_word",
        "eye_bind",
        "zoom",
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
    // SHADER_HIT_LEAD
    &["walk_word", "ray", "eye_bind", "grid", "hit", "hit_lead"],
    // SHADER_HIT_FACES
    &["walk_word", "ray", "eye_bind", "hit", "hit_faces"],
    // SHADER_BOX_ARGS
    &["tile_word", "box_args"],
    // SHADER_LIGHT_BEAM
    &[
        "walk_word",
        "world_word",
        "eye_bind",
        "zoom",
        "walk_node",
        "side",
        "resume",
        "clipmap",
        "seed",
        "clip",
        "brick",
        "level",
        "palette",
        "world_rule",
        "world",
        "world_split",
        "ask",
        "light_aim",
        "light_list",
        "beam_walk",
        "beam.light",
        "list",
        "beam",
    ],
    // SHADER_LIGHT_CELL
    &[
        "walk_word",
        "eye_bind",
        "zoom",
        "walk_node",
        "light_aim",
        "light_list",
        "window",
        "light_cell",
    ],
    // SHADER_LIGHT_SAMPLE
    &[
        "walk_word",
        "world_word",
        "eye_bind",
        "zoom",
        "walk_node",
        "side",
        "resume",
        "clipmap",
        "seed",
        "clip",
        "brick",
        "level",
        "palette",
        "world_rule",
        "world",
        "world_split",
        "ask",
        "light_aim",
        "light_resume",
        "light_list",
        "sample.light",
        "sample_put",
        "list",
        "sample",
    ],
    // SHADER_LIGHT_SAMPLE_BOX
    &[
        "walk_word",
        "raster_box",
        "eye_bind",
        "zoom",
        "walk_node",
        "side",
        "resume",
        "light_aim",
        "light_resume",
        "light_list",
        "sample.light",
        "boxes",
        "light_sample_box",
    ],
    // SHADER_SAMPLE_HIT
    &[
        "walk_word",
        "eye_bind",
        "grid",
        "zoom",
        "walk_node",
        "side",
        "resume",
        "walk",
        "sample.eye",
        "sample_put",
        "list",
        "sample_hit",
    ],
    // SHADER_MISS_HIT
    &[
        "walk_word",
        "eye_bind",
        "grid",
        "zoom",
        "walk_node",
        "side",
        "resume",
        "walk",
        "hit",
        "miss_word",
        "list",
        "miss_hit",
    ],
    // SHADER_BEAM_HIT
    &[
        "walk_word",
        "eye_bind",
        "zoom",
        "walk_node",
        "side",
        "resume",
        "walk",
        "beam.eye",
        "list",
        "beam_hit",
    ],
    // SHADER_LIGHT_MISS_HIT
    &[
        "walk_word",
        "eye_bind",
        "grid",
        "zoom",
        "walk_node",
        "side",
        "resume",
        "walk",
        "light_aim",
        "light_resume",
        "list",
        "light_miss_hit",
    ],
    // SHADER_LIGHT_SAMPLE_HIT
    &[
        "walk_word",
        "eye_bind",
        "zoom",
        "walk_node",
        "side",
        "resume",
        "walk",
        "light_aim",
        "light_resume",
        "light_list",
        "sample.light",
        "sample_put",
        "list",
        "sample_hit",
    ],
    // SHADER_LIGHT_BEAM_HIT
    &[
        "walk_word",
        "eye_bind",
        "zoom",
        "walk_node",
        "side",
        "resume",
        "walk",
        "light_aim",
        "light_list",
        "beam.light",
        "list",
        "beam_hit",
    ],
    // SHADER_CAST_TILES
    &["tile_word", "cast_tiles"],
    // SHADER_CLAIMED_BOX
    &["tile_word", "claimed_box"],
    // SHADER_DECOR_GATE
    &[
        "frame",
        "decor_word",
        "tile_word",
        "decor_ball",
        "decor_gate",
    ],
];
