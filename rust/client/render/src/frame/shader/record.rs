// pxl-render/frame/shader/record.rs

//! Shaders and their pipeline records (lanes, bind groups, primitive, depth and
//! blend state) as one source the host turns into WebGPU or wgpu descriptors.
//! build.rs assembles and validates every shader (part.rs), and reads off it
//! the stages each pipeline's groups are seen from.

pub use crate::frame::shader::part::*;

/// A compute shader's one entry point, as its source names it.
pub fn compute_entry(kind: u32) -> &'static str {
    match kind {
        SHADER_CAST => "cast_cs",
        SHADER_JOINED => "joined_cs",
        SHADER_ATMOS_FROXEL => "froxel_cs",
        SHADER_SCREEN_REACH => "screen_reach_cs",
        SHADER_SCREEN_BOUND => "screen_bound_cs",
        SHADER_SCREEN_MARK => "screen_mark_cs",
        SHADER_BOUND => "bound_cs",
        SHADER_HEIGHT => "height_cs",
        SHADER_GENERATED_MARK => "decor_mark_cs",
        SHADER_GENERATED_THIN => "decor_thin_cs",
        SHADER_GENERATED => "decor_cs",
        SHADER_GENERATED_CNT => "decor_cnt_cs",
        SHADER_GENERATED_ARGS => "decor_args_cs",
        SHADER_GENERATED_COMPACT => "decor_compact_cs",
        SHADER_WAY => "way_cs",
        SHADER_WAY_FREE => "way_free_cs",
        SHADER_WAY_TREE => "way_tree_cs",
        SHADER_WAY_FREED => "way_freed_cs",
        SHADER_BEAM => "beam_cs",
        SHADER_CELL => "cell_cs",
        SHADER_MISS => "miss_cs",
        SHADER_FORGET => "forget_cs",
        SHADER_SAMPLE => "sample_cs",
        SHADER_LIGHT => "light_cs",
        SHADER_LIGHT_MISS => "light_miss_cs",
        SHADER_SAMPLE_BOX => "sample_box_cs",
        SHADER_MISS_LIST => "miss_list_cs",
        SHADER_FACE => "face_cs",
        SHADER_LIGHT_MARK => "light_mark_cs",
        SHADER_LIGHT_MARK_DRAWN => "light_mark_drawn_cs",
        SHADER_LIGHT_REACH => "light_reach_cs",
        SHADER_LIGHT_DETAIL => "light_detail_cs",
        _ => xpute_kit::bug!(EINVAL, kind),
    }
}

include!(concat!(env!("OUT_DIR"), "/sources.rs"));

/// Pipelines past draw.rs's, which a frame runs on its own.
pub const PIPELINE_SKY: u32 = 10;
pub const PIPELINE_MIP: u32 = 11;
pub const PIPELINE_DECOR_PREPASS: u32 = 12;
pub const PIPELINE_AO_DOWNSAMPLE: u32 = 13;
pub const PIPELINE_AO_SAMPLE: u32 = 14;
pub const PIPELINE_AO_BLUR: u32 = 15;
pub const PIPELINE_AO_UPSAMPLE: u32 = 16;
pub const PIPELINE_ATMOS_TRANSMITTANCE: u32 = 17;
pub const PIPELINE_ATMOS_MULTISCATTER: u32 = 18;
pub const PIPELINE_ATMOS_SKY_VIEW: u32 = 19;
pub const PIPELINE_ATMOS_EXPOSURE: u32 = 20;
pub const PIPELINE_STAR: u32 = 21;
pub const PIPELINE_CLOUD: u32 = 22;
pub const PIPELINE_CLOUD_FIELD: u32 = 23;
pub const PIPELINE_SHEET: u32 = 24;
pub const PIPELINE_IMPOSTOR_BAKE: u32 = 25;
pub const PIPELINE_ATMOS_IRRADIANCE: u32 = 26;
/// The markers' regions, numbered, into a target of their own (pass/marker.rs).
pub const PIPELINE_MARKER_REGION: u32 = 27;
/// The debug gizmos' faces, depth only, into the gizmos' own depth (gpu/pass/debug.rs `DebugPass`).
pub const PIPELINE_DEBUG_FACE: u32 = 28;
/// The eye's rays, opaque at their depth (pxl-spec-core ray.rs).
pub const PIPELINE_SCREEN_RAY: u32 = 29;
/// Decor's and the actors' depth from the light, and the world's laid in
/// before them, into the light's shadow map (gpu/shadow.rs).
pub const PIPELINE_DECOR_DEPTH: u32 = 30;
pub const PIPELINE_SKINNED_DEPTH: u32 = 31;
pub const PIPELINE_LIGHT_MAP: u32 = 32;
pub const PIPELINE_SCREEN_PREPASS: u32 = 33;
/// The samples' boxes, the nearest face a pixel's ray enters kept with its
/// sample (gpu/ray/raster.rs).
pub const PIPELINE_RASTER: u32 = 34;
/// The world's color onto the canvas, encoded once, and its depth carried to
/// the canvas's size (gpu/finish.rs).
pub const PIPELINE_SCREEN_FINISH: u32 = 35;
/// The world's color gathered over frames (gpu/resolve.rs).
pub const PIPELINE_SCREEN_RESOLVE: u32 = 36;
/// The gate a frame not drawn whole is drawn through: the claimed tiles
/// cleared, their stencil raised (gpu/renderer.rs `frame`).
pub const PIPELINE_SCREEN_GATE: u32 = 37;
/// The finished picture kept at the canvas's size shown on the canvas
/// (gpu/finish.rs).
pub const PIPELINE_SCREEN_SHOW: u32 = 38;
/// The claimed tiles of the history the resolve wrote copied back into the
/// last (gpu/resolve.rs).
pub const PIPELINE_SCREEN_WRITE_BACK: u32 = 39;
/// The gate the samples' raster is drawn through in a frame not drawn whole:
/// the claimed tiles cleared to the far plane, the rest left at the nearest
/// (gpu/ray/raster.rs).
pub const PIPELINE_RASTER_GATE: u32 = 40;
/// The gate the light's map is drawn through where only the casters the
/// kernel bounds moved: the claimed tiles set back to the far plane, their
/// stencil raised (gpu/shadow.rs).
pub const PIPELINE_LIGHT_GATE: u32 = 41;
/// An actor drawn where it was and where it is, its fragments setting their
/// screen tiles' words (gpu/pass/skinned.rs `draw_marks`).
pub const PIPELINE_SKINNED_MARK: u32 = 42;
pub const PIPELINES: u32 = 43;

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
    use super::{COMPUTE, STORAGE, STORAGE_RW, STORAGE_TEXTURE_RW, TEXTURE_UINT_ARRAY, UNIFORM};
    use crate::frame::shader::floor;
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
/// group laid out as `b`: the same binding, stages and kind, as WebGPU
/// matches a shader's storage access to its layout entry's exactly.
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

use crate::frame::draw::{
    PIPELINE_CAMERA_GRID, PIPELINE_DEBUG, PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_DECOR,
    PIPELINE_IMPOSTOR, PIPELINE_MARKER, PIPELINE_PUCK_BEACON, PIPELINE_PUCK_ZONE, PIPELINE_SKINNED,
};
use crate::frame::shader::floor;
use pxl_spec_core::param::light;

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

const fn blended(shader: u32, cull: u32) -> Head {
    Head {
        shader,
        fragment: true,
        topology: TRIANGLES,
        cull,
        depth_write: false,
        depth_compare: GREATER,
        depth_bias: 0,
        depth_bias_slope: 0,
        blend: ALPHA,
        vertex: VS,
    }
}

/// The shadow map's casters' alike, raster and cast (pxl-spec-core `param::light::SLOPE_BIAS`).
const DEPTH_SLOPE_BIAS: f32 = light::SLOPE_BIAS;

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
        ..blended(shader, CULL_NONE)
    }
}

/// The air's bindings, numbered alike in every pass that reads it.
const AIR: [[u32; 2]; 11] = [
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
    // The air before each point, made of the rest (gpu/atmosphere.rs `froxel`).
    [22, TEXTURE_3D],
];

/// The light's shadow map and its comparing sampler (shadow.wgsl), read where
/// a lit fragment is shaded.
const SHADOW: [[u32; 2]; 2] = [[1, DEPTH], [2, SAMPLER_COMPARISON]];
const LIT_FRAME: [[u32; 2]; 15] = {
    // The air in the vertex stage, where each point's lights and veil are
    // worked out; a fragment reads its words and the eye's texel, whose white
    // it shows by (atmos_eye.wgsl). The sky's views and the Earth's picture
    // are the sky pass's, laid out unread.
    [
        [0, UNIFORM],
        SHADOW[0],
        SHADOW[1],
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
        AIR[10],
    ]
};
/// The air's entries whole, the volume among them, which the sheets read as
/// every painter does (atmos_air.wgsl `seen_from_eye`); the tables and the
/// Earth's maps it does not read are laid out because the shader declares them.
const CLOUD_GROUP: [[u32; 2]; 15] = and(
    [[0, UNIFORM], [1, UNIFORM], [2, TEXTURE], [3, SAMPLER]],
    AIR,
);
const SKIN: [[u32; 2]; 2] = [[0, TEXTURE], [1, SAMPLER]];

/// The lit pass's frame, shadow, occlusion, air and layers, all read where
/// a pixel is painted; then the rays, the looks and the eye's words: the
/// most entries any group has (`GROUP_ENTRIES`).
const SCREEN_GROUP: [[u32; 2]; 23] = and(
    LIT_FRAME,
    [
        [3, TEXTURE_UINT_ARRAY],
        [4, TEXTURE_ARRAY],
        [5, STORAGE],
        [6, SAMPLER],
        [20, STORAGE],
        [21, UNIFORM],
        [30, STORAGE],
        [31, UNIFORM],
    ],
);

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
    Lane {
        stride,
        step: PER_VERTEX,
        attributes,
    }
}

const MODEL_LANES: [Lane; 3] = [
    vertex(12, &[[0, 0, F32X3]]),
    vertex(12, &[[1, 0, F32X3]]),
    vertex(8, &[[2, 0, F32X2]]),
];
const SKINNED_LANES: [Lane; 5] = [
    MODEL_LANES[0],
    MODEL_LANES[1],
    MODEL_LANES[2],
    vertex(16, &[[3, 0, U32X4]]),
    vertex(16, &[[4, 0, F32X4]]),
];
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
/// A debug line's vertex, its place and its color, a segment's two read as
/// one instance; a debug face's vertex, its place.
pub const DEBUG_VERTEX_BYTES: u32 = 28;
pub const DEBUG_FACE_VERTEX_BYTES: u32 = 12;
const DEBUG_LANES: [Lane; 1] = [Lane {
    stride: 2 * DEBUG_VERTEX_BYTES,
    step: PER_INSTANCE,
    attributes: &[
        [0, 0, F32X3],
        [1, 12, F32X4],
        [2, DEBUG_VERTEX_BYTES, F32X3],
    ],
}];
const DEBUG_FACE_LANES: [Lane; 1] = [vertex(DEBUG_FACE_VERTEX_BYTES, &[[0, 0, F32X3]])];

/// `a` then `b`: a group laid out as another and more.
const fn and<const N: usize, const M: usize, const R: usize>(
    a: [[u32; 2]; N],
    b: [[u32; 2]; M],
) -> [[u32; 2]; R] {
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
const BOX_GROUP: [[u32; 2]; 19] = and(
    LIT_FRAME,
    [[3, STORAGE], [4, TEXTURE_ARRAY], [5, STORAGE], [6, SAMPLER]],
);
/// A draw's uniform, at a dynamic offset (gpu/pass/model.rs `DrawUniforms`),
/// and its upload: one layout each wherever a draw binds them, so a group
/// one owner makes binds in every pipeline that names them (gpu/generated.rs,
/// gpu/upload.rs).
const DRAW: [[u32; 2]; 1] = [[0, UNIFORM_DYNAMIC]];
const UPLOAD: [[u32; 2]; 1] = [[0, STORAGE]];
/// An upload's group as every pipeline binding one lays it out, which the
/// draw list's uploads are grouped in (gpu/upload.rs `Uploads::group`).
pub const UPLOAD_LAYOUT: [u32; 3] = [UPLOAD[0][0], seen(&UPLOAD, UPLOAD[0][0]), UPLOAD[0][1]];

const MODEL_GROUPS: [Group; 4] = [&LIT_FRAME, &DRAW, &SKIN, &UPLOAD];
/// A model's depth from the light: the light's frame alone in group 0, where
/// its place is read; the rest as the lit pass binds them.
const DEPTH_GROUPS: [Group; 4] = [FRAME, MODEL_GROUPS[1], MODEL_GROUPS[2], MODEL_GROUPS[3]];
/// A model's mark (tile_mark.wgsl): the eye's frame, the screen tiles' words
/// and their uniform in group 0; the rest as its depth reads them.
const MARK_FRAME: Group = &[[0, UNIFORM], [30, STORAGE_RW], [31, UNIFORM]];
const MARK_GROUPS: [Group; 4] = [
    MARK_FRAME,
    MODEL_GROUPS[1],
    MODEL_GROUPS[2],
    MODEL_GROUPS[3],
];
/// A marker set's instances, and what its pass reads at an anchor: the
/// world's depth for the regions, the regions for the sprites. Its uniform is
/// a draw's (`DRAW`).
const MARKER_SPRITES: Group = &[[0, STORAGE], [1, TEXTURE_UNFILTERED]];
const MARKER_REGIONS: Group = &[[0, STORAGE], [1, DEPTH]];
const SKY_GROUP: [[u32; 2]; 15] = and(
    [
        [0, UNIFORM],
        [19, UNIFORM],
        [2, TEXTURE_ARRAY],
        [3, SAMPLER],
    ],
    AIR,
);

/// A pipeline's record before its groups' stages: its head, its lanes and
/// each group's entries (binding, kind), which `record` sees from the
/// stages that read them.
#[derive(Clone, Copy)]
struct Descriptor {
    head: Head,
    lanes: &'static [Lane],
    groups: &'static [Group],
}

impl Descriptor {
    const fn new(head: Head, lanes: &'static [Lane], groups: &'static [Group]) -> Self {
        Self {
            head,
            lanes,
            groups,
        }
    }
}

/// Pipeline `id`'s descriptor: every id below `PIPELINES` has one, which the
/// records, worked out as the crate is built, ask of each.
const fn desc_of(id: u32) -> Descriptor {
    match id {
        PIPELINE_DECOR => {
            Descriptor::new(lit(SHADER_DECOR, CULL_NONE), &MODEL_LANES, &MODEL_GROUPS)
        }
        // What is laid on the world's faces draws at its own depth: the
        // world's stands behind it (screen_ray.wgsl `behind`).
        PIPELINE_GHOST_BOX => {
            Descriptor::new(blended(SHADER_BOX, CULL_BACK), &BOX_LANES, &[&BOX_GROUP])
        }
        PIPELINE_GHOST_DECOR => Descriptor::new(
            blended(SHADER_DECOR, CULL_NONE),
            &MODEL_LANES,
            &MODEL_GROUPS,
        ),
        // Back faces culled: drawn both sides, a closed body's inner faces fight its
        // outer ones at a distance and the character shimmers.
        PIPELINE_SKINNED => Descriptor::new(
            lit(SHADER_SKINNED, CULL_BACK),
            &SKINNED_LANES,
            &MODEL_GROUPS,
        ),
        PIPELINE_PUCK_ZONE => Descriptor::new(
            blended(SHADER_PUCK_ZONE, CULL_NONE),
            &PUCK_ZONE_LANES,
            &[FRAME, &DRAW, &UPLOAD],
        ),
        PIPELINE_PUCK_BEACON => Descriptor::new(
            lit(SHADER_PUCK_BEACON, CULL_BACK),
            &PUCK_BEACON_LANES,
            &[&LIT_FRAME, &DRAW],
        ),
        PIPELINE_MARKER => Descriptor::new(
            Head {
                topology: STRIP,
                depth_compare: GREATER_EQUAL,
                ..blended(SHADER_MARKER, CULL_NONE)
            },
            &[],
            &[FRAME, &DRAW, MARKER_SPRITES, &SKIN],
        ),
        PIPELINE_MARKER_REGION => Descriptor::new(
            Head {
                topology: STRIP,
                depth_write: true,
                depth_compare: GREATER_EQUAL,
                blend: OPAQUE,
                ..blended(SHADER_MARKER_REGION, CULL_NONE)
            },
            &[],
            &[FRAME, &DRAW, MARKER_REGIONS],
        ),
        // Against the gizmos' own faces, not the world's depth.
        PIPELINE_DEBUG => Descriptor::new(
            Head {
                blend: ALPHA,
                ..blended(SHADER_DEBUG, CULL_NONE)
            },
            &DEBUG_LANES,
            &[FRAME, &DRAW],
        ),
        // Pushed back by their slope as a rasterizer's polygon offset is, so
        // the lines that bound a face stay in front of it.
        PIPELINE_DEBUG_FACE => Descriptor::new(
            Head {
                cull: CULL_NONE,
                depth_compare: GREATER,
                depth_bias: i(-8),
                depth_bias_slope: f(-2.0),
                ..depth_only(SHADER_DEBUG)
            },
            &DEBUG_FACE_LANES,
            &[FRAME, &DRAW],
        ),
        // Over everything, against no depth.
        PIPELINE_CAMERA_GRID => Descriptor::new(
            Head {
                depth_compare: ALWAYS,
                ..blended(SHADER_CAMERA_GRID, CULL_NONE)
            },
            &[],
            &[&[[0, UNIFORM], [19, UNIFORM]]],
        ),
        PIPELINE_SCREEN_RAY => Descriptor::new(
            Head {
                depth_write: true,
                blend: OPAQUE,
                ..blended(SHADER_SCREEN_RAY, CULL_NONE)
            },
            &[],
            &[&SCREEN_GROUP],
        ),
        PIPELINE_DECOR_PREPASS => Descriptor::new(
            prepass(SHADER_DECOR, CULL_NONE),
            &MODEL_LANES,
            &MODEL_GROUPS,
        ),
        PIPELINE_DECOR_DEPTH => {
            Descriptor::new(depth_only(SHADER_DECOR), &MODEL_LANES, &DEPTH_GROUPS)
        }
        PIPELINE_SKINNED_DEPTH => {
            Descriptor::new(depth_only(SHADER_SKINNED), &SKINNED_LANES, &DEPTH_GROUPS)
        }
        // Its vertices as the depth pass lays them, its fragments writing
        // no color and testing no depth: they mark their tiles.
        PIPELINE_SKINNED_MARK => Descriptor::new(
            Head {
                fragment: true,
                depth_write: false,
                depth_compare: ALWAYS,
                depth_bias_slope: 0,
                ..depth_only(SHADER_SKINNED_MARK)
            },
            &SKINNED_LANES,
            &MARK_GROUPS,
        ),
        // Every texel of the box the world's cast holds, whatever lies
        // there, in the map's claimed tiles: its clipmap's words and the
        // box's corner, before the casters drawn over it, into the depth
        // alone (gpu/api.rs `pipeline`), its fragment the depth.
        PIPELINE_LIGHT_MAP => Descriptor::new(
            Head {
                fragment: true,
                depth_compare: ALWAYS,
                depth_bias_slope: 0,
                vertex: VS,
                ..depth_only(SHADER_LIGHT_MAP)
            },
            &[],
            &[&[
                [0, TEXTURE_UINT_ARRAY],
                [1, UNIFORM],
                [30, STORAGE],
                [31, UNIFORM],
                [32, STORAGE],
            ]],
        ),
        PIPELINE_LIGHT_GATE => Descriptor::new(
            Head {
                depth_compare: ALWAYS,
                depth_bias_slope: 0,
                vertex: VS,
                ..depth_only(SHADER_LIGHT_GATE)
            },
            &[],
            &[&[[30, STORAGE], [31, UNIFORM], [32, STORAGE]]],
        ),
        // The screen's depth alone into the occlusion's prepass (gpu/ao.rs),
        // the group the lit screen pass binds.
        PIPELINE_SCREEN_PREPASS => Descriptor::new(
            Head {
                fragment: true,
                vertex: VS,
                ..prepass(SHADER_SCREEN_DEPTH, CULL_NONE)
            },
            &[],
            &[&SCREEN_GROUP],
        ),
        // Every box's faces turned to the eye, the nearest a pixel's ray
        // enters kept, reversed: its sample's number into a target of its
        // own (gpu/ray/raster.rs). The vertex stage draws only the faces
        // turned to the eye, so none is culled by its winding.
        PIPELINE_RASTER => Descriptor::new(
            Head {
                depth_write: true,
                depth_compare: GREATER,
                blend: OPAQUE,
                ..blended(SHADER_RASTER, CULL_NONE)
            },
            &[],
            &[&[[0, UNIFORM], [1, STORAGE]]],
        ),
        // Only where the opaque world left the far plane, so covered pixels never run it.
        PIPELINE_SKY => Descriptor::new(
            Head {
                depth_compare: GREATER_EQUAL,
                blend: OPAQUE,
                ..blended(SHADER_SKY, CULL_NONE)
            },
            &[],
            &[&SKY_GROUP],
        ),
        PIPELINE_MIP => Descriptor::new(screen(SHADER_MIP), &[], &[&SKIN]),
        // The world's depth carried to the canvas's size, which what is drawn
        // over the world is tested against.
        PIPELINE_SCREEN_FINISH => Descriptor::new(
            Head {
                depth_write: true,
                depth_compare: ALWAYS,
                ..screen(SHADER_SCREEN_FINISH)
            },
            &[],
            &[&[
                [0, UNIFORM],
                [1, TEXTURE],
                [2, DEPTH],
                [30, STORAGE],
                [31, UNIFORM],
            ]],
        ),
        PIPELINE_SCREEN_GATE => Descriptor::new(
            Head {
                depth_write: true,
                depth_compare: ALWAYS,
                ..screen(SHADER_SCREEN_GATE)
            },
            &[],
            &[&[[30, STORAGE], [31, UNIFORM]]],
        ),
        PIPELINE_RASTER_GATE => Descriptor::new(
            Head {
                depth_write: true,
                depth_compare: ALWAYS,
                ..screen(SHADER_RASTER_GATE)
            },
            &[],
            &[&[[30, STORAGE], [31, UNIFORM]]],
        ),
        PIPELINE_SCREEN_SHOW => {
            Descriptor::new(screen(SHADER_SCREEN_SHOW), &[], &[&[[1, TEXTURE]]])
        }
        PIPELINE_SCREEN_RESOLVE => Descriptor::new(
            screen(SHADER_SCREEN_RESOLVE),
            &[],
            &[&[
                [0, UNIFORM],
                [1, TEXTURE],
                [2, DEPTH],
                [3, TEXTURE],
                [4, SAMPLER],
                [5, UNIFORM],
                [30, STORAGE],
                [31, UNIFORM],
            ]],
        ),
        PIPELINE_SCREEN_WRITE_BACK => Descriptor::new(
            screen(SHADER_SCREEN_WRITE_BACK),
            &[],
            &[&[[1, TEXTURE], [30, STORAGE], [31, UNIFORM]]],
        ),
        PIPELINE_IMPOSTOR => Descriptor::new(
            Head {
                topology: STRIP,
                ..lit(SHADER_IMPOSTOR, CULL_NONE)
            },
            &[],
            &[
                &LIT_FRAME,
                &DRAW,
                &[[0, TEXTURE_ARRAY], [1, SAMPLER], [2, UNIFORM]],
                &UPLOAD,
            ],
        ),
        PIPELINE_IMPOSTOR_BAKE => Descriptor::new(
            Head {
                fragment: true,
                vertex: VS,
                ..depth_only(SHADER_IMPOSTOR_BAKE)
            },
            &MODEL_LANES,
            &[&DRAW, &SKIN],
        ),
        PIPELINE_SHEET => Descriptor::new(
            Head {
                topology: STRIP,
                blend: ALPHA,
                ..screen(SHADER_SHEET)
            },
            &[],
            &[
                &[[0, UNIFORM], [1, STORAGE]],
                &[[0, TEXTURE_ARRAY], [1, SAMPLER]],
            ],
        ),
        PIPELINE_AO_DOWNSAMPLE => Descriptor::new(
            screen(SHADER_AO_DOWNSAMPLE),
            &[],
            &[&[[0, UNIFORM], [1, DEPTH], [6, UNIFORM]]],
        ),
        PIPELINE_AO_SAMPLE => Descriptor::new(
            screen(SHADER_AO_SAMPLE),
            &[],
            &[&[[1, TEXTURE_UNFILTERED], [2, TEXTURE], [6, UNIFORM]]],
        ),
        PIPELINE_AO_BLUR => Descriptor::new(
            screen(SHADER_AO_BLUR),
            &[],
            &[&[
                [1, TEXTURE_UNFILTERED],
                [2, TEXTURE],
                [3, TEXTURE],
                [4, SAMPLER],
                [5, UNIFORM_DYNAMIC],
                [6, UNIFORM],
            ]],
        ),
        PIPELINE_AO_UPSAMPLE => Descriptor::new(
            screen(SHADER_AO_UPSAMPLE),
            &[],
            &[&[
                [0, UNIFORM],
                [1, DEPTH],
                [2, TEXTURE_UNFILTERED],
                [3, TEXTURE],
                [4, SAMPLER],
                [6, UNIFORM],
            ]],
        ),
        PIPELINE_ATMOS_TRANSMITTANCE => {
            Descriptor::new(screen(SHADER_ATMOS_TRANSMITTANCE), &[], &[&[AIR[0]]])
        }
        PIPELINE_ATMOS_MULTISCATTER => Descriptor::new(
            screen(SHADER_ATMOS_MULTISCATTER),
            &[],
            &[&[AIR[0], AIR[1], AIR[3]]],
        ),
        PIPELINE_ATMOS_SKY_VIEW => Descriptor::new(
            screen(SHADER_ATMOS_SKY_VIEW),
            &[],
            &[&[
                [0, UNIFORM],
                AIR[0],
                AIR[1],
                AIR[3],
                AIR[8],
                [12, TEXTURE],
                [14, UNIFORM],
                [20, STORAGE_RW],
            ]],
        ),
        PIPELINE_ATMOS_IRRADIANCE => Descriptor::new(
            screen(SHADER_ATMOS_IRRADIANCE),
            &[],
            &[&[AIR[0], AIR[1], AIR[3], [12, TEXTURE]]],
        ),
        // The light on the land under the eye: the air's words and the tables it is lit by.
        PIPELINE_ATMOS_EXPOSURE => Descriptor::new(
            screen(SHADER_ATMOS_EXPOSURE),
            &[],
            &[&[
                [0, UNIFORM],
                AIR[0],
                AIR[1],
                AIR[2],
                AIR[3],
                AIR[5],
                AIR[6],
                AIR[7],
                AIR[8],
                AIR[9],
            ]],
        ),
        PIPELINE_STAR => Descriptor::new(
            Head {
                topology: STRIP,
                depth_compare: GREATER_EQUAL,
                blend: ADD,
                ..blended(SHADER_STAR, CULL_NONE)
            },
            &[],
            &[
                &[[0, UNIFORM], [8, UNIFORM], [9, TEXTURE], [11, SAMPLER]],
                &[[0, UNIFORM], [1, STORAGE]],
            ],
        ),
        // The sheets' planes, against the world's depth: what stands before them hides them.
        PIPELINE_CLOUD => Descriptor::new(blended(SHADER_CLOUD, CULL_NONE), &[], &[&CLOUD_GROUP]),
        PIPELINE_CLOUD_FIELD => Descriptor::new(screen(SHADER_CLOUD_FIELD), &[], &[]),
        _ => panic!("a pipeline number with no descriptor"),
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

/// The stages binding `binding` of `group` is seen from: those that read
/// it in every shader whose pipeline lays out the same group, wherever, so
/// the one layout those pipelines share holds for each, and from no stage
/// written by hand.
const fn seen(group: Group, binding: u32) -> u32 {
    let mut stages = NONE;
    let mut id = 0;
    while id < PIPELINES {
        let desc = desc_of(id);
        let mut k = 0;
        while k < desc.groups.len() {
            if alike(desc.groups[k], group) {
                stages |= stages_of(desc.head.shader, k as u32, binding);
            }
            k += 1;
        }
        id += 1;
    }
    stages
}

/// A descriptor's record's words (see Record words above).
const fn words(desc: &Descriptor) -> usize {
    let mut n = HEAD as usize + 1 + 1;
    let mut k = 0;
    while k < desc.lanes.len() {
        n += 3 + 3 * desc.lanes[k].attributes.len();
        k += 1;
    }
    k = 0;
    while k < desc.groups.len() {
        n += 1 + 3 * desc.groups[k].len();
        k += 1;
    }
    n
}

/// The most words a record has.
const RECORD_WORDS: usize = {
    let (mut most, mut id) = (0, 0);
    while id < PIPELINES {
        let n = words(&desc_of(id));
        most = if n > most { n } else { most };
        id += 1;
    }
    most
};

/// Pipeline `id`'s record and its length, each group entry seen from the
/// stages that read it (`seen`).
const fn record(id: u32) -> ([u32; RECORD_WORDS], usize) {
    let mut out = [0; RECORD_WORDS];
    let desc = desc_of(id);
    let h = desc.head;
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
    out[n] = desc.lanes.len() as u32;
    n += 1;
    let mut k = 0;
    while k < desc.lanes.len() {
        let l = desc.lanes[k];
        (out[n], out[n + 1], out[n + 2]) = (l.stride, l.step, l.attributes.len() as u32);
        n += 3;
        let mut a = 0;
        while a < l.attributes.len() {
            (out[n], out[n + 1], out[n + 2]) =
                (l.attributes[a][0], l.attributes[a][1], l.attributes[a][2]);
            (n, a) = (n + 3, a + 1);
        }
        k += 1;
    }
    out[n] = desc.groups.len() as u32;
    n += 1;
    k = 0;
    while k < desc.groups.len() {
        let g = desc.groups[k];
        out[n] = g.len() as u32;
        n += 1;
        let mut e = 0;
        while e < g.len() {
            (out[n], out[n + 1], out[n + 2]) = (g[e][0], seen(g, g[e][0]), g[e][1]);
            (n, e) = (n + 3, e + 1);
        }
        k += 1;
    }
    (out, n)
}

/// Every pipeline's record, worked out as the crate is built.
#[expect(
    clippy::large_const_arrays,
    reason = "`is_floored` reads it as the crate is built, which a static is not"
)]
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
const fn is_floored(id: u32) -> bool {
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
/// over every id, so a descriptor added is checked with no list to grow.
const _: () = {
    let mut id = 0;
    while id < PIPELINES {
        assert!(
            is_floored(id),
            "a pipeline's layout binds past WebGPU's floor (frame/shader/floor.rs)"
        );
        id += 1;
    }
};
