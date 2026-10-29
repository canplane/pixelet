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
/// toolchain's pin, the workspaces the two crates over it inherit from, the
/// cargo config the container's cargo reads from above the shader crate and
/// the compiler wrapper it names, and this generator, which writes each
/// file's head — every file on disk under them but a test (`inputs`), so a
/// new one counts before it is added. The build is of a copy of these alone
/// (pxl-tool bin/shader_wgsl.rs `snapshot`), so one it reads and the digest
/// does not name fails there rather than going unseen. The digest is all that holds
/// a file current, for a write that skips the container and for check_rules
/// alike, so it names everything a file is made by. Wider than what reaches a
/// shader: a change elsewhere in them asks for a regenerate that differs only
/// here.
const INPUT_ROOTS: [&str; 12] = [
    "rust/spec/core/",
    "rust/client/shader/",
    "rust/client/shader/build/",
    "xpute/rust/kit/",
    "rust/client/shader/Cargo.lock",
    "rust/client/shader/rust-toolchain.toml",
    "rust/Cargo.toml",
    "xpute/rust/Cargo.toml",
    "rust/.cargo/config.toml",
    "rust/.cargo/rustc.sh",
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

/// Each input under `INPUT_ROOTS`, from the checkout's root, in path order:
/// read off the disk, not git's index, so it is the same list in a checkout,
/// a worktree, or an export under an ignored target where git sees nothing.
pub fn inputs(root: &Path) -> Result<Vec<String>, String> {
    fn walk(root: &Path, at: &str, into: &mut Vec<String>) -> Result<(), String> {
        let path = root.join(at);
        if path.is_file() {
            if !at.ends_with(".test.rs") {
                into.push(at.to_string());
            }
            return Ok(());
        }
        let entries = std::fs::read_dir(&path).map_err(|e| format!("{at}: {e}"))?;
        for entry in entries {
            let name = entry.map_err(|e| format!("{at}: {e}"))?.file_name();
            let name = name.to_str().ok_or_else(|| format!("{at}: a name not UTF-8"))?;
            // A build's own output and a tool's hidden state are no source.
            if name == "target" || name.starts_with('.') {
                continue;
            }
            walk(root, &format!("{}/{name}", at.trim_end_matches('/')), into)?;
        }
        Ok(())
    }
    let mut into = Vec::new();
    for at in INPUT_ROOTS {
        walk(root, at, &mut into)?;
    }
    into.sort_unstable();
    into.dedup();
    Ok(into)
}

/// What an input means to the build (`inputs_digest`).
fn meaning(f: &str, text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    if f.ends_with(".rs") {
        let stream: proc_macro2::TokenStream =
            text.parse().map_err(|e| format!("{f}: {e:?}"))?;
        tokens(stream, &mut out);
    } else if f.ends_with(".toml") {
        let mut value: toml::Value = toml::from_str(text).map_err(|e| format!("{f}: {e}"))?;
        if INHERITED.contains(&f) {
            value = inherited(&value);
        }
        out.extend(value.to_string().into_bytes());
    } else {
        out.extend(text.as_bytes());
    }
    Ok(out)
}

/// The workspaces outside the shader's whose manifests its crates inherit
/// from (`edition.workspace = true` and the like), and so are read for that
/// alone: their profiles, patches and members are the workspace that builds
/// them, which here is the shader's own.
const INHERITED: [&str; 2] = ["rust/Cargo.toml", "xpute/rust/Cargo.toml"];

/// What a member takes from a workspace's manifest: its `package`,
/// `dependencies` and `lints` tables under `[workspace]`.
fn inherited(manifest: &toml::Value) -> toml::Value {
    let mut kept = toml::Table::new();
    if let Some(workspace) = manifest.get("workspace").and_then(toml::Value::as_table) {
        for key in ["package", "dependencies", "lints"] {
            if let Some(v) = workspace.get(key) {
                kept.insert(key.to_string(), v.clone());
            }
        }
    }
    toml::Value::Table(kept)
}

/// Each token of `stream` written into `out`, a doc attribute (`#[doc …]`,
/// `#![doc …]`, a doc comment as the lexer gives it) left out.
fn tokens(stream: proc_macro2::TokenStream, out: &mut Vec<u8>) {
    use proc_macro2::{Delimiter, Spacing, TokenTree};
    let trees: Vec<TokenTree> = stream.into_iter().collect();
    let mut k = 0;
    while k < trees.len() {
        if let TokenTree::Punct(p) = &trees[k] {
            if p.as_char() == '#' {
                let bang = matches!(trees.get(k + 1), Some(TokenTree::Punct(q)) if q.as_char() == '!');
                let at = k + 1 + usize::from(bang);
                if let Some(TokenTree::Group(g)) = trees.get(at) {
                    let doc = g.delimiter() == Delimiter::Bracket
                        && matches!(g.stream().into_iter().next(), Some(TokenTree::Ident(i)) if i == "doc");
                    if doc {
                        k = at + 1;
                        continue;
                    }
                }
            }
        }
        match &trees[k] {
            TokenTree::Ident(i) => out.extend(format!("i{i} ").into_bytes()),
            TokenTree::Literal(l) => out.extend(format!("l{l} ").into_bytes()),
            TokenTree::Punct(p) => {
                let joint = if p.spacing() == Spacing::Joint { 'j' } else { 'a' };
                out.extend(format!("p{}{joint} ", p.as_char()).into_bytes());
            }
            TokenTree::Group(g) => {
                let (open, close) = match g.delimiter() {
                    Delimiter::Parenthesis => ('(', ')'),
                    Delimiter::Brace => ('{', '}'),
                    Delimiter::Bracket => ('[', ']'),
                    Delimiter::None => ('<', '>'),
                };
                out.push(open as u8);
                tokens(g.stream(), out);
                out.push(close as u8);
            }
        }
        k += 1;
    }
}

/// The digest the checked-in entries were made from (`STAMP`), if any.
pub fn made_from(root: &Path) -> Option<String> {
    std::fs::read_to_string(root.join(DIR).join(STAMP))
        .ok()
        .map(|text| text.trim().to_string())
}

/// Each checked-in entry not made from `digest`: all of them when the stamp
/// names another, else those missing.
pub fn stale(root: &Path, digest: &str) -> Vec<String> {
    let current = made_from(root).as_deref() == Some(digest);
    generated()
        .filter(|f| !current || !root.join(f).is_file())
        .collect()
}

/// The head every generated module carries.
pub fn header() -> String {
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
