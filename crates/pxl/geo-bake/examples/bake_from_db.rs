//! Bakes pages straight out of the geo database, the whole path the offline
//! tool will walk — five queries, the packet, the rasterize — so it can be
//! held against what the TypeScript produces for the same pages.
//!
//!   cargo run -p pxl-geo-bake --example bake_from_db -- <dir> <page>...
//!
//! Reads DB_GEO_URL from the environment.

use std::fs;

use postgres::{Client, NoTls};
use pxl_geo_bake::terrain::repository::TerrainRepository;
use pxl_geo_bake::terrain::source::{fetch_tile, SourceStats};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dir, pages) = args.split_first().expect("usage: <dir> <page>...");

    let url = std::env::var("DB_GEO_URL").expect("DB_GEO_URL");
    let mut client = Client::connect(&url, NoTls).expect("connect");
    let repo = TerrainRepository::prepare(&mut client).expect("prepare");
    let mut stats = SourceStats::default();

    for page in pages {
        let idx = page.parse().expect("page_idx");
        let tile = fetch_tile(&repo, &mut client, idx, &mut stats).expect("bake");
        fs::write(format!("{dir}/{page}.db.tile"), tile.as_slice()).expect("write");
        println!("{page}\t{}", tile.length());
    }
    if stats.invalid_ele_shape > 0 {
        eprintln!("[bake] invalid ele shape on {} page(s)", stats.invalid_ele_shape);
    }
}
