// pxl-geo-bake/bin/elevation_shards.rs

//! The shards the elevation ingest warps, one a line: its name, its EPSG:3857
//! extent and the stride the bake samples elevation at, so every shard lands
//! on one grid: `name minx miny maxx maxy stride`.
//!
//!   ./infra/geo/elevation/script/setup-elevation.sh   # reads it

use std::process::ExitCode;

use pxl_geo_bake::coverage::Coverage;
use pxl_spec::bake::constant::TILE_ELE_SPAN;
use pxl_spec::space::coord::adapter::epsg3857::mer_from_u;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

fn run() -> Result<(), String> {
    let coverage = Coverage::of_source()?;
    eprintln!("[elevation_shards] {} shards", coverage.shards().count());
    let stride = mer_from_u(units(LEAF_ZOOM)) / f64::from(TILE_ELE_SPAN);
    for shard in coverage.shards() {
        let [x0, y0, x1, y1] = shard.rect().map(mer_from_u);
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
