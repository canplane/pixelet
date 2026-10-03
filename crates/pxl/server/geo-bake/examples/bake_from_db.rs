// pxl-geo-bake/examples/bake_from_db.rs

//! Bakes single pages straight from the geo DB, without the walk's threads,
//! resume or manifest, for inspecting one page.
//!
//!   cargo run -p pxl-geo-bake --example bake_from_db -- <dir> <page>...

use std::fs;
use std::sync::Arc;

use pxl_geo_bake::coverage::{Coverage, Overture};
use pxl_geo_bake::elevation::Raster;
use pxl_geo_bake::terrain::leaf::{self, bake_leaf};
use pxl_geo_bake::terrain::repository::TerrainRepository;
use pxl_geo_bake::terrain::source::SourceStats;
use pxl_geo_bake::util::{unpack, Level};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dir, pages) = args.split_first().expect("usage: <dir> <page>...");

    let coverage = Coverage::of_source().expect("coverage");
    let (_, mut client) = Overture::reading(&leaf::READS).expect("geo DB");
    let raster = Arc::new(Raster::open(&mut client).expect("raster"));
    let repo = TerrainRepository::prepare(&mut client, raster).expect("prepare");
    let mut stats = SourceStats::default();

    for page in pages {
        let idx = page.parse().expect("page_idx");
        assert!(coverage.contains(idx), "{page} is not a page of the coverage");
        let (_, packed) = bake_leaf(&repo, &mut client, &std::env::temp_dir().join("pxl-source"), idx, &mut stats, Level::Stored).expect("bake");
        let tile = unpack(&packed).expect("unpack");
        fs::write(format!("{dir}/{page}.db.tile"), &tile).expect("write");
        println!("{page}\t{}", tile.len());
    }
    if stats.flat_ele > 0 {
        eprintln!("[bake] no elevation on {} page(s)", stats.flat_ele);
    }
}
