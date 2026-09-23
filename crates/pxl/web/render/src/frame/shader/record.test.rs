// pxl-render/frame/shader/record.test.rs

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
    use crate::mesh::chunk_mesher as m;
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
        ("DIAG_RISES_PP", m::DIAG_RISES_PP),
        ("DIAG_RISES_PN", m::DIAG_RISES_PN),
        ("DIAG_RISES_NP", m::DIAG_RISES_NP),
        ("DIAG_RISES_NN", m::DIAG_RISES_NN),
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
    assert_eq!(*span as usize, FRAME_FLOATS * 4);
    let offsets: BTreeMap<&str, usize> = members.iter().map(|m| (m.name.as_deref().unwrap(), m.offset as usize / 4)).collect();
    for (name, at) in FRAME_FIELDS.iter().zip(FRAME_AT) {
        assert_eq!(offsets[name], at, "FRAME_AT for {name}");
    }
    // The environment's run is contiguous from sun_dir, as look/environment.rs
    // writes it: 24 floats to the end of its padding.
    assert_eq!(offsets["_p5"], offsets["sun_dir"] + 20);
    assert_eq!(offsets["other_color"], offsets["other_dir"] + 4);
}

/// A struct's span and its members' offsets, in floats, as `kind`'s shader
/// lays it out.
fn layout(kind: u32, name: &str) -> (usize, BTreeMap<String, usize>) {
    let (module, _) = compile(kind);
    let ty = module.types.iter().find(|(_, t)| t.name.as_deref() == Some(name)).unwrap_or_else(|| panic!("{name}")).1;
    let TypeInner::Struct { members, span } = &ty.inner else { panic!("{name} is a struct") };
    (*span as usize / 4, members.iter().map(|m| (m.name.clone().unwrap(), m.offset as usize / 4)).collect())
}

/// Each named field of `name` in `kind`'s shader sits where the Rust that
/// writes it puts it, and the struct is as long as the Rust's words.
fn holds(kind: u32, name: &str, floats: usize, fields: &[(&str, usize)]) {
    let (span, offsets) = layout(kind, name);
    assert_eq!(span, floats, "{name}'s length");
    for (field, at) in fields {
        assert_eq!(offsets[*field], *at, "{name}.{field}");
    }
}

/// A module constant's value, as the literals it is built from.
fn constant(kind: u32, name: &str) -> Vec<f64> {
    let (module, _) = compile(kind);
    let (_, c) = module.constants.iter().find(|(_, c)| c.name.as_deref() == Some(name)).unwrap_or_else(|| panic!("{name}"));
    let literal = |e: &naga::Expression| match e {
        naga::Expression::Literal(naga::Literal::F32(v)) => *v as f64,
        naga::Expression::Literal(naga::Literal::U32(v)) => *v as f64,
        other => panic!("{name} is literals, not {other:?}"),
    };
    match &module.global_expressions[c.init] {
        naga::Expression::Compose { components, .. } => components.iter().map(|h| literal(&module.global_expressions[*h])).collect(),
        other => vec![literal(other)],
    }
}

/// The uniforms the sky's passes are laid out into, and the tables they
/// read, are where the Rust that writes them puts them.
#[test]
fn the_skys_layouts_match_their_structs() {
    use crate::gpu::{atmosphere as a, clouds as c, renderer as r, stars as s};
    holds(
        SHADER_SKY,
        "Atmosphere",
        a::ATMOS_FLOATS,
        &[
            ("rayleigh_scattering", a::RAYLEIGH_SCATTERING),
            ("bottom_radius", a::BOTTOM_RADIUS),
            ("ozone_absorption", a::OZONE_ABSORPTION),
            ("top_radius", a::TOP_RADIUS),
            ("white", a::WHITE),
            ("rayleigh_scale_height", a::RAYLEIGH_SCALE_HEIGHT),
            ("sun_dir", a::SUN_DIR),
            ("mie_scattering", a::MIE_SCATTERING),
            ("camera_altitude", a::CAMERA_ALTITUDE),
            ("km_per_u", a::KM_PER_U),
            ("mie_extinction", a::MIE_EXTINCTION),
            ("mie_scale_height", a::MIE_SCALE_HEIGHT),
            ("mie_g", a::MIE_G),
            ("ozone_center", a::OZONE_CENTER),
            ("ozone_half_width", a::OZONE_HALF_WIDTH),
            ("ground_albedo", a::GROUND_ALBEDO),
            ("sun_angular_radius", a::SUN_ANGULAR_RADIUS),
            ("exposure", a::EXPOSURE),
            ("max_adaptation", a::MAX_ADAPTATION),
            ("adaptation", a::ADAPTATION),
            ("moon_dir", a::MOON_DIR),
            ("moon_light", a::MOON_LIGHT),
            ("land", a::LAND),
            ("sun_seen", a::SUN_SEEN),
            ("up", a::UP),
            ("cloud_base", a::CLOUD_BASE),
            ("cloud_frac", a::CLOUD_FRAC),
            ("cloud_time", a::CLOUD_TIME),
            ("cloud_count", a::CLOUD_COUNT),
            ("cloud_altitude_km", a::CLOUD_ALTITUDE),
            ("cloud_gone_km", a::CLOUD_GONE),
        ],
    );
    holds(SHADER_SKY, "Sky", r::SKY_FLOATS, &[("ray", 0), ("pole", r::SKY_POLE), ("mapped", r::SKY_MAPPED)]);
    holds(SHADER_CLOUDS, "Clouds", c::CLOUDS_FLOATS, &[("ray", 0), ("sun_at", c::SUN_AT), ("moon_at", c::MOON_AT)]);
    holds(
        SHADER_STARS,
        "Stars",
        s::STARS_FLOATS,
        &[
            ("view_proj", 0),
            ("sky", s::SKY),
            ("viewport", s::VIEWPORT),
            ("intensity", s::INTENSITY),
            ("depth", s::DEPTH),
            ("density", s::DENSITY),
            ("up", s::UP),
        ],
    );
    for (name, k) in [("TRANSMITTANCE_SIZE", a::TRANSMITTANCE), ("MULTISCATTER_SIZE", a::MULTISCATTER), ("SKY_VIEW_SIZE", a::SKY_VIEW)] {
        let (w, h) = a::SIZES[k];
        assert_eq!(constant(SHADER_SKY, name), [w as f64, h as f64], "{name}");
    }
    assert_eq!(constant(SHADER_CLOUDS, "CLOUD_LATTICE_CELLS"), [(1u32 << c::LATTICE_SHAMT) as f64], "the clouds' lattice");
    for (name, v) in [("CLOUD_TILE_UNITS", c::TILE_UNITS), ("CLOUD_TILE_TEXELS", c::TILE_TEXELS)] {
        assert_eq!(constant(SHADER_CLOUDS, name), [v as f64], "the clouds' tile, which the texture is made to");
    }
    // The glare's halo, Spencer's photopic point-spread function without its
    // core, over the sphere: the share of the light it carries, which the
    // shader scales to a lens's.
    let halo = |t: f64| 0.478 * 20.91 / (t + 0.02).powi(3) + 0.138 * 72.37 / (t + 0.02).powi(2);
    let (n, lo, hi) = (200_000, 1e-5f64, 180.0f64);
    let share: f64 = (0..n)
        .map(|i| {
            let (t0, t1) = (lo * (hi / lo).powf(i as f64 / n as f64), lo * (hi / lo).powf((i + 1) as f64 / n as f64));
            halo((t0 + t1) / 2.0) * core::f64::consts::TAU * ((t0 + t1) / 2.0).to_radians().sin() * (t1 - t0).to_radians()
        })
        .sum();
    assert!((constant(SHADER_SKY, "GLARE_HALO_SHARE")[0] - share).abs() < 1e-3, "the glare's halo carries {share}");
    let luma: Vec<f32> = crate::look::environment::LUMA.iter().map(|&v| v as f32).collect();
    assert_eq!(constant(SHADER_SKY, "LUMA").iter().map(|&v| v as f32).collect::<Vec<_>>(), luma, "the luminance weights");
}

/// The vertex format a location's argument type asks for.
fn format_of(module: &Module, ty: naga::Handle<naga::Type>) -> u32 {
    match &module.types[ty].inner {
        TypeInner::Scalar(s) if s.kind == ScalarKind::Float => F32,
        TypeInner::Vector { size: VectorSize::Bi, scalar } if scalar.kind == ScalarKind::Float => F32X2,
        TypeInner::Vector { size: VectorSize::Tri, scalar } if scalar.kind == ScalarKind::Float => F32X3,
        TypeInner::Vector { size: VectorSize::Quad, scalar } if scalar.kind == ScalarKind::Float => F32X4,
        TypeInner::Vector { size: VectorSize::Quad, scalar } if scalar.kind == ScalarKind::Uint => U32X4,
        TypeInner::Scalar(s) if s.kind == ScalarKind::Uint => U32,
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
                    // A dynamic offset is the bind group's business, the
                    // frame's depth is a depth texture multisampled as
                    // the frame is, and whether a float texture is
                    // filterable is its format's, not the shader's.
                    let (kind, multi) = match *kind {
                        UNIFORM_DYNAMIC => (UNIFORM, false),
                        FRAME_DEPTH => (DEPTH, msaa),
                        TEXTURE_UNFILTERED => (TEXTURE, false),
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
            // writing it, at one sample, unblended: the shadow map's
            // plain depth, or the frame's reversed one from the eye.
            if !r.fragment {
                assert!(
                    r.depth_write && (r.depth_compare == LESS || r.depth_compare == GREATER) && r.samples == ONE_SAMPLE && r.blend == OPAQUE,
                    "pipeline {id} is a depth pass"
                );
            }
        }
    }
}

/// Whether `f` calls a function named `name`, or calls one that does.
fn reaches(module: &Module, f: &naga::Function, name: &str) -> bool {
    fn block(module: &Module, b: &naga::Block, name: &str) -> bool {
        b.iter().any(|st| match st {
            naga::Statement::Call { function, .. } => {
                let callee = &module.functions[*function];
                callee.name.as_deref() == Some(name) || reaches(module, callee, name)
            }
            naga::Statement::Block(inner) => block(module, inner, name),
            naga::Statement::If { accept, reject, .. } => block(module, accept, name) || block(module, reject, name),
            naga::Statement::Loop { body, continuing, .. } => block(module, body, name) || block(module, continuing, name),
            naga::Statement::Switch { cases, .. } => cases.iter().any(|c| block(module, &c.body, name)),
            _ => false,
        })
    }
    block(module, &f.body, name)
}

#[test]
fn every_shader_that_places_the_world_lays_it_on_the_planet() {
    for kind in WORLD_SHADERS {
        let (module, _) = compile(kind);
        let vs = module
            .entry_points
            .iter()
            .find(|e| e.stage == ShaderStage::Vertex)
            .unwrap_or_else(|| panic!("shader {kind} has a vertex stage"));
        assert!(reaches(&module, &vs.function, "on_globe_of"), "shader {kind} places the world off the planet");
    }
}
