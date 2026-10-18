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
        SHADER_JOINED => "joined_cs",
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
pub const FRAME_AT: [usize; 15] = [0, 16, 19, 20, 23, 24, 27, 28, 31, 32, 35, 36, 39, 40, 44];

/// Pipelines past draw.rs's, which a frame runs on its own.
pub const PIPELINE_SKY: u32 = 10;
pub const PIPELINE_MIPS: u32 = 11;
pub const PIPELINE_BLIT: u32 = 12;
pub const PIPELINE_DECOR_PREPASS: u32 = 13;
pub const PIPELINE_SKINNED_PREPASS: u32 = 14;
pub const PIPELINE_AO_DOWNSAMPLE: u32 = 15;
pub const PIPELINE_AO_SAMPLE: u32 = 16;
pub const PIPELINE_AO_BLUR: u32 = 17;
pub const PIPELINE_AO_UPSAMPLE: u32 = 18;
pub const PIPELINE_ATMOS_TRANSMITTANCE: u32 = 19;
pub const PIPELINE_ATMOS_MULTISCATTER: u32 = 20;
pub const PIPELINE_ATMOS_SKY_VIEW: u32 = 21;
pub const PIPELINE_ATMOS_EXPOSURE: u32 = 22;
pub const PIPELINE_STARS: u32 = 23;
pub const PIPELINE_CLOUDS: u32 = 24;
pub const PIPELINE_CLOUDS_ABOVE: u32 = 25;
pub const PIPELINE_CLOUD_FIELD: u32 = 26;
pub const PIPELINE_SHEET: u32 = 27;
pub const PIPELINE_IMPOSTOR_BAKE: u32 = 28;
pub const PIPELINE_ATMOS_IRRADIANCE: u32 = 29;
/// The billboards' regions, numbered, into a target of their own (pass/badges.rs).
pub const PIPELINE_BADGE_REGION: u32 = 30;
/// The debug gizmos' faces, depth only, into the gizmos' own depth (gpu/renderer.rs `DebugPass`).
pub const PIPELINE_DEBUG_FACE: u32 = 31;
/// The eye's rays, opaque at their depth (pxl-portable ray.rs).
pub const PIPELINE_SCREEN_RAYS: u32 = 32;
pub const PIPELINES: u32 = 33;

// Record words: shader, has fragment, topology, cull, depth write, depth
// compare, bias (i32), slope scale (f32 bits), blend, vertex entry; then vertex
// buffers (count; stride, step, attribute count, (location, offset, format)
// each); then bind groups (count; entry count, (binding, visibility, kind) each).
pub use crate::api_spec::record::*;
/// A binding's stages are xpute's own bits (SHADER_STAGE), none or some.
pub use xpute_gpu::spec::SHADER_STAGE::{COMPUTE, FRAGMENT, VERTEX};
pub const NONE: u32 = 0;

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
use crate::frame::shader::floor;

const fn f(v: f32) -> u32 {
    v.to_bits()
}
const fn i(v: i32) -> u32 {
    v as u32
}

#[derive(Clone, Copy)]
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

const fn screen(shader: u32) -> Head {
    Head {
        depth_compare: NO_DEPTH,
        blend: OPAQUE,
        ..blended(shader, CULL_NONE, 0, 0.0)
    }
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

/// The lit pass's frame, occlusion, air and layers, all read where
/// a pixel is painted; then the rays, the looks and the eye's words: the
/// most entries any group has (`GROUP_ENTRIES`).
const SCREEN_GROUP: [[u32; 2]; 18] = {
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
        [20, STORAGE],
        [21, UNIFORM],
    ]
};

/// The most entries a group has, a layout's and a bind group's alike: the
/// screen's (and renderer.rs's lit entries, which it extends); a group past
/// it is refused where it is made.
pub const GROUP_ENTRIES: usize = SCREEN_GROUP.len();
/// A vertex buffer: its stride, whether it steps by vertex or instance, and
/// its attributes, each (location, offset, format).
#[derive(Clone, Copy)]
struct Lane {
    stride: u32,
    step: u32,
    attributes: &'static [[u32; 3]],
}

const fn vertex(stride: u32, attributes: &'static [[u32; 3]]) -> Lane {
    Lane { stride, step: PER_VERTEX, attributes }
}

const MODEL_LANES: [Lane; 3] = [vertex(12, &[[0, 0, F32X3]]), vertex(12, &[[1, 0, F32X3]]), vertex(8, &[[2, 0, F32X2]])];
const SKINNED_LANES: [Lane; 5] = [MODEL_LANES[0], MODEL_LANES[1], MODEL_LANES[2], vertex(16, &[[3, 0, U32X4]]), vertex(16, &[[4, 0, F32X4]])];
/// A box's words, as the box shader (box.wgsl) takes one and the hover
/// ghost writes it (mesh/ghost.rs `write_lone_box`): a vec4, then its layer.
pub const BOX_WORDS: u32 = 5;
/// The boxes are a per-instance vertex buffer, so a draw reaches its boxes by
/// its first instance.
const BOX_LANES: [Lane; 2] = [
    Lane {
        stride: BOX_WORDS * 4,
        step: PER_INSTANCE,
        attributes: &[[0, 0, U32X4]],
    },
    Lane {
        stride: BOX_WORDS * 4,
        step: PER_INSTANCE,
        attributes: &[[1, 16, U32]],
    },
];
const PUCK_ZONE_LANES: [Lane; 2] = [vertex(12, &[[0, 0, F32X3]]), vertex(8, &[[1, 0, F32X2]])];
const PUCK_BEACON_LANES: [Lane; 2] = [vertex(12, &[[0, 0, F32X3]]), vertex(12, &[[1, 0, F32X3]])];
const DEBUG_LANES: [Lane; 1] = [Lane {
    stride: 56,
    step: PER_INSTANCE,
    attributes: &[[0, 0, F32X3], [1, 12, F32X4], [2, 28, F32X3]],
}];
const DEBUG_FACE_LANES: [Lane; 1] = [vertex(12, &[[0, 0, F32X3]])];

/// `a` then `b`: a group laid out as another and more.
const fn and<const N: usize, const M: usize, const R: usize>(a: [[u32; 2]; N], b: [[u32; 2]; M]) -> [[u32; 2]; R] {
    let mut out = [[0; 2]; R];
    let mut k = 0;
    while k < R {
        out[k] = if k < N { a[k] } else { b[k - N] };
        k += 1;
    }
    out
}

type Group = &'static [[u32; 2]];
const FRAME: Group = &[[0, UNIFORM]];
const BOX_GROUP: [[u32; 2]; 16] = and(LIT_FRAME, [[3, STORAGE], [4, TEXTURE_ARRAY], [5, STORAGE], [6, SAMPLER]]);
const MODEL_GROUPS: [Group; 4] = [&LIT_FRAME, &[[0, UNIFORM_DYNAMIC]], &SKIN, &[[0, STORAGE]]];
/// A billboard set's uniform and instances, and what its pass reads at an
/// anchor: the world's depth for the regions, the regions for the sprites.
const BADGE_SPRITES: Group = &[[0, UNIFORM], [1, STORAGE], [2, TEXTURE_UNFILTERED]];
const BADGE_REGIONS: Group = &[[0, UNIFORM], [1, STORAGE], [2, DEPTH]];
const SKY_GROUP: [[u32; 2]; 14] = and([[0, UNIFORM], [19, UNIFORM], [2, TEXTURE_ARRAY], [3, SAMPLER]], AIR);

/// A pipeline's record before its groups' stages: its head, its lanes and
/// each group's entries (binding, kind), which `record` sees from the
/// stages that read them.
#[derive(Clone, Copy)]
struct Plan {
    head: Head,
    lanes: &'static [Lane],
    groups: &'static [Group],
}

const fn plan(head: Head, lanes: &'static [Lane], groups: &'static [Group]) -> Plan {
    Plan { head, lanes, groups }
}

/// Pipeline `id`'s plan: every id below `PIPELINES` has one, which the
/// records, worked out as the crate is built, ask of each.
const fn plan_of(id: u32) -> Plan {
    match id {
        PIPELINE_DECOR => plan(lit(SHADER_DECOR, CULL_NONE), &MODEL_LANES, &MODEL_GROUPS),
        // Slightly clear of a surface it sits on, strongly clear of one it sits in.
        PIPELINE_GHOST_BOX => plan(blended(SHADER_BOX, CULL_BACK, 1, 1.0), &BOX_LANES, &[&BOX_GROUP]),
        PIPELINE_GHOST_BOX_COPLANAR => plan(blended(SHADER_BOX, CULL_BACK, 8, 2.0), &BOX_LANES, &[&BOX_GROUP]),
        PIPELINE_GHOST_DECOR => plan(blended(SHADER_DECOR, CULL_NONE, 1, 1.0), &MODEL_LANES, &MODEL_GROUPS),
        // Back faces culled: drawn both sides, a closed body's inner faces fight its
        // outer ones at a distance and the character shimmers.
        PIPELINE_SKINNED => plan(lit(SHADER_SKINNED, CULL_BACK), &SKINNED_LANES, &MODEL_GROUPS),
        PIPELINE_PUCK_ZONE => plan(blended(SHADER_PUCK_ZONE, CULL_NONE, 4, 1.0), &PUCK_ZONE_LANES, &[FRAME, &[[0, UNIFORM]], &[[0, STORAGE]]]),
        PIPELINE_PUCK_BEACON => plan(lit(SHADER_PUCK_BEACON, CULL_BACK), &PUCK_BEACON_LANES, &[&[[0, UNIFORM], AIR[0], AIR[1], AIR[3]], &[[0, UNIFORM]]]),
        PIPELINE_BADGE => plan(
            Head {
                topology: STRIP,
                depth_compare: GREATER_EQUAL,
                ..blended(SHADER_BADGE, CULL_NONE, 0, 0.0)
            },
            &[],
            &[FRAME, BADGE_SPRITES, &SKIN],
        ),
        PIPELINE_BADGE_REGION => plan(
            Head {
                topology: STRIP,
                depth_write: true,
                depth_compare: GREATER_EQUAL,
                blend: OPAQUE,
                ..blended(SHADER_BADGE_REGION, CULL_NONE, 0, 0.0)
            },
            &[],
            &[FRAME, BADGE_REGIONS],
        ),
        // Against the gizmos' own faces, not the world's depth.
        PIPELINE_DEBUG => plan(
            Head {
                blend: ALPHA,
                ..blended(SHADER_DEBUG, CULL_NONE, 0, 0.0)
            },
            &DEBUG_LANES,
            &[FRAME, &[[0, UNIFORM]]],
        ),
        // Pushed back by their slope as a rasterizer's polygon offset is, so
        // the lines that bound a face stay in front of it.
        PIPELINE_DEBUG_FACE => plan(
            Head {
                cull: CULL_NONE,
                depth_compare: GREATER,
                depth_bias: i(-8),
                depth_bias_slope: f(-2.0),
                ..depth_only(SHADER_DEBUG)
            },
            &DEBUG_FACE_LANES,
            &[FRAME, &[[0, UNIFORM]]],
        ),
        PIPELINE_SCREEN_RAYS => plan(
            Head {
                depth_write: true,
                blend: OPAQUE,
                ..blended(SHADER_SCREEN_RAYS, CULL_NONE, 0, 0.0)
            },
            &[],
            &[&SCREEN_GROUP],
        ),
        PIPELINE_DECOR_PREPASS => plan(prepass(SHADER_DECOR, CULL_NONE), &MODEL_LANES, &MODEL_GROUPS),
        PIPELINE_SKINNED_PREPASS => plan(prepass(SHADER_SKINNED, CULL_BACK), &SKINNED_LANES, &MODEL_GROUPS),
        // Only where the opaque world left the far plane, so covered pixels never run it.
        PIPELINE_SKY => plan(
            Head {
                depth_compare: GREATER_EQUAL,
                blend: OPAQUE,
                ..blended(SHADER_SKY, CULL_NONE, 0, 0.0)
            },
            &[],
            &[&SKY_GROUP],
        ),
        PIPELINE_MIPS => plan(screen(SHADER_MIPS), &[], &[&SKIN]),
        // Every pixel, its depth too, so what is drawn over it meets the world's.
        PIPELINE_BLIT => plan(
            Head {
                depth_write: true,
                depth_compare: ALWAYS,
                ..screen(SHADER_BLIT)
            },
            &[],
            &[&[[0, TEXTURE_UNFILTERED], [1, DEPTH]]],
        ),
        PIPELINE_IMPOSTOR => plan(
            Head {
                topology: STRIP,
                ..lit(SHADER_IMPOSTOR, CULL_NONE)
            },
            &[],
            &[&LIT_FRAME, &[[0, UNIFORM_DYNAMIC]], &[[0, TEXTURE_ARRAY], [1, SAMPLER], [2, UNIFORM]], &[[0, STORAGE]]],
        ),
        PIPELINE_IMPOSTOR_BAKE => plan(
            Head {
                fragment: true,
                vertex: VS,
                ..depth_only(SHADER_IMPOSTOR_BAKE)
            },
            &MODEL_LANES,
            &[&[[0, UNIFORM_DYNAMIC]], &SKIN],
        ),
        PIPELINE_SHEET => plan(
            Head {
                topology: STRIP,
                blend: ALPHA,
                ..screen(SHADER_SHEET)
            },
            &[],
            &[&[[0, UNIFORM], [1, STORAGE]], &[[0, TEXTURE_ARRAY], [1, SAMPLER]]],
        ),
        PIPELINE_AO_DOWNSAMPLE => plan(screen(SHADER_AO_DOWNSAMPLE), &[], &[&[[0, UNIFORM], [1, DEPTH], [6, UNIFORM]]]),
        PIPELINE_AO_SAMPLE => plan(screen(SHADER_AO_SAMPLE), &[], &[&[[1, TEXTURE_UNFILTERED], [2, TEXTURE], [6, UNIFORM]]]),
        PIPELINE_AO_BLUR => plan(
            screen(SHADER_AO_BLUR),
            &[],
            &[&[[1, TEXTURE_UNFILTERED], [2, TEXTURE], [3, TEXTURE], [4, SAMPLER], [5, UNIFORM_DYNAMIC], [6, UNIFORM]]],
        ),
        PIPELINE_AO_UPSAMPLE => plan(
            screen(SHADER_AO_UPSAMPLE),
            &[],
            &[&[[0, UNIFORM], [1, DEPTH], [2, TEXTURE_UNFILTERED], [3, TEXTURE], [4, SAMPLER], [6, UNIFORM]]],
        ),
        PIPELINE_ATMOS_TRANSMITTANCE => plan(screen(SHADER_ATMOS_TRANSMITTANCE), &[], &[&[AIR[0]]]),
        PIPELINE_ATMOS_MULTISCATTER => plan(screen(SHADER_ATMOS_MULTISCATTER), &[], &[&[AIR[0], AIR[1], AIR[3]]]),
        PIPELINE_ATMOS_SKY_VIEW => plan(
            screen(SHADER_ATMOS_SKY_VIEW),
            &[],
            &[&[[0, UNIFORM], AIR[0], AIR[1], AIR[3], AIR[8], [12, TEXTURE], [14, UNIFORM], [20, STORAGE_RW]]],
        ),
        PIPELINE_ATMOS_IRRADIANCE => plan(screen(SHADER_ATMOS_IRRADIANCE), &[], &[&[AIR[0], AIR[1], AIR[3], [12, TEXTURE]]]),
        // The light on the land under the eye: the air's words and the tables it is lit by.
        PIPELINE_ATMOS_EXPOSURE => plan(
            screen(SHADER_ATMOS_EXPOSURE),
            &[],
            &[&[[0, UNIFORM], AIR[0], AIR[1], AIR[2], AIR[3], AIR[5], AIR[6], AIR[7], AIR[8], AIR[9]]],
        ),
        PIPELINE_STARS => plan(
            Head {
                topology: STRIP,
                depth_compare: GREATER_EQUAL,
                blend: ADD,
                ..blended(SHADER_STARS, CULL_NONE, 0, 0.0)
            },
            &[],
            &[&[[0, UNIFORM], [8, UNIFORM], [9, TEXTURE], [11, SAMPLER]], &[[0, UNIFORM], [1, STORAGE]]],
        ),
        // From under the sheets only where the world left the far plane; from over them everywhere.
        PIPELINE_CLOUDS => plan(
            Head {
                depth_compare: GREATER_EQUAL,
                ..blended(SHADER_CLOUDS, CULL_NONE, 0, 0.0)
            },
            &[],
            &[&CLOUD_GROUP],
        ),
        PIPELINE_CLOUDS_ABOVE => plan(
            Head {
                depth_compare: ALWAYS,
                ..blended(SHADER_CLOUDS, CULL_NONE, 0, 0.0)
            },
            &[],
            &[&CLOUD_GROUP],
        ),
        PIPELINE_CLOUD_FIELD => plan(screen(SHADER_CLOUD_FIELD), &[], &[]),
        _ => panic!("a pipeline number with no plan"),
    }
}

include!(concat!(env!("OUT_DIR"), "/stages.rs"));

/// The stages of shader `shader` that read binding `binding` of its group
/// `group`, as build.rs read them off it; none where none does.
const fn stages_of(shader: u32, group: u32, binding: u32) -> u32 {
    let entries = STAGES[shader as usize];
    let mut k = 0;
    while k < entries.len() {
        if entries[k][0] == group && entries[k][1] == binding {
            return entries[k][2];
        }
        k += 1;
    }
    NONE
}

/// Whether two groups are one entry for entry.
const fn alike(a: Group, b: Group) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut k = 0;
    while k < a.len() {
        if a[k][0] != b[k][0] || a[k][1] != b[k][1] {
            return false;
        }
        k += 1;
    }
    true
}

/// The stages binding `binding` of group `k` is seen from: those that read
/// it in every shader whose pipeline lays out the same group at the same
/// place, so the one layout those pipelines share holds for each, and from
/// no stage written by hand.
const fn seen(group: Group, k: usize, binding: u32) -> u32 {
    let mut stages = NONE;
    let mut id = 0;
    while id < PIPELINES {
        let p = plan_of(id);
        if k < p.groups.len() && alike(p.groups[k], group) {
            stages |= stages_of(p.head.shader, k as u32, binding);
        }
        id += 1;
    }
    stages
}

/// A plan's record's words (see Record words above).
const fn words(p: &Plan) -> usize {
    let mut n = HEAD as usize + 1 + 1;
    let mut k = 0;
    while k < p.lanes.len() {
        n += 3 + 3 * p.lanes[k].attributes.len();
        k += 1;
    }
    k = 0;
    while k < p.groups.len() {
        n += 1 + 3 * p.groups[k].len();
        k += 1;
    }
    n
}

/// The most words a record has.
const RECORD_WORDS: usize = {
    let (mut most, mut id) = (0, 0);
    while id < PIPELINES {
        let n = words(&plan_of(id));
        most = if n > most { n } else { most };
        id += 1;
    }
    most
};

/// Pipeline `id`'s record and its length, each group entry seen from the
/// stages that read it (`seen`).
const fn record(id: u32) -> ([u32; RECORD_WORDS], usize) {
    let mut out = [0; RECORD_WORDS];
    let p = plan_of(id);
    let h = p.head;
    let head = [
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
    let mut n = 0;
    while n < head.len() {
        out[n] = head[n];
        n += 1;
    }
    out[n] = p.lanes.len() as u32;
    n += 1;
    let mut k = 0;
    while k < p.lanes.len() {
        let l = p.lanes[k];
        (out[n], out[n + 1], out[n + 2]) = (l.stride, l.step, l.attributes.len() as u32);
        n += 3;
        let mut a = 0;
        while a < l.attributes.len() {
            (out[n], out[n + 1], out[n + 2]) = (l.attributes[a][0], l.attributes[a][1], l.attributes[a][2]);
            (n, a) = (n + 3, a + 1);
        }
        k += 1;
    }
    out[n] = p.groups.len() as u32;
    n += 1;
    k = 0;
    while k < p.groups.len() {
        let g = p.groups[k];
        out[n] = g.len() as u32;
        n += 1;
        let mut e = 0;
        while e < g.len() {
            (out[n], out[n + 1], out[n + 2]) = (g[e][0], seen(g, k, g[e][0]), g[e][1]);
            (n, e) = (n + 3, e + 1);
        }
        k += 1;
    }
    (out, n)
}

/// Every pipeline's record, worked out as the crate is built.
#[allow(long_running_const_eval)]
const RECORDS: [([u32; RECORD_WORDS], usize); PIPELINES as usize] = {
    let mut r = [([0; RECORD_WORDS], 0); PIPELINES as usize];
    let mut id = 0;
    while id < PIPELINES {
        r[id as usize] = record(id);
        id += 1;
    }
    r
};
static TABLE: [([u32; RECORD_WORDS], usize); PIPELINES as usize] = RECORDS;

/// Pipeline `id`'s record (see Record words above), each group entry seen
/// from the stages that read it.
pub fn pipeline(id: u32) -> &'static [u32] {
    let (words, n) = &TABLE[id as usize];
    &words[..*n]
}

/// Whether pipeline `id`'s layout binds within what every WebGPU device
/// binds (`floor::holds`), its groups as its record lays them out.
const fn floored(id: u32) -> bool {
    let (words, _) = &RECORDS[id as usize];
    let mut at = HEAD as usize;
    let lanes = words[at];
    at += 1;
    let mut k = 0;
    while k < lanes {
        at += 3 + 3 * words[at + 2] as usize;
        k += 1;
    }
    let groups = words[at] as usize;
    at += 1;
    if groups > floor::BIND_GROUPS {
        return false;
    }
    let mut stated: [&[u32]; floor::BIND_GROUPS] = [&[]; floor::BIND_GROUPS];
    let mut g = 0;
    while g < groups {
        let m = words[at] as usize;
        stated[g] = words.split_at(at + 1).1.split_at(3 * m).0;
        at += 1 + 3 * m;
        g += 1;
    }
    floor::holds(stated.split_at(groups).0)
}

/// Every pipeline's layout within the floor, checked as the crate is built,
/// over every id, so a plan added is checked with no list to grow.
const _: () = {
    let mut id = 0;
    while id < PIPELINES {
        assert!(floored(id), "a pipeline's layout binds past WebGPU's floor (frame/shader/floor.rs)");
        id += 1;
    }
};
