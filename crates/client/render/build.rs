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
    // The chunk box's mask bits, from the one statement the mesher compiles.
    std::fs::write(out.join("faces.wgsl"), MASK.iter().map(|(name, v)| format!("const {name}: u32 = {v}u;\n")).collect::<String>()).expect("the mask's shader");
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
