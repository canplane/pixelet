// pxl-geo-bake/bin/bake_poi_tiles.rs

//! One immutable POI summary snapshot. --root is a 15-digit XY region;
//! --revision names the snapshot. --input reads canonical JSONL sorted by ID
//! for offline verification; otherwise the source is the geo database.

use pxl_geo_bake::poi::{provider::overture, snapshot};
use pxl_geo_bake::util::Level;
use pxl_spec::bake::published::PUBLISHED_VERSION;
use pxl_spec::poi::{xy, Entity};
use std::fs::File;
use std::io::{BufRead, BufReader};
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
    snapshot::write(&out, root, revision, if input.is_some() { "canonical-input" } else { "overture" }, Level::Stored, |accept| {
        if let Some(path) = &input {
            for line in BufReader::new(File::open(path).map_err(|e| e.to_string())?).lines() {
                let entity: Entity = serde_json::from_str(&line.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
                accept(entity, "")?;
            }
        } else {
            let url = std::env::var("DB_GEO_URL").map_err(|_| "DB_GEO_URL required")?;
            let mut client = postgres::Client::connect(&url, postgres::NoTls).map_err(|e| e.to_string())?;
            overture::visit(&mut client, root, accept)?;
        }
        Ok(())
    })?;
    println!("[bake/poi] complete: {}", out.display());
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
