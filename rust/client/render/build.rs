// pxl-render/build.rs

// The build writes the shaders the crate ships. Each WGSL file under src/frame/shader goes out
// with its comments and indentation stripped, and nothing in it is renamed. After it goes what
// the shaders share with the Rust, written from the Rust that defines it (build/shared.rs), and
// the accessors of each uniform of word.rs the part reads. Each shader is then assembled from
// its parts (part.rs), cut to what its entry points reach (build/cut.rs), validated by naga and
// held to what the phones and the browsers take (build/check.rs), and what each of its bindings
// is read by is written beside it (build/bind.rs).

use naga::front::wgsl::parse_str;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use std::collections::BTreeMap;
use std::path::Path;

#[path = "src/look/atmosphere.rs"]
#[expect(
    dead_code,
    unreachable_pub,
    reason = "the build reads part of what the crate shares with it"
)]
mod atmosphere;
#[path = "build/bind.rs"]
mod bind;
#[path = "build/check.rs"]
mod check;
#[path = "src/look/color.rs"]
#[expect(
    dead_code,
    unreachable_pub,
    reason = "the build reads part of what the crate shares with it"
)]
mod color;
#[path = "build/cut.rs"]
mod cut;
#[path = "src/frame/shader/debug_look.spec.rs"]
pub mod debug_look;
#[path = "src/param.rs"]
#[expect(
    dead_code,
    unreachable_pub,
    reason = "the build reads part of what the crate shares with it"
)]
mod param;
#[path = "src/frame/shader/part.rs"]
#[expect(
    dead_code,
    unreachable_pub,
    reason = "the build reads part of what the crate shares with it"
)]
mod part;
#[path = "build/shared.rs"]
mod shared;
#[path = "build/wgsl.rs"]
mod wgsl;
#[path = "src/frame/shader/word.rs"]
#[expect(
    dead_code,
    unreachable_pub,
    reason = "the build reads part of what the crate shares with it"
)]
mod word;

fn main() {
    let dir = Path::new("src/frame/shader");
    let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("shader");
    // The directory is emptied first, so a part deleted from src/frame/shader cannot be read
    // from an earlier build's copy.
    match std::fs::remove_dir_all(&out) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => panic!("the shader directory: {e}"),
        _ => {}
    }
    std::fs::create_dir_all(out.join("assembled")).expect("the shader directory");
    let mut written = BTreeMap::from(shared::parts());
    for entry in std::fs::read_dir(dir).expect("the shaders") {
        let path = entry.expect("a shader").path();
        if path.extension().and_then(|e| e.to_str()) != Some("wgsl") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a shader's text");
        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .expect("a shader's name");
        let part = cut::strip_comments(&text)
            + &written.remove(name).unwrap_or_default()
            + &shared::accessors(&text);
        std::fs::write(out.join(format!("{name}.wgsl")), part).expect("the stripped shader");
    }
    for (name, part) in written {
        std::fs::write(out.join(format!("{name}.wgsl")), part).expect("a written part");
    }
    assemble(&out);
    println!("cargo:rerun-if-changed=src/frame/shader");
}

/// Assembles every shader from its parts (part.rs `PARTS`), and the world's query alone
/// (`WORLD_ASKED`), and writes each where record.rs includes it. For each shader it also
/// writes which stages read each binding (`STAGES`, in OUT_DIR), from which every pipeline's
/// groups, render and compute, take their visibility (record.rs `stages_reading`), and what
/// each compute entry point does with each binding (`ACCESS`).
fn assemble(out: &Path) {
    let read = |name: &str| {
        std::fs::read_to_string(out.join(format!("{name}.wgsl")))
            .unwrap_or_else(|e| panic!("{name}.wgsl: {e}"))
    };
    let mut declared = Vec::new();
    // A module is constant.wgsl and then its parts, cut to what its entry points reach, and
    // validated as WebGPU takes it, with no capability.
    let mut assembled = |parts: &[&str], name: &str| {
        let mut text = read("constant") + "\n";
        for part in parts {
            text += &read(part);
            text += "\n";
        }
        let text = cut::read_consts(&cut::read_structs(&cut::read_fns(&text)));
        let module =
            parse_str(&text).unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&text)));
        let info = Validator::new(ValidationFlags::all(), Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&text)));
        check::refuse_faults(&module, name);
        declared.extend(check::layouts(&module, name));
        (text, module, info)
    };
    let (mut sources, mut stages, mut access) = (String::new(), String::new(), String::new());
    for (kind, parts) in part::PARTS.iter().enumerate() {
        let name = format!("shader {kind} ({parts:?})");
        let (text, module, info) = assembled(parts, &name);
        let file = format!("{kind}.wgsl");
        std::fs::write(out.join("assembled").join(&file), &text).expect("an assembled shader");
        sources += &format!(
            "        {kind} => include_str!(concat!(env!(\"OUT_DIR\"), \"/shader/assembled/{file}\")),\n"
        );
        access += &bind::access(&module, &info);
        stages += &bind::stages(&module, &info, &name);
    }
    let (asked, ..) = assembled(&part::WORLD_ASKED, "world_asked");
    std::fs::write(out.join("assembled").join("world_asked.wgsl"), asked)
        .expect("the world's asked shader");
    for (ty, ..) in check::LAYOUTS {
        assert!(declared.contains(&ty), "no shader declares `{ty}`");
    }
    let root = out.parent().expect("OUT_DIR");
    std::fs::write(
        root.join("sources.rs"),
        format!("/// Shader `kind` as build.rs assembled and validated it (part.rs).\npub fn src(kind: u32) -> &'static str {{\n    match kind {{\n{sources}        _ => \"\",\n    }}\n}}\n"),
    )
    .expect("the shaders' sources");
    std::fs::write(
        root.join("stages.rs"),
        format!("/// Each shader's bindings, [group, binding, the stages that read it, a bit a layout kind\n/// that takes it as declared, a bit a vertex entry that reads it (`VERTEX_ENTRY`)], read off it\n/// by build.rs.\npub const STAGES: [&[[u32; 5]]; SHADER_CNT as usize] = [\n{stages}];\n/// Each shader's compute entry points, each with the bindings it uses, [group, binding, its\n/// access (access.rs `READ`, `WRITE`, `ATOMIC`)], read off it by build.rs.\npub const ACCESS: [&[EntryAccess]; SHADER_CNT as usize] = [\n{access}];\n/// A compute entry point's name and its bindings' access, as `ACCESS` holds them.\npub type EntryAccess = (&'static str, &'static [[u32; 3]]);\n"),
    )
    .expect("the shaders' stages");
}
