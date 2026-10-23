// pxl-render/build.rs

// The shipped WGSL is the shader files with comments and indentation stripped; nothing is renamed.

use std::path::Path;

#[path = "src/frame/shader/parts.rs"]
#[allow(dead_code)]
mod parts;
#[path = "src/frame/shader/words.rs"]
#[allow(dead_code)]
mod words;

fn main() {
    let dir = Path::new("src/frame/shader");
    let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("shader");
    std::fs::create_dir_all(out.join("assembled")).expect("the shader directory");
    for entry in std::fs::read_dir(dir).expect("the shaders") {
        let path = entry.expect("a shader").path();
        if path.extension().and_then(|e| e.to_str()) != Some("wgsl") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a shader's text");
        let name = path.file_name().expect("a shader's name");
        let mut shipped = stripped(&text);
        if name == "constant.wgsl" {
            shipped += &words();
        }
        shipped += &accessors(&text);
        std::fs::write(out.join(name), shipped).expect("the stripped shader");
    }
    // A unit's zoom, a unit's fraction bits in Q3 and the ground's cell, from
    // pxl-spec-core, where the cast reads them: a ray is painted by its
    // block's zoom against these, its block placed off the eye's Q3 steps.
    let lattice = format!(
        "const UNIT_ZOOM: u32 = {}u;\nconst FRAC_W: u32 = {}u;\nconst GROUND_CELL: u32 = {}u;\n",
        pxl_spec_core::UNIT_ZOOM,
        pxl_spec_core::FRAC_W,
        pxl_spec_core::lattice::GROUND.cell()
    );
    std::fs::write(out.join("lattice.wgsl"), lattice).expect("the lattice's shader");
    // A ray's first word, its fields as the cast writes them (pxl-spec-core
    // ray.rs), and each field read as the cast reads it.
    use pxl_spec_core::ray::*;
    let field = |name: &str, shift: u32, bits: u32| format!("fn ray_{name}(w: u32) -> u32 {{ return (w >> {shift}u) & {}u; }}\n", (1u32 << bits) - 1);
    // Its word of what lies about its face, and that word's fields, a bit an
    // edge or a corner (pxl-spec-core faces.rs).
    use pxl_spec_core::faces::{CORNER_SHIFT, LINE_SHIFT, RISE_SHIFT};
    let faces = format!("const RAY_FACES: u32 = {FACES}u;\nconst FACES_LINE: u32 = {LINE_SHIFT}u;\nconst FACES_RISE: u32 = {RISE_SHIFT}u;\nconst FACES_CORNER: u32 = {CORNER_SHIFT}u;\n");
    // The side of the box a block kept short of its cube is, by the eighths
    // it keeps, 0 for all eight (pxl-spec-core answer.rs `side`).
    let sides: String = (1..pxl_spec_core::answer::EIGHTHS).map(|k| format!("case {k}u: {{ return {:?}; }} ", pxl_spec_core::answer::side(k))).collect();
    let kept = format!("fn kept_side(k: u32) -> f32 {{ switch k {{ {sides}default: {{ return 1.0; }} }} }}\n");
    let ray = [field("value", 0, VALUE_BITS), field("face", FACE_SHIFT, FACE_BITS), field("zoom", ZOOM_SHIFT, ZOOM_BITS), field("kept", KEPT_SHIFT, KEPT_BITS), kept, faces];
    std::fs::write(out.join("ray.wgsl"), ray.concat()).expect("the ray's shader");
    // The eye's words the screen pass reads (pxl-spec-core eye.rs), each its
    // word, and the vec4s it binds to reach the last of them.
    use pxl_spec_core::eye::{FORWARD, ORIGIN, REACH, RIGHT, STEPS, TAN, UP};
    let last = (ORIGIN + 2).max(STEPS).max(REACH).max(FORWARD + 2).max(RIGHT + 2).max(UP + 2).max(TAN + 1);
    let eye = format!(
        "const EYE_ORIGIN: u32 = {ORIGIN}u;\nconst EYE_STEPS: u32 = {STEPS}u;\nconst EYE_REACH: u32 = {REACH}u;\nconst EYE_FORWARD: u32 = {FORWARD}u;\nconst EYE_RIGHT: u32 = {RIGHT}u;\nconst EYE_UP: u32 = {UP}u;\nconst EYE_TAN: u32 = {TAN}u;\nconst EYE_READ_VEC4S: u32 = {}u;\n",
        last / 4 + 1
    );
    std::fs::write(out.join("eye.wgsl"), eye).expect("the eye's shader");
    // A sun slot's word read as its depth, a share of the light box's
    // (pxl-spec-core sun.rs `depth_of`): its top bits over the zoom asked.
    use pxl_spec_core::sun::{DEPTH_ONE, ZOOM_BITS as SUN_ZOOM_BITS};
    let sun = format!("fn sun_depth(w: u32) -> f32 {{ return f32(w >> {SUN_ZOOM_BITS}u) / {DEPTH_ONE}.0; }}\n");
    std::fs::write(out.join("sun_word.wgsl"), sun).expect("the sun word's shader");
    std::fs::write(Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("compute.rs"), compute(dir)).expect("the compute layouts");
    assemble(&out);
    println!("cargo:rerun-if-changed=src/frame/shader");
    println!("cargo:rerun-if-changed=src/frame/shader/words.rs");
}

/// WGSL has no string literals, so `//` always starts a comment. Lines are never
/// joined, so tokens cannot run together; block comments are left alone.
fn stripped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let code = match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        };
        let code = code.trim();
        if code.is_empty() {
            continue;
        }
        out.push_str(code);
        out.push('\n');
    }
    out
}

/// Each generated compute entry's group 0 as its WGSL declares it, a binding
/// its index, its stages and its kind (frame/shader/record.rs's codes), and
/// its workgroup: what a pipeline's layout is made of and a dispatch counts
/// by, read off the one module rust-gpu wrote, never stated again.
fn compute(dir: &Path) -> String {
    let mut out = String::new();
    let mut files: Vec<_> = std::fs::read_dir(dir).expect("the shaders").map(|e| e.expect("a shader").path()).collect();
    files.sort();
    for path in files {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let Some(stem) = name.strip_suffix(".gen.wgsl") else { continue };
        let text = std::fs::read_to_string(&path).expect("a generated shader");
        let lines: Vec<&str> = text.lines().collect();
        let mut group = Vec::new();
        let mut workgroup = None;
        for (k, line) in lines.iter().enumerate() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("@group(0) @binding(") {
                let binding: u32 = rest.split(')').next().and_then(|n| n.parse().ok()).expect("a binding's index");
                let var = lines.get(k + 1).map(|l| l.trim()).unwrap_or_default();
                let kind = if var.starts_with("var<uniform>") {
                    "UNIFORM"
                } else if var.starts_with("var<storage, read_write>") {
                    "STORAGE_RW"
                } else if var.starts_with("var<storage>") {
                    "STORAGE"
                } else if var.contains("texture_storage_2d_array<r32uint,read_write>") {
                    "STORAGE_TEXTURE_RW"
                } else {
                    panic!("{name}: binding {binding} is of no kind a layout names: {var}");
                };
                group.push((binding, kind));
            }
            if let Some(rest) = line.strip_prefix("@compute @workgroup_size(") {
                workgroup = Some(rest.split(')').next().expect("a workgroup").replace(' ', ""));
            }
        }
        group.sort();
        let upper = stem.to_uppercase();
        let entries: Vec<String> = group.iter().map(|(b, k)| format!("{b}, COMPUTE, {k}")).collect();
        out += &format!("pub const {upper}_GROUP: [u32; {}] = [{}];\n", 3 * group.len(), entries.join(", "));
        out += &format!("pub const {upper}_WORKGROUP: [u32; 3] = [{}];\n", workgroup.expect("an entry's workgroup"));
        // Its layout within what every device binds (frame/shader/floor.rs).
        out += &format!("const _: () = assert!(crate::frame::shader::floor::holds(&[&{upper}_GROUP]), \"{stem}'s group binds past WebGPU's floor\");\n");
    }
    out
}

/// What the host lays out and every shader reads (frame/shader/words.rs), as
/// WGSL: a uniform's floats as its vec4s.
fn words() -> String {
    use words::*;
    let vec4s = |name: &str, floats: usize| {
        assert!(floats.is_multiple_of(4), "{name} is whole vec4s");
        format!("const {name}_VEC4S: u32 = {}u;\n", floats / 4)
    };
    [
        vec4s("DRAW", DRAW_FLOATS),
        vec4s("FRAME", FRAME_FLOATS),
        vec4s("SKY", SKY_FLOATS),
        vec4s("ATMOS", ATMOS_FLOATS),
        vec4s("CLOUDS", CLOUDS_FLOATS),
        vec4s("STARS", STARS_FLOATS),
        vec4s("AO", AO_FLOATS),
        format!("const MOON_TEXELS: u32 = {MOON_TEXELS}u;\nconst NO_LAYER: f32 = {NO_LAYER}.0;\n"),
        format!("const SLOT_BITS: u32 = {SLOT_BITS}u;\nconst NO_SLOT: u32 = {NO_SLOT}u;\n"),
        format!("const IMAGE_TOP: u32 = {IMAGE_TOP}u;\nconst IMAGE_SIDE: u32 = {IMAGE_SIDE}u;\nconst IMAGE_BOTTOM: u32 = {IMAGE_BOTTOM}u;\n"),
        format!("const EARTH_TILE_BITS: u32 = {EARTH_TILE_BITS}u;\n"),
        format!("const AIR_FROXELS: u32 = {AIR_FROXELS}u;\n"),
        format!("const CLOUD_TILE_UNITS: u32 = {CLOUD_TILE_UNITS}u;\nconst CLOUD_TILE_TEXELS: u32 = {CLOUD_TILE_TEXELS}u;\n"),
        // Q3's Z, the world's height about sea level (pxl-spec-core).
        format!("const HEIGHT_BITS: u32 = {}u;\n", pxl_spec_core::HEIGHT_BITS),
    ]
    .concat()
}

/// Each field's accessor of the uniform whose words `text` binds
/// (`<uniform>_words`, frame/shader/words.rs `UNIFORMS`): the field read off
/// its vec4s where the host wrote it, never as a struct, which lost Safari's
/// WebGPU its GPU process. Each field is checked to lie within its vec4s and
/// apart from the others.
fn accessors(text: &str) -> String {
    use words::{floats, Kind, UNIFORMS};
    let mut out = String::new();
    for (uniform, fields) in UNIFORMS {
        let words = format!("{uniform}_words");
        if !text.contains(&format!("var<uniform> {words}:")) {
            continue;
        }
        let mut taken = vec![false; floats(fields)];
        for f in fields {
            let (row, col, n) = (f.at / 4, f.at % 4, f.kind.floats());
            for t in &mut taken[f.at..f.at + n] {
                assert!(!*t, "{uniform}'s {} lies over another field", f.name);
                *t = true;
            }
            let lanes = &"xyzw"[col..(col + n).min(4)];
            let column = |k: usize| format!("{words}[{}].xyz", row + k);
            let vec4 = |k: usize| format!("{words}[{}]", row + k);
            let (ty, read) = match f.kind {
                Kind::F32 => ("f32", format!("{words}[{row}].{lanes}")),
                Kind::U32 => ("u32", format!("bitcast<u32>({words}[{row}].{lanes})")),
                Kind::Vec2 => ("vec2f", format!("{words}[{row}].{lanes}")),
                Kind::Vec2u => ("vec2u", format!("bitcast<vec2u>({words}[{row}].{lanes})")),
                Kind::Vec3 => ("vec3f", format!("{words}[{row}].{lanes}")),
                Kind::Vec4i => ("vec4i", format!("bitcast<vec4i>({})", vec4(0))),
                Kind::Vec4u => ("vec4u", format!("bitcast<vec4u>({})", vec4(0))),
                Kind::Mat3 => ("mat3x3f", format!("mat3x3f({}, {}, {})", column(0), column(1), column(2))),
                Kind::Mat4 => ("mat4x4f", format!("mat4x4f({}, {}, {}, {})", vec4(0), vec4(1), vec4(2), vec4(3))),
            };
            let whole = matches!(f.kind, Kind::Vec4i | Kind::Vec4u | Kind::Mat3 | Kind::Mat4);
            assert!(if whole { col == 0 } else { col + n <= 4 }, "{uniform}'s {} crosses a vec4", f.name);
            out += &format!("fn {uniform}_{}() -> {ty} {{ return {read}; }}\n", f.name.to_lowercase());
        }
    }
    out
}

/// Every shader assembled from its parts (parts.rs `PARTS`), validated as
/// WebGPU takes it, every capability off; written where record.rs `source`
/// includes it; and which stages read each of its bindings, read off its
/// entry points (`STAGES`, in OUT_DIR), which every pipeline's groups take their
/// visibility from (record.rs `pipeline`).
fn assemble(out: &Path) {
    use naga::valid::{Capabilities, ValidationFlags, Validator};
    let read = |name: &str| std::fs::read_to_string(out.join(format!("{name}.wgsl"))).unwrap_or_else(|e| panic!("{name}.wgsl: {e}"));
    let constant = read("constant");
    let (mut sources, mut stages) = (String::new(), String::new());
    for (kind, parts) in parts::PARTS.iter().enumerate() {
        let generated = parts.len() == 1 && parts[0].ends_with(".gen");
        let mut text = if generated { String::new() } else { constant.clone() + "\n" };
        for part in parts.iter() {
            text += &read(part);
            text += "\n";
        }
        let module = naga::front::wgsl::parse_str(&text).unwrap_or_else(|e| panic!("shader {kind} ({parts:?}): {}", e.emit_to_string(&text)));
        let info = Validator::new(ValidationFlags::all(), Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("shader {kind} ({parts:?}): {}", e.emit_to_string(&text)));
        let file = format!("{kind}.wgsl");
        std::fs::write(out.join("assembled").join(&file), &text).expect("an assembled shader");
        sources += &format!("        {kind} => include_str!(concat!(env!(\"OUT_DIR\"), \"/shader/assembled/{file}\")),\n");
        // Each binding and the stages whose entry points read it.
        let mut read_by: Vec<(u32, u32, Vec<&str>)> = Vec::new();
        for (handle, var) in module.global_variables.iter() {
            let Some(b) = var.binding else { continue };
            let mut by = Vec::new();
            for (i, entry) in module.entry_points.iter().enumerate() {
                let stage = match entry.stage {
                    naga::ShaderStage::Vertex => "VERTEX",
                    naga::ShaderStage::Fragment => "FRAGMENT",
                    _ => "COMPUTE",
                };
                if !info.get_entry_point(i)[handle].is_empty() && !by.contains(&stage) {
                    by.push(stage);
                }
            }
            read_by.push((b.group, b.binding, by));
        }
        read_by.sort();
        let entries: Vec<String> = read_by
            .iter()
            .map(|(g, b, by)| format!("[{g}, {b}, {}]", if by.is_empty() { "NONE".to_string() } else { by.join(" | ") }))
            .collect();
        stages += &format!("    &[{}],\n", entries.join(", "));
    }
    let root = out.parent().expect("OUT_DIR");
    std::fs::write(
        root.join("sources.rs"),
        format!("/// Shader `kind` as build.rs assembled and validated it (parts.rs).\npub fn source(kind: u32) -> &'static str {{\n    match kind {{\n{sources}        _ => \"\",\n    }}\n}}\n"),
    )
    .expect("the shaders' sources");
    std::fs::write(
        root.join("stages.rs"),
        format!("/// Each shader's bindings, [group, binding, the stages that read it], read off it by build.rs.\npub const STAGES: [&[[u32; 3]]; SHADERS as usize] = [\n{stages}];\n"),
    )
    .expect("the shaders' stages");
}
