// The WGSL the module ships is the shader files with their prose taken off.
//
// WebGPU compiles a shader from its source text — there is no bytecode form
// of a shader the way SPIR-V is one — so the text is in the published module
// either way (frame/shader/record.rs hands it to the host). What the GPU does
// not need is the commentary around it, which is written for whoever edits
// the shader and reads out of a published module as a description of how the
// renderer works.
//
// Only comments and indentation go. Nothing is renamed: an entry point and a
// binding are named by the pipeline records beside the shaders, and the test
// that holds a record against its shader reads whatever is written here.

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
    println!("cargo:rerun-if-changed=src/frame/shader");
}

/// `text` with its line comments and its indentation off.
///
/// WGSL has no string literal for a `//` to hide inside, so one starts a
/// comment wherever it appears. A line is kept only if anything is left of
/// it, and it is kept as its own line: nothing is joined, so no two tokens
/// can run together. A block comment is left alone — these files carry none,
/// and one that arrives later stays valid WGSL rather than being cut wrong.
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
