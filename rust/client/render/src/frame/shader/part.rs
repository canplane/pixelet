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
/// The eye's rays drawn at their pixels (pxl-spec-core ray.rs).
pub const SHADER_SCREEN_RAY: u32 = 24;
/// The eye's rays cast on the GPU into the world surface's rays, generated from
/// pxl-spec-core's descent (pxl-tool bin/shader_wgsl.rs): its one compute entry.
pub const SHADER_CAST: u32 = 25;
/// Each newly held brick's levels over its columns, a level a dispatch,
/// generated with the cast from pxl-spec-core's `fill::level` and
/// `clipmap::join`: its one compute entry.
pub const SHADER_BOUND: u32 = 26;
/// Each newly held brick's columns' ground tops off its samples,
/// generated with the cast from pxl-spec-core's `ground::height`: its one
/// compute entry.
pub const SHADER_HEIGHT: u32 = 27;
/// The decor the ground generates in the eye's clip levels, generated with the
/// cast from pxl-spec-core's `scatter`: its one compute entry.
pub const SHADER_GENERATION: u32 = 28;
/// What `SHADER_GENERATION` and placed decor leave, sized for the screen and
/// counted a draw region each; the regions laid out with their indirect
/// args; each drawn compacted into the decor's instances: pxl-spec-core
/// `generation::drawn`.
pub const SHADER_GENERATION_CNT: u32 = 29;
pub const SHADER_GENERATION_ARGS: u32 = 30;
pub const SHADER_GENERATION_COMPACT: u32 = 31;
/// Each newly held terrain frame's ways, its columns' spans off its lines,
/// generated with the cast from pxl-spec-core's `ways`: its one compute
/// entry.
pub const SHADER_WAY: u32 = 32;
/// Which eye slots' decor generates again this frame, listed for
/// `SHADER_GENERATION_THIN`'s and `SHADER_GENERATION`'s dispatches: its one
/// compute entry.
pub const SHADER_GENERATION_MARK: u32 = 33;
/// The eye's clip levels folded over their bricks, generated with the cast
/// from pxl-spec-core's `clipmap::fold`: its one compute entry.
pub const SHADER_FOLD: u32 = 34;
/// A light's share of the air's volume, a column of froxels a workgroup
/// (atmos_froxel.wgsl): its one compute entry.
pub const SHADER_ATMOS_FROXEL: u32 = 35;
/// Each handed brick's ways as a tree of cells in one pool (pxl-spec-core
/// `way::tree`), generated with the cast: a re-filled brick's old child groups
/// given back, its tree built off the pool's stack, the child groups its build
/// gave back pushed after: a compute entry each.
pub const SHADER_WAY_FREE: u32 = 36;
pub const SHADER_WAY_TREE: u32 = 37;
pub const SHADER_WAY_RETURN: u32 = 38;
/// The camera grid's beam rays, a thread a corner, and where each cell's
/// pixels start off them, generated with the cast from pxl-spec-core's
/// `beam`: a compute entry each.
pub const SHADER_BEAM: u32 = 39;
pub const SHADER_CELL: u32 = 40;
/// The pixels the cast's reading walk missed, walked again writing the
/// observed pyramid, and the pyramid forgotten (pxl-spec-core `pyramid`): a
/// compute entry each.
pub const SHADER_MISS: u32 = 41;
pub const SHADER_FORGET: u32 = 42;
/// The samples, a thread each, resumed from their cells' starts and walked
/// to the cells the pixels about them take (pxl-spec-core `beam::Grid::samples`):
/// a compute entry.
pub const SHADER_SAMPLE: u32 = 43;
/// The world's depth from the light into its clipmap, generated with the cast
/// from pxl-spec-core's `light`: every slot taking the visible sample, a
/// workgroup a tile, a compute entry; that depth laid into the map
/// (light_map.wgsl, gpu/shadow.rs); and the slots that took none walked
/// apart (pxl-spec-core `light::misses_at`), a compute entry of its own.
pub const SHADER_LIGHT: u32 = 44;
pub const SHADER_LIGHT_MAP: u32 = 45;
pub const SHADER_LIGHT_MISS: u32 = 46;
/// The occlusion's half-resolution map brought to the pixel and multiplied
/// into the world's color, as the tiles' quads (ao_apply.wgsl).
pub const SHADER_AO_APPLY: u32 = 47;
/// The camera grid's cells over the screen (camera_grid.wgsl).
pub const SHADER_CAMERA_GRID: u32 = 48;
/// The samples' hits handed to the rasterizer (pxl-spec-core `raster`):
/// their boxes written, a compute entry; and the boxes drawn, the nearest
/// face a pixel's ray enters kept with its sample (raster.wgsl), which the
/// cast's pixels read (`SHADER_CAST`).
pub const SHADER_SAMPLE_BOX: u32 = 49;
pub const SHADER_RASTER: u32 = 50;
/// The pixels the cast's reading walk missed, walked again from their list
/// (pxl-spec-core `pyramid::MARKS_AT`), a thread each: a compute entry.
pub const SHADER_MISS_LIST: u32 = 51;
/// What lies about each box's face, asked once a box (pxl-spec-core
/// `raster::ARGS_FACES`): a compute entry.
pub const SHADER_FACE: u32 = 52;
/// The tiles of the light's box its receivers stand on in a cast
/// (pxl-spec-core `light::stands_at`): where the eye's rays meet the world, and
/// where the decor and the actors drawn stand; then those they read
/// (`light::reads_at`): compute entries.
pub const SHADER_LIGHT_MARK: u32 = 53;
pub const SHADER_LIGHT_MARK_DRAWS: u32 = 54;
pub const SHADER_LIGHT_PROPAGATE: u32 = 55;
/// Which candidates of a slot `SHADER_GENERATION_MARK` listed the thinning
/// retains, a clip level cell a thread, for `SHADER_GENERATION` to generate: its
/// one compute entry.
pub const SHADER_GENERATION_THIN: u32 = 56;
/// The world's toned color encoded onto the canvas once (screen_finish.wgsl).
pub const SHADER_SCREEN_FINISH: u32 = 57;
/// The world's color gathered over frames (screen_resolve.wgsl).
pub const SHADER_SCREEN_RESOLVE: u32 = 58;
/// The claimed tiles set back to nothing at the far plane, their stencil
/// raised (screen_gate.wgsl).
pub const SHADER_SCREEN_GATE: u32 = 59;
/// The finished picture kept at the canvas's size shown on the canvas
/// (screen_show.wgsl).
pub const SHADER_SCREEN_SHOW: u32 = 60;
/// The light's map's tiles that changed propagated to the screen's
/// (screen_propagate.wgsl): its one compute entry.
pub const SHADER_SCREEN_PROPAGATE: u32 = 61;
/// The claimed tiles of the history the resolve wrote copied back into the
/// last (screen_write_back.wgsl).
pub const SHADER_SCREEN_WRITE_BACK: u32 = 62;
/// The claimed tiles of the samples' raster set back to no sample at the far
/// plane (raster_gate.wgsl).
pub const SHADER_RASTER_GATE: u32 = 63;
/// The tiles the frame's one mark covers, or every one, taking its number
/// (screen_mark.wgsl): its one compute entry.
pub const SHADER_SCREEN_MARK: u32 = 64;
/// The world's depth laid in the claimed tiles of the light's map, their
/// stencil raised (light_gate.wgsl).
pub const SHADER_LIGHT_GATE: u32 = 65;
/// Each square of the screen's least and most depth, set in the tiles drawn
/// (screen_bound.wgsl): its one compute entry.
pub const SHADER_SCREEN_BOUND: u32 = 66;
/// An actor where it was and where it is, its fragments marking their screen
/// tiles (skinned_place.wgsl, tile_mark.wgsl).
pub const SHADER_SKINNED_MARK: u32 = 67;
/// The tiles of the light's box the box slid onto, or whose eye moved past
/// the margin of what their walks asked (pxl-spec-core `light::holds`): a
/// compute entry.
pub const SHADER_LIGHT_DETAIL: u32 = 68;
/// Decor, an impostor, the location indicator's zone and its beacon where each
/// was and where it is, their fragments marking their tiles (tile_mark.wgsl).
pub const SHADER_DECOR_MARK: u32 = 69;
pub const SHADER_IMPOSTOR_MARK: u32 = 70;
pub const SHADER_LOCATION_ZONE_MARK: u32 = 71;
pub const SHADER_LOCATION_BEACON_MARK: u32 = 72;
/// The shadow map's beam rays and samples, the eye's passes in entries of
/// their own, which tell what their walks ask of (pxl-spec-core
/// `cast::Margin`): compute entries.
pub const SHADER_LIGHT_BEAM: u32 = 73;
pub const SHADER_LIGHT_SAMPLE: u32 = 74;
/// The missed pixels' hits whose faces are asked once a square of them,
/// the one of each square that asks listed (pxl-spec-core `beam::HIT_WORDS`), and
/// the faces asked put at their pixels: compute entries.
pub const SHADER_HIT_LEAD: u32 = 75;
pub const SHADER_HIT_FACES: u32 = 76;
/// The indirect args of a quad a tile of the box the frame's marks cover
/// (box_args.wgsl): its one compute entry.
pub const SHADER_BOX_ARGS: u32 = 77;
/// What each tile the light's cast casts holds by, gathered off its beam
/// rays and samples, and lowered by its listed slots' walks (pxl-spec-core
/// `light::HOLD_MARGIN`, `light::miss_word`): compute entries.
pub const SHADER_LIGHT_MARGIN: u32 = 78;
pub const SHADER_LIGHT_MISS_MARGIN: u32 = 79;
pub const SHADERS: u32 = 80;

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
        "finish",
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_sky",
        "atmos_eye_unit",
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
        "atmos_eye_unit",
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
        "screen_shade"
    ],
    // SHADER_CAST
    &["cast.gen"],
    // SHADER_BOUND
    &["bound.gen"],
    // SHADER_HEIGHT
    &["height.gen"],
    // SHADER_GENERATION
    &["decor.gen"],
    // SHADER_GENERATION_CNT
    &["decor_cnt.gen"],
    // SHADER_GENERATION_ARGS
    &["decor_args.gen"],
    // SHADER_GENERATION_COMPACT
    &["decor_compact.gen"],
    // SHADER_WAY
    &["way.gen"],
    // SHADER_GENERATION_MARK
    &["decor_mark.gen"],
    // SHADER_FOLD
    &["fold.gen"],
    // SHADER_ATMOS_FROXEL
    &[
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_march",
        "atmos_froxel",
    ],
    // SHADER_WAY_FREE
    &["way_free.gen"],
    // SHADER_WAY_TREE
    &["way_tree.gen"],
    // SHADER_WAY_RETURN
    &["way_return.gen"],
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
    // SHADER_LIGHT
    &["light.gen"],
    // SHADER_LIGHT_MAP
    &["light_word", "light_lay", "light_map"],
    // SHADER_LIGHT_MISS
    &["light_miss.gen"],
    // SHADER_AO_APPLY
    &["tile_word", "screen_tiles", "effect", "ao_view", "ao_apply"],
    // SHADER_CAMERA_GRID
    &["frame", "encode", "sky_word", "debug_look", "camera_grid"],
    // SHADER_SAMPLE_BOX
    &["sample_box.gen"],
    // SHADER_RASTER
    &["raster_box", "px", "raster"],
    // SHADER_MISS_LIST
    &["miss_list.gen"],
    // SHADER_FACE
    &["face.gen"],
    // SHADER_LIGHT_MARK
    &["light_mark.gen"],
    // SHADER_LIGHT_MARK_DRAWS
    &["light_mark_draws.gen"],
    // SHADER_LIGHT_PROPAGATE
    &["light_propagate.gen"],
    // SHADER_GENERATION_THIN
    &["decor_thin.gen"],
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
    // SHADER_LIGHT_DETAIL
    &["light_detail.gen"],
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
    // SHADER_LIGHT_BEAM
    &["light_beam.gen"],
    // SHADER_LIGHT_SAMPLE
    &["light_sample.gen"],
    // SHADER_HIT_LEAD
    &["hit_lead.gen"],
    // SHADER_HIT_FACES
    &["hit_faces.gen"],
    // SHADER_BOX_ARGS
    &["box_args"],
    // SHADER_LIGHT_MARGIN
    &["light_margin.gen"],
    // SHADER_LIGHT_MISS_MARGIN
    &["light_miss_margin.gen"],
];
