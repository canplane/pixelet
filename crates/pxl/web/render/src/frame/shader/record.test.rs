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
    for (name, bit) in [("CAP_PZ", m::CAP_PZ), ("CAP_NZ", m::CAP_NZ), ("UNDER_AS_TOP", m::UNDER_AS_TOP)] {
        assert_eq!(value(name), bit, "{name}");
    }
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
            ("land_altitude_km", a::LAND_ALTITUDE),
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
        TypeInner::Vector { size: VectorSize::Bi, scalar } if scalar.kind == ScalarKind::Uint => U32X2,
        TypeInner::Scalar(s) if s.kind == ScalarKind::Uint => U32,
        other => panic!("no vertex format for {other:?}"),
    }
}

/// The vertex shader's inputs by location, and their formats.
fn vertex_inputs(module: &Module, entry: u32) -> BTreeMap<u32, u32> {
    let name = VERTEX_ENTRY[entry as usize];
    let vs = module.entry_points.iter().find(|e| e.stage == ShaderStage::Vertex && e.name == name).unwrap();
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
/// that use it, its kind, whether a depth texture is multisampled — which
/// none may be).
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
        let shader = r.shader;
        {
            let (module, info) = compile(shader);
            // Every input the vertex shader declares is fed by exactly
            // one attribute of the same format, and no attribute feeds
            // an input that is not there.
            let inputs = vertex_inputs(&module, r.vertex);
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
                    // whether a float texture is filterable is its
                    // format's, not the shader's.
                    let (kind, multi) = match *kind {
                        UNIFORM_DYNAMIC => (UNIFORM, false),
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
            // writing it, unblended: the shadow map's
            // plain depth, or the frame's reversed one from the eye.
            if !r.fragment {
                assert!(
                    r.depth_write && (r.depth_compare == LESS || r.depth_compare == GREATER) && r.blend == OPAQUE,
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

/// The shaders that place a point of the world, each of which lays it on the
/// planet as the ground is (globe.wgsl's `on_globe_of`) before it projects
/// it: what is drawn over the map and not laid on it floats off the ground
/// as the planet falls away. The rest draw the sky or work on the screen.
const WORLD_SHADERS: [u32; 7] = [SHADER_CHUNK, SHADER_DECOR, SHADER_SKINNED, SHADER_PUCK_ZONE, SHADER_PUCK_BEACON, SHADER_BADGE, SHADER_DEBUG];

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

/// Whether a type the host shares with a shader has no room inside it: its
/// words run end to end, so no layout rule has a gap to place differently.
/// A four-byte scalar, a four-lane vector of them, an array of those whose
/// stride is its element, or a struct of those whose members follow one
/// another with nothing between. A `vec3`, a matrix and a padded struct are
/// where a Galaxy S23's Vulkan read what the WGSL did not say
/// (issue/20260927-a-matrix-the-driver-reads-its-own-way.md).
fn has_no_room(module: &Module, ty: naga::Handle<naga::Type>) -> bool {
    let ctx = module.to_ctx();
    match &module.types[ty].inner {
        TypeInner::Scalar(s) => s.width == 4,
        TypeInner::Vector { size: VectorSize::Quad, scalar } => scalar.width == 4,
        TypeInner::Array { base, stride, .. } => has_no_room(module, *base) && *stride == module.types[*base].inner.size(ctx),
        TypeInner::Struct { members, span } => {
            let mut end = 0;
            for m in members {
                if m.offset != end || !has_no_room(module, m.ty) {
                    return false;
                }
                end += module.types[m.ty].inner.size(ctx);
            }
            end == *span
        }
        _ => false,
    }
}

#[test]
fn what_a_shader_shares_with_the_host_has_no_room_inside_it() {
    for kind in 0..SHADERS {
        let (module, _) = compile(kind);
        for (_, var) in module.global_variables.iter() {
            if matches!(var.space, AddressSpace::Uniform | AddressSpace::Storage { .. }) {
                let name = var.name.as_deref().unwrap_or("?");
                assert!(has_no_room(&module, var.ty), "shader {kind}: `{name}` is a type a driver may lay out its own way");
            }
        }
    }
}

/// The floats of the words global an expression reads, in order: a load of a
/// word or of a lane, and what a swizzle, a lane, a bitcast or a constructor
/// makes of those.
fn floats_read(f: &naga::Function, e: naga::Handle<naga::Expression>) -> Vec<u32> {
    use naga::Expression as E;
    // A pointer into the words: how deep it is (0 the array, 1 a word, 2 a
    // lane) and the float it starts at.
    fn place(f: &naga::Function, e: naga::Handle<naga::Expression>) -> Option<(u32, u32)> {
        let index = |i: naga::Handle<naga::Expression>| match f.expressions[i] {
            E::Literal(naga::Literal::U32(v)) => v,
            E::Literal(naga::Literal::I32(v)) => v as u32,
            E::Literal(naga::Literal::AbstractInt(v)) => v as u32,
            ref other => panic!("a constant index, not {other:?}"),
        };
        let step = |base, i: u32| {
            let (depth, at) = place(f, base)?;
            Some(match depth {
                0 => (1, i * 4),
                1 => (2, at + i),
                _ => panic!("no deeper than a lane"),
            })
        };
        match f.expressions[e] {
            E::GlobalVariable(_) => Some((0, 0)),
            E::AccessIndex { base, index: i } => step(base, i),
            E::Access { base, index: i } => step(base, index(i)),
            _ => None,
        }
    }
    match f.expressions[e] {
        E::Load { pointer } => match place(f, pointer) {
            Some((1, at)) => (at..at + 4).collect(),
            Some((2, at)) => vec![at],
            other => panic!("a load of a word or a lane, not {other:?}"),
        },
        E::Swizzle { size, vector, pattern } => {
            let v = floats_read(f, vector);
            pattern[..size as usize].iter().map(|c| v[*c as usize]).collect()
        }
        E::AccessIndex { base, index } => vec![floats_read(f, base)[index as usize]],
        E::As { expr, convert: None, .. } => floats_read(f, expr),
        E::Compose { ref components, .. } => components.iter().flat_map(|c| floats_read(f, *c)).collect(),
        ref other => panic!("{other:?} reads no word"),
    }
}

/// The floats a value of `ty` at byte `offset` lies on, as WGSL lays it out:
/// a matrix's columns each start on their alignment.
fn floats_of(module: &Module, ty: naga::Handle<naga::Type>, offset: u32) -> Vec<u32> {
    let at = offset / 4;
    match module.types[ty].inner {
        TypeInner::Scalar(_) => vec![at],
        TypeInner::Vector { size, .. } => (at..at + size as u32).collect(),
        TypeInner::Matrix { columns, rows, .. } => {
            let column = if rows == VectorSize::Bi { 2 } else { 4 };
            (0..columns as u32).flat_map(|c| (0..rows as u32).map(move |r| at + c * column + r)).collect()
        }
        ref other => panic!("no field of {other:?} in a shared block"),
    }
}

/// Each shared block read back by its function (`frame()` for the `Frame`
/// its words hold): every field is the word and the lane WGSL lays that
/// field at, so the struct the Rust's offsets are held against by the tests
/// above is the struct the shader reads.
#[test]
fn every_block_reads_each_field_from_where_it_lies() {
    let blocks = [
        (SHADER_CHUNK, "frame", "Frame"),
        (SHADER_SKY, "atmos", "Atmosphere"),
        (SHADER_ATMOS_TRANSMITTANCE, "atmos", "Atmosphere"),
        (SHADER_CLOUDS, "clouds", "Clouds"),
        (SHADER_AO_SAMPLE, "ao", "Ao"),
        (SHADER_BADGE, "badge", "Badge"),
        (SHADER_DECOR, "draw", "Draw"),
        (SHADER_DEBUG, "draw", "Draw"),
        (SHADER_PUCK_ZONE, "draw", "Draw"),
        (SHADER_SKINNED, "draw", "Draw"),
        (SHADER_SKY, "sky", "Sky"),
        (SHADER_STARS, "stars", "Stars"),
    ];
    for (kind, fun, block) in blocks {
        let (module, _) = compile(kind);
        let (_, f) = module
            .functions
            .iter()
            .find(|(_, f)| f.name.as_deref() == Some(fun))
            .unwrap_or_else(|| panic!("shader {kind}: fn {fun}"));
        let ty = f.result.as_ref().expect("a block").ty;
        assert_eq!(module.types[ty].name.as_deref(), Some(block), "shader {kind}: {fun}() is a {block}");
        let TypeInner::Struct { ref members, .. } = module.types[ty].inner else {
            panic!("{block} is a struct")
        };
        let value = f
            .body
            .iter()
            .find_map(|st| match st {
                naga::Statement::Return { value } => *value,
                _ => None,
            })
            .expect("a return");
        let naga::Expression::Compose { ref components, .. } = f.expressions[value] else {
            panic!("{fun}() returns a {block} it builds")
        };
        assert_eq!(components.len(), members.len(), "shader {kind}: {fun}() gives every field");
        for (m, c) in members.iter().zip(components) {
            let name = m.name.as_deref().unwrap_or("?");
            assert_eq!(floats_read(f, *c), floats_of(&module, m.ty, m.offset), "shader {kind}: {block}.{name} is read from where it lies");
        }
    }
}
