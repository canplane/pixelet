// pxl-render/build.rs

// The shipped WGSL is the shader files with comments and indentation stripped; nothing is renamed.

use std::path::Path;

#[path = "src/frame/shader/words.rs"]
mod words;

fn main() {
    let dir = Path::new("src/frame/shader");
    let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("shader");
    std::fs::create_dir_all(&out).expect("the shader directory");
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
        std::fs::write(out.join(name), shipped).expect("the stripped shader");
    }
    // A block's face bits, from their one statement (pxl-portable mask.bits.rs).
    std::fs::write(out.join("faces.wgsl"), MASK.iter().map(|(name, v)| format!("const {name}: u32 = {v}u;\n")).collect::<String>()).expect("the mask's shader");
    // A unit's zoom, a unit's fraction bits in Q3 and the ground's cell, from
    // pxl-portable, where the cast reads them: a ray is painted by its
    // block's zoom against these, its block placed off the eye's Q3 steps.
    let lattice = format!(
        "const UNIT_ZOOM: u32 = {}u;\nconst FRAC_W: u32 = {}u;\nconst GROUND_CELL: u32 = {}u;\n",
        pxl_portable::UNIT_ZOOM,
        pxl_portable::FRAC_W,
        pxl_portable::lattice::GROUND.cell()
    );
    std::fs::write(out.join("lattice.wgsl"), lattice).expect("the lattice's shader");
    // A ray's first word, its fields as the cast writes them (pxl-portable
    // ray.rs), and each field read as the cast reads it.
    use pxl_portable::ray::*;
    let field = |name: &str, shift: u32, bits: u32| format!("fn ray_{name}(w: u32) -> u32 {{ return (w >> {shift}u) & {}u; }}\n", (1u32 << bits) - 1);
    let ray = [
        format!("const RAY_KIND_NONE: u32 = {KIND_NONE}u;\nconst RAY_KIND_GROUND: u32 = {KIND_GROUND}u;\nconst RAY_KIND_PLACED: u32 = {KIND_PLACED}u;\n"),
        field("value", 0, VALUE_BITS),
        field("kind", KIND_SHIFT, KIND_BITS),
        field("face", FACE_SHIFT, FACE_BITS),
        field("zoom", ZOOM_SHIFT, ZOOM_BITS),
    ];
    std::fs::write(out.join("ray.wgsl"), ray.concat()).expect("the ray's shader");
    // The eye's words the screen pass reads (pxl-portable eye.rs), each its
    // word, and the vec4s it binds to reach the last of them.
    use pxl_portable::eye::{ORIGIN, REACH, STEPS};
    let last = (ORIGIN + 2).max(STEPS).max(REACH);
    let eye = format!(
        "const EYE_ORIGIN: u32 = {ORIGIN}u;\nconst EYE_STEPS: u32 = {STEPS}u;\nconst EYE_REACH: u32 = {REACH}u;\nconst EYE_READ_VEC4S: u32 = {}u;\n",
        last / 4 + 1
    );
    std::fs::write(out.join("eye.wgsl"), eye).expect("the eye's shader");
    std::fs::write(Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("compute.rs"), compute(dir)).expect("the compute layouts");
    println!("cargo:rerun-if-changed=src/frame/shader");
    println!("cargo:rerun-if-changed=src/frame/shader/words.rs");
    println!("cargo:rerun-if-changed=../../portable/src/mask.bits.rs");
}

macro_rules! bits {
    ($($name:ident = $v:expr),* $(,)?) => {
        const MASK: &[(&str, u32)] = &[$((stringify!($name), $v)),*];
    };
}
include!("../../portable/src/mask.bits.rs");

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
        format!("const GROUND_LOOKS: u32 = {GROUND_LOOKS}u;\nconst EARTH_TILE_BITS: u32 = {EARTH_TILE_BITS}u;\n"),
    ]
    .concat()
}
