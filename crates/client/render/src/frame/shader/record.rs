// pxl-render/frame/shader/record.rs

//! Shaders and their pipeline records (lanes, bind groups, primitive, depth and
//! blend state) as one source the host turns into WebGPU or wgpu descriptors.
//! `cargo test` compiles and validates every shader with naga.

pub const SHADER_BOX: u32 = 0;
pub const SHADER_DECOR: u32 = 1;
pub const SHADER_SKINNED: u32 = 2;
pub const SHADER_PUCK_ZONE: u32 = 3;
pub const SHADER_PUCK_BEACON: u32 = 4;
pub const SHADER_BADGE: u32 = 5;
pub const SHADER_SKY: u32 = 6;
pub const SHADER_DEBUG: u32 = 7;
pub const SHADER_MIPS: u32 = 8;
pub const SHADER_AO_DOWNSAMPLE: u32 = 9;
pub const SHADER_AO_SAMPLE: u32 = 10;
pub const SHADER_AO_BLUR: u32 = 11;
pub const SHADER_AO_UPSAMPLE: u32 = 12;
pub const SHADER_ATMOS_TRANSMITTANCE: u32 = 13;
pub const SHADER_ATMOS_MULTISCATTER: u32 = 14;
pub const SHADER_ATMOS_SKY_VIEW: u32 = 15;
pub const SHADER_ATMOS_EXPOSURE: u32 = 16;
pub const SHADER_STARS: u32 = 17;
pub const SHADER_CLOUDS: u32 = 18;
pub const SHADER_CLOUD_FIELD: u32 = 19;
pub const SHADER_SHEET: u32 = 20;
pub const SHADER_IMPOSTOR: u32 = 21;
pub const SHADER_IMPOSTOR_BAKE: u32 = 22;
pub const SHADER_ATMOS_IRRADIANCE: u32 = 23;
pub const SHADER_BADGE_REGION: u32 = 24;
/// The camera grid's cells drawn straight to the screen (frame/screen_cells.rs).
pub const SHADER_SCREEN_CELLS: u32 = 25;
/// The eye's rays cast on the GPU into the screen's lane, generated from
/// pxl-portable's descent (script/portable-wgsl.ts): its one compute entry.
pub const SHADER_CAST: u32 = 26;
/// Each newly held terrain frame's bound over its cells' heights, a level a
/// dispatch, generated with the cast from pxl-portable's `ground::join`: its
/// one compute entry.
pub const SHADER_BOUND: u32 = 27;
/// Each newly held terrain frame's cells' heights off its samples,
/// generated with the cast from pxl-portable's `ground::height`: its one
/// compute entry.
pub const SHADER_HEIGHTS: u32 = 28;
pub const SHADERS: u32 = 29;

/// The shader files with comments and indentation stripped by build.rs;
/// constant.wgsl first.
macro_rules! wgsl {
    ($($name:literal),+) => { concat!(include_str!(concat!(env!("OUT_DIR"), "/shader/constant.wgsl")), "\n", $(include_str!(concat!(env!("OUT_DIR"), "/shader/", $name, ".wgsl")), "\n"),+) };
}

pub fn source(kind: u32) -> &'static str {
    match kind {
        SHADER_BOX => wgsl!(
            "frame",
            "globe",
            "encode",
            "atmos_medium",
            "atmos_bind",
            "atmos_lut",
            "atmos_sky",
            "atmos_eye",
            "atmos_march",
            "atmos_view",
            "atmos_ground",
            "shade",
            "finish",
            "shadow",
            "ao_map",
            "faces",
            "paint",
            "box"
        ),
        SHADER_DECOR => wgsl!(
            "frame",
            "globe",
            "encode",
            "atmos_medium",
            "atmos_bind",
            "atmos_lut",
            "atmos_sky",
            "atmos_eye",
            "atmos_march",
            "atmos_view",
            "atmos_ground",
            "shade",
            "finish",
            "shadow",
            "ao_map",
            "place",
            "decor"
        ),
        SHADER_SKINNED => wgsl!(
            "frame",
            "globe",
            "encode",
            "atmos_medium",
            "atmos_bind",
            "atmos_lut",
            "atmos_sky",
            "atmos_eye",
            "atmos_march",
            "atmos_view",
            "atmos_ground",
            "shade",
            "finish",
            "shadow",
            "ao_map",
            "place",
            "skinned"
        ),
        SHADER_PUCK_ZONE => wgsl!("frame", "globe", "encode", "finish", "puck", "puck_zone"),
        SHADER_PUCK_BEACON => wgsl!("frame", "globe", "encode", "finish", "atmos_medium", "atmos_bind", "atmos_lut", "place", "puck", "puck_beacon"),
        SHADER_BADGE => wgsl!("frame", "globe", "encode", "badge", "badge_sprite"),
        SHADER_BADGE_REGION => wgsl!("frame", "globe", "encode", "badge", "badge_region"),
        SHADER_SKY => wgsl!(
            "frame",
            "globe",
            "encode",
            "finish",
            "dither",
            "atmos_medium",
            "atmos_bind",
            "atmos_lut",
            "atmos_sky",
            "atmos_eye",
            "atmos_march",
            "atmos_view",
            "atmos_ground",
            "sky_words",
            "sky"
        ),
        SHADER_DEBUG => wgsl!("frame", "globe", "encode", "debug"),
        SHADER_SCREEN_CELLS => wgsl!(
            "frame",
            "globe",
            "encode",
            "atmos_medium",
            "atmos_bind",
            "atmos_lut",
            "atmos_sky",
            "atmos_eye",
            "atmos_march",
            "atmos_view",
            "atmos_ground",
            "shade",
            "finish",
            "shadow",
            "ao_map",
            "faces",
            "lattice",
            "paint",
            "sky_words",
            "screen_cells"
        ),
        // As generated: no constants of ours before it to clash with its names.
        SHADER_CAST => include_str!(concat!(env!("OUT_DIR"), "/shader/cast.gen.wgsl")),
        SHADER_BOUND => include_str!(concat!(env!("OUT_DIR"), "/shader/bound.gen.wgsl")),
        SHADER_HEIGHTS => include_str!(concat!(env!("OUT_DIR"), "/shader/heights.gen.wgsl")),
        SHADER_MIPS => wgsl!("mips"),
        SHADER_SHEET => wgsl!("sheet"),
        SHADER_IMPOSTOR => wgsl!(
            "frame",
            "globe",
            "encode",
            "atmos_medium",
            "atmos_bind",
            "atmos_lut",
            "atmos_sky",
            "atmos_eye",
            "atmos_march",
            "atmos_view",
            "atmos_ground",
            "shade",
            "finish",
            "shadow",
            "ao_map",
            "impostor"
        ),
        SHADER_IMPOSTOR_BAKE => wgsl!("impostor_bake"),
        SHADER_AO_DOWNSAMPLE => wgsl!("frame", "globe", "ao_view", "ao_downsample"),
        SHADER_AO_SAMPLE => wgsl!("ao_view", "ao_sample"),
        SHADER_AO_BLUR => wgsl!("ao_view", "ao_blur"),
        SHADER_AO_UPSAMPLE => wgsl!("frame", "globe", "ao_view", "ao_upsample"),
        SHADER_ATMOS_TRANSMITTANCE => wgsl!("atmos_medium", "atmos_pass", "atmos_transmittance"),
        SHADER_ATMOS_MULTISCATTER => wgsl!("atmos_medium", "atmos_bind", "atmos_lut", "atmos_pass", "atmos_multiscatter"),
        SHADER_ATMOS_SKY_VIEW => wgsl!("frame", "globe", "atmos_medium", "atmos_bind", "atmos_lut", "atmos_pass", "atmos_march", "atmos_skyview"),
        SHADER_ATMOS_IRRADIANCE => wgsl!("atmos_medium", "atmos_bind", "atmos_lut", "atmos_pass", "atmos_march", "atmos_irradiance"),
        SHADER_ATMOS_EXPOSURE => wgsl!(
            "frame",
            "globe",
            "encode",
            "atmos_medium",
            "atmos_bind",
            "atmos_lut",
            "atmos_sky",
            "atmos_eye_unit",
            "atmos_march",
            "atmos_view",
            "atmos_ground",
            "atmos_pass",
            "atmos_exposure"
        ),
        SHADER_STARS => wgsl!("frame", "globe", "encode", "stars"),
        SHADER_CLOUD_FIELD => wgsl!("noise", "cloud_tile", "atmos_pass", "cloud_bake"),
        SHADER_CLOUDS => wgsl!(
            "frame",
            "globe",
            "encode",
            "finish",
            "noise",
            "atmos_medium",
            "atmos_bind",
            "atmos_lut",
            "atmos_sky",
            "atmos_eye",
            "atmos_march",
            "atmos_view",
            "atmos_ground",
            "cloud_tile",
            "cloud_field",
            "clouds"
        ),
        _ => "",
    }
}

pub const FRAME_FLOATS: usize = 92;
/// Frame uniform field offsets in floats, in frame.wgsl's order.
pub const FRAME_AT: [usize; 20] = [0, 16, 19, 20, 23, 39, 43, 44, 46, 47, 48, 49, 50, 52, 68, 76, 80, 84, 87, 88];

/// Pipelines past draw.rs's, which a frame runs on its own.
pub const PIPELINE_SKY: u32 = 13;
pub const PIPELINE_MIPS: u32 = 14;
pub const PIPELINE_DECOR_PREPASS: u32 = 16;
pub const PIPELINE_SKINNED_PREPASS: u32 = 17;
pub const PIPELINE_AO_DOWNSAMPLE: u32 = 18;
pub const PIPELINE_AO_SAMPLE: u32 = 19;
pub const PIPELINE_AO_BLUR: u32 = 20;
pub const PIPELINE_AO_UPSAMPLE: u32 = 21;
pub const PIPELINE_ATMOS_TRANSMITTANCE: u32 = 22;
pub const PIPELINE_ATMOS_MULTISCATTER: u32 = 23;
pub const PIPELINE_ATMOS_SKY_VIEW: u32 = 24;
pub const PIPELINE_ATMOS_EXPOSURE: u32 = 25;
pub const PIPELINE_STARS: u32 = 26;
pub const PIPELINE_CLOUDS: u32 = 27;
pub const PIPELINE_CLOUDS_ABOVE: u32 = 28;
pub const PIPELINE_CLOUD_FIELD: u32 = 29;
pub const PIPELINE_SHEET: u32 = 33;
pub const PIPELINE_IMPOSTOR_BAKE: u32 = 35;
pub const PIPELINE_ATMOS_IRRADIANCE: u32 = 37;
/// The billboards' regions, numbered, into a target of their own (pass/badges.rs).
pub const PIPELINE_BADGE_REGION: u32 = 39;
/// The debug gizmos' faces, depth only, into the gizmos' own depth (gpu/renderer.rs `DebugPass`).
pub const PIPELINE_DEBUG_FACE: u32 = 40;
/// The camera grid's cells, opaque at their rays' depth (frame/screen_cells.rs).
pub const PIPELINE_SCREEN_CELLS: u32 = 41;
pub const PIPELINES: u32 = 42;

// Record words: shader, has fragment, topology, cull, depth write, depth
// compare, bias (i32), slope scale (f32 bits), blend, vertex entry; then vertex
// buffers (count; stride, step, attribute count, (location, offset, format)
// each); then bind groups (count; entry count, (binding, visibility, kind) each).
pub use crate::gpu::api_spec::record::*;

use crate::frame::draw::{
    PIPELINE_BADGE, PIPELINE_DEBUG, PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_BOX_COPLANAR, PIPELINE_GHOST_DECOR, PIPELINE_IMPOSTOR, PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE,
    PIPELINE_SKINNED,
};
use crate::mesh::ghost::BOX_WORDS;

const fn f(v: f32) -> u32 {
    v.to_bits()
}
const fn i(v: i32) -> u32 {
    v as u32
}

struct Head {
    shader: u32,
    fragment: bool,
    topology: u32,
    cull: u32,
    depth_write: bool,
    depth_compare: u32,
    depth_bias: u32,
    depth_bias_slope: u32,
    blend: u32,
    vertex: u32,
}

/// The shell holds the same list against the same numbers.
pub const VERTEX_ENTRY: [&str; 2] = ["vs", "vs_depth"];

const fn lit(shader: u32, cull: u32) -> Head {
    Head {
        shader,
        fragment: true,
        topology: TRIANGLES,
        cull,
        depth_write: true,
        depth_compare: GREATER,
        depth_bias: 0,
        depth_bias_slope: 0,
        blend: OPAQUE,
        vertex: VS,
    }
}

/// Positive bias is toward the eye under reversed depth.
const fn blended(shader: u32, cull: u32, bias: i32, slope: f32) -> Head {
    Head {
        shader,
        fragment: true,
        topology: TRIANGLES,
        cull,
        depth_write: false,
        depth_compare: GREATER,
        depth_bias: i(bias),
        depth_bias_slope: f(slope),
        blend: ALPHA,
        vertex: VS,
    }
}

const DEPTH_SLOPE_BIAS: f32 = 2.0;

/// Depth alone: plain less-than, no fragment stage, both sides, pushed back
/// by its slope as a rasterizer's polygon offset.
const fn depth_only(shader: u32) -> Head {
    Head {
        shader,
        fragment: false,
        topology: TRIANGLES,
        cull: CULL_NONE,
        depth_write: true,
        depth_compare: LESS,
        depth_bias: 0,
        depth_bias_slope: f(DEPTH_SLOPE_BIAS),
        blend: OPAQUE,
        vertex: VS_DEPTH,
    }
}

/// Depth from the eye for the screen-space occlusion, culled as the lit pass is.
const fn prepass(shader: u32, cull: u32) -> Head {
    Head {
        cull,
        depth_compare: GREATER,
        depth_bias_slope: 0,
        ..depth_only(shader)
    }
}

/// A billboard set's uniform and instances, and what its pass reads at an
/// anchor: the world's depth for the regions, the regions for the sprites.
fn badge_group(read: u32) -> Vec<u32> {
    group(&[[0, VERTEX, UNIFORM], [1, VERTEX, STORAGE], [2, VERTEX, read]])
}

const fn screen(shader: u32) -> Head {
    Head {
        depth_compare: NO_DEPTH,
        blend: OPAQUE,
        ..blended(shader, CULL_NONE, 0, 0.0)
    }
}

fn lane(stride: u32, location: u32, format: u32) -> Vec<u32> {
    vec![stride, PER_VERTEX, 1, location, 0, format]
}

fn group(entries: &[[u32; 3]]) -> Vec<u32> {
    let mut out = vec![entries.len() as u32];
    for e in entries {
        out.extend_from_slice(e);
    }
    out
}

fn record(h: Head, lanes: &[Vec<u32>], groups: &[Vec<u32>]) -> Vec<u32> {
    let mut out = vec![
        h.shader,
        h.fragment as u32,
        h.topology,
        h.cull,
        h.depth_write as u32,
        h.depth_compare,
        h.depth_bias,
        h.depth_bias_slope,
        h.blend,
        h.vertex,
    ];
    out.push(lanes.len() as u32);
    for l in lanes {
        out.extend_from_slice(l);
    }
    out.push(groups.len() as u32);
    for g in groups {
        out.extend_from_slice(g);
    }
    out
}

/// The air's bindings, numbered alike in every pass that reads it.
const AIR_SLOTS: [[u32; 2]; 10] = [
    [8, UNIFORM],
    [9, TEXTURE],
    [10, TEXTURE],
    [11, SAMPLER],
    [13, TEXTURE],
    [15, TEXTURE],
    [16, TEXTURE_ARRAY],
    [17, TEXTURE_ARRAY],
    [18, TEXTURE],
    [12, TEXTURE],
];

/// The air's bindings, each seen from the stage at its place in `AIR_SLOTS`.
const fn air(stages: [u32; 10]) -> [[u32; 3]; 10] {
    let mut out = [[0; 3]; 10];
    let mut i = 0;
    while i < out.len() {
        out[i] = [AIR_SLOTS[i][0], stages[i], AIR_SLOTS[i][1]];
        i += 1;
    }
    out
}

const AIR: [[u32; 3]; 10] = air([FRAGMENT; 10]);
/// Laid out where atmos_view.wgsl declares it and nothing reads it.
const AIR_UNREAD: [[u32; 3]; 10] = air([NONE; 10]);
const LIT_FRAME: [[u32; 3]; 14] = {
    // The air in the vertex stage, where each point's lights and veil are
    // worked out; a fragment reads only its words. The sky's views and the
    // Earth's picture are the sky pass's, laid out unread.
    let a = air([BOTH, VERTEX, NONE, VERTEX, VERTEX, NONE, NONE, NONE, VERTEX, VERTEX]);
    [
        [0, BOTH, UNIFORM],
        [1, FRAGMENT, DEPTH],
        [2, FRAGMENT, SAMPLER_COMPARISON],
        [7, FRAGMENT, TEXTURE],
        a[0],
        a[1],
        a[2],
        a[3],
        a[4],
        a[5],
        a[6],
        a[7],
        a[8],
        a[9],
    ]
};
/// The sky-view and moonlit-sky tables are laid out unread because the shader declares them.
const CLOUD_GROUP: [[u32; 3]; 14] = {
    let a = air([FRAGMENT, FRAGMENT, NONE, FRAGMENT, FRAGMENT, NONE, NONE, NONE, FRAGMENT, FRAGMENT]);
    [
        [0, FRAGMENT, UNIFORM],
        [1, FRAGMENT, UNIFORM],
        [2, FRAGMENT, TEXTURE],
        [3, FRAGMENT, SAMPLER],
        a[0],
        a[1],
        a[2],
        a[3],
        a[4],
        a[5],
        a[6],
        a[7],
        a[8],
        a[9],
    ]
};
const SKIN: [[u32; 3]; 2] = [[0, FRAGMENT, TEXTURE], [1, FRAGMENT, SAMPLER]];

/// The boxes are a per-instance vertex buffer, so a draw reaches its boxes by
/// its first instance.
fn box_lanes() -> Vec<Vec<u32>> {
    let stride = BOX_WORDS * 4;
    vec![vec![stride, PER_INSTANCE, 1, 0, 0, U32X4], vec![stride, PER_INSTANCE, 1, 1, 16, U32X2]]
}
fn box_groups() -> Vec<Vec<u32>> {
    let mut g0 = LIT_FRAME.to_vec();
    g0.extend([[3, VERTEX, STORAGE], [4, FRAGMENT, TEXTURE_ARRAY], [5, FRAGMENT, STORAGE], [6, FRAGMENT, SAMPLER]]);
    vec![group(&g0)]
}
/// The lit pass's frame, shadow, occlusion, air and layers, all read where
/// a pixel is painted; then the lane, the sky's words, the looks and the
/// eye's place.
fn screen_groups() -> Vec<u32> {
    let a = air([FRAGMENT, FRAGMENT, NONE, FRAGMENT, FRAGMENT, NONE, NONE, NONE, FRAGMENT, FRAGMENT]);
    let mut g = vec![[0, FRAGMENT, UNIFORM], [1, FRAGMENT, DEPTH], [2, FRAGMENT, SAMPLER_COMPARISON], [7, FRAGMENT, TEXTURE]];
    g.extend(a);
    g.extend([[3, FRAGMENT, STORAGE], [4, FRAGMENT, TEXTURE_ARRAY], [5, FRAGMENT, STORAGE], [6, FRAGMENT, SAMPLER]]);
    g.extend([[19, FRAGMENT, UNIFORM], [20, FRAGMENT, STORAGE], [21, FRAGMENT, UNIFORM]]);
    group(&g)
}
fn model_lanes(skinned: bool) -> Vec<Vec<u32>> {
    let mut lanes = vec![lane(12, 0, F32X3), lane(12, 1, F32X3), lane(8, 2, F32X2)];
    if skinned {
        lanes.push(lane(16, 3, U32X4));
        lanes.push(lane(16, 4, F32X4));
    }
    lanes
}
fn model_groups() -> Vec<Vec<u32>> {
    vec![group(&LIT_FRAME), group(&[[0, BOTH, UNIFORM_DYNAMIC]]), group(&SKIN), group(&[[0, VERTEX, STORAGE]])]
}

pub fn pipeline(id: u32) -> Vec<u32> {
    let frame = |visibility: u32| group(&[[0, visibility, UNIFORM]]);
    match id {
        PIPELINE_DECOR => record(lit(SHADER_DECOR, CULL_NONE), &model_lanes(false), &model_groups()),
        // Slightly clear of a surface it sits on, strongly clear of one it sits in.
        PIPELINE_GHOST_BOX => record(blended(SHADER_BOX, CULL_BACK, 1, 1.0), &box_lanes(), &box_groups()),
        PIPELINE_GHOST_BOX_COPLANAR => record(blended(SHADER_BOX, CULL_BACK, 8, 2.0), &box_lanes(), &box_groups()),
        PIPELINE_GHOST_DECOR => record(blended(SHADER_DECOR, CULL_NONE, 1, 1.0), &model_lanes(false), &model_groups()),
        // Back faces culled: drawn both sides, a closed body's inner faces fight its
        // outer ones at a distance and the character shimmers.
        PIPELINE_SKINNED => record(lit(SHADER_SKINNED, CULL_BACK), &model_lanes(true), &model_groups()),
        PIPELINE_PUCK_ZONE => record(
            blended(SHADER_PUCK_ZONE, CULL_NONE, 4, 1.0),
            &[lane(12, 0, F32X3), lane(8, 1, F32X2)],
            &[frame(BOTH), group(&[[0, BOTH, UNIFORM]]), group(&[[0, VERTEX, STORAGE]])],
        ),
        PIPELINE_PUCK_BEACON => record(
            lit(SHADER_PUCK_BEACON, CULL_BACK),
            &[lane(12, 0, F32X3), lane(12, 1, F32X3)],
            &[group(&[[0, BOTH, UNIFORM], AIR[0], AIR[1], AIR[3]]), group(&[[0, BOTH, UNIFORM]])],
        ),
        PIPELINE_BADGE => record(
            Head {
                topology: STRIP,
                depth_compare: GREATER_EQUAL,
                ..blended(SHADER_BADGE, CULL_NONE, 0, 0.0)
            },
            &[],
            &[frame(BOTH), badge_group(TEXTURE_UNFILTERED), group(&SKIN)],
        ),
        PIPELINE_BADGE_REGION => record(
            Head {
                topology: STRIP,
                depth_write: true,
                depth_compare: GREATER_EQUAL,
                blend: OPAQUE,
                ..blended(SHADER_BADGE_REGION, CULL_NONE, 0, 0.0)
            },
            &[],
            &[frame(BOTH), badge_group(DEPTH)],
        ),
        // Against the gizmos' own faces, not the world's depth.
        PIPELINE_DEBUG => record(
            Head {
                blend: ALPHA,
                ..blended(SHADER_DEBUG, CULL_NONE, 0, 0.0)
            },
            &[vec![56, PER_INSTANCE, 3, 0, 0, F32X3, 1, 12, F32X4, 2, 28, F32X3]],
            &[frame(VERTEX), group(&[[0, VERTEX, UNIFORM]])],
        ),
        // Pushed back by their slope as a rasterizer's polygon offset is, so
        // the lines that bound a face stay in front of it.
        PIPELINE_DEBUG_FACE => record(
            Head {
                cull: CULL_NONE,
                depth_compare: GREATER,
                depth_bias: i(-8),
                depth_bias_slope: f(-2.0),
                ..depth_only(SHADER_DEBUG)
            },
            &[lane(12, 0, F32X3)],
            &[frame(VERTEX), group(&[[0, VERTEX, UNIFORM]])],
        ),
        PIPELINE_SCREEN_CELLS => record(
            Head {
                depth_write: true,
                blend: OPAQUE,
                ..blended(SHADER_SCREEN_CELLS, CULL_NONE, 0, 0.0)
            },
            &[],
            &[screen_groups()],
        ),
        PIPELINE_DECOR_PREPASS => record(prepass(SHADER_DECOR, CULL_NONE), &model_lanes(false), &model_groups()),
        PIPELINE_SKINNED_PREPASS => record(prepass(SHADER_SKINNED, CULL_BACK), &model_lanes(true), &model_groups()),
        // Only where the opaque world left the far plane, so covered pixels never run it.
        PIPELINE_SKY => record(
            Head {
                depth_compare: GREATER_EQUAL,
                blend: OPAQUE,
                ..blended(SHADER_SKY, CULL_NONE, 0, 0.0)
            },
            &[],
            &[group(&[
                [0, FRAGMENT, UNIFORM],
                [19, FRAGMENT, UNIFORM],
                [2, FRAGMENT, TEXTURE_ARRAY],
                [3, FRAGMENT, SAMPLER],
                AIR[0],
                AIR[1],
                AIR[2],
                AIR[3],
                AIR[4],
                AIR[5],
                AIR[6],
                AIR[7],
                AIR[8],
                AIR_UNREAD[9],
            ])],
        ),
        PIPELINE_MIPS => record(screen(SHADER_MIPS), &[], &[group(&SKIN)]),
        PIPELINE_IMPOSTOR => record(
            Head {
                topology: STRIP,
                ..lit(SHADER_IMPOSTOR, CULL_NONE)
            },
            &[],
            &[
                group(&LIT_FRAME),
                group(&[[0, BOTH, UNIFORM_DYNAMIC]]),
                group(&[[0, FRAGMENT, TEXTURE_ARRAY], [1, FRAGMENT, SAMPLER], [2, VERTEX, UNIFORM]]),
                group(&[[0, VERTEX, STORAGE]]),
            ],
        ),
        PIPELINE_IMPOSTOR_BAKE => record(
            Head {
                fragment: true,
                vertex: VS,
                ..depth_only(SHADER_IMPOSTOR_BAKE)
            },
            &model_lanes(false),
            &[group(&[[0, VERTEX, UNIFORM_DYNAMIC]]), group(&SKIN)],
        ),
        PIPELINE_SHEET => record(
            Head {
                topology: STRIP,
                blend: ALPHA,
                ..screen(SHADER_SHEET)
            },
            &[],
            &[group(&[[0, VERTEX, UNIFORM], [1, BOTH, STORAGE]]), group(&[[0, FRAGMENT, TEXTURE_ARRAY], [1, FRAGMENT, SAMPLER]])],
        ),
        PIPELINE_AO_DOWNSAMPLE => record(screen(SHADER_AO_DOWNSAMPLE), &[], &[group(&[[0, FRAGMENT, UNIFORM], [1, FRAGMENT, DEPTH], [6, FRAGMENT, UNIFORM]])]),
        PIPELINE_AO_SAMPLE => record(
            screen(SHADER_AO_SAMPLE),
            &[],
            &[group(&[[1, FRAGMENT, TEXTURE_UNFILTERED], [2, FRAGMENT, TEXTURE], [6, FRAGMENT, UNIFORM]])],
        ),
        PIPELINE_AO_BLUR => record(
            screen(SHADER_AO_BLUR),
            &[],
            &[group(&[
                [1, FRAGMENT, TEXTURE_UNFILTERED],
                [2, FRAGMENT, TEXTURE],
                [3, FRAGMENT, TEXTURE],
                [4, FRAGMENT, SAMPLER],
                [5, FRAGMENT, UNIFORM_DYNAMIC],
                [6, FRAGMENT, UNIFORM],
            ])],
        ),
        PIPELINE_AO_UPSAMPLE => record(
            screen(SHADER_AO_UPSAMPLE),
            &[],
            &[group(&[
                [0, FRAGMENT, UNIFORM],
                [1, FRAGMENT, DEPTH],
                [2, FRAGMENT, TEXTURE_UNFILTERED],
                [3, FRAGMENT, TEXTURE],
                [4, FRAGMENT, SAMPLER],
                [6, FRAGMENT, UNIFORM],
            ])],
        ),
        PIPELINE_ATMOS_TRANSMITTANCE => record(screen(SHADER_ATMOS_TRANSMITTANCE), &[], &[group(&[AIR[0]])]),
        PIPELINE_ATMOS_MULTISCATTER => record(screen(SHADER_ATMOS_MULTISCATTER), &[], &[group(&[AIR[0], AIR[1], AIR[3]])]),
        PIPELINE_ATMOS_SKY_VIEW => record(
            screen(SHADER_ATMOS_SKY_VIEW),
            &[],
            &[group(&[[0, FRAGMENT, UNIFORM], AIR[0], AIR[1], AIR[3], [12, FRAGMENT, TEXTURE], [14, FRAGMENT, UNIFORM]])],
        ),
        PIPELINE_ATMOS_IRRADIANCE => record(screen(SHADER_ATMOS_IRRADIANCE), &[], &[group(&[AIR[0], AIR[1], AIR[3], [12, FRAGMENT, TEXTURE]])]),
        // The light on the land under the eye: the air's words and the tables it is lit by.
        PIPELINE_ATMOS_EXPOSURE => record(
            screen(SHADER_ATMOS_EXPOSURE),
            &[],
            &[group(&[
                [0, FRAGMENT, UNIFORM],
                AIR[0],
                AIR[1],
                AIR_UNREAD[2],
                AIR[3],
                AIR_UNREAD[5],
                AIR_UNREAD[6],
                AIR_UNREAD[7],
                AIR[8],
                AIR_UNREAD[9],
            ])],
        ),
        PIPELINE_STARS => record(
            Head {
                topology: STRIP,
                depth_compare: GREATER_EQUAL,
                blend: ADD,
                ..blended(SHADER_STARS, CULL_NONE, 0, 0.0)
            },
            &[],
            &[frame(VERTEX), group(&[[0, VERTEX, UNIFORM], [1, VERTEX, STORAGE]])],
        ),
        // From under the sheets only where the world left the far plane; from over them everywhere.
        PIPELINE_CLOUDS => record(
            Head {
                depth_compare: GREATER_EQUAL,
                ..blended(SHADER_CLOUDS, CULL_NONE, 0, 0.0)
            },
            &[],
            &[group(&CLOUD_GROUP)],
        ),
        PIPELINE_CLOUDS_ABOVE => record(
            Head {
                depth_compare: ALWAYS,
                ..blended(SHADER_CLOUDS, CULL_NONE, 0, 0.0)
            },
            &[],
            &[group(&CLOUD_GROUP)],
        ),
        PIPELINE_CLOUD_FIELD => record(screen(SHADER_CLOUD_FIELD), &[], &[]),
        _ => Vec::new(),
    }
}

#[cfg(test)]
#[path = "record.test.rs"]
mod test;
