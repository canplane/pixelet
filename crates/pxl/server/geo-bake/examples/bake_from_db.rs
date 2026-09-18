//! Bakes pages straight out of the geo database, the whole path the offline
//! tool walks — five queries, the packet, the rasterize — and writes each
//! tile beside the last. One page at a time, without the walk's threads,
//! resume or manifest, which is what makes it useful for looking at a single
//! page that came out wrong.
//!
//! It had a sibling, `bake_from_packet`, that baked from a packet already on
//! disk so the same bytes could go through this pipeline and the TypeScript's.
//! Its only producer was `dump_page_source.ts`, and both went when the
//! TypeScript did.
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
