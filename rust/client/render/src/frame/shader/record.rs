// pxl-render/frame/shader/record.rs

//! Every pipeline the renderer makes, stated once, which the host turns into WebGPU or wgpu
//! descriptors: each compute pipeline as its shader and entry point, and each render pipeline
//! as a record of its vertex buffers, bind groups, primitive, depth and blend state. build.rs
//! assembles and validates every shader (part.rs) and reads off it which stages read each
//! binding, so each group entry here is visible to the stages its shaders read it in and to
//! no other.

pub use crate::frame::shader::part::*;

/// Each compute pipeline the renderer makes, by its number (`compute`).
pub const COMPUTE_ATMOS_EXPOSURE: u32 = 0;
pub const COMPUTE_CAST: u32 = 1;
pub const COMPUTE_GENERATION: u32 = 2;
pub const COMPUTE_GENERATION_CNT: u32 = 3;
pub const COMPUTE_GENERATION_ARGS: u32 = 4;
pub const COMPUTE_GENERATION_COMPACT: u32 = 5;
pub const COMPUTE_GENERATION_MARK: u32 = 6;
pub const COMPUTE_ATMOS_FROXEL: u32 = 7;
/// The first walk over the node cache alone of the eye's beam rays (`SHADER_QUEUE_CORNERS`).
pub const COMPUTE_QUEUE_CORNERS: u32 = 8;
pub const COMPUTE_CELL: u32 = 9;
/// One thread writes the workgroup count of the drawn receivers' pass
/// (`SHADER_LIGHT_MARK_DRAWS`).
pub const COMPUTE_LIGHT_MARK_DRAWS_OPEN: u32 = 10;
/// The eye's holes' queue (pxl-engine `ray::QUEUE_LANES`) runs five compute pipelines: its first
/// and second walks over the node cache (`COMPUTE_QUEUE`, `COMPUTE_QUEUE_PARKED`), the asks of the
/// nodes each of those walks was parked at (`COMPUTE_QUEUE_ASK`, `COMPUTE_QUEUE_ASK_PARKED`),
/// and its last walk, which asks the world (`COMPUTE_QUEUE_LAST`). The two asks serve every
/// run's queue (ask_run.wgsl).
pub const COMPUTE_QUEUE_ASK: u32 = 11;
pub const COMPUTE_FORGET: u32 = 12;
/// The last walk of the eye's samples (`SHADER_QUEUE_LAST_SAMPLES`).
pub const COMPUTE_QUEUE_LAST_SAMPLES: u32 = 13;
pub const COMPUTE_LIGHT: u32 = 14;
pub const COMPUTE_QUEUE_ASK_PARKED: u32 = 15;
pub const COMPUTE_SAMPLE_HIT: u32 = 16;
pub const COMPUTE_FACE: u32 = 17;
/// Stands as receivers the pixels whose rays the holes' queue walked, found by their walked
/// bits (pxl-engine `ray::hole_bit`, light_mark.wgsl).
pub const COMPUTE_LIGHT_MARK_WALKED: u32 = 18;
pub const COMPUTE_LIGHT_MARK_DRAWS: u32 = 19;
pub const COMPUTE_LIGHT_PROPAGATE: u32 = 20;
pub const COMPUTE_GENERATION_THIN: u32 = 21;
pub const COMPUTE_SCREEN_PROPAGATE: u32 = 22;
pub const COMPUTE_SCREEN_MARK: u32 = 23;
pub const COMPUTE_SCREEN_BOUND: u32 = 24;
pub const COMPUTE_LIGHT_SLIDE: u32 = 25;
pub const COMPUTE_QUEUE_LAST: u32 = 26;
/// Puts at each hole's pixel what lies about the face its stop entered, read from the record
/// (`SHADER_REC_HITS`).
pub const COMPUTE_REC_FACES: u32 = 27;
pub const COMPUTE_RECT_ARGS: u32 = 28;
pub const COMPUTE_QUEUE_LIGHT_CORNERS: u32 = 29;
pub const COMPUTE_LIGHT_CELL: u32 = 30;
pub const COMPUTE_QUEUE_LAST_LIGHT_SAMPLES: u32 = 31;
pub const COMPUTE_LIGHT_SAMPLE_HIT: u32 = 32;
/// The first walk over the node cache alone of the eye's samples (`SHADER_QUEUE_SAMPLES`).
pub const COMPUTE_QUEUE_SAMPLES: u32 = 33;
pub const COMPUTE_QUEUE: u32 = 34;
pub const COMPUTE_QUEUE_LAST_CORNERS: u32 = 35;
pub const COMPUTE_QUEUE_PARKED: u32 = 36;
pub const COMPUTE_QUEUE_LIGHT_SAMPLES: u32 = 37;
pub const COMPUTE_QUEUE_LAST_LIGHT_CORNERS: u32 = 38;
pub const COMPUTE_CAST_TILES: u32 = 39;
pub const COMPUTE_DAMAGE_RECT: u32 = 40;
pub const COMPUTE_DECOR_CULL_ARGS: u32 = 41;
pub const COMPUTE_DECOR_CULL: u32 = 42;
/// Writes the node cache's drawable entries as the boxes of the eye's raster
/// (record_draw.wgsl).
pub const COMPUTE_REC_DRAW: u32 = 43;
/// Writes the stops of the eye's holes into the node cache's drawable entries
/// (record_hits.wgsl).
pub const COMPUTE_REC_HITS: u32 = 44;
/// Writes the node cache's drawable entries as the boxes of the light's raster
/// (record_draw.wgsl).
pub const COMPUTE_LIGHT_REC_DRAW: u32 = 45;
/// Writes the stops of the light's holes into the node cache's drawable entries
/// (record_hits.wgsl over record_hits.light.wgsl).
pub const COMPUTE_LIGHT_REC_HITS: u32 = 46;
/// Damages each tile the light's cast lists as read that has not been cast in its epoch
/// (light_propagate.wgsl `light_damage_cs`).
pub const COMPUTE_LIGHT_DAMAGE: u32 = 47;
/// The rest of the samples' runs, the eye's and the light's: the second walk over the node
/// cache alone (`SHADER_QUEUE_SAMPLES`).
pub const COMPUTE_QUEUE_SAMPLES_PARKED: u32 = 48;
pub const COMPUTE_QUEUE_LIGHT_SAMPLES_PARKED: u32 = 49;
/// The rest of the beam rays' runs, the eye's and the light's: the second walk over the node
/// cache alone (`SHADER_QUEUE_CORNERS`).
pub const COMPUTE_QUEUE_CORNERS_PARKED: u32 = 50;
pub const COMPUTE_QUEUE_LIGHT_CORNERS_PARKED: u32 = 51;
/// The holes' queue's walks for the light's map (`SHADER_QUEUE_LIGHT`,
/// `SHADER_QUEUE_LAST_LIGHT`), as `COMPUTE_QUEUE`, `COMPUTE_QUEUE_PARKED` and
/// `COMPUTE_QUEUE_LAST` are the eye's.
pub const COMPUTE_QUEUE_LIGHT: u32 = 52;
pub const COMPUTE_QUEUE_LIGHT_PARKED: u32 = 53;
pub const COMPUTE_QUEUE_LAST_LIGHT: u32 = 54;
pub const COMPUTES: u32 = 55;

/// Compute pipeline `id`'s shader and the entry point it runs, as the
/// source names it.
pub const fn compute(id: u32) -> (u32, &'static str) {
    match id {
        COMPUTE_ATMOS_EXPOSURE => (SHADER_ATMOS_EXPOSURE, "exposure_cs"),
        COMPUTE_CAST => (SHADER_CAST, "cast_cs"),
        COMPUTE_CAST_TILES => (SHADER_CAST_TILES, "cast_tiles_cs"),
        COMPUTE_DAMAGE_RECT => (SHADER_DAMAGE_RECT, "damage_rect_cs"),
        COMPUTE_DECOR_CULL_ARGS => (SHADER_DECOR_CULL, "decor_cull_args_cs"),
        COMPUTE_DECOR_CULL => (SHADER_DECOR_CULL, "decor_cull_cs"),
        COMPUTE_GENERATION => (SHADER_GENERATION, "decor_pose_cs"),
        COMPUTE_GENERATION_CNT => (SHADER_GENERATION_CNT, "decor_cnt_cs"),
        COMPUTE_GENERATION_ARGS => (SHADER_GENERATION_ARGS, "decor_args_cs"),
        COMPUTE_GENERATION_COMPACT => (SHADER_GENERATION_COMPACT, "decor_compact_cs"),
        COMPUTE_GENERATION_MARK => (SHADER_GENERATION_MARK, "decor_mark_cs"),
        COMPUTE_ATMOS_FROXEL => (SHADER_ATMOS_FROXEL, "froxel_cs"),
        COMPUTE_QUEUE_CORNERS => (SHADER_QUEUE_CORNERS, "queue_cs"),
        COMPUTE_QUEUE_CORNERS_PARKED => (SHADER_QUEUE_CORNERS, "queue_parked_cs"),
        COMPUTE_CELL => (SHADER_CELL, "cell_cs"),
        COMPUTE_QUEUE_ASK => (SHADER_QUEUE_ASK, "queue_ask_cs"),
        COMPUTE_FORGET => (SHADER_FORGET, "forget_cs"),
        COMPUTE_QUEUE_LAST_SAMPLES => (SHADER_QUEUE_LAST_SAMPLES, "queue_last_cs"),
        COMPUTE_LIGHT => (SHADER_LIGHT, "light_cs"),
        COMPUTE_QUEUE_ASK_PARKED => (SHADER_QUEUE_ASK, "queue_ask_parked_cs"),
        COMPUTE_SAMPLE_HIT => (SHADER_SAMPLE_HIT, "sample_hit_cs"),
        COMPUTE_FACE => (SHADER_FACE, "face_cs"),
        COMPUTE_LIGHT_MARK_WALKED => (SHADER_LIGHT_MARK, "light_mark_walked_cs"),
        COMPUTE_LIGHT_MARK_DRAWS => (SHADER_LIGHT_MARK_DRAWS, "light_mark_draws_cs"),
        COMPUTE_LIGHT_MARK_DRAWS_OPEN => (SHADER_LIGHT_MARK_DRAWS_OPEN, "light_mark_draws_open_cs"),
        COMPUTE_LIGHT_PROPAGATE => (SHADER_LIGHT_PROPAGATE, "light_propagate_cs"),
        COMPUTE_LIGHT_DAMAGE => (SHADER_LIGHT_PROPAGATE, "light_damage_cs"),
        COMPUTE_GENERATION_THIN => (SHADER_GENERATION_THIN, "decor_thin_cs"),
        COMPUTE_SCREEN_PROPAGATE => (SHADER_SCREEN_PROPAGATE, "screen_propagate_cs"),
        COMPUTE_SCREEN_MARK => (SHADER_SCREEN_MARK, "screen_mark_cs"),
        COMPUTE_SCREEN_BOUND => (SHADER_SCREEN_BOUND, "screen_bound_cs"),
        COMPUTE_LIGHT_SLIDE => (SHADER_LIGHT_SLIDE, "light_slide_cs"),
        COMPUTE_QUEUE_LAST => (SHADER_QUEUE_LAST, "queue_last_cs"),
        COMPUTE_REC_FACES => (SHADER_REC_HITS, "rec_faces_cs"),
        COMPUTE_RECT_ARGS => (SHADER_RECT_ARGS, "rect_args_cs"),
        COMPUTE_QUEUE_LIGHT_CORNERS => (SHADER_QUEUE_LIGHT_CORNERS, "queue_cs"),
        COMPUTE_QUEUE_LIGHT_CORNERS_PARKED => (SHADER_QUEUE_LIGHT_CORNERS, "queue_parked_cs"),
        COMPUTE_LIGHT_CELL => (SHADER_LIGHT_CELL, "light_cell_cs"),
        COMPUTE_QUEUE_LAST_LIGHT_SAMPLES => (SHADER_QUEUE_LAST_LIGHT_SAMPLES, "queue_last_cs"),
        COMPUTE_LIGHT_SAMPLE_HIT => (SHADER_LIGHT_SAMPLE_HIT, "sample_hit_cs"),
        COMPUTE_QUEUE_SAMPLES => (SHADER_QUEUE_SAMPLES, "queue_cs"),
        COMPUTE_QUEUE_SAMPLES_PARKED => (SHADER_QUEUE_SAMPLES, "queue_parked_cs"),
        COMPUTE_QUEUE => (SHADER_QUEUE, "queue_cs"),
        COMPUTE_QUEUE_LAST_CORNERS => (SHADER_QUEUE_LAST_CORNERS, "queue_last_cs"),
        COMPUTE_QUEUE_PARKED => (SHADER_QUEUE, "queue_parked_cs"),
        COMPUTE_QUEUE_LIGHT_SAMPLES => (SHADER_QUEUE_LIGHT_SAMPLES, "queue_cs"),
        COMPUTE_QUEUE_LIGHT_SAMPLES_PARKED => (SHADER_QUEUE_LIGHT_SAMPLES, "queue_parked_cs"),
        COMPUTE_QUEUE_LAST_LIGHT_CORNERS => (SHADER_QUEUE_LAST_LIGHT_CORNERS, "queue_last_cs"),
        COMPUTE_REC_DRAW => (SHADER_REC_DRAW, "rec_draw_cs"),
        COMPUTE_REC_HITS => (SHADER_REC_HITS, "rec_hits_cs"),
        COMPUTE_LIGHT_REC_DRAW => (SHADER_REC_DRAW, "light_rec_draw_cs"),
        COMPUTE_LIGHT_REC_HITS => (SHADER_LIGHT_REC_HITS, "rec_hits_cs"),
        COMPUTE_QUEUE_LIGHT => (SHADER_QUEUE_LIGHT, "queue_cs"),
        COMPUTE_QUEUE_LIGHT_PARKED => (SHADER_QUEUE_LIGHT, "queue_parked_cs"),
        COMPUTE_QUEUE_LAST_LIGHT => (SHADER_QUEUE_LAST_LIGHT, "queue_last_cs"),
        _ => panic!("a compute pipeline number with no shader"),
    }
}

include!(concat!(env!("OUT_DIR"), "/sources.rs"));

/// A shader that runs the world's one query over a list of asks and writes what it tells,
/// word for word, for tests/world.rs and for Tint (world_asked.wgsl). No shader the page runs
/// includes it.
pub const WORLD_ASKED: &str = include_str!(concat!(
    env!("OUT_DIR"),
    "/shader/assembled/world_asked.wgsl"
));

/// Each render pipeline by its number. The pipelines a draw record names come first
/// (gpu/draw.rs `DRAW_PIPELINES`), then those a frame runs on its own. The tuples below are
/// the words of each pipeline's draw record. A box draw is (0, 1, 0, uniform, pipeline): one
/// box and no upload, all it says being in its uniform. A decor draw is (upload handle, group,
/// instance count, uniform, pipeline), and its uniform carries the group's first column word,
/// its stride and the start of its index list.
pub const PIPELINE_DECOR: u32 = 0;
/// The hover ghost's box, half transparent. It draws at the depth of the faces it lies on,
/// and those faces stand behind it (screen_ray.wgsl `behind`).
pub const PIPELINE_GHOST_BOX: u32 = 1;
pub const PIPELINE_GHOST_DECOR: u32 = 2;
/// A character's draw record is (model handle, joint palette handle, model id, uniform,
/// pipeline).
pub const PIPELINE_SKINNED: u32 = 3;
/// The location indicator's draw records. The zone's is (zone upload handle, instance count,
/// 0, uniform, pipeline), and the beacon's is (0, 0, 0, uniform, pipeline).
pub const PIPELINE_LOCATION_ZONE: u32 = 4;
pub const PIPELINE_LOCATION_BEACON: u32 = 5;
/// A marker set's draw record is (upload handle, count, 0, uniform, pipeline).
pub const PIPELINE_MARKER: u32 = 6;
/// The debug gizmos' line segments. Their draw record holds the handle, the line vertex count
/// and the face vertex count.
pub const PIPELINE_DEBUG: u32 = 7;
/// Decor drawn as impostors where a variant is too small on screen for its model.
pub const PIPELINE_IMPOSTOR: u32 = 8;
pub const PIPELINE_SKY: u32 = 9;
pub const PIPELINE_MIP: u32 = 10;
/// The occlusion's half-resolution map laid over the world's color in the
/// damaged tiles (pass/ao.rs `apply`).
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
/// Draws the markers' regions, each by its number, into a target of their own
/// (pass/marker.rs).
pub const PIPELINE_MARKER_REGION: u32 = 24;
/// The debug gizmos' faces, depth only, into the gizmos' own depth (pass/debug.rs `DebugPass`).
pub const PIPELINE_DEBUG_FACE: u32 = 25;
/// The eye's rays, opaque at their depth (pxl-engine ray.rs).
pub const PIPELINE_SCREEN_RAY: u32 = 26;
/// The three pipelines of the shadow map's depth (pass/shadow.rs): the decor's and the
/// characters' depth from the light, and the world's depth, which is laid in before them.
pub const PIPELINE_DECOR_DEPTH: u32 = 27;
pub const PIPELINE_SKINNED_DEPTH: u32 = 28;
pub const PIPELINE_LIGHT_MAP: u32 = 29;
/// Draws the record's boxes, and keeps at each pixel the voxel whose face the pixel's ray
/// enters first (ray/raster.rs).
pub const PIPELINE_RASTER: u32 = 30;
/// Encodes the world's color onto the canvas, once, and carries the world's depth up to the
/// canvas's size (pass/finish.rs).
pub const PIPELINE_SCREEN_FINISH: u32 = 31;
/// The world's color gathered over frames (pass/resolve.rs).
pub const PIPELINE_SCREEN_RESOLVE: u32 = 32;
/// The gate a frame not drawn whole is drawn through: it clears the damaged tiles and raises
/// their stencil (renderer.rs `frame`).
pub const PIPELINE_SCREEN_MASK: u32 = 33;
/// Shows on the canvas the finished picture kept at the canvas's size (pass/finish.rs).
pub const PIPELINE_SCREEN_SHOW: u32 = 34;
/// Copies the damaged tiles of the history the resolve wrote this frame back into the history
/// the frame before wrote (pass/resolve.rs).
pub const PIPELINE_SCREEN_WRITE_BACK: u32 = 35;
/// The gate the samples' raster is drawn through in a frame not drawn whole. It clears the
/// damaged tiles to the far plane and leaves the rest at the nearest depth, where no box
/// passes the depth test (ray/raster.rs).
pub const PIPELINE_RASTER_MASK: u32 = 36;
/// The gate the light's map is drawn through when only the listed casters that changed have
/// moved. It lays the world's depth in the damaged tiles and raises their stencil
/// (pass/shadow.rs).
pub const PIPELINE_LIGHT_MASK: u32 = 37;
/// Draws a character where it was and where it is, and each fragment sets its screen tile's
/// word (pass/skinned_mesh.rs `draw_marks`).
pub const PIPELINE_SKINNED_MARK: u32 = 38;
/// The same marks for decor, an impostor, the location indicator's zone and its beacon
/// (pass/mark.rs `MarkPass`).
pub const PIPELINE_DECOR_MARK: u32 = 39;
pub const PIPELINE_IMPOSTOR_MARK: u32 = 40;
pub const PIPELINE_LOCATION_ZONE_MARK: u32 = 41;
pub const PIPELINE_LOCATION_BEACON_MARK: u32 = 42;
/// The camera grid over the canvas, after the debug gizmos' lines, while the frame's gizmo word
/// is set (pass/debug.rs).
pub const PIPELINE_CAMERA_GRID: u32 = 43;
pub const PIPELINES: u32 = 44;

// Record words. A record's header comes first: the shader, whether it has a fragment stage,
// the topology, the cull, whether it writes depth, the depth compare, the depth bias as an
// i32, the slope scale as f32 bits, the blend and the vertex entry. The vertex buffers follow:
// their count, then for each its stride, its step, its attribute count, and a (location,
// offset, format) for each attribute. The bind groups come last: their count, then for each
// its entry count and a (binding, visibility, kind) for each entry.
pub use crate::api_spec::record::*;
/// A binding's stages are xpute's own bits (SHADER_STAGE), and `NONE` is no stage.
pub use xpute_gpu::spec::SHADER_STAGE::{COMPUTE, FRAGMENT, VERTEX};
pub const NONE: u32 = 0;

/// `group` with its uniform at `binding` bound at a dynamic offset. The CPU decides that, and
/// the shader cannot say it.
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

/// Whether every entry of `a` is one of `b`'s, with the same binding, stages and kind, so
/// that a pipeline laid out with `a` takes a group laid out as `b`. The kind must match
/// exactly, as WebGPU matches a shader's storage access to its layout entry's exactly.
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

use crate::frame::shader::floor;
use pxl_engine::param::light;

const fn f(v: f32) -> u32 {
    v.to_bits()
}
const fn i(v: i32) -> u32 {
    v as u32
}

#[derive(Clone, Copy)]
struct Header {
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

const fn lit(shader: u32, cull: u32) -> Header {
    Header {
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

const fn blended(shader: u32, cull: u32) -> Header {
    Header {
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

/// The slope bias of the shadow map's casters, the same for those the raster draws and for
/// those the cast finds (pxl-engine `param::light::SLOPE_BIAS`).
const DEPTH_SLOPE_BIAS: f32 = light::SLOPE_BIAS;

/// A depth-only header: a plain less-than test rather than the reversed one, no fragment
/// stage, both sides drawn, and the depth pushed back by its slope as a rasterizer's polygon
/// offset pushes it.
const fn depth_only(shader: u32) -> Header {
    Header {
        shader,
        fragment: false,
        topology: TRIANGLES,
        cull: CULL_NONE,
        depth_write: true,
        depth_compare: LESS,
        depth_bias: 0,
        depth_bias_slope: f(DEPTH_SLOPE_BIAS),
        blend: OPAQUE,
        vertex: VS_PLACE,
    }
}

/// The header of a changed draw's mark (tile_mark.wgsl). Its vertices are laid out by its
/// `vs_place`, both sides are drawn, and its fragments write no color and test no
/// depth.
const fn mark(shader: u32) -> Header {
    Header {
        fragment: true,
        depth_write: false,
        depth_compare: ALWAYS,
        depth_bias_slope: 0,
        ..depth_only(shader)
    }
}

const fn screen(shader: u32) -> Header {
    Header {
        depth_compare: NO_DEPTH,
        blend: OPAQUE,
        ..blended(shader, CULL_NONE)
    }
}

/// The air's bindings that every painter and the sky read, numbered alike in every pass that
/// reads them (pass/atmosphere.rs `entries`).
const AIR: [[u32; 2]; 6] = [
    [8, UNIFORM],
    [9, TEXTURE],
    [11, SAMPLER],
    [13, TEXTURE],
    [18, TEXTURE],
    // The air's volume before each point, which is made from the bindings above
    // (pass/atmosphere.rs `froxel`).
    [22, TEXTURE_3D],
];
/// The air's bindings the sky alone reads: the sky views, the sun's and the moon's, and the
/// Earth's picture and tiles (pass/atmosphere.rs `sky_entries`).
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
/// The frame, the shadow and the air a painter reads. The vertex stage reads the air, because
/// each point's lights and veil are worked out there. The fragment stage reads the air's words
/// and the eye's texel, which holds the white a fragment's color is shown against
/// (atmos_eye.wgsl).
const LIT_FRAME: [[u32; 2]; 9] = and([[0, UNIFORM], SHADOW[0], SHADOW[1]], AIR);
/// The cloud sheets' own words and tiles, and the air as every painter reads it
/// (atmos_air.wgsl `seen_from_eye`).
const CLOUD_GROUP: [[u32; 2]; 10] = and(
    [[0, UNIFORM], [1, UNIFORM], [2, TEXTURE], [3, SAMPLER]],
    AIR,
);
const SKIN: [[u32; 2]; 2] = [[0, TEXTURE], [1, SAMPLER]];

/// The screen rays' group: the lit pass's frame, shadow, air and layers, all read where a
/// pixel is painted, then the rays, the looks and the eye's words. No group has more entries
/// (`GROUP_ENTRIES`).
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

/// The most entries a group has, a layout's and a bind group's alike. It is the count of the
/// screen rays' group, which extends the lit entries of renderer.rs. A group with more entries
/// is refused where it is made.
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

const MODEL_BUFS: [VertexBufferLayout; 3] = [
    vertex(12, &[[0, 0, F32X3]]),
    vertex(12, &[[1, 0, F32X3]]),
    vertex(8, &[[2, 0, F32X2]]),
];
const SKINNED_BUFS: [VertexBufferLayout; 5] = [
    MODEL_BUFS[0],
    MODEL_BUFS[1],
    MODEL_BUFS[2],
    vertex(16, &[[3, 0, U32X4]]),
    vertex(16, &[[4, 0, F32X4]]),
];
const LOCATION_ZONE_BUFS: [VertexBufferLayout; 2] =
    [vertex(12, &[[0, 0, F32X3]]), vertex(8, &[[1, 0, F32X2]])];
const LOCATION_BEACON_BUFS: [VertexBufferLayout; 2] =
    [vertex(12, &[[0, 0, F32X3]]), vertex(12, &[[1, 0, F32X3]])];
/// The bytes of a debug line's vertex, which holds its place and its color, and of a debug
/// face's vertex, which holds its place alone. A segment's two line vertices are read as one
/// instance.
pub const DEBUG_VERTEX_BYTES: u32 = 28;
pub const DEBUG_FACE_VERTEX_BYTES: u32 = 12;
const DEBUG_BUFS: [VertexBufferLayout; 1] = [VertexBufferLayout {
    stride: 2 * DEBUG_VERTEX_BYTES,
    step: PER_INSTANCE,
    attributes: &[
        [0, 0, F32X3],
        [1, 12, F32X4],
        [2, DEBUG_VERTEX_BYTES, F32X3],
    ],
}];
const DEBUG_FACE_BUFS: [VertexBufferLayout; 1] =
    [vertex(DEBUG_FACE_VERTEX_BYTES, &[[0, 0, F32X3]])];

/// The entries of `a` followed by those of `b`, for a group laid out as another with more.
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
/// A draw's uniform, at a dynamic offset (pass/model.rs `UniformBuffer`). It and a draw's
/// upload (`UPLOAD`) each have one layout wherever a draw binds them, so a group one owner
/// makes binds in every pipeline that names it (gpu/generator.rs, gpu/upload.rs).
const DRAW: [[u32; 2]; 1] = [[0, UNIFORM_DYNAMIC]];
/// An upload's words, and the list that orders a draw's instances. The list is the upload's
/// words themselves, except in a frame drawn through the gate (instance.wgsl `order`).
const UPLOAD: [[u32; 2]; 2] = [[0, STORAGE], [1, STORAGE]];
/// An upload's group as every pipeline that binds one lays it out. The draw list's uploads
/// are grouped in it (gpu/upload.rs `UploadTable::group`).
pub const UPLOAD_LAYOUT: [u32; 6] = [
    UPLOAD[0][0],
    stages_reading(&UPLOAD, UPLOAD[0][0]),
    UPLOAD[0][1],
    UPLOAD[1][0],
    stages_reading(&UPLOAD, UPLOAD[1][0]),
    UPLOAD[1][1],
];

const MODEL_GROUPS: [Group; 4] = [&LIT_FRAME, &DRAW, &SKIN, &UPLOAD];
/// The groups of a model's depth from the light: the light's frame alone in group 0, where
/// the model's place is read, and the rest as the lit pass binds them.
const DEPTH_GROUPS: [Group; 4] = [FRAME, MODEL_GROUPS[1], MODEL_GROUPS[2], MODEL_GROUPS[3]];
/// The draw list's uniforms by slot, read-only. A character's draw and the beacon's read
/// their uniform at the slot their first instance names (skinned_draw.slot.wgsl,
/// location.slot.wgsl). Each pass makes one such group (pass/skinned_mesh.rs
/// `SkinnedPass::set_draws`, pass/location.rs `LocationPass::set_draws`).
const DRAWS: [[u32; 2]; 1] = [[0, STORAGE]];
/// A character's groups, lit and for its depth from the light: a model's, except that its
/// draw is read from the list's uniforms (`DRAWS`).
const SKINNED_GROUPS: [Group; 4] = [&LIT_FRAME, &DRAWS, &SKIN, &UPLOAD];
const SKINNED_DEPTH_GROUPS: [Group; 4] = [FRAME, &DRAWS, &SKIN, &UPLOAD];
/// Group 0 of a draw's mark (tile_mark.wgsl): the eye's frame or the light's, the tiles'
/// words, their uniform, and the box of the tiles the frame's marks cover. The mark's other
/// groups are those its depth reads.
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
/// A marker set's instances, and what its pass reads at an anchor: the regions for the
/// sprites, and the world's depth for the regions. Its uniform is a draw's (`DRAW`).
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

/// A pipeline's record before its groups' stages are known: its header, its vertex buffers,
/// and each group's entries as (binding, kind). `record` adds to each entry the stages that
/// read it.
#[derive(Clone, Copy)]
struct Descriptor {
    header: Header,
    bufs: &'static [VertexBufferLayout],
    groups: &'static [Group],
}

impl Descriptor {
    const fn new(
        header: Header,
        bufs: &'static [VertexBufferLayout],
        groups: &'static [Group],
    ) -> Self {
        Self {
            header,
            bufs,
            groups,
        }
    }
}

/// Pipeline `id`'s descriptor. Every id below `PIPELINES` must have one, since the records,
/// worked out as the crate is built, ask every id for its own.
const fn desc_of(id: u32) -> Descriptor {
    match id {
        PIPELINE_DECOR => Descriptor::new(lit(SHADER_DECOR, CULL_NONE), &MODEL_BUFS, &MODEL_GROUPS),
        // What is laid on the world's faces draws at its own depth, and the world's faces
        // stand behind it (screen_ray.wgsl `behind`).
        PIPELINE_GHOST_BOX => Descriptor::new(blended(SHADER_BOX, CULL_BACK), &[], &[&BOX_GROUP]),
        PIPELINE_GHOST_DECOR => {
            Descriptor::new(blended(SHADER_DECOR, CULL_NONE), &MODEL_BUFS, &MODEL_GROUPS)
        }
        // Back faces are culled. Drawn from both sides, a closed body's inner faces fight its
        // outer ones at a distance, and the character shimmers.
        PIPELINE_SKINNED => Descriptor::new(
            lit(SHADER_SKINNED, CULL_BACK),
            &SKINNED_BUFS,
            &SKINNED_GROUPS,
        ),
        PIPELINE_LOCATION_ZONE => Descriptor::new(
            blended(SHADER_LOCATION_ZONE, CULL_NONE),
            &LOCATION_ZONE_BUFS,
            &[FRAME, &DRAW, &UPLOAD],
        ),
        PIPELINE_LOCATION_BEACON => Descriptor::new(
            lit(SHADER_LOCATION_BEACON, CULL_BACK),
            &LOCATION_BEACON_BUFS,
            &[&LIT_FRAME, &DRAWS],
        ),
        PIPELINE_MARKER => Descriptor::new(
            Header {
                topology: STRIP,
                depth_compare: GREATER_EQUAL,
                ..blended(SHADER_MARKER, CULL_NONE)
            },
            &[],
            &[FRAME, &DRAW, MARKER_SPRITES, &SKIN],
        ),
        PIPELINE_MARKER_REGION => Descriptor::new(
            Header {
                topology: STRIP,
                depth_write: true,
                depth_compare: GREATER_EQUAL,
                blend: OPAQUE,
                ..blended(SHADER_MARKER_REGION, CULL_NONE)
            },
            &[],
            &[FRAME, &DRAW, MARKER_REGIONS],
        ),
        // The gizmos' lines are tested against the gizmos' own faces, not the world's depth.
        PIPELINE_DEBUG => Descriptor::new(
            Header {
                blend: ALPHA,
                ..blended(SHADER_DEBUG, CULL_NONE)
            },
            &DEBUG_BUFS,
            &[FRAME, &DRAW],
        ),
        // Over everything, against no depth.
        PIPELINE_CAMERA_GRID => Descriptor::new(
            Header {
                depth_compare: ALWAYS,
                ..blended(SHADER_CAMERA_GRID, CULL_NONE)
            },
            &[],
            &[&[[19, UNIFORM]]],
        ),
        // The faces are pushed back by their slope, as a rasterizer's polygon offset pushes
        // them, so the lines that bound a face stay in front of it.
        PIPELINE_DEBUG_FACE => Descriptor::new(
            Header {
                cull: CULL_NONE,
                depth_compare: GREATER,
                depth_bias: i(-8),
                depth_bias_slope: f(-2.0),
                ..depth_only(SHADER_DEBUG)
            },
            &DEBUG_FACE_BUFS,
            &[FRAME, &DRAW],
        ),
        PIPELINE_SCREEN_RAY => Descriptor::new(
            Header {
                depth_write: true,
                blend: OPAQUE,
                ..blended(SHADER_SCREEN_RAY, CULL_NONE)
            },
            &[],
            &[&SCREEN_GROUP],
        ),
        // The occlusion's half-resolution map, brought to each pixel and multiplied into the
        // world's color, drawn as the damaged tiles' quads. The multiply is black blended over
        // by the occlusion's darkening, so the apply is a draw of the world's pass that
        // neither tests nor writes depth.
        PIPELINE_AO_APPLY => Descriptor::new(
            Header {
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
            Descriptor::new(depth_only(SHADER_DECOR), &MODEL_BUFS, &DEPTH_GROUPS)
        }
        PIPELINE_SKINNED_DEPTH => Descriptor::new(
            depth_only(SHADER_SKINNED),
            &SKINNED_BUFS,
            &SKINNED_DEPTH_GROUPS,
        ),
        // Its vertices are laid out as the depth pass lays them, and its fragments write no
        // color and test no depth: they only mark their tiles.
        PIPELINE_SKINNED_MARK => {
            Descriptor::new(mark(SHADER_SKINNED_MARK), &SKINNED_BUFS, &MARK_GROUPS)
        }
        PIPELINE_DECOR_MARK => Descriptor::new(mark(SHADER_DECOR_MARK), &MODEL_BUFS, &MARK_GROUPS),
        PIPELINE_IMPOSTOR_MARK => Descriptor::new(
            Header {
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
            &LOCATION_ZONE_BUFS,
            &[MARK_FRAME, &DRAW, &UPLOAD],
        ),
        PIPELINE_LOCATION_BEACON_MARK => Descriptor::new(
            mark(SHADER_LOCATION_BEACON_MARK),
            &LOCATION_BEACON_BUFS,
            &[MARK_FRAME, &DRAW],
        ),
        // Lays the depth the world's cast holds into the shadow map, in the tiles a receiver
        // reads, before the casters are drawn over it: one quad a listed tile, into the depth
        // alone (gpu/api.rs `pipeline`). It binds the clipmap's words, the box's corner, the
        // tiles' reads and the list of the tiles read, and its fragment writes the depth.
        PIPELINE_LIGHT_MAP => Descriptor::new(
            Header {
                fragment: true,
                depth_compare: ALWAYS,
                depth_bias_slope: 0,
                vertex: VS,
                ..depth_only(SHADER_LIGHT_MAP)
            },
            &[],
            &[&[[0, TEXTURE_UINT_ARRAY], [1, UNIFORM], [2, STORAGE]]],
        ),
        // The same in the map's damaged tiles alone, one quad a tile of the marks' box, and it
        // raises their stencil (gpu/api.rs `Stencil::Mask`).
        PIPELINE_LIGHT_MASK => Descriptor::new(
            Header {
                fragment: true,
                depth_compare: ALWAYS,
                depth_bias_slope: 0,
                vertex: VS,
                ..depth_only(SHADER_LIGHT_MASK)
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
        // Each box's faces turned to the eye are drawn, and the reversed depth test keeps the
        // nearest face a pixel's ray enters. The pixel takes that face's voxel number, in a
        // target of its own (ray/raster.rs). The vertex stage emits only the faces turned
        // to the eye, so no face is culled by its winding.
        PIPELINE_RASTER => Descriptor::new(
            Header {
                depth_write: true,
                depth_compare: GREATER,
                blend: OPAQUE,
                ..blended(SHADER_RASTER, CULL_NONE)
            },
            &[],
            &[&[[1, UNIFORM], [2, STORAGE], [9, STORAGE]]],
        ),
        // The sky draws only where the opaque world left the far plane.
        PIPELINE_SKY => Descriptor::new(
            Header {
                depth_compare: GREATER_EQUAL,
                blend: OPAQUE,
                ..blended(SHADER_SKY, CULL_NONE)
            },
            &[],
            &[&SKY_GROUP],
        ),
        PIPELINE_MIP => Descriptor::new(screen(SHADER_MIP), &[], &[&SKIN]),
        // It also writes the world's depth at the canvas's size, which what is drawn over the
        // world is tested against.
        PIPELINE_SCREEN_FINISH => Descriptor::new(
            Header {
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
        PIPELINE_SCREEN_MASK => Descriptor::new(
            Header {
                depth_write: true,
                depth_compare: ALWAYS,
                ..screen(SHADER_SCREEN_MASK)
            },
            &[],
            &[&[[30, STORAGE], [31, UNIFORM]]],
        ),
        PIPELINE_RASTER_MASK => Descriptor::new(
            Header {
                depth_write: true,
                depth_compare: ALWAYS,
                ..screen(SHADER_RASTER_MASK)
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
            Header {
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
            Header {
                fragment: true,
                vertex: VS,
                ..depth_only(SHADER_IMPOSTOR_BAKE)
            },
            &MODEL_BUFS,
            &[&DRAW, &SKIN],
        ),
        PIPELINE_ATLAS => Descriptor::new(
            Header {
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
            Header {
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
        // The cloud sheets' planes are tested against the world's depth, so what stands before
        // them hides them.
        PIPELINE_CLOUD => Descriptor::new(blended(SHADER_CLOUD, CULL_NONE), &[], &[&CLOUD_GROUP]),
        PIPELINE_CLOUD_FIELD => Descriptor::new(screen(SHADER_CLOUD_FIELD), &[], &[]),
        _ => panic!("a pipeline number with no descriptor"),
    }
}

include!(concat!(env!("OUT_DIR"), "/stages.rs"));

/// The stages of shader `shader` that read binding `binding` of its group `group`, as build.rs
/// read them off it, or `NONE` when no stage does.
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

/// Whether two groups hold the same entries in the same order.
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

/// The stages binding `binding` of `group` is visible to: every stage that reads it in any
/// shader whose pipeline lays out the same group, at any index. Those pipelines share one
/// layout, so the layout must hold for each of them.
const fn stages_reading(group: Group, binding: u32) -> u32 {
    let mut stages = NONE;
    let mut id = 0;
    while id < PIPELINES {
        let desc = desc_of(id);
        let mut k = 0;
        while k < desc.groups.len() {
            if alike(desc.groups[k], group) {
                stages |= stages_of(desc.header.shader, k as u32, binding);
            }
            k += 1;
        }
        id += 1;
    }
    stages
}

/// The number of words in a descriptor's record (see Record words above).
const fn words(desc: &Descriptor) -> usize {
    let mut n = HEADER as usize + 1 + 1;
    let mut k = 0;
    while k < desc.bufs.len() {
        n += 3 + 3 * desc.bufs[k].attributes.len();
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
const REC_WORDS: usize = {
    let (mut most, mut id) = (0, 0);
    while id < PIPELINES {
        let n = words(&desc_of(id));
        most = if n > most { n } else { most };
        id += 1;
    }
    most
};

/// Pipeline `id`'s record and its length, each group entry visible to the stages that read it
/// (`stages_reading`).
const fn rec(id: u32) -> ([u32; REC_WORDS], usize) {
    let mut out = [0; REC_WORDS];
    let desc = desc_of(id);
    let h = desc.header;
    let header = [
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
    while n < header.len() {
        out[n] = header[n];
        n += 1;
    }
    out[n] = desc.bufs.len() as u32;
    n += 1;
    let mut k = 0;
    while k < desc.bufs.len() {
        let l = desc.bufs[k];
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
const RECS: [([u32; REC_WORDS], usize); PIPELINES as usize] = {
    let mut r = [([0; REC_WORDS], 0); PIPELINES as usize];
    let mut id = 0;
    while id < PIPELINES {
        r[id as usize] = rec(id);
        id += 1;
    }
    r
};
static TABLE: [([u32; REC_WORDS], usize); PIPELINES as usize] = RECS;

/// Pipeline `id`'s record (see Record words above), each group entry visible to the stages
/// that read it.
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

/// Whether render pipeline `id`'s shader reads the light's shadow map, that is, whether its
/// parts include shadow.wgsl (`PARTS`); a record's first word is its shader. Whatever such a
/// pipeline draws is a receiver of the map, and stands on the map's tiles (gpu/draw.rs
/// `DrawList`).
pub fn reads_shadow_map(id: u32) -> bool {
    PARTS[pipeline(id)[0] as usize].contains(&"shadow")
}

/// Whether pipeline `id`'s layout, with its groups as its record lays them out, binds within
/// what every WebGPU device binds (`floor::holds`).
const fn is_floored(id: u32) -> bool {
    let (words, _) = &RECS[id as usize];
    let mut at = HEADER as usize;
    let bufs = words[at];
    at += 1;
    let mut k = 0;
    while k < bufs {
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

/// Holds every pipeline's layout within the floor as the crate is built.
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

/// Whether pipeline `id`'s groups lay out exactly what its shader reads. Every binding read by
/// the entry points it runs (its vertex entry, `Header::vertex`, and `fs` when it has a
/// fragment stage) must have an entry in its group, of a kind the binding's declaration takes
/// (build.rs `layout_kinds`). Every entry must be read by some pipeline that lays out the same
/// group, as build.rs `BINDS` requires of a compute group.
const fn lays_out(id: u32) -> bool {
    let desc = desc_of(id);
    let entries = STAGES[desc.header.shader as usize];
    let mut k = 0;
    while k < entries.len() {
        let [g, b, by, takes, vertex] = entries[k];
        if vertex & 1 << desc.header.vertex != 0 || (desc.header.fragment && by & FRAGMENT != 0) {
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

/// Holds every pipeline's groups to what its shader reads as the crate is built.
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
