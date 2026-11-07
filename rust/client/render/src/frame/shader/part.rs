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
/// build.rs reads which of them reads each binding.
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
/// Each drawn pixel's ray off the visible voxel or the sky, the rest marked a bit each for
/// their walk again, by hand (cast.wgsl): its one compute entry.
pub const SHADER_CAST: u32 = 25;
/// The decor the ground generates in the eye's clip levels, a record a thread, on its
/// column's surface and posed (decor_pose.wgsl): its one compute entry.
pub const SHADER_GENERATION: u32 = 26;
/// What `SHADER_GENERATION` and placed decor leave, sized for the screen and
/// counted a draw region each; the regions laid out with their indirect
/// args (decor_args.wgsl); each drawn compacted into the decor's instances:
/// pxl-engine `generation::drawn`.
pub const SHADER_GENERATION_CNT: u32 = 27;
pub const SHADER_GENERATION_ARGS: u32 = 28;
pub const SHADER_GENERATION_COMPACT: u32 = 29;
/// Which eye slots' decor generates again this frame, listed for
/// `SHADER_GENERATION_THIN`'s and `SHADER_GENERATION`'s dispatches: its one
/// compute entry.
pub const SHADER_GENERATION_MARK: u32 = 30;
/// A light's share of the air's volume, a column of froxels a workgroup
/// (atmos_froxel.wgsl): its one compute entry.
pub const SHADER_ATMOS_FROXEL: u32 = 31;
/// The eye's beam rays' run's walks over the node cache alone (queue_walk.wgsl over
/// corners.wgsl, beam.eye.wgsl): what they park `SHADER_QUEUE_ASK_CORNERS` asks and
/// `SHADER_QUEUE_LAST_CORNERS` ends.
pub const SHADER_QUEUE_CORNERS: u32 = 32;
/// Where each of the camera grid's cells' rays start off its beam rays, by hand
/// (cell.wgsl): its one compute entry.
pub const SHADER_CELL: u32 = 33;
/// The holes' queue's last walk, which asks the world (queue_last.wgsl), and the node cache
/// forgotten (pxl-engine `node_cache`), by hand (forget.wgsl): compute entries.
pub const SHADER_QUEUE_LAST: u32 = 34;
pub const SHADER_FORGET: u32 = 35;
/// The eye's samples' run's last walk, which asks the world (queue_last.wgsl over samples.wgsl,
/// sample.eye.wgsl): a compute entry.
pub const SHADER_QUEUE_LAST_SAMPLES: u32 = 36;
/// The world's depth from the light into its clipmap (pxl-engine `light`):
/// every slot taking the visible voxel, a workgroup a tile (light.wgsl), a
/// compute entry; and that depth laid into the map (light_map.wgsl,
/// gpu/shadow.rs). The nodes the holes' queue's walks were parked at, asked once
/// a node (queue_ask.wgsl): compute entries.
pub const SHADER_LIGHT: u32 = 37;
pub const SHADER_LIGHT_MAP: u32 = 38;
pub const SHADER_QUEUE_ASK: u32 = 39;
/// The occlusion's half-resolution map brought to the pixel and multiplied
/// into the world's color, as the tiles' quads (ao_apply.wgsl).
pub const SHADER_AO_APPLY: u32 = 40;
/// The eye's samples' stops into the record of voxels, by hand (pxl-engine `record`;
/// sample_box.wgsl), two compute entries; and the boxes drawn, the eye's and the light's, the
/// nearest face a pixel's ray enters kept with its voxel (raster.wgsl), which the cast's
/// pixels read (`SHADER_CAST`).
pub const SHADER_SAMPLE_BOX: u32 = 41;
pub const SHADER_RASTER: u32 = 42;
/// What lies about each face of the record's voxels, asked once a face (pxl-engine
/// `raster::ARGS_FACES`, `record::R_FACES`), and each missed pixel's hit's, by hand
/// (face.wgsl): its one compute entry.
pub const SHADER_FACE: u32 = 43;
/// The tiles of the light's box its receivers stand on (pxl-engine
/// `light::LISTED_STOOD`): the pixels the eye walked apart (its voxels' boxes stood by the
/// record's draw, `SHADER_RECORD_DRAW`), and where the decor and what the kernel draws that
/// reads the map stand; then those they read (`light::LISTED_READ`,
/// `light::reads_at`), by hand (light_mark.wgsl, light_mark_draws.wgsl,
/// light_propagate.wgsl): compute entries.
pub const SHADER_LIGHT_MARK: u32 = 44;
pub const SHADER_LIGHT_MARK_DRAWS: u32 = 45;
pub const SHADER_LIGHT_PROPAGATE: u32 = 46;
/// Which candidates of a slot `SHADER_GENERATION_MARK` listed the thinning
/// retains, a clip level cell a thread, for `SHADER_GENERATION` to generate: its
/// one compute entry.
pub const SHADER_GENERATION_THIN: u32 = 47;
/// The world's toned color encoded onto the canvas once (screen_finish.wgsl).
pub const SHADER_SCREEN_FINISH: u32 = 48;
/// The world's color gathered over frames (screen_resolve.wgsl).
pub const SHADER_SCREEN_RESOLVE: u32 = 49;
/// The claimed tiles set back to nothing at the far plane, their stencil
/// raised (screen_gate.wgsl).
pub const SHADER_SCREEN_GATE: u32 = 50;
/// The finished picture kept at the canvas's size shown on the canvas
/// (screen_show.wgsl).
pub const SHADER_SCREEN_SHOW: u32 = 51;
/// The light's map's tiles that changed propagated to the screen's
/// (screen_propagate.wgsl): its one compute entry.
pub const SHADER_SCREEN_PROPAGATE: u32 = 52;
/// The claimed tiles of the history the resolve wrote copied back into the
/// last (screen_write_back.wgsl).
pub const SHADER_SCREEN_WRITE_BACK: u32 = 53;
/// The claimed tiles of the samples' raster set back to no sample at the far
/// plane (raster_gate.wgsl).
pub const SHADER_RASTER_GATE: u32 = 54;
/// The tiles the frame's one mark covers, or every one, taking its number
/// (screen_mark.wgsl): its one compute entry.
pub const SHADER_SCREEN_MARK: u32 = 55;
/// The world's depth laid in the claimed tiles of the light's map, their
/// stencil raised (light_gate.wgsl).
pub const SHADER_LIGHT_GATE: u32 = 56;
/// Each square of the screen's least and most depth, set in the tiles drawn
/// (screen_bound.wgsl): its one compute entry.
pub const SHADER_SCREEN_BOUND: u32 = 57;
/// A character where it was and where it is, its fragments marking their screen
/// tiles (skinned_place.wgsl, tile_mark.wgsl).
pub const SHADER_SKINNED_MARK: u32 = 58;
/// The tiles of the light's box the box slid onto, or slid into another
/// band (pxl-engine `Light::keeps`), by hand (light_slide.wgsl): a compute
/// entry.
pub const SHADER_LIGHT_SLIDE: u32 = 59;
/// Decor, an impostor, the location indicator's zone and its beacon where each
/// was and where it is, their fragments marking their tiles (tile_mark.wgsl).
pub const SHADER_DECOR_MARK: u32 = 60;
pub const SHADER_IMPOSTOR_MARK: u32 = 61;
pub const SHADER_LOCATION_ZONE_MARK: u32 = 62;
pub const SHADER_LOCATION_BEACON_MARK: u32 = 63;
/// The indirect args of a quad a tile of the box the frame's marks cover
/// (box_args.wgsl): its one compute entry.
pub const SHADER_BOX_ARGS: u32 = 64;
/// The shadow map's beam rays' walks, cells, samples' last walk and samples' stops into the
/// record: `SHADER_QUEUE_CORNERS`, `SHADER_CELL`, `SHADER_QUEUE_LAST_SAMPLES` and
/// `SHADER_SAMPLE_BOX` for the light's map (beam.light.wgsl, light_cell.wgsl,
/// sample.light.wgsl, sample_box.light.wgsl).
pub const SHADER_QUEUE_LIGHT_CORNERS: u32 = 65;
pub const SHADER_LIGHT_CELL: u32 = 66;
pub const SHADER_QUEUE_LAST_LIGHT_SAMPLES: u32 = 67;
pub const SHADER_LIGHT_SAMPLE_BOX: u32 = 68;
/// The eye's samples' run's walks over the node cache alone (queue_walk.wgsl over
/// samples.wgsl, sample.eye.wgsl): what they park `SHADER_QUEUE_ASK_SAMPLES` asks and
/// `SHADER_QUEUE_LAST_SAMPLES` ends.
pub const SHADER_QUEUE_SAMPLES: u32 = 69;
/// The holes' queue's walks over the node cache alone (queue_walk.wgsl), the eye's pixels' and
/// the light's slots': what they park `SHADER_QUEUE_ASK` asks and `SHADER_QUEUE_LAST` ends.
pub const SHADER_QUEUE: u32 = 70;
/// The eye's beam rays' run's last walk, which asks the world (queue_last.wgsl over
/// corners.wgsl, beam.eye.wgsl).
pub const SHADER_QUEUE_LAST_CORNERS: u32 = 71;
/// `SHADER_QUEUE_SAMPLES` for the light's map.
pub const SHADER_QUEUE_LIGHT_SAMPLES: u32 = 72;
/// `SHADER_QUEUE_LAST_CORNERS` for the light's map.
pub const SHADER_QUEUE_LAST_LIGHT_CORNERS: u32 = 73;
/// The tiles claimed, listed for `SHADER_CAST`.
pub const SHADER_CAST_TILES: u32 = 74;
/// The box of the screen's claimed tiles, a frame through the gate's (`SHADER_DECOR_GATE`).
pub const SHADER_CLAIMED_BOX: u32 = 75;
/// Decor generated a frame through the gate draws: the instances that meet its claimed tiles.
pub const SHADER_DECOR_GATE: u32 = 76;
/// The node cache's drawable entries over every slot, by hand (record_draw.wgsl): those in view
/// written as the raster's boxes, each stood on by the light's receivers where the world's are
/// stood this frame, and those on the light's box as its raster's.
pub const SHADER_RECORD_DRAW: u32 = 77;
/// The holes' stops into the node cache's drawable entries, and what lies about each one's face
/// put at its pixel, by hand (record_stops.wgsl).
pub const SHADER_RECORD_STOPS: u32 = 78;
/// `SHADER_RECORD_STOPS` for the light's stops (record_stops.light.wgsl).
pub const SHADER_LIGHT_RECORD_STOPS: u32 = 79;
/// The drawn receivers' dispatch opened (light_mark_draws_open.wgsl), a module apart from the
/// receivers' pass that reads it.
pub const SHADER_LIGHT_MARK_DRAWS_OPEN: u32 = 80;
/// The nodes the samples' runs' walks were parked at, asked once a node (queue_ask.wgsl), the
/// eye's and the light's.
pub const SHADER_QUEUE_ASK_SAMPLES: u32 = 81;
pub const SHADER_QUEUE_ASK_LIGHT_SAMPLES: u32 = 82;
/// The nodes the beam rays' runs' walks were parked at, asked once a node (queue_ask.wgsl), the
/// eye's and the light's.
pub const SHADER_QUEUE_ASK_CORNERS: u32 = 83;
pub const SHADER_QUEUE_ASK_LIGHT_CORNERS: u32 = 84;
pub const SHADERS: u32 = 85;

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
        "screen_tiles",
        "tile_quad",
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
        "level",
        "palette.none",
        "world_rule",
        "answer",
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
        "corners_run.eye",
        "corners",
        "listed",
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
        "level",
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
        "holes_run",
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
        "level",
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
        "samples_run.eye",
        "samples",
        "listed",
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
        "level",
        "eye_bind",
        "walk_node",
        "answer",
        "world",
        "world_split",
        "ask",
        "light_aim",
        "holes_run",
        "queue_ask",
    ],
    // SHADER_AO_APPLY
    &[
        "tile_word",
        "screen_tiles",
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
        "level",
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
        "light_stood",
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
        "light_stood",
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
        "screen_tiles",
        "tile_quad",
        "screen_finish",
    ],
    // SHADER_SCREEN_RESOLVE
    &[
        "frame",
        "tile_word",
        "screen_tiles",
        "tile_quad",
        "screen_resolve",
    ],
    // SHADER_SCREEN_GATE
    &["tile_word", "screen_tiles", "tile_quad", "screen_gate"],
    // SHADER_SCREEN_SHOW
    &["screen_show"],
    // SHADER_SCREEN_PROPAGATE
    &["tile_word", "screen_propagate"],
    // SHADER_SCREEN_WRITE_BACK
    &[
        "tile_word",
        "screen_tiles",
        "tile_quad",
        "screen_write_back",
    ],
    // SHADER_RASTER_GATE
    &["tile_word", "screen_tiles", "tile_quad", "raster_gate"],
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
    // SHADER_BOX_ARGS
    &["tile_word", "box_args"],
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
        "corners_run.light",
        "corners",
        "listed",
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
        "level",
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
        "samples_run.light",
        "samples",
        "listed",
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
        "samples_run.eye",
        "samples",
        "listed",
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
        "holes_run",
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
        "level",
        "palette",
        "world_rule",
        "answer",
        "world",
        "world_split",
        "ask",
        "walk",
        "beam.eye",
        "parked",
        "corners_run.eye",
        "corners",
        "listed",
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
        "samples_run.light",
        "samples",
        "listed",
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
        "level",
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
        "corners_run.light",
        "corners",
        "listed",
        "queue_last",
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
        "light_stood",
        "record_draw",
    ],
    // SHADER_RECORD_STOPS
    &[
        "walk_word",
        "raster_box",
        "eye_bind",
        "walk_node",
        "hit",
        "record",
        "record_put",
        "record_stops.eye",
        "record_hits",
        "record_stops",
    ],
    // SHADER_LIGHT_RECORD_STOPS
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
        "record_stops.light",
        "record_hits",
        "record_stops",
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
        "light_stood",
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
        "level",
        "eye_bind",
        "walk_node",
        "answer",
        "world",
        "world_split",
        "ask",
        "light_aim",
        "samples_run.eye",
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
        "level",
        "eye_bind",
        "walk_node",
        "answer",
        "world",
        "world_split",
        "ask",
        "light_aim",
        "samples_run.light",
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
        "level",
        "eye_bind",
        "walk_node",
        "answer",
        "world",
        "world_split",
        "ask",
        "light_aim",
        "corners_run.eye",
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
        "level",
        "eye_bind",
        "walk_node",
        "answer",
        "world",
        "world_split",
        "ask",
        "light_aim",
        "corners_run.light",
        "queue_ask",
    ],
];
