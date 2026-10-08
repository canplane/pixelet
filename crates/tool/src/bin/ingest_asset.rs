// pxl-tool/bin/ingest_asset.rs

//! Every shipped asset into the bucket's form, named by its path with a hash
//! of its bytes in the stem: assets are served with a year's `max-age` and no
//! purge, so a changed file must get a new name.
//!
//!   deno task ingest:asset

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sha2::{Digest, Sha256};

use pxl_tool::asset::table::{Pack, Row};
use pxl_tool::asset::{glb, pack, symbol, table};
use pxl_tool::fmt::formatted;
use pxl_tool::root;

/// Mirrored by the Worker's `KEY_RE` and the shell's `asset_url`.
const KEY_PREFIX: &str = "asset/";

/// 48 bits, as Vite gives a chunk; the path is in the name, so this only has
/// to tell one file's versions apart.
const KEY_HEX: usize = 12;

/// `cube/original/` is the resize source, never fetched by the client, so it is skipped.
fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, base, out)?;
            continue;
        }
        let rel = path.strip_prefix(base).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
        if (rel.ends_with(".glb") || rel.ends_with(".png") || rel.ends_with(".webp")) && !rel.starts_with("cube/original/") {
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

fn named(rel: &str, hex: &str) -> String {
    match rel.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}-{hex}.{ext}"),
        None => format!("{rel}-{hex}"),
    }
}

fn write_formatted(path: &Path, text: &str) -> Result<(), String> {
    let text = formatted(&path.to_string_lossy(), text.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
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

    // A look's top ships drawn toward its Carto color; a side, a bottom and a color are not looks.
    let slots = pack::load(&asset_dir.join("cube"))?;
    let carto = pxl_tool::spec::block::ground_colors()?;
    let toward: BTreeMap<String, u32> = slots
        .iter()
        .filter_map(|i| match &i.slot {
            pack::Slot::Top { group, ty } if group == "ground" => carto.get(ty).map(|&c| (format!("cube/{}", i.file), c)),
            _ => None,
        })
        .collect();

    let mut rows: Vec<Row> = Vec::with_capacity(files.len());
    let mut shipped: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut total: u64 = 0;
    for (rel, path) in &files {
        let read = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let bytes = match toward.get(rel) {
            Some(&color) => pack::toward_carto_png(&read, color).map_err(|e| format!("{rel}: {e}"))?,
            None => read,
        };
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
        if rel.starts_with("cube/") {
            shipped.insert(rel.clone(), bytes);
        }
    }

    let tiles: Vec<String> = rows
        .iter()
        .filter(|r| r.path.ends_with(".png") && (r.path.starts_with("cube/terrain/") || r.path.starts_with("cube/block/")))
        .map(|r| r.path.clone())
        .collect();
    let tile_slots = tiles
        .iter()
        .map(|t| {
            slots
                .iter()
                .find(|i| format!("cube/{}", i.file) == *t)
                .map(|i| i.slot.clone())
                .ok_or(format!("{t} has no entry in asset/cube/manifest.json"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(i) = slots.iter().find(|i| !tiles.contains(&format!("cube/{}", i.file))) {
        return Err(format!("{} names {}, which is not under asset/cube", i.label, i.file));
    }
    let faces = table::faces(&tile_slots, &pxl_tool::spec::block::catalog()?)?;
    let composed = pack::compose(&tiles.iter().map(|t| (t.clone(), shipped[t].as_slice())).collect::<Vec<_>>())?;
    let image_name = named("cube/pack.webp", &hex(&composed.bytes));
    let at = out_dir.join(&image_name);
    std::fs::create_dir_all(at.parent().ok_or("the pack has a parent")?).map_err(|e| e.to_string())?;
    std::fs::write(&at, &composed.bytes).map_err(|e| e.to_string())?;

    let laid = Pack {
        key: format!("{KEY_PREFIX}{image_name}"),
        bytes: composed.bytes.len() as u64,
        cell: pack::TILE_SIZE,
        columns: pack::COLUMNS,
        tiles: composed.tiles,
        means: composed.means,
        faces,
    };

    // Carto's symbols, which the client draws from its own binary rather than fetches.
    let symbols = symbol::symbols(&asset_dir.join("place/symbol"))?;
    let masks: Vec<u8> = symbols.iter().flat_map(|s| s.mask.iter().copied()).collect();
    std::fs::write(root.join("crates/spec/src/asset/symbol.mask"), &masks).map_err(|e| e.to_string())?;
    write_formatted(&root.join("crates/spec/src/asset/symbol.spec.rs"), &symbol::render_rs(&symbols))?;
    write_formatted(&root.join("apps/web/src/feature/search/place-symbol.ts"), &symbol::render_ts(&symbols))?;

    write_formatted(&root.join("apps/web/src/catalog/asset/ingested.spec.ts"), &table::render_ts(&rows, &laid))?;
    write_formatted(&root.join("crates/spec/src/asset/ingested.spec.rs"), &table::render_rs(&rows, &laid))?;

    println!(
        "[ingest] {} assets, {:.1} MiB, pack {} tiles {:.0} KiB -> out/asset/{{glb,png,webp}}, tables written",
        rows.len(),
        total as f64 / 1024.0 / 1024.0,
        laid.tiles.len(),
        laid.bytes as f64 / 1024.0
    );
    Ok(())
}
