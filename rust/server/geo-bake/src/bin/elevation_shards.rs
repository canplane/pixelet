// pxl-geo-bake/bin/elevation_shards.rs

//! The shards the elevation ingest warps, one a line: its name, its EPSG:3857
//! extent with its apron, the stride the bake samples elevation at and the
//! raster's block, so every shard lands on one grid of blocks:
//! `name minx miny maxx maxy stride block block_merc`. The apron is the shard's
//! (`SHARD_APRON`) taken out to whole blocks, so a block a neighbor warps
//! too is the same block, stored once.
//!
//! With `--blocks <shard>`, the shard's blocks instead, one a line, `bx by
//! tile`: a block's place on that grid (its west and north edges over
//! `block_merc`, as the ingest finds a warped block by) and the name of the tile
//! at `BLOCK_ZOOM` it is, which the raster keeps it under.
//!
//!   ./infra/geo/elevation/ingest.sh   # reads it

use std::process::ExitCode;

use pxl_geo_bake::coverage::{Coverage, SHARD_APRON};
use pxl_geo_bake::elevation::{BLOCK, BLOCK_ZOOM};
use pxl_spec::bake::constant::TILE_ELE_SPAN;
use pxl_spec::space::coord::adapter::epsg3857::merc_from_u;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::tile::{tile_at, tile_rows, LEAF_ZOOM};
use xpute_kit::math::scalar::round;

fn run() -> Result<(), String> {
    let coverage = Coverage::of_source()?;
    let stride = merc_from_u(units(LEAF_ZOOM)) / f64::from(TILE_ELE_SPAN);
    let block_u = units(LEAF_ZOOM) / f64::from(TILE_ELE_SPAN) * f64::from(BLOCK);
    let ring = (SHARD_APRON / block_u).ceil();
    let apron = ring * block_u;
    let block_merc = stride * f64::from(BLOCK);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [flag, name] = args.as_slice() {
        if flag != "--blocks" {
            return Err(format!("unknown flag: {flag}"));
        }
        let shard = coverage
            .shards()
            .find(|s| s.name() == *name)
            .ok_or(format!("{name}: no shard of the coverage"))?;
        let inner = shard.tiles(BLOCK_ZOOM);
        let (xs, ys) = (inner.iter().map(|&p| p[0]), inner.iter().map(|&p| p[1]));
        let (x0, x1) = (xs.clone().min().unwrap_or(0), xs.max().unwrap_or(0));
        let (y0, y1) = (ys.clone().min().unwrap_or(0), ys.max().unwrap_or(0));
        let ring = ring as i32;
        for y in (y0 - ring..=y1 + ring).filter(|y| tile_rows(BLOCK_ZOOM).contains(y)) {
            for x in x0 - ring..=x1 + ring {
                let tile = tile_at(BLOCK_ZOOM, [x, y]);
                let (west, south, side) = tile.rect();
                let (w, n) = (merc_from_u(west), merc_from_u(south + side));
                // A block is the tile: its side is the block's, or the grid is not the pyramid's.
                if (merc_from_u(west + side) - w - block_merc).abs() > 1e-6 * block_merc {
                    return Err(format!("{}: its side is not a block's", tile.name()));
                }
                println!(
                    "{} {} {}",
                    round(w / block_merc) as i64,
                    round(n / block_merc) as i64,
                    tile.name()
                );
            }
        }
        return Ok(());
    }
    eprintln!("[elevation_shards] {} shards", coverage.shards().count());
    for shard in coverage.shards() {
        let [x0, y0, x1, y1] = shard.rect();
        let [x0, y0, x1, y1] = [x0 - apron, y0 - apron, x1 + apron, y1 + apron].map(merc_from_u);
        println!(
            "{} {x0} {y0} {x1} {y1} {stride} {BLOCK} {block_merc}",
            shard.name()
        );
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
