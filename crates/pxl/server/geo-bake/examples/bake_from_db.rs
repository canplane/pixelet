// pxl-geo-bake/examples/bake_from_db.rs

//! Bakes single pages straight from the geo DB, without the walk's threads,
//! resume or manifest, for inspecting one page.
//!
//!   cargo run -p pxl-geo-bake --example bake_from_db -- <dir> <page>...

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
        assert!(coverage.contains(idx), "{page} is not a page of the coverage");
        let reader = &mut deck::Reader {
            client: &mut client,
            repo: &repo,
            store: &mut Disk(&memo),
            memo: &memo.join("structure"),
        };
        let (terrain, off_raster) = bake_leaf(reader, idx).expect("bake");
        let tile = encode_terrain(&terrain).expect("encode");
        fs::write(format!("{dir}/{page}.db.tile"), &tile).expect("write");
        println!("{page}\t{}", tile.len());
    }
    if stats.invalid_ele_shape > 0 {
        eprintln!("[bake] invalid ele shape on {} page(s)", stats.invalid_ele_shape);
    }
}
