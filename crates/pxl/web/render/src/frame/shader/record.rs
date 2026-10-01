// pxl-render/frame/shader/record.rs

//! The shaders and the pipelines that run them, as their one source: the
//! WGSL in `shader/`, assembled here from its shared pieces, and each
//! pipeline's definition — its vertex lanes, its bind group layouts, its
//! primitive, depth and blend state — as a record of words the shell turns
//! into WebGPU's descriptor and a native host into wgpu's. What the shell
//! decides is nothing: which shader a pipeline runs and how its lanes are
//! read are the same facts the mesher and the draw list (draw.rs) are built
//! on, and they are written once, here.
//!
//! The WGSL is text the GPU process compiles either way; what this buys is
//! that a lane's layout, a binding's slot and a constant have one home, and
//! that `cargo test` compiles every shader with naga and holds each record
//! against what the shader declares — the vertex inputs by location, the
//! bindings by group and slot, the frame uniform's offsets — so a mismatch
//! fails here rather than in a browser console.

/// The shaders, by kind.
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

/// A shader's pieces, as build.rs wrote them: the files beside this one with
/// their comments and indentation off, since what ships is what the GPU
/// compiles and the GPU does not read the prose; constant.wgsl, which every
/// shader shares, first.
macro_rules! wgsl {
    ($($name:literal),+) => { concat!(include_str!(concat!(env!("OUT_DIR"), "/shader/constant.wgsl")), "\n", $(include_str!(concat!(env!("OUT_DIR"), "/shader/", $name, ".wgsl")), "\n"),+) };
}

/// A shader's WGSL; empty for a kind that is not one.
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
        SHADER_BADGE => wgsl!("globe", "grade", "depth", "badge_depth", "badge"),
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

/// Floats the frame uniform (frame.wgsl's Frame) holds.
pub const FRAME_FLOATS: usize = 84;
/// Where each field of the frame uniform starts, in floats, in this order:
/// view_proj, camera_pos, edge_strength, the environment's run (sun_dir
/// through its padding, look/environment.rs's output as it is), low_spec,
/// edge_width, rim_intensity, viewport, shadow_strength, shadow_texel,
/// vignette, ao_strength, screen_ao, light_view_proj, the second light's run
/// (other_dir through other_color, look/environment.rs's ENV_OTHER_LIGHT),
/// the planet and the globe; and whether the vertices take the air. The
/// renderer reads the table rather than counting; the test below holds it to
/// the struct.
pub const FRAME_AT: [usize; 18] = [0, 16, 19, 20, 23, 39, 43, 44, 46, 47, 48, 49, 50, 52, 68, 76, 80, 51];
#[cfg(test)]
const FRAME_FIELDS: [&str; 18] = [
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
    "air_at_vertices",
];

/// The pipelines, by id: draw.rs's ten, which a draw names, then the ones
/// a frame runs on its own — the depth-only passes into the shadow map,
/// the sky, the mip builder, the depth-only passes from the eye that the
/// screen-space occlusion reads, the occlusion's four (gpu/ao.rs), and the
/// atmosphere's three tables and its exposure (gpu/atmosphere.rs), the stars
/// (gpu/stars.rs) and the clouds (gpu/clouds.rs).
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
/// The coarse box format's own pipelines: the same shader and groups, over a
/// vertex layout of four words rather than six (`chunk_lanes`).
pub const PIPELINE_CHUNK_COARSE_DEPTH: u32 = 30;
pub const PIPELINE_CHUNK_COARSE_PREPASS: u32 = 31;
/// An entry of the billboard sheet (gpu/sheet.rs).
pub const PIPELINE_SHEET: u32 = 33;
/// A decor variant's impostor sprite, drawn once into its layer
/// (gpu/pass/impostor.rs).
pub const PIPELINE_IMPOSTOR_BAKE: u32 = 35;
pub const PIPELINES: u32 = 36;

// A pipeline record's words, in order: the shader; whether it has a fragment
// stage; the topology; the cull mode; whether it writes depth; the depth
// compare; the depth bias (i32) and its slope scale (f32 bits); the blend;
// the vertex entry point; then the vertex buffers — a
// count, and for each its stride, its step mode, and its attributes as a
// count and (location, offset, format) each; then the bind group layouts —
// a count, and for each its entries as a count and (binding, visibility,
// kind) each. The codes are the host's too (gpu/api.json).
pub use crate::gpu::api_spec::record::*;

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
    /// Which vertex entry point of `shader` the pipeline enters. A format the
    /// pipeline already fixes is the pipeline's, not a number carried on every
    /// draw for the shader to branch on.
    vertex: u32,
}

/// The vertex entry points a record can name, indexed by VS and kin. The
/// shell holds the same list against the same numbers (apps/web/src/platform/web/webgpu.ts).
pub const VERTEX_ENTRY: [&str; 2] = ["vs", "vs_coarse"];

/// The same pipeline entered at the coarse boxes' vertex entry point.
const fn at_coarse(h: Head) -> Head {
    Head { vertex: VS_COARSE, ..h }
}

/// An opaque lit pipeline: writes depth, nearer wins.
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

/// A blended pipeline over what is drawn: no depth written, biased toward
/// the eye — positive under reversed depth.
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

/// A depth-only pass into the shadow map: the pass's own vertex shader
/// under the light's frame, no fragment stage, both sides, a plain
/// less-than depth.
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
        vertex: VS,
    }
}

/// A depth-only pass from the eye: the depth the screen-space occlusion reads, under the frame's reversed
/// depth and the lit pass's culling, so it holds the surfaces the lit pass
/// draws.
const fn prepass(shader: u32, cull: u32) -> Head {
    Head {
        cull,
        depth_compare: GREATER,
        ..depth_only(shader)
    }
}

/// One of the screen-space occlusion's full-screen passes: no depth, into
/// the target's own format.
const fn screen(shader: u32) -> Head {
    Head {
        depth_compare: NO_DEPTH,
        blend: OPAQUE,
        ..blended(shader, CULL_NONE, 0, 0.0)
    }
}

/// One vertex buffer feeding one location from its start.
fn lane(stride: u32, location: u32, format: u32) -> Vec<u32> {
    vec![stride, PER_VERTEX, 1, location, 0, format]
}

/// A bind group layout's entries, (binding, visibility, kind) each.
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

/// The atmosphere's uniform, its transmittance and sky-view tables, their
/// sampler, the adapted exposure and the moonlit sky's table (atmos_bind.wgsl,
/// atmos_sky.wgsl, atmos_view.wgsl), from 8: what a pass that shows the air
/// reads it through, in its fragments.
const AIR: [[u32; 3]; 6] = [
    [8, FRAGMENT, UNIFORM],
    [9, FRAGMENT, TEXTURE],
    [10, FRAGMENT, TEXTURE],
    [11, FRAGMENT, SAMPLER],
    [13, FRAGMENT, TEXTURE],
    [15, FRAGMENT, TEXTURE],
];
/// The same, read in the vertex stage: a lit surface takes the air in front
/// of it at its vertices (atmos_view.wgsl's seen_from_eye).
const AIR_AT_VERTICES: [[u32; 3]; 6] = [
    [8, VERTEX, UNIFORM],
    [9, VERTEX, TEXTURE],
    [10, VERTEX, TEXTURE],
    [11, VERTEX, SAMPLER],
    [13, VERTEX, TEXTURE],
    [15, VERTEX, TEXTURE],
];
/// The frame uniform, the shadow map and its sampler, and the screen-space
/// occlusion map at 7, past the chunk's own, then the air: group 0 in every
/// lit pass.
const LIT_FRAME: [[u32; 3]; 10] = [
    [0, BOTH, UNIFORM],
    [1, FRAGMENT, DEPTH],
    [2, FRAGMENT, SAMPLER_COMPARISON],
    [7, FRAGMENT, TEXTURE],
    // The air's uniform in both stages, its tables in the vertex alone: the
    // vertices take the air in front of them, and a fragment reads the land's
    // color for the light a face takes from the ground under it
    // (atmos_view.wgsl's ground_bounce).
    [8, BOTH, UNIFORM],
    AIR_AT_VERTICES[1],
    AIR_AT_VERTICES[2],
    AIR_AT_VERTICES[3],
    AIR_AT_VERTICES[4],
    AIR_AT_VERTICES[5],
];
/// The clouds' group: the frame, their own uniform, the tile the shape is
/// baked into and its sampler, and the air. The air in front of a cloud is
/// lit where it stands (atmos_view.wgsl's `veil`) and reads neither the
/// transmittance nor the sky-view table, which the layout still lays, since
/// the shader declares them with the rest.
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
/// A skin and its sampler.
const SKIN: [[u32; 3]; 2] = [[0, FRAGMENT, TEXTURE], [1, FRAGMENT, SAMPLER]];

/// A chunk's boxes, one record an instance, out of the arena or a block
/// (gpu/arena.rs): the mesh is a vertex buffer stepped per instance rather
/// than a storage buffer the shader indexes, so a draw reaches its own boxes
/// by the offset it binds at and thirty-six vertices share one record.
///
/// The two formats differ only in stride — six words for a leaf's own tier,
/// four for every other (mesh/chunk_mesher.rs) — and a pipeline's stride is
/// fixed, so there is a pipeline for each. What is not duplicated is the
/// shader: a coarse draw binds the same buffer a second time for the two
/// words it does not have, which cost nothing to read and are never used,
/// and one `BoxWords` then satisfies both layouts.
fn chunk_lanes(fine: bool) -> Vec<Vec<u32>> {
    let stride = if fine { FINE_BOX_WORDS * 4 } else { COARSE_BOX_WORDS * 4 };
    let rest = if fine { 16 } else { 0 };
    vec![vec![stride, PER_INSTANCE, 1, 0, 0, U32X4], vec![stride, PER_INSTANCE, 1, 1, rest, U32X2]]
}
/// The chunk's groups: the lit frame, the draws, the texture pack's array,
/// the table its faces are found through and their sampler. The mesh is no
/// group of its own any more — it arrives as vertex attributes.
fn chunk_groups() -> Vec<Vec<u32>> {
    let mut g0 = LIT_FRAME.to_vec();
    g0.extend([[3, BOTH, STORAGE], [4, FRAGMENT, TEXTURE_ARRAY], [5, FRAGMENT, STORAGE], [6, FRAGMENT, SAMPLER]]);
    vec![group(&g0)]
}
/// A model's lanes (model.rs): position, normal, uv; then, skinned, the
/// joints and the weights.
fn model_lanes(skinned: bool) -> Vec<Vec<u32>> {
    let mut lanes = vec![lane(12, 0, F32X3), lane(12, 1, F32X3), lane(8, 2, F32X2)];
    if skinned {
        lanes.push(lane(16, 3, U32X4));
        lanes.push(lane(16, 4, F32X4));
    }
    lanes
}
/// Decor's and the actors' groups: the lit frame; the draw at a dynamic
/// offset; the skin; the block the vertex shader reads (the instances, or
/// the joint palette).
fn model_groups() -> Vec<Vec<u32>> {
    vec![group(&LIT_FRAME), group(&[[0, BOTH, UNIFORM_DYNAMIC]]), group(&SKIN), group(&[[0, VERTEX, STORAGE]])]
}

/// A pipeline's record; empty for an id that is not one.
pub fn pipeline(id: u32) -> Vec<u32> {
    let frame = |visibility: u32| group(&[[0, visibility, UNIFORM]]);
    match id {
        crate::frame::draw::PIPELINE_CHUNK => record(lit(SHADER_CHUNK, CULL_BACK), &chunk_lanes(true), &chunk_groups()),
        crate::frame::draw::PIPELINE_CHUNK_COARSE => record(at_coarse(lit(SHADER_CHUNK, CULL_BACK)), &chunk_lanes(false), &chunk_groups()),
        crate::frame::draw::PIPELINE_DECOR => record(lit(SHADER_DECOR, CULL_NONE), &model_lanes(false), &model_groups()),
        // The hover ghost, biased toward the eye: slightly clear of a
        // surface it sits on, strongly clear of one it sits in.
        crate::frame::draw::PIPELINE_GHOST_BOX => record(blended(SHADER_CHUNK, CULL_BACK, 1, 1.0), &chunk_lanes(true), &chunk_groups()),
        crate::frame::draw::PIPELINE_GHOST_BOX_COPLANAR => record(blended(SHADER_CHUNK, CULL_BACK, 8, 2.0), &chunk_lanes(true), &chunk_groups()),
        crate::frame::draw::PIPELINE_GHOST_DECOR => record(blended(SHADER_DECOR, CULL_NONE, 1, 1.0), &model_lanes(false), &model_groups()),
        // Back faces culled, as three's material (FrontSide) did: drawn both
        // sides, a closed body's inner faces fight its outer ones for the
        // same depth at a distance, and the character shimmers.
        crate::frame::draw::PIPELINE_SKINNED => record(lit(SHADER_SKINNED, CULL_BACK), &model_lanes(true), &model_groups()),
        crate::frame::draw::PIPELINE_PUCK_ZONE => record(
            blended(SHADER_PUCK_ZONE, CULL_NONE, 4, 1.0),
            &[lane(12, 0, F32X3), lane(8, 1, F32X2)],
            &[frame(BOTH), group(&[[0, BOTH, UNIFORM]]), group(&[[0, VERTEX, STORAGE]])],
        ),
        crate::frame::draw::PIPELINE_PUCK_BEACON => record(
            lit(SHADER_PUCK_BEACON, CULL_BACK),
            &[lane(12, 0, F32X3), lane(12, 1, F32X3)],
            &[frame(BOTH), group(&[[0, BOTH, UNIFORM]])],
        ),
        // The badges: a strip of four corners an instance, blended over the
        // world, tested against its depth and no nearer than it.
        crate::frame::draw::PIPELINE_BADGE => record(
            Head {
                topology: STRIP,
                depth_compare: GREATER_EQUAL,
                ..blended(SHADER_BADGE, CULL_NONE, 0, 0.0)
            },
            &[],
            &[group(&[[0, BOTH, UNIFORM], [1, VERTEX, STORAGE], [2, VERTEX, DEPTH]]), group(&SKIN)],
        ),
        // The gizmos: a line list over everything, no depth test.
        crate::frame::draw::PIPELINE_DEBUG => record(
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
        // The sky: one triangle over the screen at the far plane, after the
        // opaque world and only where it left the far plane standing, so a
        // pixel the ground covers never runs it; writing no depth; the frame
        // beside its own uniform, for the vignette, the Moon's face and the
        // air.
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
        // A mip level from the one above: no depth, into the texture's own
        // format.
        PIPELINE_MIPS => record(screen(SHADER_MIPS), &[], &[group(&SKIN)]),
        // Decor's impostors: a strip of four corners an instance, opaque
        // where the sprite covers and discarded where it does not, both sides.
        crate::frame::draw::PIPELINE_IMPOSTOR => record(
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
        // A variant drawn into its sprite: its own color target and depth,
        // nearer wins under a plain depth.
        PIPELINE_IMPOSTOR_BAKE => record(
            Head {
                fragment: true,
                ..depth_only(SHADER_IMPOSTOR_BAKE)
            },
            &model_lanes(false),
            &[group(&[[0, VERTEX, UNIFORM_DYNAMIC]]), group(&SKIN)],
        ),
        // An entry of the billboard sheet: its shapes and lines of text laid
        // over each other in the entry's own texture, no depth.
        PIPELINE_SHEET => record(
            Head {
                topology: STRIP,
                blend: ALPHA,
                ..screen(SHADER_SHEET)
            },
            &[],
            &[group(&[[0, VERTEX, UNIFORM], [1, BOTH, STORAGE]]), group(&[[0, FRAGMENT, TEXTURE_ARRAY], [1, FRAGMENT, SAMPLER]])],
        ),
        // The screen-space occlusion (gpu/ao.rs): the frame's depth to half
        // resolution with a normal; the occlusion from that; the denoise,
        // reading the blue noise, the pass before it and which run it is;
        // the occlusion back at full resolution against the frame's depth.
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
        // The atmosphere's tables (gpu/atmosphere.rs), each a full-screen
        // pass into its own: the transmittance from the uniform alone, the
        // multiple scattering from that, the sky from both, and the exposure
        // from the sky.
        PIPELINE_ATMOS_TRANSMITTANCE => record(screen(SHADER_ATMOS_TRANSMITTANCE), &[], &[group(&[AIR[0]])]),
        PIPELINE_ATMOS_MULTISCATTER => record(screen(SHADER_ATMOS_MULTISCATTER), &[], &[group(&[AIR[0], AIR[1], AIR[3]])]),
        PIPELINE_ATMOS_SKY_VIEW => record(
            screen(SHADER_ATMOS_SKY_VIEW),
            &[],
            &[group(&[AIR[0], AIR[1], AIR[3], [12, FRAGMENT, TEXTURE], [14, FRAGMENT, UNIFORM]])],
        ),
        PIPELINE_ATMOS_EXPOSURE => record(screen(SHADER_ATMOS_EXPOSURE), &[], &[group(&[AIR[0], AIR[1], AIR[2], AIR[3]])]),
        // The stars: a strip of four corners a star, added over the sky and
        // under everything drawn after it, no depth read or written.
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
        // The clouds: one triangle over the screen at the far plane after
        // the opaque world, blended over it and writing no depth. From under
        // the layer, only where the world left the far plane; from over it,
        // everywhere. The frame beside their own uniform, and the air.
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
        // The cloud tile (cloud_bake.wgsl): one triangle over a texture of
        // its own, once for a device, reading nothing.
        PIPELINE_CLOUD_FIELD => record(screen(SHADER_CLOUD_FIELD), &[], &[]),
        _ => Vec::new(),
    }
}

/// A vertex buffer as a record has it: its stride, its step mode, and its
/// attributes as (location, offset, format).
#[cfg(test)]
pub type Buffer = (u32, u32, Vec<(u32, u32, u32)>);

/// A record read back into its parts, as the shell reads it: what the
/// tests check each pipeline's words against its shader by.
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
    /// Per group, its entries as (binding, visibility, kind).
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
