// pxl-geo-bake/bin/elevation_shards.rs

//! The shards the elevation ingest warps, one a line: its name, its EPSG:3857
//! extent and the stride the bake samples elevation at, so every shard lands
//! on one grid: `name minx miny maxx maxy stride`.
//!
//!   ./infra/geo/elevation/script/setup-elevation.sh   # reads it

use std::process::ExitCode;

use pxl_geo_bake::coverage::Coverage;
use pxl_spec::bake::constant::TILE_ELE_SPAN;
use pxl_spec::space::coord::adapter::epsg3857::to_db;
use pxl_spec::space::pyramid::LEAF_Z;
use pxl_spec::space::coord::zoom::units;

fn run() -> Result<(), String> {
    let coverage = Coverage::of_source()?;
    eprintln!("[elevation_shards] {} shards", coverage.shards().count());
    let stride = to_db(units(LEAF_Z)) / f64::from(TILE_ELE_SPAN);
    for shard in coverage.shards() {
        let [x0, y0, x1, y1] = shard.rect().map(to_db);
        println!("{} {x0} {y0} {x1} {y1} {stride}", shard.name());
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
