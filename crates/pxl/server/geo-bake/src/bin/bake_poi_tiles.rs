// pxl-geo-bake/bin/bake_poi_tiles.rs

//! One immutable POI summary snapshot. --root is a 15-digit XY region;
//! --revision names the snapshot. --input reads canonical JSONL sorted by ID
//! for offline verification; otherwise the source is the geo database.

use pxl_geo_bake::poi::{bake::Hierarchy, provider::overture};
use pxl_geo_bake::util::{pack, write_atomic, Level, CONTENT_ENCODING};
use pxl_spec::bake::published::PUBLISHED_VERSION;
use pxl_spec::poi::{wire, xy, Entity};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

fn run() -> Result<(), String> {
    let mut root = None;
    let mut revision = None;
    let mut input = None;
    let mut out = PathBuf::from(format!("out/poi/{}", PUBLISHED_VERSION.poi));
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or("missing flag value")?;
        match flag.as_str() {
            "--root" => root = xy::parse(&value),
            "--revision" => revision = Some(value.parse::<u64>().map_err(|e| e.to_string())?),
            "--input" => input = Some(PathBuf::from(value)),
            "--out" => out = PathBuf::from(value),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    let root = root.ok_or("--root <15-digit quadkey> required")?;
    let revision = revision.ok_or("--revision <nonzero snapshot id> required")?;
    let mut tree = Hierarchy::new(root, revision)?;
    if out.exists() {
        return Err("snapshot directory already exists; choose a new --out".into());
    }
    let stage = out.with_extension("part");
    if stage.exists() {
        return Err("unfinished staging directory exists; inspect it before retrying".into());
    }
    fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    // Canonical records and provenance are not tile objects. Keep them outside
    // the served directory and never expose them through the tile endpoint.
    let backing = out.with_extension("canonical.jsonl");
    let provenance = out.with_extension("source.jsonl");
    let mut records = BufWriter::new(File::options().write(true).create_new(true).open(&backing).map_err(|e| e.to_string())?);
    let mut sources = BufWriter::new(File::options().write(true).create_new(true).open(&provenance).map_err(|e| e.to_string())?);
    let mut accept = |entity: Entity, source: &str| -> Result<(), String> {
        if tree.add(entity.clone())? {
            serde_json::to_writer(&mut records, &entity).map_err(|e| e.to_string())?;
            writeln!(records).map_err(|e| e.to_string())?;
            serde_json::to_writer(
                &mut sources,
                &serde_json::json!({"entity_id":entity.id,"source_id":source,"provider":if input.is_some(){"canonical-input"}else{"overture"}}),
            )
            .map_err(|e| e.to_string())?;
            writeln!(sources).map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    if let Some(path) = &input {
        for line in BufReader::new(File::open(path).map_err(|e| e.to_string())?).lines() {
            let entity: Entity = serde_json::from_str(&line.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            accept(entity, "")?;
        }
    } else {
        let url = std::env::var("DB_GEO_URL").map_err(|_| "DB_GEO_URL required")?;
        let mut client = postgres::Client::connect(&url, postgres::NoTls).map_err(|e| e.to_string())?;
        overture::visit(&mut client, root, &mut accept)?;
    }
    records.flush().map_err(|e| e.to_string())?;
    sources.flush().map_err(|e| e.to_string())?;
    let nodes = tree.finish();
    for node in &nodes {
        let raw = wire::encode(node)?;
        let packed = pack(&raw, Level::Stored).map_err(|e| e.to_string())?;
        write_atomic(&stage.join(xy::name(node.summary.key)), &packed).map_err(|e| e.to_string())?;
        if node.summary.key == root {
            write_atomic(&stage.join("_root"), &packed).map_err(|e| e.to_string())?;
        }
    }
    let manifest = serde_json::json!({"revision":revision,"root":xy::name(root),"nodes":nodes.len(),"count":nodes[0].summary.count,"complete":true,"content_encoding":CONTENT_ENCODING,"content_type":"application/octet-stream","format":"PXS1 summary hierarchy"});
    fs::write(out.with_extension("manifest.json"), serde_json::to_vec_pretty(&manifest).unwrap()).map_err(|e| e.to_string())?;
    fs::rename(stage, &out).map_err(|e| e.to_string())?;
    println!("[bake/poi] {} nodes, {} entities, revision {revision}: {}", nodes.len(), nodes[0].summary.count, out.display());
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[bake/poi] {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
