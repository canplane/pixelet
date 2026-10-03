// pxl-geo-bake/bin/rekey_tiles.rs

//! A pyramid bake's objects linked under their quadkeys (page/rekey.rs).
//!
//!   rekey_tiles link --bake terrain --from ./out/terrain/v7 --to ./out/terrain/v7.bucket

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pxl_geo_bake::cli::Flags;
use pxl_geo_bake::page::rekey;

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
    rekey::of(&flags.get("--bake")?)?;
    let from = PathBuf::from(flags.get("--from")?);
    let to = PathBuf::from(flags.get("--to")?);
    if to.exists() {
        std::fs::remove_dir_all(&to).map_err(|e| format!("{}: {e}", to.display()))?;
    }
    std::fs::create_dir_all(&to).map_err(|e| format!("{}: {e}", to.display()))?;

    let mut found = Vec::new();
    objects(&from, "", &mut found)?;
    for old in &found {
        let new = rekey::rename(old).map_err(|e| format!("{old}: {e}"))?;
        std::fs::hard_link(from.join(old), to.join(&new)).map_err(|e| format!("{old} -> {new}: {e}"))?;
    }
    eprintln!("[rekey] {} object(s) linked from {} to {}", found.len(), from.display(), to.display());
    Ok(())
}

fn run() -> Result<(), String> {
    let mut flags = Flags::from_env();
    let command = flags.arg();
    match command.as_deref() {
        Some("link") => link(&Named::read(&mut flags)?),
        _ => Err("usage: rekey_tiles link --bake <kind> --from <dir> --to <dir>".into()),
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("[rekey] {err}");
        std::process::exit(1);
    }
}
