// pxl-geo-bake/poi/snapshot.rs

use super::bake::Hierarchy;
use crate::util::{pack, write_atomic, Level, CONTENT_ENCODING};
use pxl_spec::poi::{wire, xy, Entity};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

type Accept<'a> = dyn FnMut(Entity, &str) -> Result<(), String> + 'a;

/// Canonical backing and all summary nodes become visible together. Only
/// quadkey files are served; backing and provenance stay server-owned.
pub fn write(out: &Path, root: u64, revision: u64, provider: &str, packing: Level, visit: impl FnOnce(&mut Accept<'_>) -> Result<(), String>) -> Result<(), String> {
    let mut tree = Hierarchy::new(root, revision)?;
    if out.exists() {
        return Err("snapshot directory already exists; choose a new --out".into());
    }
    let stage = out.with_extension("part");
    if stage.exists() {
        return Err("unfinished staging directory exists; inspect it before retrying".into());
    }
    fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    // Canonical records and provenance are not tile objects. Only `tile/`
    // is published; the whole directory is one atomic snapshot.
    let backing = stage.join("canonical.jsonl");
    let provenance = stage.join("source.jsonl");
    let mut records = BufWriter::new(File::options().write(true).create_new(true).open(&backing).map_err(|e| e.to_string())?);
    let mut sources = BufWriter::new(File::options().write(true).create_new(true).open(&provenance).map_err(|e| e.to_string())?);
    let mut accept = |entity: Entity, source: &str| -> Result<(), String> {
        if tree.add(entity.clone())? {
            serde_json::to_writer(&mut records, &entity).map_err(|e| e.to_string())?;
            writeln!(records).map_err(|e| e.to_string())?;
            serde_json::to_writer(&mut sources, &serde_json::json!({"entity_id":entity.id,"source_id":source,"provider":provider})).map_err(|e| e.to_string())?;
            writeln!(sources).map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    visit(&mut accept)?;
    records.flush().map_err(|e| e.to_string())?;
    sources.flush().map_err(|e| e.to_string())?;
    let nodes = tree.finish();
    let tiles = stage.join("tile");
    fs::create_dir_all(&tiles).map_err(|e| e.to_string())?;
    for node in &nodes {
        let raw = wire::encode(node)?;
        let packed = pack(&raw, packing).map_err(|e| e.to_string())?;
        write_atomic(&tiles.join(xy::name(node.summary.key)), &packed).map_err(|e| e.to_string())?;
    }
    let manifest = serde_json::json!({"revision":revision,"root":xy::name(root),"nodes":nodes.len(),"count":nodes[0].summary.count,"complete":true,"content_encoding":CONTENT_ENCODING,"content_type":"application/octet-stream","format":"PXS1 summary hierarchy"});
    fs::write(stage.join("manifest.json"), serde_json::to_vec_pretty(&manifest).unwrap()).map_err(|e| e.to_string())?;
    fs::rename(stage, out).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
#[path = "snapshot.test.rs"]
mod test;
