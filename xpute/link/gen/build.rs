// xpute-link-gen/build.rs

// Embeds the files of xpute's own packages that the generator reads, their
// WIT and the link's frame.json5, which xpute/ beside this crate mirrors by
// links into xpute's tree, so that a program's bindings are written with no
// xpute checkout. It writes `(path, text)` for each, its path from xpute's
// root (wit.rs `FILES`), in the order of their paths.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    println!("cargo:rerun-if-changed={}", dir.display());
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .path();
        if path.is_dir() {
            walk(&path, files);
        } else {
            files.push(path);
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("xpute");
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    let mut out = String::from("&[\n");
    for f in &files {
        let at = f.strip_prefix(&root).expect("a file under xpute/");
        let _ = writeln!(
            out,
            "    ({:?}, include_str!({:?})),",
            at.to_string_lossy(),
            f.to_string_lossy()
        );
    }
    out.push_str("]\n");
    let dst = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("xpute.rs");
    std::fs::write(&dst, out).unwrap_or_else(|e| panic!("{}: {e}", dst.display()));
}
