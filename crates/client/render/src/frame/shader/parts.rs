// pxl-render/frame/shader/parts.rs

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
pub const SHADER_BADGE: u32 = 5;
pub const SHADER_SKY: u32 = 6;
pub const SHADER_DEBUG: u32 = 7;
pub const SHADER_MIPS: u32 = 8;
pub const SHADER_ATMOS_TRANSMITTANCE: u32 = 9;
pub const SHADER_ATMOS_MULTISCATTER: u32 = 10;
pub const SHADER_ATMOS_SKY_VIEW: u32 = 11;
pub const SHADER_ATMOS_EXPOSURE: u32 = 12;
pub const SHADER_STARS: u32 = 13;
pub const SHADER_CLOUDS: u32 = 14;
pub const SHADER_CLOUD_FIELD: u32 = 15;
pub const SHADER_SHEET: u32 = 16;
pub const SHADER_IMPOSTOR: u32 = 17;
pub const SHADER_IMPOSTOR_BAKE: u32 = 18;
pub const SHADER_ATMOS_IRRADIANCE: u32 = 19;
pub const SHADER_BADGE_REGION: u32 = 20;
/// The eye's rays drawn at their pixels (pxl-portable ray.rs).
pub const SHADER_SCREEN_RAYS: u32 = 21;
/// The eye's rays cast on the GPU into the world surface's rays, generated from
/// pxl-portable's descent (script/portable-wgsl.ts): its one compute entry.
pub const SHADER_CAST: u32 = 22;
/// Each newly held square's levels over its columns, a level a dispatch,
/// generated with the cast from pxl-portable's `fill::level` and
/// `window::join`: its one compute entry.
pub const SHADER_BOUND: u32 = 23;
/// Each newly held square's columns' ground tops off its samples,
/// generated with the cast from pxl-portable's `ground::height`: its one
/// compute entry.
pub const SHADER_HEIGHTS: u32 = 24;
/// The decor the ground grows in the eye's windows, generated with the cast
/// from pxl-portable's `scatter`: its one compute entry.
pub const SHADER_GROWN: u32 = 25;
/// What `SHADER_GROWN` and placed decor leave, sized for the screen and
/// counted a draw region each; the regions laid out with their indirect
/// args; each drawn into the decor block: pxl-portable `scatter::drawn`.
pub const SHADER_GROWN_COUNT: u32 = 26;
pub const SHADER_GROWN_ARGS: u32 = 27;
pub const SHADER_GROWN_PLACE: u32 = 28;
/// Each newly held terrain frame's ways, its columns' runs along z off its
/// lines, generated with the cast from pxl-portable's `ways`: its one
/// compute entry.
pub const SHADER_WAYS: u32 = 29;
/// The world image laid on a frame (blit.wgsl).
pub const SHADER_BLIT: u32 = 30;
/// Where every ray of the cast starts alike, asked once a frame before it,
/// generated with the cast from pxl-portable's `cast::start`: its one
/// compute entry.
pub const SHADER_START: u32 = 31;
/// Which eye slots' decor grows again this frame, listed for
/// `SHADER_GROWN`'s dispatch: its one compute entry.
pub const SHADER_GROWN_MARK: u32 = 32;
/// The eye's windows folded over their squares, generated with the cast
/// from pxl-portable's `window::fold`: its one compute entry.
pub const SHADER_JOINED: u32 = 33;
/// The air's volume, a froxel a thread (atmos_froxel.wgsl): its one
/// compute entry.
pub const SHADER_ATMOS_FROXEL: u32 = 34;
/// Each tile's beam, where its rays start, generated with the cast from
/// pxl-portable's `cast::Beam`: its one compute entry.
pub const SHADER_BEAM: u32 = 35;
pub const SHADERS: u32 = 36;

pub const PARTS: [&[&str]; SHADERS as usize] = [
    // SHADER_BOX
    &[
        "frame",
        "encode",
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_sky",
        "atmos_eye",
        "atmos_march",
        "atmos_light",
        "atmos_view",
        "atmos_air",
        "atmos_ground",
        "shade",
        "finish",
        "paint",
        "box",
    ],
    // SHADER_DECOR
    &[
        "frame",
        "encode",
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_sky",
        "atmos_eye",
        "atmos_march",
        "atmos_light",
        "atmos_view",
        "atmos_air",
        "atmos_ground",
        "shade",
        "finish",
        "place",
        "decor",
    ],
    // SHADER_SKINNED
    &[
        "frame",
        "encode",
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_sky",
        "atmos_eye",
        "atmos_march",
        "atmos_light",
        "atmos_view",
        "atmos_air",
        "atmos_ground",
        "shade",
        "finish",
        "place",
        "skinned",
    ],
    // SHADER_PUCK_ZONE
    &["frame", "encode", "finish", "puck", "puck_zone"],
    // SHADER_PUCK_BEACON
    &["frame", "encode", "finish", "atmos_medium", "atmos_bind", "atmos_lut", "atmos_eye_unit", "place", "puck", "puck_beacon"],
    // SHADER_BADGE
    &["frame", "encode", "badge", "badge_sprite"],
    // SHADER_SKY
    &[
        "frame",
        "encode",
        "finish",
        "dither",
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_sky",
        "atmos_eye",
        "atmos_march",
        "atmos_light",
        "atmos_view",
        "atmos_air",
        "atmos_ground",
        "sky_words",
        "sky",
    ],
    // SHADER_DEBUG
    &["frame", "encode", "debug"],
    // SHADER_MIPS
    &["mips"],
    // SHADER_ATMOS_TRANSMITTANCE
    &["atmos_medium", "atmos_pass", "atmos_transmittance"],
    // SHADER_ATMOS_MULTISCATTER
    &["atmos_medium", "atmos_bind", "atmos_lut", "atmos_pass", "atmos_multiscatter"],
    // SHADER_ATMOS_SKY_VIEW
    &[
        "frame",
        "encode",
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
    // SHADER_STARS
    &["frame", "encode", "atmos_medium", "atmos_bind", "atmos_lut", "stars"],
    // SHADER_CLOUDS
    &[
        "frame",
        "encode",
        "finish",
        "noise",
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_sky",
        "atmos_eye",
        "atmos_march",
        "atmos_light",
        "atmos_view",
        "atmos_ground",
        "cloud_tile",
        "cloud_field",
        "clouds",
    ],
    // SHADER_CLOUD_FIELD
    &["noise", "cloud_tile", "atmos_pass", "cloud_bake"],
    // SHADER_SHEET
    &["sheet"],
    // SHADER_IMPOSTOR
    &[
        "frame",
        "encode",
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_sky",
        "atmos_eye",
        "atmos_march",
        "atmos_light",
        "atmos_view",
        "atmos_air",
        "atmos_ground",
        "shade",
        "finish",
        "impostor",
    ],
    // SHADER_IMPOSTOR_BAKE
    &["impostor_bake"],
    // SHADER_ATMOS_IRRADIANCE
    &["atmos_medium", "atmos_bind", "atmos_lut", "atmos_eye_unit", "atmos_pass", "atmos_march", "atmos_irradiance"],
    // SHADER_BADGE_REGION
    &["frame", "encode", "badge", "badge_region"],
    // SHADER_SCREEN_RAYS
    &[
        "frame",
        "encode",
        "atmos_medium",
        "atmos_bind",
        "atmos_lut",
        "atmos_sky",
        "atmos_eye",
        "atmos_march",
        "atmos_light",
        "atmos_view",
        "atmos_air",
        "atmos_ground",
        "shade",
        "finish",
        "lattice",
        "ray",
        "eye",
        "paint",
        "screen_rays",
    ],
    // SHADER_CAST
    &["cast.gen"],
    // SHADER_BOUND
    &["bound.gen"],
    // SHADER_HEIGHTS
    &["heights.gen"],
    // SHADER_GROWN
    &["decor.gen"],
    // SHADER_GROWN_COUNT
    &["decor_count.gen"],
    // SHADER_GROWN_ARGS
    &["decor_args.gen"],
    // SHADER_GROWN_PLACE
    &["decor_place.gen"],
    // SHADER_WAYS
    &["ways.gen"],
    // SHADER_BLIT
    &["blit"],
    // SHADER_START
    &["start.gen"],
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
        "sky_words",
        "atmos_froxel",
    ],
    // SHADER_BEAM
    &["beam.gen"],
];
