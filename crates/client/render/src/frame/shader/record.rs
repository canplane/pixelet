// pxl-render/frame/shader/record.rs

//! Shaders and their pipeline records (lanes, bind groups, primitive, depth and
//! blend state) as one source the host turns into WebGPU or wgpu descriptors.
//! build.rs assembles and validates every shader (parts.rs), and reads off it
//! the stages each pipeline's groups are seen from.

pub use crate::frame::shader::parts::*;

/// A compute shader's one entry point, as its source names it.
pub fn compute_entry(kind: u32) -> &'static str {
    match kind {
        SHADER_CAST => "cast_cs",
        SHADER_START => "start_cs",
        SHADER_FACE => "face_cs",
        SHADER_BOUND => "bound_cs",
        SHADER_HEIGHTS => "heights_cs",
        SHADER_GROWN_MARK => "decor_mark_cs",
        SHADER_GROWN => "decor_cs",
        SHADER_GROWN_COUNT => "decor_count_cs",
        SHADER_GROWN_ARGS => "decor_args_cs",
        SHADER_GROWN_PLACE => "decor_place_cs",
        SHADER_WAYS => "ways_cs",
        _ => xpute_kit::bug!(EINVAL, kind),
    }
}

include!(concat!(env!("OUT_DIR"), "/sources.rs"));

/// Frame uniform field offsets in floats, in frame.wgsl's order (its size
/// frame/shader/words.rs's `FRAME_FLOATS`).
pub const FRAME_AT: [usize; 16] = [0, 16, 19, 20, 23, 24, 27, 28, 31, 32, 35, 36, 38, 39, 40, 44];

/// Pipelines past draw.rs's, which a frame runs on its own.
pub const PIPELINE_SKY: u32 = 13;
pub const PIPELINE_MIPS: u32 = 14;
pub const PIPELINE_BLIT: u32 = 15;
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
/// The eye's rays, opaque at their depth (pxl-portable ray.rs).
pub const PIPELINE_SCREEN_RAYS: u32 = 41;
pub const PIPELINES: u32 = 42;

// Record words: shader, has fragment, topology, cull, depth write, depth
// compare, bias (i32), slope scale (f32 bits), blend, vertex entry; then vertex
// buffers (count; stride, step, attribute count, (location, offset, format)
// each); then bind groups (count; entry count, (binding, visibility, kind) each).
pub use crate::gpu::api_spec::record::*;
/// A binding's stages are xpute's own bits (SHADER_STAGE), none, one or both.
pub use xpute_gpu::spec::SHADER_STAGE::{COMPUTE, FRAGMENT, VERTEX};
pub const NONE: u32 = 0;
pub const BOTH: u32 = VERTEX | FRAGMENT;

/// Each generated compute entry's group 0 and its workgroup as its WGSL
/// declares them (build.rs `compute`): `CAST_GROUP`, `CAST_WORKGROUP` and
/// so on, by the stem of the entry's file.
pub mod compute {
    use super::{COMPUTE, STORAGE, STORAGE_RW, STORAGE_TEXTURE_RW, UNIFORM};
    include!(concat!(env!("OUT_DIR"), "/compute.rs"));
}

/// `group` with its uniform at `binding` taken at a dynamic offset: how a
/// host binds it, which the shader does not say.
pub const fn dynamic<const N: usize>(mut group: [u32; N], binding: u32) -> [u32; N] {
    let mut k = 0;
    while k < N {
        if group[k] == binding && group[k + 2] == UNIFORM {
            group[k + 2] = UNIFORM_DYNAMIC;
        }
        k += 3;
    }
    group
}

/// Whether every entry of `a` is one of `b`'s: a pipeline over `a` takes a
/// group laid out as `b`.
pub const fn within<const N: usize, const M: usize>(a: [u32; N], b: [u32; M]) -> bool {
    let mut k = 0;
    while k < N {
        let mut found = false;
        let mut j = 0;
        while j < M {
            found |= a[k] == b[j] && a[k + 1] == b[j + 1] && a[k + 2] == b[j + 2];
            j += 3;
        }
        if !found {
            return false;
        }
        k += 3;
    }
    true
}

/// Whether two groups are one layout, entry for entry.
pub const fn same<const N: usize, const M: usize>(a: [u32; N], b: [u32; M]) -> bool {
    if N != M {
        return false;
    }
    let mut k = 0;
    while k < N {
        if a[k] != b[k] {
            return false;
        }
        k += 1;
    }
    true
}

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
    group(&[[0, UNIFORM], [1, STORAGE], [2, read]])
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

fn group(entries: &[[u32; 2]]) -> Vec<u32> {
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

/// The air's bindings, as every pass that binds the air lays them out.
const AIR: [[u32; 2]; 10] = AIR_SLOTS;
const LIT_FRAME: [[u32; 2]; 12] = {
    // The air in the vertex stage, where each point's lights and veil are
    // worked out; a fragment reads its words and the eye's texel, whose white
    // it shows by (atmos_eye.wgsl). The sky's views and the Earth's picture
    // are the sky pass's, laid out unread.
    [[0, UNIFORM], [7, TEXTURE], AIR[0], AIR[1], AIR[2], AIR[3], AIR[4], AIR[5], AIR[6], AIR[7], AIR[8], AIR[9]]
};
/// The sky-view and moonlit-sky tables are laid out unread because the shader declares them.
const CLOUD_GROUP: [[u32; 2]; 14] = {
    [
        [0, UNIFORM],
        [1, UNIFORM],
        [2, TEXTURE],
        [3, SAMPLER],
        AIR[0],
        AIR[1],
        AIR[2],
        AIR[3],
        AIR[4],
        AIR[5],
        AIR[6],
        AIR[7],
        AIR[8],
        AIR[9],
    ]
};
const SKIN: [[u32; 2]; 2] = [[0, TEXTURE], [1, SAMPLER]];

/// The boxes are a per-instance vertex buffer, so a draw reaches its boxes by
/// its first instance.
fn box_lanes() -> Vec<Vec<u32>> {
    let stride = BOX_WORDS * 4;
    vec![vec![stride, PER_INSTANCE, 1, 0, 0, U32X4], vec![stride, PER_INSTANCE, 1, 1, 16, U32X2]]
}
fn box_groups() -> Vec<Vec<u32>> {
    let mut g0 = LIT_FRAME.to_vec();
    g0.extend([[3, STORAGE], [4, TEXTURE_ARRAY], [5, STORAGE], [6, SAMPLER]]);
    vec![group(&g0)]
}
/// The lit pass's frame, occlusion, air and layers, all read where
/// a pixel is painted; then the rays, the sky's words, the looks and the
/// eye's Q3 steps: the most entries any group has (`GROUP_ENTRIES`).
const SCREEN_GROUP: [[u32; 2]; 19] = {
    [
        [0, UNIFORM],
        [7, TEXTURE],
        AIR[0],
        AIR[1],
        AIR[2],
        AIR[3],
        AIR[4],
        AIR[5],
        AIR[6],
        AIR[7],
        AIR[8],
        AIR[9],
        [3, TEXTURE_UINT_ARRAY],
        [4, TEXTURE_ARRAY],
        [5, STORAGE],
        [6, SAMPLER],
        [19, UNIFORM],
        [20, STORAGE],
        [21, UNIFORM],
    ]
};

/// The most entries a group has, a layout's and a bind group's alike: the
/// screen's (and renderer.rs's lit entries, which it extends); a group past
/// it is refused where it is made.
pub const GROUP_ENTRIES: usize = SCREEN_GROUP.len();
fn model_lanes(skinned: bool) -> Vec<Vec<u32>> {
    let mut lanes = vec![lane(12, 0, F32X3), lane(12, 1, F32X3), lane(8, 2, F32X2)];
    if skinned {
        lanes.push(lane(16, 3, U32X4));
        lanes.push(lane(16, 4, F32X4));
    }
    lanes
}
fn model_groups() -> Vec<Vec<u32>> {
    vec![group(&LIT_FRAME), group(&[[0, UNIFORM_DYNAMIC]]), group(&SKIN), group(&[[0, STORAGE]])]
}

/// A pipeline's record before its groups' stages: each group's entries
/// (binding, kind), which `pipeline` sees from the stages that read them.
fn plan(id: u32) -> Vec<u32> {
    let frame = || group(&[[0, UNIFORM]]);
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
            &[frame(), group(&[[0, UNIFORM]]), group(&[[0, STORAGE]])],
        ),
        PIPELINE_PUCK_BEACON => record(
            lit(SHADER_PUCK_BEACON, CULL_BACK),
            &[lane(12, 0, F32X3), lane(12, 1, F32X3)],
            &[group(&[[0, UNIFORM], AIR[0], AIR[1], AIR[3]]), group(&[[0, UNIFORM]])],
        ),
        PIPELINE_BADGE => record(
            Head {
                topology: STRIP,
                depth_compare: GREATER_EQUAL,
                ..blended(SHADER_BADGE, CULL_NONE, 0, 0.0)
            },
            &[],
            &[frame(), badge_group(TEXTURE_UNFILTERED), group(&SKIN)],
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
            &[frame(), badge_group(DEPTH)],
        ),
        // Against the gizmos' own faces, not the world's depth.
        PIPELINE_DEBUG => record(
            Head {
                blend: ALPHA,
                ..blended(SHADER_DEBUG, CULL_NONE, 0, 0.0)
            },
            &[vec![56, PER_INSTANCE, 3, 0, 0, F32X3, 1, 12, F32X4, 2, 28, F32X3]],
            &[frame(), group(&[[0, UNIFORM]])],
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
            &[frame(), group(&[[0, UNIFORM]])],
        ),
        PIPELINE_SCREEN_RAYS => record(
            Head {
                depth_write: true,
                blend: OPAQUE,
                ..blended(SHADER_SCREEN_RAYS, CULL_NONE, 0, 0.0)
            },
            &[],
            &[group(&SCREEN_GROUP)],
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
                [0, UNIFORM],
                [19, UNIFORM],
                [2, TEXTURE_ARRAY],
                [3, SAMPLER],
                AIR[0],
                AIR[1],
                AIR[2],
                AIR[3],
                AIR[4],
                AIR[5],
                AIR[6],
                AIR[7],
                AIR[8],
                AIR[9],
            ])],
        ),
        PIPELINE_MIPS => record(screen(SHADER_MIPS), &[], &[group(&SKIN)]),
        // Every pixel, its depth too, so what is drawn over it meets the world's.
        PIPELINE_BLIT => record(
            Head {
                depth_write: true,
                depth_compare: ALWAYS,
                ..screen(SHADER_BLIT)
            },
            &[],
            &[group(&[[0, TEXTURE_UNFILTERED], [1, DEPTH]])],
        ),
        PIPELINE_IMPOSTOR => record(
            Head {
                topology: STRIP,
                ..lit(SHADER_IMPOSTOR, CULL_NONE)
            },
            &[],
            &[
                group(&LIT_FRAME),
                group(&[[0, UNIFORM_DYNAMIC]]),
                group(&[[0, TEXTURE_ARRAY], [1, SAMPLER], [2, UNIFORM]]),
                group(&[[0, STORAGE]]),
            ],
        ),
        PIPELINE_IMPOSTOR_BAKE => record(
            Head {
                fragment: true,
                vertex: VS,
                ..depth_only(SHADER_IMPOSTOR_BAKE)
            },
            &model_lanes(false),
            &[group(&[[0, UNIFORM_DYNAMIC]]), group(&SKIN)],
        ),
        PIPELINE_SHEET => record(
            Head {
                topology: STRIP,
                blend: ALPHA,
                ..screen(SHADER_SHEET)
            },
            &[],
            &[group(&[[0, UNIFORM], [1, STORAGE]]), group(&[[0, TEXTURE_ARRAY], [1, SAMPLER]])],
        ),
        PIPELINE_AO_DOWNSAMPLE => record(screen(SHADER_AO_DOWNSAMPLE), &[], &[group(&[[0, UNIFORM], [1, DEPTH], [6, UNIFORM]])]),
        PIPELINE_AO_SAMPLE => record(screen(SHADER_AO_SAMPLE), &[], &[group(&[[1, TEXTURE_UNFILTERED], [2, TEXTURE], [6, UNIFORM]])]),
        PIPELINE_AO_BLUR => record(
            screen(SHADER_AO_BLUR),
            &[],
            &[group(&[[1, TEXTURE_UNFILTERED], [2, TEXTURE], [3, TEXTURE], [4, SAMPLER], [5, UNIFORM_DYNAMIC], [6, UNIFORM]])],
        ),
        PIPELINE_AO_UPSAMPLE => record(
            screen(SHADER_AO_UPSAMPLE),
            &[],
            &[group(&[[0, UNIFORM], [1, DEPTH], [2, TEXTURE_UNFILTERED], [3, TEXTURE], [4, SAMPLER], [6, UNIFORM]])],
        ),
        PIPELINE_ATMOS_TRANSMITTANCE => record(screen(SHADER_ATMOS_TRANSMITTANCE), &[], &[group(&[AIR[0]])]),
        PIPELINE_ATMOS_MULTISCATTER => record(screen(SHADER_ATMOS_MULTISCATTER), &[], &[group(&[AIR[0], AIR[1], AIR[3]])]),
        PIPELINE_ATMOS_SKY_VIEW => record(
            screen(SHADER_ATMOS_SKY_VIEW),
            &[],
            &[group(&[[0, UNIFORM], AIR[0], AIR[1], AIR[3], AIR[8], [12, TEXTURE], [14, UNIFORM], [20, STORAGE_RW]])],
        ),
        PIPELINE_ATMOS_IRRADIANCE => record(screen(SHADER_ATMOS_IRRADIANCE), &[], &[group(&[AIR[0], AIR[1], AIR[3], [12, TEXTURE]])]),
        // The light on the land under the eye: the air's words and the tables it is lit by.
        PIPELINE_ATMOS_EXPOSURE => record(
            screen(SHADER_ATMOS_EXPOSURE),
            &[],
            &[group(&[[0, UNIFORM], AIR[0], AIR[1], AIR[2], AIR[3], AIR[5], AIR[6], AIR[7], AIR[8], AIR[9]])],
        ),
        PIPELINE_STARS => record(
            Head {
                topology: STRIP,
                depth_compare: GREATER_EQUAL,
                blend: ADD,
                ..blended(SHADER_STARS, CULL_NONE, 0, 0.0)
            },
            &[],
            &[group(&[[0, UNIFORM], [8, UNIFORM], [9, TEXTURE], [11, SAMPLER]]), group(&[[0, UNIFORM], [1, STORAGE]])],
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

include!(concat!(env!("OUT_DIR"), "/stages.rs"));

/// The stages of shader `shader` that read binding `binding` of its group
/// `group`, as build.rs read them off it; none where none does.
fn stages_of(shader: u32, group: u32, binding: u32) -> u32 {
    STAGES[shader as usize].iter().find(|e| e[0] == group && e[1] == binding).map_or(NONE, |e| e[2])
}

/// A plan's shader and its groups' entries, as `plan` lays them out.
fn groups_of(plan: &[u32]) -> (u32, Vec<&[u32]>) {
    let mut at = HEAD as usize;
    let lanes = plan[at];
    at += 1;
    for _ in 0..lanes {
        at += 3 + 3 * plan[at + 2] as usize;
    }
    let groups = plan[at];
    at += 1;
    let mut out = Vec::new();
    for _ in 0..groups {
        let n = plan[at] as usize;
        out.push(&plan[at + 1..at + 1 + 2 * n]);
        at += 1 + 2 * n;
    }
    (plan[0], out)
}

/// Pipeline `id`'s record, each group entry seen from the stages that read
/// it: in every shader whose pipeline lays out the same group at the same
/// place, so the one layout those pipelines share holds for each, and from
/// no stage written by hand.
pub fn pipeline(id: u32) -> Vec<u32> {
    let own = plan(id);
    if own.is_empty() {
        return own;
    }
    let plans: Vec<Vec<u32>> = (0..PIPELINES).map(plan).collect();
    let others: Vec<(u32, Vec<&[u32]>)> = plans.iter().filter(|p| !p.is_empty()).map(|p| groups_of(p)).collect();
    let (_, groups) = groups_of(&own);
    let head = own.len() - groups.iter().map(|g| 1 + g.len()).sum::<usize>() - 1;
    let mut out = own[..head].to_vec();
    out.push(groups.len() as u32);
    for (k, g) in groups.iter().enumerate() {
        out.push((g.len() / 2) as u32);
        for e in g.chunks(2) {
            let seen = others.iter().filter(|(_, gs)| gs.get(k) == Some(g)).fold(NONE, |s, (shader, _)| s | stages_of(*shader, k as u32, e[0]));
            out.extend_from_slice(&[e[0], seen, e[1]]);
        }
    }
    out
}
