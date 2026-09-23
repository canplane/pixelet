// pxl-geo-bake/bin/rekey_tiles.rs

//! Objects renamed onto their quadkeys (pxl-geo-bake page/rekey.rs), in the
//! two places a rename is needed.
//!
//! **What is already published**, renamed inside the bucket. A rename there is
//! a server-side copy, so the bytes never leave R2:
//!
//!   s5cmd --endpoint-url $R2 ls 's3://pixelet-tiles/terrain/v5/*' \
//!     | awk '{print $NF}' \
//!     | rekey_tiles emit --bucket pixelet-tiles --from terrain/v5 --to terrain/v6 \
//!     | s5cmd --endpoint-url $R2 run
//!
//! **A fresh terrain bake**, whose disk keeps `<level>/<page index>` because
//! the bake redoes and resumes a level by its directory. The tree to upload is
//! hard links, so it costs no bytes:
//!
//!   rekey_tiles link --bake terrain --from ./out/terrain/v6 --to ./out/terrain/v6.bucket
//!
//! POI and geocode need neither: they keep no level directories, so the bake
//! writes the bucket's name directly.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use pxl_geo_bake::page::rekey::Bake;

/// What the bake leaves beside an object for its own use and the bucket never
/// holds.
const LOCAL_ONLY: [&str; 2] = [".part", ".empty"];

fn flag(argv: &[String], name: &str) -> Result<String, String> {
    let at = argv.iter().position(|a| a == name).ok_or_else(|| format!("{name} is wanted"))?;
    argv.get(at + 1).cloned().ok_or_else(|| format!("{name} wants a value"))
}

/// A key a line in, an `s5cmd` copy a line out.
fn emit(argv: &[String]) -> Result<(), String> {
    let bucket = flag(argv, "--bucket")?;
    let from = flag(argv, "--from")?.trim_matches('/').to_string();
    let to = flag(argv, "--to")?.trim_matches('/').to_string();
    let bake = Bake::of(&from)?;

    let out = io::stdout();
    let mut out = io::BufWriter::new(out.lock());
    let mut n = 0u64;
    for line in io::stdin().lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        let key = line.trim();
        if key.is_empty() {
            continue;
        }
        let rest = key.strip_prefix(&from).and_then(|r| r.strip_prefix('/')).ok_or_else(|| format!("{key}: not under {from}"))?;
        let name = bake.rename(rest).map_err(|e| format!("{key}: {e}"))?;
        writeln!(out, "cp s3://{bucket}/{key} s3://{bucket}/{to}/{name}").map_err(|e| e.to_string())?;
        n += 1;
    }
    out.flush().map_err(|e| e.to_string())?;
    eprintln!("[rekey] {n} object(s) from {from} to {to}");
    Ok(())
}

/// Every object under `dir`, as the name the bake gave it relative to `dir`.
fn objects(dir: &Path, under: &str, out: &mut Vec<String>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let here = if under.is_empty() { name.clone() } else { format!("{under}/{name}") };
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() {
            objects(&entry.path(), &here, out)?;
        } else if !LOCAL_ONLY.iter().any(|s| name.ends_with(s)) {
            out.push(here);
        }
    }
    Ok(())
}

/// A tree of hard links under the names the bucket serves.
fn link(argv: &[String]) -> Result<(), String> {
    let bake = Bake::of(&flag(argv, "--bake")?)?;
    let from = PathBuf::from(flag(argv, "--from")?);
    let to = PathBuf::from(flag(argv, "--to")?);
    if to.exists() {
        std::fs::remove_dir_all(&to).map_err(|e| format!("{}: {e}", to.display()))?;
    }
    std::fs::create_dir_all(&to).map_err(|e| format!("{}: {e}", to.display()))?;

    let mut found = Vec::new();
    objects(&from, "", &mut found)?;
    for old in &found {
        let new = bake.rename(old).map_err(|e| format!("{old}: {e}"))?;
        std::fs::hard_link(from.join(old), to.join(&new)).map_err(|e| format!("{old} -> {new}: {e}"))?;
    }
    eprintln!("[rekey] {} object(s) linked from {} to {}", found.len(), from.display(), to.display());
    Ok(())
}

fn run() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match argv.first().map(String::as_str) {
        Some("emit") => emit(&argv),
        Some("link") => link(&argv),
        _ => Err("usage: rekey_tiles emit --bucket <b> --from <prefix> --to <prefix>\n       rekey_tiles link --bake <kind> --from <dir> --to <dir>".into()),
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("[rekey] {err}");
        std::process::exit(1);
    }
}
