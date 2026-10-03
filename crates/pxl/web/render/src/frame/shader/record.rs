// pxl-render/frame/shader/record.rs

//! Shaders and their pipeline records (lanes, bind groups, primitive, depth and
//! blend state) as one source the host turns into WebGPU or wgpu descriptors.
//! `cargo test` compiles every shader with naga and holds each record against
//! what the shader declares.

pub const SHADER_CHUNK: u32 = 0;
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
pub const SHADERS: u32 = 23;

/// The shader files with comments and indentation stripped by build.rs;
/// constant.wgsl first.
macro_rules! wgsl {
    ($($name:literal),+) => { concat!(include_str!(concat!(env!("OUT_DIR"), "/shader/constant.wgsl")), "\n", $(include_str!(concat!(env!("OUT_DIR"), "/shader/", $name, ".wgsl")), "\n"),+) };
}

pub fn source(kind: u32) -> &'static str {
    match kind {
        SHADER_CHUNK => wgsl!(
            "frame",
            "globe",
            "grade",
            "atmos_medium",
            "atmos_bind",
            "atmos_lut",
            "atmos_sky",
            "atmos_view",
            "shade",
            "finish",
            "shadow",
            "ao_map",
            "faces",
            "chunk"
        ),
        SHADER_DECOR => wgsl!(
            "frame",
            "globe",
            "grade",
            "atmos_medium",
            "atmos_bind",
            "atmos_lut",
            "atmos_sky",
            "atmos_view",
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
            "grade",
            "atmos_medium",
            "atmos_bind",
            "atmos_lut",
            "atmos_sky",
            "atmos_view",
            "shade",
            "finish",
            "shadow",
            "ao_map",
            "place",
            "skinned"
        ),
        SHADER_PUCK_ZONE => wgsl!("frame", "globe", "grade", "finish", "puck", "puck_zone"),
        SHADER_PUCK_BEACON => wgsl!("frame", "globe", "grade", "finish", "place", "puck", "puck_beacon"),
        SHADER_BADGE => wgsl!("frame", "globe", "grade", "depth", "badge_depth", "badge"),
        SHADER_SKY => wgsl!(
            "frame",
            "globe",
            "grade",
            "finish",
            "dither",
            "atmos_medium",
            "atmos_bind",
            "atmos_lut",
            "atmos_sky",
            "atmos_view",
            "sky"
        ),
        SHADER_DEBUG => wgsl!("frame", "globe", "grade", "debug"),
        SHADER_MIPS => wgsl!("mips"),
        SHADER_SHEET => wgsl!("sheet"),
        SHADER_IMPOSTOR => wgsl!(
            "frame",
            "globe",
            "grade",
            "atmos_medium",
            "atmos_bind",
            "atmos_lut",
            "atmos_sky",
            "atmos_view",
            "shade",
            "finish",
            "shadow",
            "ao_map",
            "impostor"
        ),
        SHADER_IMPOSTOR_BAKE => wgsl!("impostor_bake"),
        SHADER_AO_DOWNSAMPLE => wgsl!("ao_view", "ao_downsample"),
        SHADER_AO_SAMPLE => wgsl!("ao_view", "ao_sample"),
        SHADER_AO_BLUR => wgsl!("ao_view", "ao_blur"),
        SHADER_AO_UPSAMPLE => wgsl!("ao_view", "ao_upsample"),
        SHADER_ATMOS_TRANSMITTANCE => wgsl!("atmos_medium", "atmos_pass", "atmos_transmittance"),
        SHADER_ATMOS_MULTISCATTER => wgsl!("atmos_medium", "atmos_bind", "atmos_lut", "atmos_pass", "atmos_multiscatter"),
        SHADER_ATMOS_SKY_VIEW => wgsl!("atmos_medium", "atmos_bind", "atmos_lut", "atmos_pass", "atmos_skyview"),
        SHADER_ATMOS_EXPOSURE => wgsl!("atmos_medium", "atmos_bind", "atmos_lut", "atmos_sky", "atmos_pass", "atmos_exposure"),
        SHADER_STARS => wgsl!("grade", "stars"),
        SHADER_CLOUD_FIELD => wgsl!("noise", "cloud_tile", "atmos_pass", "cloud_bake"),
        SHADER_CLOUDS => wgsl!(
            "frame",
            "globe",
            "grade",
            "finish",
            "noise",
            "atmos_medium",
            "atmos_bind",
            "atmos_lut",
            "atmos_sky",
            "atmos_view",
            "cloud_tile",
            "cloud_field",
            "clouds"
        ),
        _ => "",
    }
}

pub const FRAME_FLOATS: usize = 84;
/// Frame uniform field offsets in floats, in frame.wgsl's order; a test holds it to the struct.
pub const FRAME_AT: [usize; 17] = [0, 16, 19, 20, 23, 39, 43, 44, 46, 47, 48, 49, 50, 52, 68, 76, 80];
#[cfg(test)]
const FRAME_FIELDS: [&str; 17] = [
    "view_proj",
    "camera_pos",
    "edge_strength",
    "sun_dir",
    "low_spec",
    "edge_width",
    "rim_intensity",
    "viewport",
    "shadow_strength",
    "shadow_texel",
    "vignette",
    "ao_strength",
    "screen_ao",
    "light_view_proj",
    "other_dir",
    "planet",
    "globe",
];

/// Pipelines past draw.rs's, which a frame runs on its own.
pub const PIPELINE_CHUNK_DEPTH: u32 = 10;
pub const PIPELINE_DECOR_DEPTH: u32 = 11;
pub const PIPELINE_SKINNED_DEPTH: u32 = 12;
pub const PIPELINE_SKY: u32 = 13;
pub const PIPELINE_MIPS: u32 = 14;
pub const PIPELINE_CHUNK_PREPASS: u32 = 15;
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
pub const PIPELINE_CHUNK_COARSE_DEPTH: u32 = 30;
pub const PIPELINE_CHUNK_COARSE_PREPASS: u32 = 31;
pub const PIPELINE_SHEET: u32 = 33;
pub const PIPELINE_IMPOSTOR_BAKE: u32 = 35;
pub const PIPELINES: u32 = 36;

// Record words: shader, has fragment, topology, cull, depth write, depth
// compare, bias (i32), slope scale (f32 bits), blend, vertex entry; then vertex
// buffers (count; stride, step, attribute count, (location, offset, format)
// each); then bind groups (count; entry count, (binding, visibility, kind) each).
pub use crate::gpu::api_spec::record::*;

use crate::frame::draw::{
    PIPELINE_BADGE, PIPELINE_CHUNK, PIPELINE_CHUNK_COARSE, PIPELINE_DEBUG, PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_BOX_COPLANAR, PIPELINE_GHOST_DECOR, PIPELINE_IMPOSTOR,
    PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE, PIPELINE_SKINNED,
};
use crate::mesh::chunk_mesher::{COARSE_BOX_WORDS, FINE_BOX_WORDS};

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
pub const VERTEX_ENTRY: [&str; 4] = ["vs", "vs_coarse", "vs_depth", "vs_coarse_depth"];

const fn at_coarse(h: Head) -> Head {
    let vertex = if h.vertex == VS_DEPTH { VS_COARSE_DEPTH } else { VS_COARSE };
    Head { vertex, ..h }
}

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

/// Into the shadow map: plain less-than depth, no fragment stage, both sides.
const fn depth_only(shader: u32) -> Head {
    Head {
        shader,
        fragment: false,
        topology: TRIANGLES,
        cull: CULL_NONE,
        depth_write: true,
        depth_compare: LESS,
        depth_bias: 0,
        depth_bias_slope: 0,
        blend: OPAQUE,
        vertex: VS_DEPTH,
    }
}

/// Depth from the eye for the screen-space occlusion, culled as the lit pass is.
const fn prepass(shader: u32, cull: u32) -> Head {
    Head {
        cull,
        depth_compare: GREATER,
        ..depth_only(shader)
    }
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

const AIR: [[u32; 3]; 6] = [
    [8, FRAGMENT, UNIFORM],
    [9, FRAGMENT, TEXTURE],
    [10, FRAGMENT, TEXTURE],
    [11, FRAGMENT, SAMPLER],
    [13, FRAGMENT, TEXTURE],
    [15, FRAGMENT, TEXTURE],
];
const AIR_AT_VERTICES: [[u32; 3]; 6] = [
    [8, VERTEX, UNIFORM],
    [9, VERTEX, TEXTURE],
    [10, VERTEX, TEXTURE],
    [11, VERTEX, SAMPLER],
    [13, VERTEX, TEXTURE],
    [15, VERTEX, TEXTURE],
];
const LIT_FRAME: [[u32; 3]; 10] = [
    [0, BOTH, UNIFORM],
    [1, FRAGMENT, DEPTH],
    [2, FRAGMENT, SAMPLER_COMPARISON],
    [7, FRAGMENT, TEXTURE],
    // Tables in the vertex stage too: a fragment reads the land's color for the ground bounce.
    [8, BOTH, UNIFORM],
    AIR_AT_VERTICES[1],
    AIR_AT_VERTICES[2],
    AIR_AT_VERTICES[3],
    AIR_AT_VERTICES[4],
    AIR_AT_VERTICES[5],
];
/// The transmittance and sky-view tables are laid out unread because the shader declares them.
const CLOUD_GROUP: [[u32; 3]; 10] = [
    [0, FRAGMENT, UNIFORM],
    [1, FRAGMENT, UNIFORM],
    [2, FRAGMENT, TEXTURE],
    [3, FRAGMENT, SAMPLER],
    AIR[0],
    [9, NONE, TEXTURE],
    [10, NONE, TEXTURE],
    AIR[3],
    AIR[4],
    AIR[5],
];
const SKIN: [[u32; 3]; 2] = [[0, FRAGMENT, TEXTURE], [1, FRAGMENT, SAMPLER]];

/// The mesh is a per-instance vertex buffer, so a draw reaches its boxes by
/// its first instance. The two formats differ only in stride, so a coarse draw
/// binds the same buffer again for the two words it lacks.
fn chunk_lanes(fine: bool) -> Vec<Vec<u32>> {
    let stride = if fine { FINE_BOX_WORDS * 4 } else { COARSE_BOX_WORDS * 4 };
    let rest = if fine { 16 } else { 0 };
    vec![vec![stride, PER_INSTANCE, 1, 0, 0, U32X4], vec![stride, PER_INSTANCE, 1, 1, rest, U32X2]]
}
fn chunk_groups() -> Vec<Vec<u32>> {
    let mut g0 = LIT_FRAME.to_vec();
    g0.extend([[3, VERTEX, STORAGE], [4, FRAGMENT, TEXTURE_ARRAY], [5, FRAGMENT, STORAGE], [6, FRAGMENT, SAMPLER]]);
    vec![group(&g0)]
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
        PIPELINE_CHUNK => record(lit(SHADER_CHUNK, CULL_BACK), &chunk_lanes(true), &chunk_groups()),
        PIPELINE_CHUNK_COARSE => record(at_coarse(lit(SHADER_CHUNK, CULL_BACK)), &chunk_lanes(false), &chunk_groups()),
        PIPELINE_DECOR => record(lit(SHADER_DECOR, CULL_NONE), &model_lanes(false), &model_groups()),
        // Slightly clear of a surface it sits on, strongly clear of one it sits in.
        PIPELINE_GHOST_BOX => record(blended(SHADER_CHUNK, CULL_BACK, 1, 1.0), &chunk_lanes(true), &chunk_groups()),
        PIPELINE_GHOST_BOX_COPLANAR => record(blended(SHADER_CHUNK, CULL_BACK, 8, 2.0), &chunk_lanes(true), &chunk_groups()),
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
            &[frame(BOTH), group(&[[0, BOTH, UNIFORM]])],
        ),
        PIPELINE_BADGE => record(
            Head {
                topology: STRIP,
                depth_compare: GREATER_EQUAL,
                ..blended(SHADER_BADGE, CULL_NONE, 0, 0.0)
            },
            &[],
            &[frame(BOTH), group(&[[0, VERTEX, UNIFORM], [1, VERTEX, STORAGE], [2, VERTEX, DEPTH]]), group(&SKIN)],
        ),
        PIPELINE_DEBUG => record(
            Head {
                topology: LINES,
                depth_compare: ALWAYS,
                blend: OPAQUE,
                ..blended(SHADER_DEBUG, CULL_NONE, 0, 0.0)
            },
            &[vec![24, PER_VERTEX, 2, 0, 0, F32X3, 1, 12, F32X3]],
            &[frame(VERTEX), group(&[[0, VERTEX, UNIFORM]])],
        ),
        PIPELINE_CHUNK_DEPTH => record(depth_only(SHADER_CHUNK), &chunk_lanes(true), &chunk_groups()),
        PIPELINE_CHUNK_COARSE_DEPTH => record(at_coarse(depth_only(SHADER_CHUNK)), &chunk_lanes(false), &chunk_groups()),
        PIPELINE_DECOR_DEPTH => record(depth_only(SHADER_DECOR), &model_lanes(false), &model_groups()),
        PIPELINE_SKINNED_DEPTH => record(depth_only(SHADER_SKINNED), &model_lanes(true), &model_groups()),
        PIPELINE_CHUNK_PREPASS => record(prepass(SHADER_CHUNK, CULL_BACK), &chunk_lanes(true), &chunk_groups()),
        PIPELINE_CHUNK_COARSE_PREPASS => record(at_coarse(prepass(SHADER_CHUNK, CULL_BACK)), &chunk_lanes(false), &chunk_groups()),
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
                [1, FRAGMENT, UNIFORM],
                [2, FRAGMENT, TEXTURE_ARRAY],
                [3, FRAGMENT, SAMPLER],
                AIR[0],
                AIR[1],
                AIR[2],
                AIR[3],
                AIR[4],
                AIR[5],
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
        PIPELINE_AO_DOWNSAMPLE => record(screen(SHADER_AO_DOWNSAMPLE), &[], &[group(&[[0, FRAGMENT, UNIFORM], [1, FRAGMENT, DEPTH]])]),
        PIPELINE_AO_SAMPLE => record(
            screen(SHADER_AO_SAMPLE),
            &[],
            &[group(&[[0, FRAGMENT, UNIFORM], [1, FRAGMENT, TEXTURE_UNFILTERED], [2, FRAGMENT, TEXTURE]])],
        ),
        PIPELINE_AO_BLUR => record(
            screen(SHADER_AO_BLUR),
            &[],
            &[group(&[
                [0, FRAGMENT, UNIFORM],
                [1, FRAGMENT, TEXTURE_UNFILTERED],
                [2, FRAGMENT, TEXTURE],
                [3, FRAGMENT, TEXTURE],
                [4, FRAGMENT, SAMPLER],
                [5, FRAGMENT, UNIFORM_DYNAMIC],
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
            ])],
        ),
        PIPELINE_ATMOS_TRANSMITTANCE => record(screen(SHADER_ATMOS_TRANSMITTANCE), &[], &[group(&[AIR[0]])]),
        PIPELINE_ATMOS_MULTISCATTER => record(screen(SHADER_ATMOS_MULTISCATTER), &[], &[group(&[AIR[0], AIR[1], AIR[3]])]),
        PIPELINE_ATMOS_SKY_VIEW => record(
            screen(SHADER_ATMOS_SKY_VIEW),
            &[],
            &[group(&[AIR[0], AIR[1], AIR[3], [12, FRAGMENT, TEXTURE], [14, FRAGMENT, UNIFORM]])],
        ),
        PIPELINE_ATMOS_EXPOSURE => record(screen(SHADER_ATMOS_EXPOSURE), &[], &[group(&[AIR[0], AIR[1], AIR[2], AIR[3]])]),
        PIPELINE_STARS => record(
            Head {
                topology: STRIP,
                depth_compare: GREATER_EQUAL,
                blend: ADD,
                ..blended(SHADER_STARS, CULL_NONE, 0, 0.0)
            },
            &[],
            &[group(&[[0, VERTEX, UNIFORM], [1, VERTEX, STORAGE]])],
        ),
        // From under the layer only where the world left the far plane; from over it everywhere.
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
pub type Buffer = (u32, u32, Vec<(u32, u32, u32)>);

#[cfg(test)]
pub struct Record {
    pub shader: u32,
    pub fragment: bool,
    pub topology: u32,
    pub cull: u32,
    pub depth_write: bool,
    pub depth_compare: u32,
    pub depth_bias: i32,
    pub depth_bias_slope: f32,
    pub blend: u32,
    pub vertex: u32,
    pub buffers: Vec<Buffer>,
    pub groups: Vec<Vec<(u32, u32, u32)>>,
}

#[cfg(test)]
impl Record {
    pub fn read(words: &[u32]) -> Record {
        let mut at = HEAD as usize;
        let mut next = || {
            at += 1;
            words[at - 1]
        };
        let n = next();
        let buffers = (0..n)
            .map(|_| {
                let stride = next();
                let step = next();
                let m = next();
                (stride, step, (0..m).map(|_| (next(), next(), next())).collect())
            })
            .collect();
        let g = next();
        let groups = (0..g)
            .map(|_| {
                let e = next();
                (0..e).map(|_| (next(), next(), next())).collect()
            })
            .collect();
        xpute_core::ensure!(at == words.len(), ENOTRECOVERABLE);
        Record {
            shader: words[0],
            fragment: words[1] != 0,
            topology: words[2],
            cull: words[3],
            depth_write: words[4] != 0,
            depth_compare: words[5],
            depth_bias: words[6] as i32,
            depth_bias_slope: f32::from_bits(words[7]),
            blend: words[8],
            vertex: words[9],
            buffers,
            groups,
        }
    }
}

#[cfg(test)]
#[path = "record.test.rs"]
mod test;
