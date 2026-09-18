// pxl-asset-bake/tool/ingest_asset.rs

//! Every shipped asset into the form the bucket holds and the catalogs name:
//! one object per file, keyed by the SHA-256 of its bytes, with a model's
//! bounds measured here rather than typed in.
//!
//! Two outputs. `./out/asset/{glb,png,webp}/<sha256>` is a copy of every file
//! under its key, split by type so that `aws s3 sync` can set one content-type
//! per directory — the object name carries no extension, so the type has to be
//! stored with it. And the two generated tables, written from the same rows so
//! that the shell's and the kernel's cannot drift.
//!
//! The texture pack is also ingested as one object: every image under
//! `asset/cube/{terrain,block}` composed into a sheet and encoded once
//! (sheet.rs).
//!
//!   deno task ingest:asset

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sha2::{Digest, Sha256};

use pxl_asset_bake::table::{self, Row, Sheet};
use pxl_asset_bake::{glb, sheet};

/// What the bucket prefix for content-addressed objects is; a key is this over
/// the sha256 hex. Mirrored by the Worker's `KEY_RE` and by the shell's
/// `asset_url`.
const KEY_PREFIX: &str = "asset/";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap_or_else(|_| PathBuf::from("."))
}

/// The asset tree as the table walks it: every `.glb` and `.png`, sorted by
/// path.
///
/// `cube/original/` is the native-resolution resize source and `brand/` is the
/// icon renderer's — neither is something the client ever fetches by key, so
/// neither becomes an object.
fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, base, out)?;
            continue;
        }
        let rel = path.strip_prefix(base).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
        if (rel.ends_with(".glb") || rel.ends_with(".png")) && !rel.starts_with("cube/original/") && !rel.starts_with("brand/") {
            out.push((rel, path));
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Through the formatter the tree is checked with, so that writing a table and
/// then formatting the repository do not disagree — `deno fmt --check` and
/// `cargo fmt --check` are both gates, and a generator that emits unformatted
/// output makes the next unrelated run of either fail.
fn write_formatted(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let rust = path.extension().is_some_and(|e| e == "rs");
    let mut cmd = if rust {
        let mut c = Command::new("rustfmt");
        c.args(["--edition", "2021", "--emit", "stdout", "--config-path"]).arg(root().join("crates/rustfmt.toml"));
        c
    } else {
        let mut c = Command::new("deno");
        c.args(["fmt", "--ext", "ts", "-"]).current_dir(root());
        c
    };
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().map_err(|e| format!("fmt {}: {e}", path.display()))?;
    child.stdin.take().ok_or("no stdin")?.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("fmt {}: exited {}", path.display(), out.status.code().unwrap_or(-1)));
    }
    std::fs::write(path, out.stdout).map_err(|e| format!("{}: {e}", path.display()))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[ingest] {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let root = root();
    let asset_dir = root.join("asset");
    let out_dir = root.join("out/asset");

    let mut files = Vec::new();
    walk(&asset_dir, &asset_dir, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut rows: Vec<Row> = Vec::with_capacity(files.len());
    let mut total: u64 = 0;
    for (rel, path) in &files {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let kind = if rel.ends_with(".glb") { "glb" } else { "png" };
        let key_hex = hex(&bytes);
        let extent = if kind == "glb" { Some(glb::extent(&bytes).map_err(|e| format!("{rel}: {e}"))?) } else { None };

        let dir = out_dir.join(kind);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(&key_hex), &bytes).map_err(|e| e.to_string())?;

        total += bytes.len() as u64;
        rows.push(Row {
            path: rel.clone(),
            key: format!("{KEY_PREFIX}{key_hex}"),
            bytes: bytes.len() as u64,
            extent,
        });
    }

    // The pack's tiles, in the same path order the rows are in — which is what
    // the table records as the sheet's layout.
    let tiles: Vec<String> = rows
        .iter()
        .filter(|r| r.path.ends_with(".png") && (r.path.starts_with("cube/terrain/") || r.path.starts_with("cube/block/")))
        .map(|r| r.path.clone())
        .collect();
    let composed = sheet::compose(&asset_dir, &tiles)?;
    let sheet_hex = hex(&composed.bytes);
    let webp_dir = out_dir.join("webp");
    std::fs::create_dir_all(&webp_dir).map_err(|e| e.to_string())?;
    std::fs::write(webp_dir.join(&sheet_hex), &composed.bytes).map_err(|e| e.to_string())?;

    let sheet = Sheet {
        key: format!("{KEY_PREFIX}{sheet_hex}"),
        bytes: composed.bytes.len() as u64,
        cell: sheet::CELL,
        columns: sheet::COLUMNS,
        tiles: composed.tiles,
    };

    write_formatted(&root.join("packages/pxl/shared/src/asset/ingested.ts"), &table::render_ts(&rows, &sheet))?;
    write_formatted(
        &root.join("crates/pxl/spec/src/asset/ingested.rs"),
        &table::render_rs(&rows, &sheet, pxl_spec::cell::cube_type::cube_types::CHANNELS),
    )?;

    println!(
        "[ingest] {} assets, {:.1} MiB, sheet {} tiles {:.0} KiB -> out/asset/{{glb,png,webp}}, tables written",
        rows.len(),
        total as f64 / 1024.0 / 1024.0,
        sheet.tiles.len(),
        sheet.bytes as f64 / 1024.0
    );
    Ok(())
}
