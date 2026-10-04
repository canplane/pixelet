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
    // The environment's run is contiguous from sun_dir: 24 floats to its padding's end.
    assert_eq!(offsets["_p5"], offsets["sun_dir"] + 20);
    assert_eq!(offsets["other_color"], offsets["other_dir"] + 4);
}

fn layout(kind: u32, name: &str) -> (usize, BTreeMap<String, usize>) {
    let (module, _) = compile(kind);
    let ty = module.types.iter().find(|(_, t)| t.name.as_deref() == Some(name)).unwrap_or_else(|| panic!("{name}")).1;
    let TypeInner::Struct { members, span } = &ty.inner else { panic!("{name} is a struct") };
    (*span as usize / 4, members.iter().map(|m| (m.name.clone().unwrap(), m.offset as usize / 4)).collect())
}

fn holds(kind: u32, name: &str, floats: usize, fields: &[(&str, usize)]) {
    let (span, offsets) = layout(kind, name);
    assert_eq!(span, floats, "{name}'s length");
    for (field, at) in fields {
        assert_eq!(offsets[*field], *at, "{name}.{field}");
    }
}

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

#[test]
fn the_skys_layouts_match_their_structs() {
    use crate::gpu::{atmosphere as a, clouds as c, renderer as r, stars as s};
    assert_eq!(constant(SHADER_SKY, "ATMOS_VEC4S"), [(a::ATMOS_FLOATS / 4) as f64], "the air's words, declared once");
    assert_eq!(constant(SHADER_SKY, "EARTH_PAGE_BITS"), [a::PAGE_BITS as f64], "the Earth window's packing");
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
            ("sun_seen", a::SUN_SEEN),
            ("center_lon", a::CENTER_LON),
            ("night_sky", a::NIGHT_SKY),
            ("land_altitude_km", a::LAND_ALTITUDE),
            ("cloud_base", a::CLOUD_BASE),
            ("cloud_frac", a::CLOUD_FRAC),
            ("cloud_time", a::CLOUD_TIME),
            ("cloud_count", a::CLOUD_COUNT),
            ("cloud_altitude_km", a::CLOUD_ALTITUDE),
            ("cloud_gone_km", a::CLOUD_GONE),
            ("city_light", a::CITY_LIGHT),
            ("earth_reach", a::EARTH_REACH),
            ("earth_window", a::EARTH_WINDOW),
            ("earth_pages", a::EARTH_PAGES),
        ],
    );
    holds(SHADER_SKY, "Sky", r::SKY_FLOATS, &[("ray", 0), ("pole", r::SKY_POLE), ("mapped", r::SKY_MAPPED)]);
    holds(SHADER_CLOUDS, "Clouds", c::CLOUDS_FLOATS, &[("ray", 0)]);
    holds(
        SHADER_STARS,
        "Stars",
        s::STARS_FLOATS,
        &[
            ("view_proj", 0),
            ("sky", s::SKY),
            ("depth", s::DEPTH),
            ("density", s::DENSITY),
            ("intensity", s::INTENSITY),
            ("horizon", s::HORIZON),
            ("moon", s::MOON),
        ],
    );
    for (name, k) in [("TRANSMITTANCE_SIZE", a::TRANSMITTANCE), ("MULTISCATTER_SIZE", a::MULTISCATTER), ("SKY_VIEW_SIZE", a::SKY_VIEW), ("IRRADIANCE_SIZE", a::IRRADIANCE)] {
        let (w, h) = a::SIZES[k];
        assert_eq!(constant(SHADER_SKY, name), [w as f64, h as f64], "{name}");
    }
    let [across, up, slices] = constant(SHADER_SKY, "AERIAL_SIZE")[..] else { panic!("the aerial perspective's three axes") };
    assert_eq!(a::SIZES[a::AERIAL], ((across * slices) as u32, up as u32), "the aerial perspective's slices side by side");
    assert_eq!(constant(SHADER_CLOUDS, "CLOUD_LATTICE_CELLS"), [(1u32 << c::LATTICE_SHAMT) as f64], "the clouds' lattice");
    let [cell] = constant(SHADER_CLOUDS, "WEATHER_CELL")[..] else { panic!("one weather cell") };
    assert!(c::LATTICE_COUNT.is_multiple_of(cell as u32), "the weather wraps with the map in whole cells");
    for (name, v) in [("CLOUD_TILE_UNITS", c::TILE_UNITS), ("CLOUD_TILE_TEXELS", c::TILE_TEXELS)] {
        assert_eq!(constant(SHADER_CLOUDS, name), [v as f64], "the clouds' tile, which the texture is made to");
    }
    // Spencer's photopic PSF without its core: the share of light the halo carries.
    let halo = |t: f64| 0.478 * 20.91 / (t + 0.02).powi(3) + 0.138 * 72.37 / (t + 0.02).powi(2);
    let (n, lo, hi) = (200_000, 1e-5f64, 180.0f64);
    let share: f64 = (0..n)
        .map(|i| {
            let (t0, t1) = (lo * libm::pow(hi / lo, i as f64 / n as f64), lo * libm::pow(hi / lo, (i + 1) as f64 / n as f64));
            halo((t0 + t1) / 2.0) * core::f64::consts::TAU * libm::sin(((t0 + t1) / 2.0).to_radians()) * (t1 - t0).to_radians()
        })
        .sum();
    assert!((constant(SHADER_SKY, "GLARE_HALO_SHARE")[0] - share).abs() < 1e-3, "the glare's halo carries {share}");
    let luma: Vec<f32> = crate::look::environment::LUMA.iter().map(|&v| v as f32).collect();
    assert_eq!(constant(SHADER_SKY, "LUMA").iter().map(|&v| v as f32).collect::<Vec<_>>(), luma, "the luminance weights");
}

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
            let inputs = vertex_inputs(&module, r.vertex);
            let mut fed = BTreeMap::new();
            for (stride, _, attrs) in &r.buffers {
                for (location, offset, format) in attrs {
                    assert!(fed.insert(*location, *format).is_none(), "pipeline {id}: location {location} fed twice");
                    assert!(offset < stride, "pipeline {id}: location {location} lies within its stride");
                }
            }
            assert_eq!(fed, inputs, "pipeline {id}: the attributes are shader {shader}'s inputs");
            let declared = bindings(&module, &info);
            let mut laid = BTreeMap::new();
            for (group, entries) in r.groups.iter().enumerate() {
                for (binding, visibility, kind) in entries {
                    // A dynamic offset is the bind group's business, and a float texture's
                    // filterability is its format's.
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
            if !r.fragment {
                assert!(
                    r.depth_write && (r.depth_compare == LESS || r.depth_compare == GREATER) && r.blend == OPAQUE,
                    "pipeline {id} is a depth pass"
                );
            }
        }
    }
}

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

/// Shaders that place a world point must lay it on the globe before projecting,
/// or it floats off the ground as the planet falls away.
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

/// No gaps a layout rule could place differently: a vec3, a matrix or a padded
/// struct is where a Galaxy S23's Vulkan read what the WGSL did not say.
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

fn floats_read(f: &naga::Function, e: naga::Handle<naga::Expression>) -> Vec<u32> {
    use naga::Expression as E;
    // Depth (0 the array, 1 a word, 2 a lane) and the float it starts at.
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

/// Each field's accessor reads the word and lane WGSL lays that field at, and
/// nothing builds the struct back: that lost Safari's WebGPU its GPU process.
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
        (SHADER_IMPOSTOR, "draw", "Draw"),
        (SHADER_DEBUG, "draw", "Draw"),
        (SHADER_PUCK_ZONE, "draw", "Draw"),
        (SHADER_SKINNED, "draw", "Draw"),
        (SHADER_SKY, "sky", "Sky"),
        (SHADER_STARS, "stars", "Stars"),
    ];
    for (kind, prefix, block) in blocks {
        let (module, _) = compile(kind);
        let ty = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some(block))
            .unwrap_or_else(|| panic!("shader {kind}: struct {block}"))
            .0;
        let TypeInner::Struct { ref members, .. } = module.types[ty].inner else {
            panic!("{block} is a struct")
        };
        let held = module.global_variables.iter().any(|(_, g)| g.ty == ty);
        assert!(!held, "shader {kind}: a global holds a {block}");
        let functions = module.functions.iter().map(|(_, f)| f).chain(module.entry_points.iter().map(|e| &e.function));
        for f in functions {
            let builds = f.result.as_ref().is_some_and(|r| r.ty == ty)
                || f.arguments.iter().any(|a| a.ty == ty)
                || f.local_variables.iter().any(|(_, v)| v.ty == ty)
                || f.expressions.iter().any(|(_, e)| matches!(e, naga::Expression::Compose { ty: t, .. } if *t == ty));
            assert!(!builds, "shader {kind}: {} builds a {block} back", f.name.as_deref().unwrap_or("?"));
        }
        for m in members {
            let name = m.name.as_deref().unwrap_or("?");
            if name.starts_with('_') {
                continue;
            }
            let fun = format!("{prefix}_{name}");
            let (_, f) = module
                .functions
                .iter()
                .find(|(_, f)| f.name.as_deref() == Some(fun.as_str()))
                .unwrap_or_else(|| panic!("shader {kind}: fn {fun}"));
            let value = f
                .body
                .iter()
                .find_map(|st| match st {
                    naga::Statement::Return { value } => *value,
                    _ => None,
                })
                .expect("a return");
            assert_eq!(floats_read(f, value), floats_of(&module, m.ty, m.offset), "shader {kind}: {block}.{name} is read from where it lies");
        }
    }
}
