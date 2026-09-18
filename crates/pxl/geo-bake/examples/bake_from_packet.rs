//! Bakes a page from a source packet already on disk and writes the tile
//! beside it, so the same bytes can be put through this pipeline and the
//! TypeScript's and the results compared.
//!
//!   cargo run -p pxl-geo-bake --example bake_from_packet -- <dir> <page_idx>...

use std::fs;

use pxl_geo_bake::terrain::bake::bake_terrain;
use xpute_core::kit::array::U8Array;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dir, pages) = args.split_first().expect("usage: <dir> <page_idx>...");

    for page in pages {
        let src = fs::read(format!("{dir}/{page}.src")).expect("source packet");
        let tile = bake_terrain(page.parse().expect("page_idx"), U8Array::from(&src)).expect("bake");
        fs::write(format!("{dir}/{page}.rs.tile"), tile.as_slice()).expect("write");
        println!("{page}  src={}  tile={}", src.len(), tile.length());
    }
}
