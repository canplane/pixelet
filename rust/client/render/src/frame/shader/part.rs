// pxl-render/frame/shader/part.rs

//! Every shader by its number, and its parts in order: compiled into the
//! crate for its numbers, and read by build.rs, which assembles each
//! (record.rs `source`), validates it, and reads off it which stages read
//! each binding (record.rs `pipeline`), so a pipeline's groups take their
//! visibility from its shader and from nowhere else. A render shader's parts
//! follow constant.wgsl; a generated module (`.gen`) is its own whole.

pub const SHADER_BOX: u32 = 0;
pub const SHADER_DECOR: u32 = 1;
pub const SHADER_SKINNED: u32 = 2;
pub const SHADER_PUCK_ZONE: u32 = 3;
pub const SHADER_PUCK_BEACON: u32 = 4;
pub const SHADER_MARKER: u32 = 5;
pub const SHADER_SKY: u32 = 6;
pub const SHADER_DEBUG: u32 = 7;
pub const SHADER_MIP: u32 = 8;
pub const SHADER_AO_DOWNSAMPLE: u32 = 9;
pub const SHADER_AO_SAMPLE: u32 = 10;
pub const SHADER_AO_BLUR: u32 = 11;
pub const SHADER_AO_UPSAMPLE: u32 = 12;
pub const SHADER_ATMOS_TRANSMITTANCE: u32 = 13;
pub const SHADER_ATMOS_MULTISCATTER: u32 = 14;
pub const SHADER_ATMOS_SKY_VIEW: u32 = 15;
pub const SHADER_ATMOS_EXPOSURE: u32 = 16;
pub const SHADER_STAR: u32 = 17;
pub const SHADER_CLOUD: u32 = 18;
pub const SHADER_CLOUD_FIELD: u32 = 19;
pub const SHADER_SHEET: u32 = 20;
pub const SHADER_IMPOSTOR: u32 = 21;
pub const SHADER_IMPOSTOR_BAKE: u32 = 22;
pub const SHADER_ATMOS_IRRADIANCE: u32 = 23;
pub const SHADER_MARKER_REGION: u32 = 24;
/// The eye's rays drawn at their pixels (pxl-spec-core ray.rs).
pub const SHADER_SCREEN_RAY: u32 = 25;
/// The eye's rays cast on the GPU into the world surface's rays, generated from
/// pxl-spec-core's descent (pxl-tool bin/shader_wgsl.rs): its one compute entry.
pub const SHADER_CAST: u32 = 26;
/// Each newly held brick's levels over its columns, a level a dispatch,
/// generated with the cast from pxl-spec-core's `fill::level` and
/// `window::join`: its one compute entry.
pub const SHADER_BOUND: u32 = 27;
/// Each newly held brick's columns' ground tops off its samples,
/// generated with the cast from pxl-spec-core's `ground::height`: its one
/// compute entry.
pub const SHADER_HEIGHT: u32 = 28;
/// The decor the ground grows in the eye's windows, generated with the cast
/// from pxl-spec-core's `scatter`: its one compute entry.
pub const SHADER_GROWN: u32 = 29;
/// What `SHADER_GROWN` and placed decor leave, sized for the screen and
/// counted a draw region each; the regions laid out with their indirect
/// args; each drawn into the decor's instances: pxl-spec-core `scatter::drawn`.
pub const SHADER_GROWN_CNT: u32 = 30;
pub const SHADER_GROWN_ARGS: u32 = 31;
pub const SHADER_GROWN_PLACE: u32 = 32;
/// Each newly held terrain frame's ways, its columns' spans off its lines,
/// generated with the cast from pxl-spec-core's `ways`: its one compute
/// entry.
pub const SHADER_WAY: u32 = 33;
/// Which eye slots' decor grows again this frame, listed for
/// `SHADER_GROWN_STAND`'s and `SHADER_GROWN`'s dispatches: its one compute entry.
pub const SHADER_GROWN_MARK: u32 = 34;
/// The eye's windows folded over their bricks, generated with the cast
/// from pxl-spec-core's `window::fold`: its one compute entry.
pub const SHADER_JOINED: u32 = 35;
/// The air's volume, a column of froxels a workgroup (atmos_froxel.wgsl):
/// its one compute entry.
pub const SHADER_ATMOS_FROXEL: u32 = 36;
/// Each handed brick's ways as a tree of cells in one pool (pxl-spec-core
/// `way::tree`), generated with the cast: a re-filled brick's old families
/// given back, its tree built off the pool's stack, the families its build
/// gave back pushed after: a compute entry each.
pub const SHADER_WAY_FREE: u32 = 37;
pub const SHADER_WAY_TREE: u32 = 38;
pub const SHADER_WAY_FREED: u32 = 39;
/// The camera grid's beam rays, a thread a corner, and where each cell's
/// pixels start off them, generated with the cast from pxl-spec-core's
/// `beam`: a compute entry each.
pub const SHADER_BEAM: u32 = 40;
pub const SHADER_CELL: u32 = 41;
/// The pixels the cast's reading walk missed, walked again writing the
/// observed pyramid, and the pyramid forgotten (pxl-spec-core `observed`): a
/// compute entry each.
pub const SHADER_MISS: u32 = 42;
pub const SHADER_FORGET: u32 = 43;
/// The samples, a thread each, resumed from their cells' starts and walked
/// to the cells the pixels about them take (pxl-spec-core `beam::Grid::samples`):
/// a compute entry.
pub const SHADER_SAMPLE: u32 = 44;
/// The world's depth from the sun into its clipmap, generated with the cast
/// from pxl-spec-core's `sun`: every slot taking the raster's pick, a
/// workgroup a tile, a compute entry; that depth laid into the map
/// (sun_map.wgsl, gpu/shadow.rs); and the slots that took none walked
/// apart (pxl-spec-core `sun::misses_at`), a compute entry of its own.
pub const SHADER_SUN: u32 = 45;
pub const SHADER_SUN_MAP: u32 = 46;
pub const SHADER_SUN_MISS: u32 = 47;
pub const SHADER_SCREEN_DEPTH: u32 = 48;
/// The camera grid's cells over the screen (camera_grid.wgsl).
pub const SHADER_CAMERA_GRID: u32 = 49;
/// The samples' stops handed to the rasterizer (pxl-spec-core `raster`):
/// their boxes written, a compute entry; and the boxes drawn, the nearest
/// face a pixel's ray enters kept with its sample (raster.wgsl), which the
/// cast's pixels read (`SHADER_CAST`).
pub const SHADER_BOXES: u32 = 50;
pub const SHADER_RASTER: u32 = 51;
/// The pixels the cast's reading walk missed, walked again from their list
/// (pxl-spec-core `observed::MARKS_AT`), a thread each: a compute entry.
pub const SHADER_MISS_LIST: u32 = 52;
/// What lies about each box's face, asked once a box (pxl-spec-core
/// `raster::ARGS_FACES`): a compute entry.
pub const SHADER_FACE: u32 = 53;
/// The tiles of the sun's box its receivers stand on in a cast
/// (pxl-spec-core `sun::stands_at`): where the eye's rays meet the world, and
/// where the decor and the actors drawn stand; then those they read
/// (`sun::reads_at`): compute entries.
pub const SHADER_SUN_MARK: u32 = 54;
pub const SHADER_SUN_MARK_DRAWN: u32 = 55;
pub const SHADER_SUN_REACH: u32 = 56;
/// Which candidates of a slot `SHADER_GROWN_MARK` listed stand, a patch at a
/// time, for `SHADER_GROWN` to grow: its one compute entry.
pub const SHADER_GROWN_STAND: u32 = 57;
/// The world's toned color encoded onto the canvas once (screen_finish.wgsl).
pub const SHADER_SCREEN_FINISH: u32 = 58;
/// The world's color gathered over frames (screen_resolve.wgsl).
pub const SHADER_SCREEN_RESOLVE: u32 = 59;
pub const SHADERS: u32 = 60;

/// The air as the frame's shaders read it: its medium, its tables and the
/// sky, the eye in it, the march through it, its light and the view
/// through it, between a shader's own parts before and after.
macro_rules! air {
    [$($before:literal),*; $($after:literal),*] => {
        &[$($before,)* "atmos_medium", "atmos_bind", "atmos_lut", "atmos_sky", "atmos_eye", "atmos_march", "atmos_light", "atmos_view", $($after),*]
    };
}

/// What a lit draw reads before its own parts: the frame, the air and the
/// volume before each point, the ground's light, the shading, the sun's
/// shadow, the finish and the screen's occlusion.
macro_rules! lit {
    [$($own:literal),*] => {
        air!["frame", "encode"; "atmos_air", "atmos_ground", "shade", "shadow", "finish", "ao_map", $($own),*]
    };
}

pub const PARTS: [&[&str]; SHADERS as usize] = [
    // SHADER_BOX
    lit!["paint", "box"],
    // SHADER_DECOR
    lit!["place", "instance", "decor"],
    // SHADER_SKINNED
    lit!["place", "skinned"],
    // SHADER_PUCK_ZONE
    &["frame", "encode", "finish", "puck", "puck_zone"],
    // SHADER_PUCK_BEACON
    &[
        "frame",
        "encode",
        "finish",
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_eye_unit",
        "place",
        "puck",
        "puck_beacon",
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
    &["frame", "ao_view", "ao_downsample"],
    // SHADER_AO_SAMPLE
    &["ao_view", "ao_sample"],
    // SHADER_AO_BLUR
    &["ao_view", "ao_blur"],
    // SHADER_AO_UPSAMPLE
    &["frame", "ao_view", "ao_upsample"],
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
        "atmos_eye_unit",
        "atmos_light",
        "atmos_pass",
        "atmos_march",
        "atmos_skyview",
    ],
    // SHADER_ATMOS_EXPOSURE
    &[
        "frame",
        "encode",
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_sky",
        "atmos_eye_unit",
        "atmos_march",
        "atmos_light",
        "atmos_view",
        "atmos_ground",
        "atmos_pass",
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
    // SHADER_SHEET
    &["sheet"],
    // SHADER_IMPOSTOR
    lit!["instance", "impostor"],
    // SHADER_IMPOSTOR_BAKE
    &["impostor_bake"],
    // SHADER_ATMOS_IRRADIANCE
    &[
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_eye_unit",
        "atmos_pass",
        "atmos_march",
        "atmos_irradiance",
    ],
    // SHADER_MARKER_REGION
    &["frame", "encode", "marker", "marker_region"],
    // SHADER_SCREEN_RAY
    lit!["lattice", "ray", "eye", "paint", "debug_look", "screen_ray", "screen_lit"],
    // SHADER_CAST
    &["cast.gen"],
    // SHADER_BOUND
    &["bound.gen"],
    // SHADER_HEIGHT
    &["height.gen"],
    // SHADER_GROWN
    &["decor.gen"],
    // SHADER_GROWN_CNT
    &["decor_cnt.gen"],
    // SHADER_GROWN_ARGS
    &["decor_args.gen"],
    // SHADER_GROWN_PLACE
    &["decor_place.gen"],
    // SHADER_WAY
    &["way.gen"],
    // SHADER_GROWN_MARK
    &["decor_mark.gen"],
    // SHADER_JOINED
    &["joined.gen"],
    // SHADER_ATMOS_FROXEL
    &[
        "frame",
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_eye",
        "atmos_march",
        "atmos_light",
        "atmos_ground",
        "atmos_view",
        "sky_word",
        "atmos_froxel",
    ],
    // SHADER_WAY_FREE
    &["way_free.gen"],
    // SHADER_WAY_TREE
    &["way_tree.gen"],
    // SHADER_WAY_FREED
    &["way_freed.gen"],
    // SHADER_BEAM
    &["beam.gen"],
    // SHADER_CELL
    &["cell.gen"],
    // SHADER_MISS
    &["miss.gen"],
    // SHADER_FORGET
    &["forget.gen"],
    // SHADER_SAMPLE
    &["sample.gen"],
    // SHADER_SUN
    &["sun.gen"],
    // SHADER_SUN_MAP
    &["sun_word", "sun_map"],
    // SHADER_SUN_MISS
    &["sun_miss.gen"],
    // SHADER_SCREEN_DEPTH
    lit![
        "lattice",
        "ray",
        "eye",
        "paint",
        "px",
        "screen_ray",
        "screen_depth"
    ],
    // SHADER_CAMERA_GRID
    &["frame", "encode", "sky_word", "debug_look", "camera_grid"],
    // SHADER_SAMPLE_BOX
    &["sample_box.gen"],
    // SHADER_RASTER
    &["raster_box", "raster"],
    // SHADER_MISS_LIST
    &["miss_list.gen"],
    // SHADER_FACE
    &["face.gen"],
    // SHADER_SUN_MARK
    &["sun_mark.gen"],
    // SHADER_SUN_MARK_DRAWN
    &["sun_mark_drawn.gen"],
    // SHADER_SUN_REACH
    &["sun_reach.gen"],
    // SHADER_GROWN_STAND
    &["decor_stand.gen"],
    // SHADER_SCREEN_FINISH
    &["frame", "encode", "finish", "screen_finish"],
    // SHADER_SCREEN_RESOLVE
    &["frame", "screen_resolve"],
];
