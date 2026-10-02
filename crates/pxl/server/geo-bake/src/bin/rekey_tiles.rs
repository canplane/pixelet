// pxl-geo-bake/bin/rekey_tiles.rs

//! Objects renamed onto their quadkeys (page/rekey.rs).
//!
//!   s5cmd --endpoint-url $R2 ls 's3://pixelet-tiles/terrain/v5/*' \
//!     | awk '{print $NF}' \
//!     | rekey_tiles emit --bucket pixelet-tiles --from terrain/v5 --to terrain/v6 \
//!     | s5cmd --endpoint-url $R2 run
//!
//!   rekey_tiles link --bake terrain --from ./out/terrain/v6 --to ./out/terrain/v6.bucket

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use pxl_geo_bake::cli::Flags;
use pxl_geo_bake::page::rekey::Bake;

const LOCAL_ONLY: [&str; 2] = [".part", ".empty"];

struct Named(HashMap<String, String>);

impl Named {
    fn read(flags: &mut Flags) -> Result<Named, String> {
        let mut named = HashMap::new();
        while let Some(flag) = flags.arg() {
            let value = flags.value()?;
            named.insert(flag, value);
        }
        Ok(Named(named))
    }

    fn get(&self, name: &str) -> Result<String, String> {
        self.0.get(name).cloned().ok_or_else(|| format!("{name} is wanted"))
    }
}

fn emit(flags: &Named) -> Result<(), String> {
    let bucket = flags.get("--bucket")?;
    let from = flags.get("--from")?.trim_matches('/').to_string();
    let to = flags.get("--to")?.trim_matches('/').to_string();
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

fn link(flags: &Named) -> Result<(), String> {
    let bake = Bake::of(&flags.get("--bake")?)?;
    let from = PathBuf::from(flags.get("--from")?);
    let to = PathBuf::from(flags.get("--to")?);
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
    let mut flags = Flags::from_env();
    let command = flags.arg();
    match command.as_deref() {
        Some("emit") => emit(&Named::read(&mut flags)?),
        Some("link") => link(&Named::read(&mut flags)?),
        _ => Err("usage: rekey_tiles emit --bucket <b> --from <prefix> --to <prefix>\n       rekey_tiles link --bake <kind> --from <dir> --to <dir>".into()),
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("[rekey] {err}");
        std::process::exit(1);
    }
}
