// pxl-tool/shader.rs

//! pxl-spec-core's WGSL: the entries rust/client/shader makes of the one Rust
//! source the CPU compiles too (rust/spec/core, over xpute-kit's
//! math/rng.rs), through rust-gpu inside a Linux container, each a module of
//! its own checked in beside the renderer's shaders. An everyday build reads
//! those files and needs neither rust-gpu nor its nightly nor the container;
//! bin/shader_wgsl.rs makes them, and bin/check_rules.rs holds each to the
//! digest of the sources it names.

use std::path::Path;

use sha2::{Digest, Sha256};

/// Where the generated modules are checked in.
pub const DIR: &str = "rust/client/render/src/frame/shader";

/// Each entry and the file it is checked in as, in `DIR`.
pub const ENTRIES: [(&str, &str); 28] = [
    ("origin_cs", "origin.gen.wgsl"),
    ("beam_cs", "beam.gen.wgsl"),
    ("cell_cs", "cell.gen.wgsl"),
    ("sample_cs", "sample.gen.wgsl"),
    ("cast_cs", "cast.gen.wgsl"),
    ("sun_cs", "sun.gen.wgsl"),
    ("sun_miss_cs", "sun_miss.gen.wgsl"),
    ("sun_mark_cs", "sun_mark.gen.wgsl"),
    ("miss_cs", "miss.gen.wgsl"),
    ("miss_list_cs", "miss_list.gen.wgsl"),
    ("boxes_cs", "boxes.gen.wgsl"),
    ("faces_cs", "faces.gen.wgsl"),
    ("forget_cs", "forget.gen.wgsl"),
    ("joined_cs", "joined.gen.wgsl"),
    ("height_cs", "height.gen.wgsl"),
    ("bound_cs", "bound.gen.wgsl"),
    ("decor_mark_cs", "decor_mark.gen.wgsl"),
    ("decor_stand_cs", "decor_stand.gen.wgsl"),
    ("decor_cs", "decor.gen.wgsl"),
    ("decor_cnt_cs", "decor_cnt.gen.wgsl"),
    ("decor_args_cs", "decor_args.gen.wgsl"),
    ("decor_place_cs", "decor_place.gen.wgsl"),
    ("way_cs", "way.gen.wgsl"),
    ("way_free_cs", "way_free.gen.wgsl"),
    ("way_tree_cs", "way_tree.gen.wgsl"),
    ("way_freed_cs", "way_freed.gen.wgsl"),
];

/// rust-gpu's own pin (rustc_codegen_spirv 0.10.0-alpha.1 build.rs), as
/// rust/client/shader/rust-toolchain.toml holds it.
pub const NIGHTLY: &str = "nightly-2026-04-11";

/// The one record of what the entries in `DIR` were made from: the digest
/// of their sources (`inputs_digest`), beside them. One file for all, since
/// what an entry reads is only known once it is compiled, so an entry whose
/// bytes did not change has nothing to change.
pub const STAMP: &str = "gen.digest";

/// What makes the entries: pxl-spec-core, the shader crate, the builder that
/// reads its SPIR-V into WGSL, xpute-kit beneath them, their lock and the
/// toolchain's pin, the cargo config the container's cargo reads from above
/// the shader crate, and this generator, which writes each file's head —
/// every Rust, TOML and lock file under them the tree holds (`git ls-files
/// -co --exclude-standard`, as check_rules lists it), so a new one counts
/// before it is added and nothing ignored does. The digest is all that holds
/// a file current, for a write that skips the container and for check_rules
/// alike, so it names everything a file is made by. Wider than what reaches a
/// shader: a change elsewhere in them asks for a regenerate that differs only
/// here.
const INPUT_ROOTS: [&str; 9] = [
    "rust/spec/core/",
    "rust/client/shader/",
    "rust/client/shader/build/",
    "xpute/rust/kit/",
    "rust/client/shader/Cargo.lock",
    "rust/client/shader/rust-toolchain.toml",
    "rust/.cargo/config.toml",
    "rust/tool/src/shader.rs",
    "rust/tool/src/bin/shader_wgsl.rs",
];

/// Each checked-in entry's path from the checkout's root.
pub fn generated() -> impl Iterator<Item = String> {
    ENTRIES.iter().map(|(_, file)| format!("{DIR}/{file}"))
}

/// SHA-256 over what each input means to the build, in path order: the path,
/// a NUL, its meaning's length in decimal, a NUL, then the meaning. A Rust
/// file means its tokens, without its comments and doc attributes, which reach
/// no SPIR-V; a TOML file its parsed value, without its comments or layout;
/// any other, a lock or the compiler wrapper, its bytes. A test file (`*.test.rs`) is not compiled for a shader and
/// is not an input. So an edit to a comment or a test leaves the digest, and
/// rust-gpu is not run again for it.
pub fn inputs_digest(root: &Path) -> Result<String, String> {
    let mut hash = Sha256::new();
    for f in inputs(root)? {
        let f = f.as_str();
        let text = std::fs::read_to_string(root.join(f)).map_err(|e| format!("{f}: {e}"))?;
        let meant = meaning(f, &text)?;
        hash.update(format!("{f}\0{}\0", meant.len()).as_bytes());
        hash.update(&meant);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The digest of the sources a generated module was made from, as its head
/// names it.
pub fn made_from(text: &str) -> Option<&str> {
    text.lines().find_map(|l| l.strip_prefix(INPUTS))
}

/// Each checked-in entry that does not name `digest`, missing ones too.
pub fn stale(root: &Path, digest: &str) -> Vec<String> {
    generated()
        .filter(|f| {
            std::fs::read_to_string(root.join(f))
                .map_or(true, |text| made_from(&text) != Some(digest))
        })
        .collect()
}

/// The head every generated module carries, its digest last.
pub fn header(digest: &str) -> String {
    [
        "// GENERATED by pxl-tool bin/shader_wgsl.rs; do not edit. Regenerate with",
        "// `cargo shader-wgsl` in rust/; `--check` fails when it is stale. What it",
        "// was made from is gen.digest's, beside it.",
        "// Source: rust/spec/core (pxl-spec-core) over xpute/rust/kit/src/math/rng.rs,",
        "// through rust/client/shader, by",
        &format!("// rust-gpu 0.10.0-alpha.1 ({NIGHTLY}) for spirv-unknown-vulkan1.3, read by naga in rust/client/shader/build."),
        "",
    ]
    .join("\n")
}

#[cfg(test)]
#[path = "shader.test.rs"]
mod test;
