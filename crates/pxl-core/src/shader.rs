//! The shaders and the pipelines that run them, as their one source: the
//! WGSL in `shader/`, assembled here from its shared pieces, and each
//! pipeline's definition — its vertex lanes, its bind group layouts, its
//! primitive, depth and blend state — as a record of words the glue turns
//! into WebGPU's descriptor and a native shell into wgpu's. What the glue
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

/// The shaders, by kind. A pipeline names one for a frame drawn at one
/// sample a pixel and one for a multisampled frame; the badge shader is
/// the one that differs, reading the depth as a multisampled texture.
pub const SHADER_CHUNK: u32 = 0;
pub const SHADER_DECOR: u32 = 1;
pub const SHADER_SKINNED: u32 = 2;
pub const SHADER_PUCK_ZONE: u32 = 3;
pub const SHADER_PUCK_BEACON: u32 = 4;
pub const SHADER_BADGE: u32 = 5;
pub const SHADER_BADGE_MSAA: u32 = 6;
pub const SHADER_SKY: u32 = 7;
pub const SHADER_DEBUG: u32 = 8;
pub const SHADER_MIPS: u32 = 9;
pub const SHADERS: u32 = 10;

macro_rules! wgsl {
    ($($name:literal),+) => { concat!($(include_str!(concat!("shader/", $name, ".wgsl")), "\n"),+) };
}

/// A shader's WGSL; empty for a kind that is not one.
pub fn source(kind: u32) -> &'static str {
    match kind {
        SHADER_CHUNK => wgsl!("frame", "grade", "shade", "shadow", "chunk"),
        SHADER_DECOR => wgsl!("frame", "grade", "shade", "shadow", "decor"),
        SHADER_SKINNED => wgsl!("frame", "grade", "shade", "shadow", "skinned"),
        SHADER_PUCK_ZONE => wgsl!("frame", "grade", "shade", "puck", "puck_zone"),
        SHADER_PUCK_BEACON => wgsl!("frame", "grade", "shade", "puck", "puck_beacon"),
        SHADER_BADGE => wgsl!("grade", "depth", "badge_depth", "badge"),
        SHADER_BADGE_MSAA => wgsl!("grade", "depth", "badge_depth_msaa", "badge"),
        SHADER_SKY => wgsl!("grade", "sky"),
        SHADER_DEBUG => wgsl!("frame", "grade", "debug"),
        SHADER_MIPS => wgsl!("mips"),
        _ => "",
    }
}

/// Floats the frame uniform (frame.wgsl's Frame) holds.
pub const FRAME_FLOATS: u32 = 68;
/// Where each field of the frame uniform starts, in floats, in this order:
/// view_proj, camera_pos, edge_strength, the environment's run (sun_dir
/// through fog_color, environment.rs's output as it is), low_spec,
/// edge_width, rim_intensity, viewport, shadow_strength, shadow_texel,
/// vignette, light_view_proj. The glue reads the table rather than
/// counting; the test below holds it to the struct.
pub const FRAME_AT: [u32; 12] = [0, 16, 19, 20, 23, 39, 43, 44, 46, 47, 48, 52];
#[cfg(test)]
const FRAME_FIELDS: [&str; 12] = [
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
    "light_view_proj",
];

/// The pipelines, by id: draw.rs's ten, which a draw names, then the ones
/// a frame runs on its own — the depth-only passes into the shadow map,
/// the sky, and the mip builder.
pub const PIPELINE_CHUNK_DEPTH: u32 = 10;
pub const PIPELINE_DECOR_DEPTH: u32 = 11;
pub const PIPELINE_SKINNED_DEPTH: u32 = 12;
pub const PIPELINE_SKY: u32 = 13;
pub const PIPELINE_MIPS: u32 = 14;
pub const PIPELINES: u32 = 15;

// A pipeline record's words, in order: the shader for a frame at one
// sample and the one for a multisampled frame; whether it has a fragment
// stage; the topology; the cull mode; whether it writes depth; the depth
// compare; the depth bias (i32) and its slope scale (f32 bits); the blend;
// the sample count (the frame's, or one); then the vertex buffers — a
// count, and for each its stride, its step mode, and its attributes as a
// count and (location, offset, format) each; then the bind group layouts —
// a count, and for each its entries as a count and (binding, visibility,
// kind) each.
pub const TRIANGLES: u32 = 0;
pub const STRIP: u32 = 1;
pub const LINES: u32 = 2;
pub const CULL_NONE: u32 = 0;
pub const CULL_BACK: u32 = 1;
/// Depth compares, under reversed Z: nearer is greater. LESS is the shadow
/// map's, drawn with a plain orthographic depth. NO_DEPTH means the pass has
/// no depth attachment at all.
pub const ALWAYS: u32 = 0;
pub const GREATER: u32 = 1;
pub const GREATER_EQUAL: u32 = 2;
pub const LESS: u32 = 3;
pub const NO_DEPTH: u32 = 4;
pub const OPAQUE: u32 = 0;
/// Straight alpha over what is drawn.
pub const ALPHA: u32 = 1;
pub const FRAME_SAMPLES: u32 = 0;
pub const ONE_SAMPLE: u32 = 1;
pub const PER_VERTEX: u32 = 0;
pub const F32: u32 = 0;
pub const F32X2: u32 = 1;
pub const F32X3: u32 = 2;
pub const F32X4: u32 = 3;
pub const U32X4: u32 = 4;
pub const VERTEX: u32 = 1;
pub const FRAGMENT: u32 = 2;
pub const BOTH: u32 = VERTEX | FRAGMENT;
/// A binding's kind: a uniform buffer, one bound at a dynamic offset, a
/// read-only storage buffer, a float texture, a float texture array, a
/// depth texture, a filtering sampler, a comparison sampler, and the
/// frame's depth — a depth texture multisampled as the frame is.
pub const UNIFORM: u32 = 0;
pub const UNIFORM_DYNAMIC: u32 = 1;
pub const STORAGE: u32 = 2;
pub const TEXTURE: u32 = 3;
pub const TEXTURE_ARRAY: u32 = 4;
pub const DEPTH: u32 = 5;
pub const SAMPLER: u32 = 6;
pub const SAMPLER_COMPARISON: u32 = 7;
pub const FRAME_DEPTH: u32 = 8;

const fn f(v: f32) -> u32 {
    v.to_bits()
}
const fn i(v: i32) -> u32 {
    v as u32
}

struct Head {
    shader: u32,
    shader_msaa: u32,
    fragment: bool,
    topology: u32,
    cull: u32,
    depth_write: bool,
    depth_compare: u32,
    depth_bias: u32,
    depth_bias_slope: u32,
    blend: u32,
    samples: u32,
}

/// An opaque lit pipeline: writes depth, nearer wins, drawn at the frame's
/// sample count.
const fn lit(shader: u32, cull: u32) -> Head {
    Head {
        shader,
        shader_msaa: shader,
        fragment: true,
        topology: TRIANGLES,
        cull,
        depth_write: true,
        depth_compare: GREATER,
        depth_bias: 0,
        depth_bias_slope: 0,
        blend: OPAQUE,
        samples: FRAME_SAMPLES,
    }
}

/// A blended pipeline over what is drawn: no depth written, biased toward
/// the eye — positive under reversed depth.
const fn blended(shader: u32, cull: u32, bias: i32, slope: f32) -> Head {
    Head {
        shader,
        shader_msaa: shader,
        fragment: true,
        topology: TRIANGLES,
        cull,
        depth_write: false,
        depth_compare: GREATER,
        depth_bias: i(bias),
        depth_bias_slope: f(slope),
        blend: ALPHA,
        samples: FRAME_SAMPLES,
    }
}

/// A depth-only pass into the shadow map: the pass's own vertex shader
/// under the light's frame, no fragment stage, both sides, a plain
/// less-than depth at one sample.
const fn depth_only(shader: u32) -> Head {
    Head {
        shader,
        shader_msaa: shader,
        fragment: false,
        topology: TRIANGLES,
        cull: CULL_NONE,
        depth_write: true,
        depth_compare: LESS,
        depth_bias: 0,
        depth_bias_slope: 0,
        blend: OPAQUE,
        samples: ONE_SAMPLE,
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
        h.shader_msaa,
        h.fragment as u32,
        h.topology,
        h.cull,
        h.depth_write as u32,
        h.depth_compare,
        h.depth_bias,
        h.depth_bias_slope,
        h.blend,
        h.samples,
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

/// The frame uniform, the shadow map and its sampler: group 0's first
/// three entries in every lit pass.
const LIT_FRAME: [[u32; 3]; 3] = [[0, BOTH, UNIFORM], [1, FRAGMENT, DEPTH], [2, FRAGMENT, SAMPLER_COMPARISON]];
/// A skin and its sampler.
const SKIN: [[u32; 3]; 2] = [[0, FRAGMENT, TEXTURE], [1, FRAGMENT, SAMPLER]];

/// The chunk's lanes as the mesher lays them out (mesher.rs): position,
/// normal, local_pos, box_scale, rgb, layer_taps, faces — a vertex slot a
/// lane, at the lane's offset from the vertex count.
fn chunk_lanes() -> Vec<Vec<u32>> {
    vec![
        lane(12, 0, F32X3),
        lane(12, 1, F32X3),
        lane(12, 2, F32X3),
        lane(12, 3, F32X3),
        lane(12, 4, F32X3),
        lane(16, 5, F32X4),
        lane(4, 6, F32),
    ]
}
/// The chunk's one group: the lit frame, the draws, the texture pack's two
/// arrays and their sampler.
fn chunk_groups() -> Vec<Vec<u32>> {
    let mut g0 = LIT_FRAME.to_vec();
    g0.extend([[3, BOTH, STORAGE], [4, FRAGMENT, TEXTURE_ARRAY], [5, FRAGMENT, TEXTURE_ARRAY], [6, FRAGMENT, SAMPLER]]);
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
        crate::draw::PIPELINE_CHUNK => record(lit(SHADER_CHUNK, CULL_BACK), &chunk_lanes(), &chunk_groups()),
        crate::draw::PIPELINE_DECOR => record(lit(SHADER_DECOR, CULL_NONE), &model_lanes(false), &model_groups()),
        // The hover ghost: biased toward the eye as hover_cursor.tsx's
        // polygon offsets were — slightly clear of a surface, strongly in
        // one.
        crate::draw::PIPELINE_GHOST_BOX => record(blended(SHADER_CHUNK, CULL_BACK, 1, 1.0), &chunk_lanes(), &chunk_groups()),
        crate::draw::PIPELINE_GHOST_BOX_COPLANAR => record(blended(SHADER_CHUNK, CULL_BACK, 8, 2.0), &chunk_lanes(), &chunk_groups()),
        crate::draw::PIPELINE_GHOST_DECOR => record(blended(SHADER_DECOR, CULL_NONE, 1, 1.0), &model_lanes(false), &model_groups()),
        // Back faces culled, as three's material (FrontSide) did: drawn both
        // sides, a closed body's inner faces fight its outer ones for the
        // same depth at a distance, and the character shimmers.
        crate::draw::PIPELINE_SKINNED => record(lit(SHADER_SKINNED, CULL_BACK), &model_lanes(true), &model_groups()),
        crate::draw::PIPELINE_PUCK_ZONE => record(
            blended(SHADER_PUCK_ZONE, CULL_NONE, 4, 1.0),
            &[lane(12, 0, F32X3), lane(8, 1, F32X2)],
            &[frame(BOTH), group(&[[0, BOTH, UNIFORM]]), group(&[[0, VERTEX, STORAGE]])],
        ),
        crate::draw::PIPELINE_PUCK_BEACON => record(
            lit(SHADER_PUCK_BEACON, CULL_BACK),
            &[lane(12, 0, F32X3), lane(12, 1, F32X3)],
            &[frame(BOTH), group(&[[0, BOTH, UNIFORM]])],
        ),
        // The badges: a strip of four corners an instance, blended over the
        // world, tested against its depth and no nearer than it.
        crate::draw::PIPELINE_BADGE => record(
            Head {
                topology: STRIP,
                shader_msaa: SHADER_BADGE_MSAA,
                depth_compare: GREATER_EQUAL,
                ..blended(SHADER_BADGE, CULL_NONE, 0, 0.0)
            },
            &[],
            &[group(&[[0, VERTEX, UNIFORM], [1, VERTEX, STORAGE], [2, VERTEX, FRAME_DEPTH]]), group(&SKIN)],
        ),
        // The gizmos: a line list over everything, no depth test.
        crate::draw::PIPELINE_DEBUG => record(
            Head {
                topology: LINES,
                depth_compare: ALWAYS,
                blend: OPAQUE,
                ..blended(SHADER_DEBUG, CULL_NONE, 0, 0.0)
            },
            &[vec![24, PER_VERTEX, 2, 0, 0, F32X3, 1, 12, F32X3]],
            &[frame(VERTEX), group(&[[0, VERTEX, UNIFORM]])],
        ),
        PIPELINE_CHUNK_DEPTH => record(depth_only(SHADER_CHUNK), &chunk_lanes(), &chunk_groups()),
        PIPELINE_DECOR_DEPTH => record(depth_only(SHADER_DECOR), &model_lanes(false), &model_groups()),
        PIPELINE_SKINNED_DEPTH => record(depth_only(SHADER_SKINNED), &model_lanes(true), &model_groups()),
        // The sky: one triangle over the screen, first, writing no depth.
        PIPELINE_SKY => record(
            Head {
                depth_compare: ALWAYS,
                blend: OPAQUE,
                ..blended(SHADER_SKY, CULL_NONE, 0, 0.0)
            },
            &[],
            &[frame(FRAGMENT)],
        ),
        // A mip level from the one above: no depth, one sample, into the
        // texture's own format.
        PIPELINE_MIPS => record(
            Head {
                depth_compare: NO_DEPTH,
                blend: OPAQUE,
                samples: ONE_SAMPLE,
                ..blended(SHADER_MIPS, CULL_NONE, 0, 0.0)
            },
            &[],
            &[group(&SKIN)],
        ),
        _ => Vec::new(),
    }
}

/// A vertex buffer as a record has it: its stride, its step mode, and its
/// attributes as (location, offset, format).
pub type Buffer = (u32, u32, Vec<(u32, u32, u32)>);

/// A record read back into its parts, as the glue reads it.
pub struct Record {
    pub shader: u32,
    pub shader_msaa: u32,
    pub fragment: bool,
    pub topology: u32,
    pub cull: u32,
    pub depth_write: bool,
    pub depth_compare: u32,
    pub depth_bias: i32,
    pub depth_bias_slope: f32,
    pub blend: u32,
    pub samples: u32,
    pub buffers: Vec<Buffer>,
    /// Per group, its entries as (binding, visibility, kind).
    pub groups: Vec<Vec<(u32, u32, u32)>>,
}

impl Record {
    pub fn read(words: &[u32]) -> Record {
        let mut at = 11;
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
        assert_eq!(at, words.len(), "a record's words end with its groups");
        Record {
            shader: words[0],
            shader_msaa: words[1],
            fragment: words[2] != 0,
            topology: words[3],
            cull: words[4],
            depth_write: words[5] != 0,
            depth_compare: words[6],
            depth_bias: words[7] as i32,
            depth_bias_slope: f32::from_bits(words[8]),
            blend: words[9],
            samples: words[10],
            buffers,
            groups,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use naga::valid::{Capabilities, ValidationFlags, Validator};
    use naga::{AddressSpace, Binding, ImageClass, Module, ScalarKind, ShaderStage, TypeInner, VectorSize};
    use std::collections::BTreeMap;

    fn compile(kind: u32) -> (Module, naga::valid::ModuleInfo) {
        let src = source(kind);
        assert!(!src.is_empty(), "shader {kind} has a source");
        let module = naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("shader {kind}: {}", e.emit_to_string(src)));
        let info = Validator::new(ValidationFlags::all(), Capabilities::all())
            .validate(&module)
            .unwrap_or_else(|e| panic!("shader {kind}: {}", e.emit_to_string(src)));
        (module, info)
    }

    #[test]
    fn every_shader_compiles() {
        for kind in 0..SHADERS {
            let (module, _) = compile(kind);
            let stages: Vec<_> = module.entry_points.iter().map(|e| (e.name.clone(), e.stage)).collect();
            assert!(stages.contains(&("vs".into(), ShaderStage::Vertex)), "shader {kind} has vs");
            assert!(stages.contains(&("fs".into(), ShaderStage::Fragment)), "shader {kind} has fs");
        }
    }

    #[test]
    fn frame_layout_matches_the_struct() {
        let (module, _) = compile(SHADER_CHUNK);
        let frame = module.types.iter().find(|(_, t)| t.name.as_deref() == Some("Frame")).expect("Frame").1;
        let TypeInner::Struct { members, span } = &frame.inner else { panic!("Frame is a struct") };
        assert_eq!(*span, FRAME_FLOATS * 4);
        let offsets: BTreeMap<&str, u32> = members.iter().map(|m| (m.name.as_deref().unwrap(), m.offset / 4)).collect();
        for (name, at) in FRAME_FIELDS.iter().zip(FRAME_AT) {
            assert_eq!(offsets[name], at, "FRAME_AT for {name}");
        }
        // The environment's run is contiguous from sun_dir, as environment.rs
        // writes it: 24 floats to the end of fog_color's padding.
        assert_eq!(offsets["fog_color"], offsets["sun_dir"] + 20);
    }

    /// The vertex format a location's argument type asks for.
    fn format_of(module: &Module, ty: naga::Handle<naga::Type>) -> u32 {
        match &module.types[ty].inner {
            TypeInner::Scalar(s) if s.kind == ScalarKind::Float => F32,
            TypeInner::Vector { size: VectorSize::Bi, scalar } if scalar.kind == ScalarKind::Float => F32X2,
            TypeInner::Vector { size: VectorSize::Tri, scalar } if scalar.kind == ScalarKind::Float => F32X3,
            TypeInner::Vector { size: VectorSize::Quad, scalar } if scalar.kind == ScalarKind::Float => F32X4,
            TypeInner::Vector { size: VectorSize::Quad, scalar } if scalar.kind == ScalarKind::Uint => U32X4,
            other => panic!("no vertex format for {other:?}"),
        }
    }

    /// The vertex shader's inputs by location, and their formats.
    fn vertex_inputs(module: &Module) -> BTreeMap<u32, u32> {
        let vs = module.entry_points.iter().find(|e| e.stage == ShaderStage::Vertex).unwrap();
        let mut out = BTreeMap::new();
        for arg in &vs.function.arguments {
            match &arg.binding {
                Some(Binding::Location { location, .. }) => {
                    out.insert(*location, format_of(module, arg.ty));
                }
                Some(Binding::BuiltIn(_)) => {}
                None => {
                    let TypeInner::Struct { members, .. } = &module.types[arg.ty].inner else {
                        panic!("an argument is a struct")
                    };
                    for m in members {
                        if let Some(Binding::Location { location, .. }) = &m.binding {
                            out.insert(*location, format_of(module, m.ty));
                        }
                    }
                }
            }
        }
        out
    }

    /// The module's resource bindings: (group, binding) to (the stages
    /// that use it, its kind, whether a depth texture is multisampled).
    fn bindings(module: &Module, info: &naga::valid::ModuleInfo) -> BTreeMap<(u32, u32), (u32, u32, bool)> {
        let mut out = BTreeMap::new();
        for (handle, var) in module.global_variables.iter() {
            let Some(rb) = &var.binding else { continue };
            let mut used = 0;
            for (k, ep) in module.entry_points.iter().enumerate() {
                if !info.get_entry_point(k)[handle].is_empty() {
                    used |= if ep.stage == ShaderStage::Vertex { VERTEX } else { FRAGMENT };
                }
            }
            let (kind, multi) = match (&var.space, &module.types[var.ty].inner) {
                (AddressSpace::Uniform, _) => (UNIFORM, false),
                (AddressSpace::Storage { .. }, _) => (STORAGE, false),
                (AddressSpace::Handle, TypeInner::Sampler { comparison: true }) => (SAMPLER_COMPARISON, false),
                (AddressSpace::Handle, TypeInner::Sampler { comparison: false }) => (SAMPLER, false),
                (AddressSpace::Handle, TypeInner::Image { arrayed, class, .. }) => match class {
                    ImageClass::Sampled { .. } if *arrayed => (TEXTURE_ARRAY, false),
                    ImageClass::Sampled { .. } => (TEXTURE, false),
                    ImageClass::Depth { multi } => (DEPTH, *multi),
                    other => panic!("no binding kind for {other:?}"),
                },
                other => panic!("no binding kind for {other:?}"),
            };
            out.insert((rb.group, rb.binding), (used, kind, multi));
        }
        out
    }

    #[test]
    fn every_pipeline_matches_its_shader() {
        for id in 0..PIPELINES {
            let words = pipeline(id);
            assert!(!words.is_empty(), "pipeline {id} has a record");
            let r = Record::read(&words);
            for (shader, msaa) in [(r.shader, false), (r.shader_msaa, true)] {
                let (module, info) = compile(shader);
                // Every input the vertex shader declares is fed by exactly
                // one attribute of the same format, and no attribute feeds
                // an input that is not there.
                let inputs = vertex_inputs(&module);
                let mut fed = BTreeMap::new();
                for (stride, _, attrs) in &r.buffers {
                    for (location, offset, format) in attrs {
                        assert!(fed.insert(*location, *format).is_none(), "pipeline {id}: location {location} fed twice");
                        assert!(offset < stride, "pipeline {id}: location {location} lies within its stride");
                    }
                }
                assert_eq!(fed, inputs, "pipeline {id}: the attributes are shader {shader}'s inputs");
                // Every binding the shader declares has a layout entry of
                // its kind, visible to every stage that reads it; and no
                // entry stands for a binding the shader does not have.
                let declared = bindings(&module, &info);
                let mut laid = BTreeMap::new();
                for (group, entries) in r.groups.iter().enumerate() {
                    for (binding, visibility, kind) in entries {
                        // A dynamic offset is the bind group's business, and
                        // the frame's depth is a depth texture multisampled
                        // as the frame is.
                        let (kind, multi) = match *kind {
                            UNIFORM_DYNAMIC => (UNIFORM, false),
                            FRAME_DEPTH => (DEPTH, msaa),
                            k => (k, false),
                        };
                        assert!(
                            laid.insert((group as u32, *binding), (*visibility, kind, multi)).is_none(),
                            "pipeline {id}: group {group} binding {binding} laid twice"
                        );
                    }
                }
                assert_eq!(
                    laid.keys().collect::<Vec<_>>(),
                    declared.keys().collect::<Vec<_>>(),
                    "pipeline {id}: the layout's slots are shader {shader}'s bindings"
                );
                for (slot, (visibility, kind, multi)) in &laid {
                    let (used, declared_kind, declared_multi) = declared[slot];
                    assert_eq!((*kind, *multi), (declared_kind, declared_multi), "pipeline {id}: the kind at {slot:?}");
                    assert_eq!(*visibility, used, "pipeline {id}: the visibility at {slot:?} is the stages that read it");
                }
                // A pipeline without a fragment stage draws depth only,
                // writing it, at one sample, unblended.
                if !r.fragment {
                    assert!(
                        r.depth_write && r.depth_compare == LESS && r.samples == ONE_SAMPLE && r.blend == OPAQUE,
                        "pipeline {id} is a depth pass"
                    );
                }
            }
        }
    }
}
