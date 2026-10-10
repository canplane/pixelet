// pxl-geo-bake/bin/elevation_shards.rs

//! The shards the elevation ingest warps, one a line: its name, its EPSG:3857
//! extent with its apron, the stride the bake samples elevation at and the
//! raster's block, so every shard lands on one grid of blocks:
//! `name minx miny maxx maxy stride block block_m`. The apron is the shard's
//! (`SHARD_APRON`) taken out to whole blocks, so a block a neighbour warps
//! too is the same block, stored once.
//!
//!   ./infra/geo/elevation/script/setup-elevation.sh   # reads it

use std::process::ExitCode;

use pxl_geo_bake::coverage::{Coverage, SHARD_APRON};
use pxl_geo_bake::elevation::BLOCK;
use pxl_spec::bake::constant::TILE_ELE_SPAN;
use pxl_spec::space::coord::adapter::epsg3857::mer_from_u;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

fn run() -> Result<(), String> {
    let coverage = Coverage::of_source()?;
    eprintln!("[elevation_shards] {} shards", coverage.shards().count());
    let stride = mer_from_u(units(LEAF_ZOOM)) / f64::from(TILE_ELE_SPAN);
    let block_u = units(LEAF_ZOOM) / f64::from(TILE_ELE_SPAN) * f64::from(BLOCK);
    let apron = (SHARD_APRON / block_u).ceil() * block_u;
    let block_m = stride * f64::from(BLOCK);
    for shard in coverage.shards() {
        let [x0, y0, x1, y1] = shard.rect();
        let [x0, y0, x1, y1] = [x0 - apron, y0 - apron, x1 + apron, y1 + apron].map(mer_from_u);
        println!("{} {x0} {y0} {x1} {y1} {stride} {BLOCK} {block_m}", shard.name());
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[elevation_shards] {e}");
            ExitCode::FAILURE
        }
    }
}
