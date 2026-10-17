// pxl-render/build.rs

// The shipped WGSL is the shader files with comments and indentation stripped; nothing is renamed.

use std::path::Path;

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
        std::fs::write(out.join(name), stripped(&text)).expect("the stripped shader");
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
    println!("cargo:rerun-if-changed=src/frame/shader");
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
