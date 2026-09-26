// pxl-tool/bin/ingest_asset.rs

//! Every shipped asset into the form the bucket holds and the catalogs name:
//! one object per file, named by its own path with a hash of its bytes in the
//! stem, with a model's bounds measured here rather than typed in.
//!
//! The hash is what makes the name immutable, and it is the whole reason an
//! asset is not named by its path alone: an asset goes out under a year's
//! `max-age`, so a changed file that kept its name would be served stale by
//! every edge and every browser for that year, with no purge to call. A
//! changed file is a new name instead. The path is there because a bucket
//! nobody can read is one nobody can check, and because the type stays in the
//! extension where the upload can see it.
//!
//! Two outputs. `./out/asset/` is a copy of every file under its key, the tree
//! mirroring the bucket's exactly. And the two generated tables, written from
//! the same rows so that the shell's and the kernel's cannot drift.
//!
//! The texture pack is also ingested as one object: every image under
//! `asset/cube/{terrain,block}` composed into a sheet and encoded once
//! (sheet.rs).
//!
//!   deno task ingest:asset

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sha2::{Digest, Sha256};

use pxl_tool::asset::table::{Row, Sheet};
use pxl_tool::asset::{glb, sheet, table};
use pxl_tool::root;

/// What the bucket prefix for an asset is; a key is this over the asset's own
/// path. Mirrored by the Worker's `KEY_RE` and by the shell's `asset_url`.
const KEY_PREFIX: &str = "asset/";

/// Hex digits of the SHA-256 kept in a name. Twelve is 48 bits, which is the
/// entropy Vite gives a bundle chunk for the same job, and short enough that
/// the name is still the file's. A collision between two assets is not
/// possible whatever this is — their paths differ, and the path is in the name
/// — so what it buys is only that one file's next version cannot land on the
/// name its last one had.
const KEY_HEX: usize = 12;

/// The asset tree as the table walks it: every `.glb` and `.png`, sorted by
/// path.
///
/// `cube/original/` is the native-resolution source the tiles are resized
/// from, not something the client ever fetches by key, so it becomes no
/// object.
fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, base, out)?;
            continue;
        }
        let rel = path.strip_prefix(base).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
        if (rel.ends_with(".glb") || rel.ends_with(".png")) && !rel.starts_with("cube/original/") {
            out.push((rel, path));
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect::<String>()[..KEY_HEX].to_string()
}

/// A file's name in the bucket: its own path with the hash in the stem, the
/// way a bundler names a chunk (`beaver-20f91809e69c.glb`).
fn named(rel: &str, hex: &str) -> String {
    match rel.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}-{hex}.{ext}"),
        None => format!("{rel}-{hex}"),
    }
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
        c.args(["--edition", "2021", "--emit", "stdout", "--config-path"]).arg(root().join("rustfmt.toml"));
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
        let is_model = rel.ends_with(".glb");
        let name = named(rel, &hex(&bytes));
        let extent = if is_model { Some(glb::extent(&bytes).map_err(|e| format!("{rel}: {e}"))?) } else { None };

        let at = out_dir.join(&name);
        std::fs::create_dir_all(at.parent().ok_or("an asset has a parent")?).map_err(|e| e.to_string())?;
        std::fs::write(&at, &bytes).map_err(|e| e.to_string())?;

        total += bytes.len() as u64;
        rows.push(Row {
            path: rel.clone(),
            key: format!("{KEY_PREFIX}{name}"),
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
    // The one object with no source file of its own: it is composed from every
    // tile under `cube/`, so it is named there.
    let sheet_name = named("cube/pack.webp", &hex(&composed.bytes));
    let at = out_dir.join(&sheet_name);
    std::fs::create_dir_all(at.parent().ok_or("the pack has a parent")?).map_err(|e| e.to_string())?;
    std::fs::write(&at, &composed.bytes).map_err(|e| e.to_string())?;

    let sheet = Sheet {
        key: format!("{KEY_PREFIX}{sheet_name}"),
        bytes: composed.bytes.len() as u64,
        cell: sheet::CELL,
        columns: sheet::COLUMNS,
        tiles: composed.tiles,
        means: composed.means,
    };

    write_formatted(&root.join("apps/web/src/catalog/asset/ingested.spec.ts"), &table::render_ts(&rows, &sheet))?;
    write_formatted(
        &root.join("crates/pxl/spec/src/asset/ingested.spec.rs"),
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
