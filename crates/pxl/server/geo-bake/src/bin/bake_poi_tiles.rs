// pxl-geo-bake/bin/bake_poi_tiles.rs

//! The POI pyramid into `out/poi/<version>`.
//!
//!   --out <dir>          where the version goes (default out/poi/v<format>.<UTC day>.<n>, the day's next)
//!   --concurrency <n>    parallel folds (default 8)

use std::fs;
use std::path::PathBuf;

use pxl_geo_bake::cli::Flags;
use pxl_geo_bake::coverage::{Coverage, Overture};
use pxl_geo_bake::poi::{self, leaves};
use pxl_geo_bake::pyramid::{build, prune, write_manifest, Layer, TileSet};
use pxl_geo_bake::util::{git_sha, next_version, pack, refuse_mixed, Level};
use pxl_spec::bake::poi_tile::PoiTile;
use pxl_spec::bake::published::FORMAT;
use pxl_spec::space::pyramid::{Tile, LEAF_Z};

fn run() -> Result<(), String> {
    let built = git_sha();
    let mut out = None;
    let mut concurrency = 8;
    let mut flags = Flags::from_env();
    while let Some(flag) = flags.arg() {
        match flag.as_str() {
            "--out" => out = Some(PathBuf::from(flags.value()?)),
            "--concurrency" => concurrency = flags.parse()?,
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    let (source, mut client) = Overture::reading(&[&poi::READS])?;
    let out = out.unwrap_or_else(|| next_version("./out/poi", FORMAT.poi));
    let set = TileSet::new(&out);
    if set.root.exists() {
        refuse_mixed(&set.root)?;
        fs::remove_dir_all(&set.root).map_err(|e| e.to_string())?;
    }
    let tiles = leaves(&mut client, &Coverage::of_source()?, None)?;
    for (tile, poi) in &tiles {
        set.write(Tile::new(LEAF_Z, *tile), &pack(&poi.encode()?, Level::Stored).map_err(|e| e.to_string())?)?;
    }
    println!("[bake/poi] z{LEAF_Z}: {} tiles", tiles.len());
    let top = build(&set, &PoiTile::default(), concurrency)?;
    println!("[bake/poi] {} tiles their parent settles let go", prune::<PoiTile>(&set, top)?);
    write_manifest(&set, top, built.as_deref(), serde_json::json!({ "release": source.release, "source": source.manifest() }))?;
    println!("[bake/poi] complete to z{top}: {}", out.display());
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
