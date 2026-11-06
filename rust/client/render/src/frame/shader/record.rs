// pxl-render/frame/shader/record.rs

//! Shaders and their pipeline records (vertex buffers, bind groups,
//! primitive, depth and blend state) as one source the host turns into WebGPU
//! or wgpu descriptors. build.rs assembles and validates every shader
//! (part.rs), and reads off it the stages each pipeline's groups are seen
//! from.

pub use crate::frame::shader::part::*;

/// Each compute pipeline the renderer makes, by its number (`compute`).
pub const COMPUTE_ATMOS_EXPOSURE: u32 = 0;
pub const COMPUTE_CAST: u32 = 1;
pub const COMPUTE_BOUND: u32 = 2;
pub const COMPUTE_HEIGHT: u32 = 3;
pub const COMPUTE_GENERATION: u32 = 4;
pub const COMPUTE_GENERATION_CNT: u32 = 5;
pub const COMPUTE_GENERATION_ARGS: u32 = 6;
pub const COMPUTE_GENERATION_COMPACT: u32 = 7;
pub const COMPUTE_WAY: u32 = 8;
pub const COMPUTE_GENERATION_MARK: u32 = 9;
pub const COMPUTE_FOLD: u32 = 10;
pub const COMPUTE_ATMOS_FROXEL: u32 = 11;
pub const COMPUTE_WAY_FREE: u32 = 12;
pub const COMPUTE_WAY_TREE: u32 = 13;
pub const COMPUTE_WAY_RETURN: u32 = 14;
pub const COMPUTE_BEAM: u32 = 15;
pub const COMPUTE_CELL: u32 = 16;
/// The drawn receivers' workgroups, a thread (`SHADER_LIGHT_MARK_DRAWS`).
pub const COMPUTE_LIGHT_MARK_DRAWS_OPEN: u32 = 17;
/// The holes' queue (pxl-engine `ray::QUEUE_LANES`): the asks of the nodes its first and its
/// second walk over the node cache were parked at, its last walk, which asks the world, and
/// those two walks.
pub const COMPUTE_QUEUE_ASK: u32 = 18;
pub const COMPUTE_FORGET: u32 = 19;
pub const COMPUTE_SAMPLE: u32 = 20;
pub const COMPUTE_LIGHT: u32 = 21;
pub const COMPUTE_QUEUE_ASK_PARKED: u32 = 22;
pub const COMPUTE_SAMPLE_BOX: u32 = 23;
pub const COMPUTE_FACE: u32 = 24;
/// The pixels the eye's cast walked apart, stood on off their list (pxl-engine
/// `ray::WALKED_LIST`; light_mark.wgsl).
pub const COMPUTE_LIGHT_MARK_WALKED: u32 = 25;
pub const COMPUTE_LIGHT_MARK_DRAWS: u32 = 26;
pub const COMPUTE_LIGHT_PROPAGATE: u32 = 27;
pub const COMPUTE_GENERATION_THIN: u32 = 28;
pub const COMPUTE_SCREEN_PROPAGATE: u32 = 29;
pub const COMPUTE_SCREEN_MARK: u32 = 30;
pub const COMPUTE_SCREEN_BOUND: u32 = 31;
pub const COMPUTE_LIGHT_SLIDE: u32 = 32;
pub const COMPUTE_QUEUE_LAST: u32 = 33;
/// What lies about each hole's stop's face put at its pixel off the record
/// (`SHADER_RECORD_STOPS`).
pub const COMPUTE_RECORD_FACES: u32 = 34;
pub const COMPUTE_BOX_ARGS: u32 = 35;
pub const COMPUTE_LIGHT_BEAM: u32 = 36;
pub const COMPUTE_LIGHT_CELL: u32 = 37;
pub const COMPUTE_LIGHT_SAMPLE: u32 = 38;
pub const COMPUTE_LIGHT_SAMPLE_BOX: u32 = 39;
pub const COMPUTE_SAMPLE_HIT: u32 = 40;
pub const COMPUTE_QUEUE: u32 = 41;
pub const COMPUTE_BEAM_HIT: u32 = 42;
pub const COMPUTE_QUEUE_PARKED: u32 = 43;
pub const COMPUTE_LIGHT_SAMPLE_HIT: u32 = 44;
pub const COMPUTE_LIGHT_BEAM_HIT: u32 = 45;
pub const COMPUTE_CAST_TILES: u32 = 46;
pub const COMPUTE_CLAIMED_BOX: u32 = 47;
pub const COMPUTE_DECOR_GATE_ARGS: u32 = 48;
pub const COMPUTE_DECOR_GATE: u32 = 49;
/// The node cache's drawable entries drawn as the raster's boxes (record_draw.wgsl).
pub const COMPUTE_RECORD_DRAW: u32 = 50;
/// The missed pixels' hits into the node cache's drawable entries (record_stops.wgsl).
pub const COMPUTE_RECORD_STOPS: u32 = 51;
/// The tiles the light's cast wants claimed nearest first (light_propagate.wgsl
/// `light_claim_cs`), from the module that lists them.
pub const COMPUTE_LIGHT_CLAIM: u32 = 52;
/// The node cache's drawable entries drawn as the light's raster's boxes (record_draw.wgsl).
pub const COMPUTE_LIGHT_RECORD_DRAW: u32 = 53;
/// The screen's tiles that hold a parked pixel set changed (parked_tiles.wgsl).
pub const COMPUTE_PARKED_TILES: u32 = 54;
/// The light's holes' stops into the node cache's drawable entries (record_stops.wgsl over
/// record_stops.light.wgsl).
pub const COMPUTE_LIGHT_RECORD_STOPS: u32 = 55;
/// Each tile the light's cast lists read, wanted where its epoch has not cast it or it is owed
/// (light_propagate.wgsl `light_want_cs`).
pub const COMPUTE_LIGHT_WANT: u32 = 56;
pub const COMPUTES: u32 = 57;

/// Compute pipeline `id`'s shader and the entry point it runs, as the
/// source names it.
pub const fn compute(id: u32) -> (u32, &'static str) {
    match id {
        COMPUTE_ATMOS_EXPOSURE => (SHADER_ATMOS_EXPOSURE, "exposure_cs"),
        COMPUTE_CAST => (SHADER_CAST, "cast_cs"),
        COMPUTE_CAST_TILES => (SHADER_CAST_TILES, "cast_tiles_cs"),
        COMPUTE_CLAIMED_BOX => (SHADER_CLAIMED_BOX, "claimed_box_cs"),
        COMPUTE_DECOR_GATE_ARGS => (SHADER_DECOR_GATE, "decor_gate_args_cs"),
        COMPUTE_DECOR_GATE => (SHADER_DECOR_GATE, "decor_gate_cs"),
        COMPUTE_BOUND => (SHADER_BOUND, "bound_cs"),
        COMPUTE_HEIGHT => (SHADER_HEIGHT, "height_cs"),
        COMPUTE_GENERATION => (SHADER_GENERATION, "decor_cs"),
        COMPUTE_GENERATION_CNT => (SHADER_GENERATION_CNT, "decor_cnt_cs"),
        COMPUTE_GENERATION_ARGS => (SHADER_GENERATION_ARGS, "decor_args_cs"),
        COMPUTE_GENERATION_COMPACT => (SHADER_GENERATION_COMPACT, "decor_compact_cs"),
        COMPUTE_WAY => (SHADER_WAY, "way_cs"),
        COMPUTE_GENERATION_MARK => (SHADER_GENERATION_MARK, "decor_mark_cs"),
        COMPUTE_FOLD => (SHADER_FOLD, "fold_cs"),
        COMPUTE_ATMOS_FROXEL => (SHADER_ATMOS_FROXEL, "froxel_cs"),
        COMPUTE_WAY_FREE => (SHADER_WAY_FREE, "way_free_cs"),
        COMPUTE_WAY_TREE => (SHADER_WAY_TREE, "way_tree_cs"),
        COMPUTE_WAY_RETURN => (SHADER_WAY_RETURN, "way_return_cs"),
        COMPUTE_BEAM => (SHADER_BEAM, "beam_cs"),
        COMPUTE_CELL => (SHADER_CELL, "cell_cs"),
        COMPUTE_QUEUE_ASK => (SHADER_QUEUE_ASK, "queue_ask_cs"),
        COMPUTE_FORGET => (SHADER_FORGET, "forget_cs"),
        COMPUTE_SAMPLE => (SHADER_SAMPLE, "sample_cs"),
        COMPUTE_LIGHT => (SHADER_LIGHT, "light_cs"),
        COMPUTE_QUEUE_ASK_PARKED => (SHADER_QUEUE_ASK, "queue_ask_parked_cs"),
        COMPUTE_SAMPLE_BOX => (SHADER_SAMPLE_BOX, "sample_box_cs"),
        COMPUTE_FACE => (SHADER_FACE, "face_cs"),
        COMPUTE_LIGHT_MARK_WALKED => (SHADER_LIGHT_MARK, "light_mark_walked_cs"),
        COMPUTE_LIGHT_MARK_DRAWS => (SHADER_LIGHT_MARK_DRAWS, "light_mark_draws_cs"),
        COMPUTE_LIGHT_MARK_DRAWS_OPEN => (SHADER_LIGHT_MARK_DRAWS_OPEN, "light_mark_draws_open_cs"),
        COMPUTE_LIGHT_PROPAGATE => (SHADER_LIGHT_PROPAGATE, "light_propagate_cs"),
        COMPUTE_LIGHT_CLAIM => (SHADER_LIGHT_CLAIM, "light_claim_cs"),
        COMPUTE_LIGHT_WANT => (SHADER_LIGHT_PROPAGATE, "light_want_cs"),
        COMPUTE_GENERATION_THIN => (SHADER_GENERATION_THIN, "decor_thin_cs"),
        COMPUTE_SCREEN_PROPAGATE => (SHADER_SCREEN_PROPAGATE, "screen_propagate_cs"),
        COMPUTE_SCREEN_MARK => (SHADER_SCREEN_MARK, "screen_mark_cs"),
        COMPUTE_SCREEN_BOUND => (SHADER_SCREEN_BOUND, "screen_bound_cs"),
        COMPUTE_LIGHT_SLIDE => (SHADER_LIGHT_SLIDE, "light_slide_cs"),
        COMPUTE_QUEUE_LAST => (SHADER_QUEUE_LAST, "queue_last_cs"),
        COMPUTE_RECORD_FACES => (SHADER_RECORD_STOPS, "record_faces_cs"),
        COMPUTE_BOX_ARGS => (SHADER_BOX_ARGS, "box_args_cs"),
        COMPUTE_LIGHT_BEAM => (SHADER_LIGHT_BEAM, "beam_cs"),
        COMPUTE_LIGHT_CELL => (SHADER_LIGHT_CELL, "light_cell_cs"),
        COMPUTE_LIGHT_SAMPLE => (SHADER_LIGHT_SAMPLE, "sample_cs"),
        COMPUTE_LIGHT_SAMPLE_BOX => (SHADER_LIGHT_SAMPLE_BOX, "sample_box_cs"),
        COMPUTE_SAMPLE_HIT => (SHADER_SAMPLE_HIT, "sample_hit_cs"),
        COMPUTE_QUEUE => (SHADER_QUEUE, "queue_cs"),
        COMPUTE_BEAM_HIT => (SHADER_BEAM_HIT, "beam_hit_cs"),
        COMPUTE_QUEUE_PARKED => (SHADER_QUEUE, "queue_parked_cs"),
        COMPUTE_LIGHT_SAMPLE_HIT => (SHADER_LIGHT_SAMPLE_HIT, "sample_hit_cs"),
        COMPUTE_LIGHT_BEAM_HIT => (SHADER_LIGHT_BEAM_HIT, "beam_hit_cs"),
        COMPUTE_RECORD_DRAW => (SHADER_RECORD_DRAW, "record_draw_cs"),
        COMPUTE_RECORD_STOPS => (SHADER_RECORD_STOPS, "record_stops_cs"),
        COMPUTE_LIGHT_RECORD_DRAW => (SHADER_RECORD_DRAW, "light_record_draw_cs"),
        COMPUTE_PARKED_TILES => (SHADER_PARKED_TILES, "parked_tiles_cs"),
        COMPUTE_LIGHT_RECORD_STOPS => (SHADER_LIGHT_RECORD_STOPS, "record_stops_cs"),
        _ => panic!("a compute pipeline number with no shader"),
    }
}

include!(concat!(env!("OUT_DIR"), "/sources.rs"));

/// The world's one query asked word for word, as tests/world.rs and Tint take it
/// (world_asked.wgsl), in no shader the page runs.
pub const WORLD_ASKED: &str = include_str!(concat!(
    env!("OUT_DIR"),
    "/shader/assembled/world_asked.wgsl"
));

/// Pipelines past draw.rs's, which a frame runs on its own.
pub const PIPELINE_SKY: u32 = 9;
pub const PIPELINE_MIP: u32 = 10;
/// The occlusion's half-resolution map laid over the world's color in the
/// claimed tiles (gpu/ao.rs `apply`).
pub const PIPELINE_AO_APPLY: u32 = 11;
pub const PIPELINE_AO_DOWNSAMPLE: u32 = 12;
pub const PIPELINE_AO_SAMPLE: u32 = 13;
pub const PIPELINE_AO_BLUR: u32 = 14;
pub const PIPELINE_ATMOS_TRANSMITTANCE: u32 = 15;
pub const PIPELINE_ATMOS_MULTISCATTER: u32 = 16;
pub const PIPELINE_ATMOS_SKY_VIEW: u32 = 17;
pub const PIPELINE_STAR: u32 = 18;
pub const PIPELINE_CLOUD: u32 = 19;
pub const PIPELINE_CLOUD_FIELD: u32 = 20;
pub const PIPELINE_ATLAS: u32 = 21;
pub const PIPELINE_IMPOSTOR_BAKE: u32 = 22;
pub const PIPELINE_ATMOS_IRRADIANCE: u32 = 23;
/// The markers' regions, numbered, into a target of their own (pass/marker.rs).
pub const PIPELINE_MARKER_REGION: u32 = 24;
/// The debug gizmos' faces, depth only, into the gizmos' own depth (gpu/pass/debug.rs `DebugPass`).
pub const PIPELINE_DEBUG_FACE: u32 = 25;
/// The eye's rays, opaque at their depth (pxl-engine ray.rs).
pub const PIPELINE_SCREEN_RAY: u32 = 26;
/// Decor's and the actors' depth from the light, and the world's laid in
/// before them, into the light's shadow map (gpu/shadow.rs).
pub const PIPELINE_DECOR_DEPTH: u32 = 27;
pub const PIPELINE_SKINNED_DEPTH: u32 = 28;
pub const PIPELINE_LIGHT_MAP: u32 = 29;
/// The samples' boxes, the nearest face a pixel's ray enters kept with its
/// sample (gpu/ray/raster.rs).
pub const PIPELINE_RASTER: u32 = 30;
/// The world's color onto the canvas, encoded once, and its depth carried to
/// the canvas's size (gpu/finish.rs).
pub const PIPELINE_SCREEN_FINISH: u32 = 31;
/// The world's color gathered over frames (gpu/resolve.rs).
pub const PIPELINE_SCREEN_RESOLVE: u32 = 32;
/// The gate a frame not drawn whole is drawn through: the claimed tiles
/// cleared, their stencil raised (gpu/renderer.rs `frame`).
pub const PIPELINE_SCREEN_GATE: u32 = 33;
/// The finished picture kept at the canvas's size shown on the canvas
/// (gpu/finish.rs).
pub const PIPELINE_SCREEN_SHOW: u32 = 34;
/// The claimed tiles of the history the resolve wrote copied back into the
/// last (gpu/resolve.rs).
pub const PIPELINE_SCREEN_WRITE_BACK: u32 = 35;
/// The gate the samples' raster is drawn through in a frame not drawn whole:
/// the claimed tiles cleared to the far plane, the rest left at the nearest
/// (gpu/ray/raster.rs).
pub const PIPELINE_RASTER_GATE: u32 = 36;
/// The gate the light's map is drawn through where only the listed
/// casters that changed moved: the world's depth laid in the claimed tiles,
/// their stencil raised (gpu/shadow.rs).
pub const PIPELINE_LIGHT_GATE: u32 = 37;
/// An actor drawn where it was and where it is, its fragments setting their
/// screen tiles' words (gpu/pass/skinned_mesh.rs `draw_marks`).
pub const PIPELINE_SKINNED_MARK: u32 = 38;
/// The same of decor, an impostor, the location indicator's zone and its beacon
/// (gpu/pass/mark.rs `Marks`).
pub const PIPELINE_DECOR_MARK: u32 = 39;
pub const PIPELINE_IMPOSTOR_MARK: u32 = 40;
pub const PIPELINE_LOCATION_ZONE_MARK: u32 = 41;
pub const PIPELINE_LOCATION_BEACON_MARK: u32 = 42;
pub const PIPELINES: u32 = 43;

// Record words: shader, has fragment, topology, cull, depth write, depth
// compare, bias (i32), slope scale (f32 bits), blend, vertex entry; then vertex
// buffers (count; stride, step, attribute count, (location, offset, format)
// each); then bind groups (count; entry count, (binding, visibility, kind) each).
pub use crate::api_spec::record::*;
/// A binding's stages are xpute's own bits (SHADER_STAGE), none or some.
pub use xpute_gpu::spec::SHADER_STAGE::{COMPUTE, FRAGMENT, VERTEX};
pub const NONE: u32 = 0;

/// `group` with its uniform at `binding` taken at a dynamic offset: how the
/// CPU binds it, which the shader does not say.
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
    PIPELINE_DEBUG, PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_DECOR, PIPELINE_IMPOSTOR,
    PIPELINE_LOCATION_BEACON, PIPELINE_LOCATION_ZONE, PIPELINE_MARKER, PIPELINE_SKINNED,
};
use crate::frame::shader::floor;
use pxl_engine::param::light;

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

/// The shadow map's casters' alike, raster and cast (pxl-engine `param::light::SLOPE_BIAS`).
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

/// A changed draw's mark (tile_mark.wgsl): its vertices as its depth pass
/// lays them, both sides, its fragments writing no color and testing no
/// depth.
const fn mark(shader: u32) -> Head {
    Head {
        fragment: true,
        depth_write: false,
        depth_compare: ALWAYS,
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

/// The air's bindings, numbered alike in every pass that reads them: those every painter and
/// the sky read (gpu/atmosphere.rs `entries`).
const AIR: [[u32; 2]; 6] = [
    [8, UNIFORM],
    [9, TEXTURE],
    [11, SAMPLER],
    [13, TEXTURE],
    [18, TEXTURE],
    // The air before each point, made of the rest (gpu/atmosphere.rs `froxel`).
    [22, TEXTURE_3D],
];
/// The air the sky alone reads: its views, the sun's and the moon's, and the Earth's picture
/// and tiles (gpu/atmosphere.rs `sky_entries`).
const SKY_AIR: [[u32; 2]; 4] = [
    [10, TEXTURE],
    [15, TEXTURE],
    [16, TEXTURE_ARRAY],
    [17, TEXTURE_ARRAY],
];
/// The multiple scattering's table, which the tables made after it read.
const MULTISCATTER: [u32; 2] = [12, TEXTURE];

/// The light's shadow map and its comparing sampler (shadow.wgsl), read where
/// a lit fragment is shaded.
const SHADOW: [[u32; 2]; 2] = [[1, DEPTH], [2, SAMPLER_COMPARISON]];
/// The frame, the shadow and the air a painter reads: the air in the vertex stage, where each
/// point's lights and veil are worked out; a fragment reads its words and the eye's texel,
/// whose white it shows by (atmos_eye.wgsl).
const LIT_FRAME: [[u32; 2]; 9] = and([[0, UNIFORM], SHADOW[0], SHADOW[1]], AIR);
/// The sheets' own words and tiles, and the air as every painter reads it (atmos_air.wgsl
/// `seen_from_eye`).
const CLOUD_GROUP: [[u32; 2]; 10] = and(
    [[0, UNIFORM], [1, UNIFORM], [2, TEXTURE], [3, SAMPLER]],
    AIR,
);
const SKIN: [[u32; 2]; 2] = [[0, TEXTURE], [1, SAMPLER]];

/// The lit pass's frame, shadow, air and layers, all read where
/// a pixel is painted; then the rays, the looks and the eye's words: the
/// most entries any group has (`GROUP_ENTRIES`).
const SCREEN_GROUP: [[u32; 2]; 17] = and(
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
struct VertexBufferLayout {
    stride: u32,
    step: u32,
    attributes: &'static [[u32; 3]],
}

const fn vertex(stride: u32, attributes: &'static [[u32; 3]]) -> VertexBufferLayout {
    VertexBufferLayout {
        stride,
        step: PER_VERTEX,
        attributes,
    }
}

const MODEL_BUFFERS: [VertexBufferLayout; 3] = [
    vertex(12, &[[0, 0, F32X3]]),
    vertex(12, &[[1, 0, F32X3]]),
    vertex(8, &[[2, 0, F32X2]]),
];
const SKINNED_BUFFERS: [VertexBufferLayout; 5] = [
    MODEL_BUFFERS[0],
    MODEL_BUFFERS[1],
    MODEL_BUFFERS[2],
    vertex(16, &[[3, 0, U32X4]]),
    vertex(16, &[[4, 0, F32X4]]),
];
/// A box's words, as the box shader (box.wgsl) takes one and the hover
/// ghost writes it (mesh/ghost.rs `write_lone_box`): a vec4, then its layer.
pub const BOX_WORDS: u32 = 5;
/// The boxes are a per-instance vertex buffer, so a draw reaches its boxes by
/// its first instance.
const BOX_BUFFERS: [VertexBufferLayout; 2] = [
    VertexBufferLayout {
        stride: BOX_WORDS * 4,
        step: PER_INSTANCE,
        attributes: &[[0, 0, U32X4]],
    },
    VertexBufferLayout {
        stride: BOX_WORDS * 4,
        step: PER_INSTANCE,
        attributes: &[[1, 16, U32]],
    },
];
const LOCATION_ZONE_BUFFERS: [VertexBufferLayout; 2] =
    [vertex(12, &[[0, 0, F32X3]]), vertex(8, &[[1, 0, F32X2]])];
const LOCATION_BEACON_BUFFERS: [VertexBufferLayout; 2] =
    [vertex(12, &[[0, 0, F32X3]]), vertex(12, &[[1, 0, F32X3]])];
/// A debug line's vertex, its place and its color, a segment's two read as
/// one instance; a debug face's vertex, its place.
pub const DEBUG_VERTEX_BYTES: u32 = 28;
pub const DEBUG_FACE_VERTEX_BYTES: u32 = 12;
const DEBUG_BUFFERS: [VertexBufferLayout; 1] = [VertexBufferLayout {
    stride: 2 * DEBUG_VERTEX_BYTES,
    step: PER_INSTANCE,
    attributes: &[
        [0, 0, F32X3],
        [1, 12, F32X4],
        [2, DEBUG_VERTEX_BYTES, F32X3],
    ],
}];
const DEBUG_FACE_BUFFERS: [VertexBufferLayout; 1] =
    [vertex(DEBUG_FACE_VERTEX_BYTES, &[[0, 0, F32X3]])];

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
const BOX_GROUP: [[u32; 2]; 13] = and(
    LIT_FRAME,
    [[3, STORAGE], [4, TEXTURE_ARRAY], [5, STORAGE], [6, SAMPLER]],
);
/// A draw's uniform, at a dynamic offset (gpu/pass/model.rs `DrawUniforms`),
/// and its upload: one layout each wherever a draw binds them, so a group
/// one owner makes binds in every pipeline that names them (gpu/generator.rs,
/// gpu/upload.rs).
const DRAW: [[u32; 2]; 1] = [[0, UNIFORM_DYNAMIC]];
/// An upload's words, and the list a draw reads its instances in order by, the words
/// themselves but in a frame through the gate (instance.wgsl `order`).
const UPLOAD: [[u32; 2]; 2] = [[0, STORAGE], [1, STORAGE]];
/// An upload's group as every pipeline binding one lays it out, which the
/// draw list's uploads are grouped in (gpu/upload.rs `Uploads::group`).
pub const UPLOAD_LAYOUT: [u32; 6] = [
    UPLOAD[0][0],
    stages_reading(&UPLOAD, UPLOAD[0][0]),
    UPLOAD[0][1],
    UPLOAD[1][0],
    stages_reading(&UPLOAD, UPLOAD[1][0]),
    UPLOAD[1][1],
];

const MODEL_GROUPS: [Group; 4] = [&LIT_FRAME, &DRAW, &SKIN, &UPLOAD];
/// A model's depth from the light: the light's frame alone in group 0, where
/// its place is read; the rest as the lit pass binds them.
const DEPTH_GROUPS: [Group; 4] = [FRAME, MODEL_GROUPS[1], MODEL_GROUPS[2], MODEL_GROUPS[3]];
/// The list's uniforms by slot, read only, which an actor's draw and the
/// beacon's read at the slot their first instance names (skinned_draw.slot.wgsl,
/// location.slot.wgsl): one group a pass (gpu/pass/skinned_mesh.rs
/// `SkinnedPass::set_draws`, gpu/pass/location.rs `LocationPass::set_draws`).
const DRAWS: [[u32; 2]; 1] = [[0, STORAGE]];
/// An actor's, lit and its depth from the light, as a model's but its draw
/// read off the list's uniforms (`DRAWS`).
const SKINNED_GROUPS: [Group; 4] = [&LIT_FRAME, &DRAWS, &SKIN, &UPLOAD];
const SKINNED_DEPTH_GROUPS: [Group; 4] = [FRAME, &DRAWS, &SKIN, &UPLOAD];
/// A draw's mark (tile_mark.wgsl): the eye's frame or the light's, the
/// tiles' words, their uniform and the box of the tiles the frame's marks
/// cover in group 0; the rest as its depth reads them.
const MARK_FRAME: Group = &[
    [0, UNIFORM],
    [30, STORAGE_RW],
    [31, UNIFORM],
    [32, STORAGE_RW],
];
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
const SKY_GROUP: [[u32; 2]; 14] = and(
    and::<4, 4, 8>(
        [
            [0, UNIFORM],
            [19, UNIFORM],
            [2, TEXTURE_ARRAY],
            [3, SAMPLER],
        ],
        SKY_AIR,
    ),
    AIR,
);

/// A pipeline's record before its groups' stages: its head, its vertex buffers and
/// each group's entries (binding, kind), which `record` sees from the
/// stages that read them.
#[derive(Clone, Copy)]
struct Descriptor {
    head: Head,
    buffers: &'static [VertexBufferLayout],
    groups: &'static [Group],
}

impl Descriptor {
    const fn new(
        head: Head,
        buffers: &'static [VertexBufferLayout],
        groups: &'static [Group],
    ) -> Self {
        Self {
            head,
            buffers,
            groups,
        }
    }
}

/// Pipeline `id`'s descriptor: every id below `PIPELINES` has one, which the
/// records, worked out as the crate is built, ask of each.
const fn desc_of(id: u32) -> Descriptor {
    match id {
        PIPELINE_DECOR => {
            Descriptor::new(lit(SHADER_DECOR, CULL_NONE), &MODEL_BUFFERS, &MODEL_GROUPS)
        }
        // What is laid on the world's faces draws at its own depth: the
        // world's stands behind it (screen_ray.wgsl `behind`).
        PIPELINE_GHOST_BOX => {
            Descriptor::new(blended(SHADER_BOX, CULL_BACK), &BOX_BUFFERS, &[&BOX_GROUP])
        }
        PIPELINE_GHOST_DECOR => Descriptor::new(
            blended(SHADER_DECOR, CULL_NONE),
            &MODEL_BUFFERS,
            &MODEL_GROUPS,
        ),
        // Back faces culled: drawn both sides, a closed body's inner faces fight its
        // outer ones at a distance and the character shimmers.
        PIPELINE_SKINNED => Descriptor::new(
            lit(SHADER_SKINNED, CULL_BACK),
            &SKINNED_BUFFERS,
            &SKINNED_GROUPS,
        ),
        PIPELINE_LOCATION_ZONE => Descriptor::new(
            blended(SHADER_LOCATION_ZONE, CULL_NONE),
            &LOCATION_ZONE_BUFFERS,
            &[FRAME, &DRAW, &UPLOAD],
        ),
        PIPELINE_LOCATION_BEACON => Descriptor::new(
            lit(SHADER_LOCATION_BEACON, CULL_BACK),
            &LOCATION_BEACON_BUFFERS,
            &[&LIT_FRAME, &DRAWS],
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
            &DEBUG_BUFFERS,
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
            &DEBUG_FACE_BUFFERS,
            &[FRAME, &DRAW],
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
        // Its half-resolution map brought to each pixel and multiplied into
        // the world's color, as the tiles' quads: black blended over by the
        // occlusion's darkening, a draw of the world's pass, its depth not
        // tested or written.
        PIPELINE_AO_APPLY => Descriptor::new(
            Head {
                blend: ALPHA,
                depth_compare: ALWAYS,
                ..screen(SHADER_AO_APPLY)
            },
            &[],
            &[&[
                [1, TEXTURE_UNFILTERED],
                [2, TEXTURE],
                [6, UNIFORM],
                [30, STORAGE],
                [31, UNIFORM],
            ]],
        ),
        PIPELINE_DECOR_DEPTH => {
            Descriptor::new(depth_only(SHADER_DECOR), &MODEL_BUFFERS, &DEPTH_GROUPS)
        }
        PIPELINE_SKINNED_DEPTH => Descriptor::new(
            depth_only(SHADER_SKINNED),
            &SKINNED_BUFFERS,
            &SKINNED_DEPTH_GROUPS,
        ),
        // Its vertices as the depth pass lays them, its fragments writing
        // no color and testing no depth: they mark their tiles.
        PIPELINE_SKINNED_MARK => {
            Descriptor::new(mark(SHADER_SKINNED_MARK), &SKINNED_BUFFERS, &MARK_GROUPS)
        }
        PIPELINE_DECOR_MARK => {
            Descriptor::new(mark(SHADER_DECOR_MARK), &MODEL_BUFFERS, &MARK_GROUPS)
        }
        PIPELINE_IMPOSTOR_MARK => Descriptor::new(
            Head {
                topology: STRIP,
                ..mark(SHADER_IMPOSTOR_MARK)
            },
            &[],
            &[
                MARK_FRAME,
                &DRAW,
                &[[0, TEXTURE_ARRAY], [1, SAMPLER], [2, UNIFORM]],
                &UPLOAD,
            ],
        ),
        PIPELINE_LOCATION_ZONE_MARK => Descriptor::new(
            mark(SHADER_LOCATION_ZONE_MARK),
            &LOCATION_ZONE_BUFFERS,
            &[MARK_FRAME, &DRAW, &UPLOAD],
        ),
        PIPELINE_LOCATION_BEACON_MARK => Descriptor::new(
            mark(SHADER_LOCATION_BEACON_MARK),
            &LOCATION_BEACON_BUFFERS,
            &[MARK_FRAME, &DRAW],
        ),
        // The texels of the box the world's cast holds in the tiles a receiver reads, a quad a
        // tile listed read: its clipmap's words, the box's corner, the tiles' reads and the
        // list of those read, before the casters drawn over it, into the depth alone (gpu/api.rs
        // `pipeline`), its fragment the depth.
        PIPELINE_LIGHT_MAP => Descriptor::new(
            Head {
                fragment: true,
                depth_compare: ALWAYS,
                depth_bias_slope: 0,
                vertex: VS,
                ..depth_only(SHADER_LIGHT_MAP)
            },
            &[],
            &[&[[0, TEXTURE_UINT_ARRAY], [1, UNIFORM], [2, STORAGE]]],
        ),
        // The same in the map's claimed tiles alone, a quad a tile of the
        // marks' box, raising their stencil (gpu/api.rs `Stencil::Gate`).
        PIPELINE_LIGHT_GATE => Descriptor::new(
            Head {
                fragment: true,
                depth_compare: ALWAYS,
                depth_bias_slope: 0,
                vertex: VS,
                ..depth_only(SHADER_LIGHT_GATE)
            },
            &[],
            &[&[
                [0, TEXTURE_UINT_ARRAY],
                [1, UNIFORM],
                [2, STORAGE],
                [30, STORAGE],
                [31, UNIFORM],
                [33, STORAGE],
            ]],
        ),
        // Every box's faces turned to the eye, the nearest a pixel's ray
        // enters kept, reversed: its voxel's number into a target of its
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
        // Only where the opaque world left the far plane.
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
            &MODEL_BUFFERS,
            &[&DRAW, &SKIN],
        ),
        PIPELINE_ATLAS => Descriptor::new(
            Head {
                topology: STRIP,
                blend: ALPHA,
                ..screen(SHADER_ATLAS)
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
            &[&[[0, UNIFORM], [1, DEPTH]]],
        ),
        PIPELINE_AO_SAMPLE => Descriptor::new(
            screen(SHADER_AO_SAMPLE),
            &[],
            &[&[
                [0, UNIFORM],
                [1, TEXTURE_UNFILTERED],
                [2, TEXTURE],
                [3, DEPTH],
                [6, UNIFORM],
            ]],
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
        PIPELINE_ATMOS_TRANSMITTANCE => {
            Descriptor::new(screen(SHADER_ATMOS_TRANSMITTANCE), &[], &[&[AIR[0]]])
        }
        PIPELINE_ATMOS_MULTISCATTER => Descriptor::new(
            screen(SHADER_ATMOS_MULTISCATTER),
            &[],
            &[&[AIR[0], AIR[1], AIR[2]]],
        ),
        PIPELINE_ATMOS_SKY_VIEW => Descriptor::new(
            screen(SHADER_ATMOS_SKY_VIEW),
            &[],
            &[&[
                AIR[0],
                AIR[1],
                AIR[2],
                AIR[4],
                MULTISCATTER,
                [14, UNIFORM],
                [20, STORAGE_RW],
            ]],
        ),
        PIPELINE_ATMOS_IRRADIANCE => Descriptor::new(
            screen(SHADER_ATMOS_IRRADIANCE),
            &[],
            &[&[AIR[0], AIR[1], AIR[2], MULTISCATTER]],
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
/// the one layout those pipelines share holds for each.
const fn stages_reading(group: Group, binding: u32) -> u32 {
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
    while k < desc.buffers.len() {
        n += 3 + 3 * desc.buffers[k].attributes.len();
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
/// stages that read it (`stages_reading`).
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
    out[n] = desc.buffers.len() as u32;
    n += 1;
    let mut k = 0;
    while k < desc.buffers.len() {
        let l = desc.buffers[k];
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
            (out[n], out[n + 1], out[n + 2]) = (g[e][0], stages_reading(g, g[e][0]), g[e][1]);
            (n, e) = (n + 3, e + 1);
        }
        k += 1;
    }
    (out, n)
}

/// Every pipeline's record, worked out as the crate is built.
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

/// Whether render pipeline `id`'s groups lay out a buffer as writable storage.
pub const fn lays_out_writable(id: u32) -> bool {
    let groups = desc_of(id).groups;
    let mut g = 0;
    while g < groups.len() {
        let mut e = 0;
        while e < groups[g].len() {
            if groups[g][e][1] == STORAGE_RW {
                return true;
            }
            e += 1;
        }
        g += 1;
    }
    false
}

/// Whether render pipeline `id`'s shader reads the light's shadow map: whether its parts hold
/// shadow.wgsl's (`PARTS`), its record's first word its shader. Whatever is drawn by one is a
/// receiver of the map, and stands (frame/draw.rs `DrawList`).
pub fn reads_shadow_map(id: u32) -> bool {
    PARTS[pipeline(id)[0] as usize].contains(&"shadow")
}

/// Whether pipeline `id`'s layout binds within what every WebGPU device
/// binds (`floor::holds`), its groups as its record lays them out.
const fn is_floored(id: u32) -> bool {
    let (words, _) = &RECORDS[id as usize];
    let mut at = HEAD as usize;
    let buffers = words[at];
    at += 1;
    let mut k = 0;
    while k < buffers {
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
/// over every id.
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

/// Whether pipeline `id`'s groups lay out exactly what its shader reads: every binding the
/// entry points it runs read (its vertex entry, `Head::vertex`, and `fs` where it has a
/// fragment stage) an entry in its group of a kind the binding's declaration takes (build.rs
/// `layout_kinds`); and every entry read by a pipeline that lays out the same group, as
/// build.rs `BINDS` holds a compute group.
const fn lays_out(id: u32) -> bool {
    let desc = desc_of(id);
    let entries = STAGES[desc.head.shader as usize];
    let mut k = 0;
    while k < entries.len() {
        let [g, b, by, takes, vertex] = entries[k];
        if vertex & 1 << desc.head.vertex != 0 || (desc.head.fragment && by & FRAGMENT != 0) {
            if g as usize >= desc.groups.len() {
                return false;
            }
            let group = desc.groups[g as usize];
            let mut found = false;
            let mut e = 0;
            while e < group.len() {
                found |= group[e][0] == b && takes & 1 << group[e][1] != 0;
                e += 1;
            }
            if !found {
                return false;
            }
        }
        k += 1;
    }
    let mut g = 0;
    while g < desc.groups.len() {
        let group = desc.groups[g];
        let mut e = 0;
        while e < group.len() {
            if stages_reading(group, group[e][0]) == NONE {
                return false;
            }
            e += 1;
        }
        g += 1;
    }
    true
}

/// Every pipeline's groups against what its shader reads, checked as the crate is built,
/// over every id.
const _: () = {
    let mut id = 0;
    while id < PIPELINES {
        assert!(
            lays_out(id),
            "a pipeline's groups lay out other than its shader reads (record.rs `lays_out`)"
        );
        id += 1;
    }
};
