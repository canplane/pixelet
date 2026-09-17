// pxl_render/shader.test.rs

use std::collections::BTreeMap;

use naga::valid::{Capabilities, ValidationFlags, Validator};
use naga::{AddressSpace, Binding, ImageClass, Module, ScalarKind, ShaderStage, TypeInner, VectorSize};

use super::*;

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

/// The shader's face bits are the mesher's: each named constant in
/// faces.wgsl equals the mesher's, and the lateral groups sit at
/// consecutive bits from their base in the order the shader multiplies
/// by.
#[test]
fn the_face_bits_are_the_meshers() {
    use crate::layer::pipeline::chunk_mesher as m;
    let (module, _) = compile(SHADER_CHUNK);
    let value = |name: &str| -> u32 {
        let (_, c) = module
            .constants
            .iter()
            .find(|(_, c)| c.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("faces.wgsl has {name}"));
        match module.global_expressions[c.init] {
            naga::Expression::Literal(naga::Literal::U32(v)) => v,
            ref other => panic!("{name} is a literal, not {other:?}"),
        }
    };
    for (name, base, group) in [
        ("FACE_BASE", m::FACE_PX, [m::FACE_PX, m::FACE_NX, m::FACE_PY, m::FACE_NY]),
        ("RISES_BASE", m::RISES_PX, [m::RISES_PX, m::RISES_NX, m::RISES_PY, m::RISES_NY]),
        ("DIPS_BASE", m::DIPS_PX, [m::DIPS_PX, m::DIPS_NX, m::DIPS_PY, m::DIPS_NY]),
        ("CREASE_BASE", m::CREASE_PX, [m::CREASE_PX, m::CREASE_NX, m::CREASE_PY, m::CREASE_NY]),
    ] {
        assert_eq!(value(name), base, "{name}");
        assert_eq!(group, [base, base * 2, base * 4, base * 8], "{name}'s laterals in the shader's order");
    }
    for (name, bit) in [
        ("DIAG_PP", m::DIAG_PP),
        ("DIAG_PN", m::DIAG_PN),
        ("DIAG_NP", m::DIAG_NP),
        ("DIAG_NN", m::DIAG_NN),
        ("CAP_PZ", m::CAP_PZ),
        ("CAP_NZ", m::CAP_NZ),
    ] {
        assert_eq!(value(name), bit, "{name}");
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
    assert_eq!(offsets["other_color"], offsets["other_dir"] + 4);
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
