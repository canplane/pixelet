// pxl-tool/shader.rs

//! pxl-spec-core's WGSL: the entries rust/client/shader makes of the one Rust
//! source the CPU compiles too (rust/spec/core, over xpute-kit's
//! math/rng.rs), through rust-gpu inside a Linux container, each a module of
//! its own checked in beside the renderer's shaders. An everyday build reads
//! those files and needs neither rust-gpu nor its nightly nor the container;
//! bin/shader_wgsl.rs makes them, and bin/check_rules.rs holds each to the
//! digest of the sources it names.

use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

/// Where the generated modules are checked in.
pub const DIR: &str = "rust/client/render/src/frame/shader";

/// Each entry and the file it is checked in as, in `DIR`.
pub const ENTRIES: [(&str, &str); 31] = [
    ("beam_cs", "beam.gen.wgsl"),
    ("cell_cs", "cell.gen.wgsl"),
    ("sample_cs", "sample.gen.wgsl"),
    ("cast_cs", "cast.gen.wgsl"),
    ("sun_cs", "sun.gen.wgsl"),
    ("sun_beam_cs", "sun_beam.gen.wgsl"),
    ("sun_cell_cs", "sun_cell.gen.wgsl"),
    ("sun_sample_cs", "sun_sample.gen.wgsl"),
    ("sun_sample_box_cs", "sun_sample_box.gen.wgsl"),
    ("sun_miss_cs", "sun_miss.gen.wgsl"),
    ("sun_mark_cs", "sun_mark.gen.wgsl"),
    ("miss_cs", "miss.gen.wgsl"),
    ("miss_list_cs", "miss_list.gen.wgsl"),
    ("sample_box_cs", "sample_box.gen.wgsl"),
    ("face_cs", "face.gen.wgsl"),
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

/// Where a generated file names the digest of the sources that made it.
pub const INPUTS: &str = "// Inputs: ";

/// What makes the entries: pxl-spec-core, the shader crate, the builder that
/// reads its SPIR-V into WGSL, xpute-kit beneath them, their lock and the
/// toolchain's pin — every Rust, TOML and lock file under them the tree holds
/// (`git ls-files -co --exclude-standard`, as check_rules lists it), so a new
/// one counts before it is added and nothing ignored does. Wider than what
/// reaches a shader: a change elsewhere in them asks for a regenerate that
/// differs only here.
const INPUT_ROOTS: [&str; 6] = [
    "rust/spec/core/",
    "rust/client/shader/",
    "rust/client/shader/build/",
    "xpute/rust/kit/",
    "rust/client/shader/Cargo.lock",
    "rust/client/shader/rust-toolchain.toml",
];

/// Each checked-in entry's path from the checkout's root.
pub fn generated() -> impl Iterator<Item = String> {
    ENTRIES.iter().map(|(_, file)| format!("{DIR}/{file}"))
}

/// SHA-256 over each input's path and bytes, in path order: the path, a NUL,
/// its length in decimal, a NUL, then its bytes.
pub fn inputs_digest(root: &Path) -> Result<String, String> {
    let out = Command::new("git")
        .args(["ls-files", "-co", "--exclude-standard"])
        .args(INPUT_ROOTS)
        .current_dir(root)
        .output()
        .map_err(|e| format!("git ls-files: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git ls-files: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let listed = String::from_utf8(out.stdout).map_err(|e| format!("git ls-files: {e}"))?;
    let mut inputs: Vec<&str> = listed
        .lines()
        .filter(|f| {
            [".rs", ".toml", ".lock"].iter().any(|e| f.ends_with(e)) && root.join(f).is_file()
        })
        .collect();
    inputs.sort_unstable();
    inputs.dedup();
    let mut hash = Sha256::new();
    for f in inputs {
        let bytes = std::fs::read(root.join(f)).map_err(|e| format!("{f}: {e}"))?;
        hash.update(format!("{f}\0{}\0", bytes.len()).as_bytes());
        hash.update(&bytes);
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
            let name = name
                .to_str()
                .ok_or_else(|| format!("{at}: a name not UTF-8"))?;
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
        let stream: proc_macro2::TokenStream = text.parse().map_err(|e| format!("{f}: {e:?}"))?;
        tokens(stream, &mut out);
    } else if f.ends_with(".toml") {
        let mut val: toml::Value = toml::from_str(text).map_err(|e| format!("{f}: {e}"))?;
        if INHERITED.contains(&f) {
            val = inherited(&val);
        }
        out.extend(val.to_string().into_bytes());
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
                let bang =
                    matches!(trees.get(k + 1), Some(TokenTree::Punct(q)) if q.as_char() == '!');
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
                let joint = if p.spacing() == Spacing::Joint {
                    'j'
                } else {
                    'a'
                };
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
    let cur = made_from(root).as_deref() == Some(digest);
    generated()
        .filter(|f| !cur || !root.join(f).is_file())
        .collect()
}

/// The head every generated module carries.
pub fn header() -> String {
    [
        "// GENERATED by pxl-tool bin/shader_wgsl.rs; do not edit. Regenerate with",
        "// `cargo shader-wgsl` in rust/; `--check` fails when it is stale.",
        "// Source: rust/spec/core (pxl-spec-core) over xpute/rust/kit/src/math/rng.rs,",
        "// through rust/client/shader, by",
        &format!("// rust-gpu 0.10.0-alpha.1 ({NIGHTLY}) for spirv-unknown-vulkan1.3, read by naga in rust/client/shader/build."),
        &format!("{INPUTS}{digest}"),
        "",
    ]
    .join("\n")
}
