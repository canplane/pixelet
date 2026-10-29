// pxl-geo-bake/examples/bake_from_db.rs

//! Bakes single tiles straight from the geo DB, without the walk's threads,
//! resume or manifest, for inspecting one tile.
//!
//!   cargo run -p pxl-geo-bake --example bake_from_db -- <dir> <leaf name>...

use std::fs;
use std::sync::Arc;

use pxl_geo_bake::coverage::{Coverage, Overture};
use pxl_geo_bake::elevation::Raster;
use pxl_geo_bake::separation::{self, source::Disk};
use pxl_geo_bake::terrain::leaf::{self, bake_leaf};
use pxl_geo_bake::terrain::repository::TerrainRepository;
use pxl_spec::bake::tile::terrain;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dir, tiles) = args.split_first().expect("usage: <dir> <leaf name>...");

    let coverage = Coverage::of_source().expect("coverage");
    let (_, mut client) = Overture::reading(
        &leaf::READS,
        &Coverage::of_source().expect("coverage").shards().collect(),
    )
    .expect("geo DB");
    let raster = Arc::new(Raster::open(&mut client).expect("raster"));
    let repo = TerrainRepository::prepare(&mut client, raster).expect("prepare");
    let memo = std::env::temp_dir().join("pxl-source");

    for tile in tiles {
        let idx = Tile::of_name(tile)
            .filter(|t| t.zoom == LEAF_ZOOM)
            .expect("a leaf's name")
            .at;
        assert!(
            coverage.contains(idx),
            "{tile} is not a tile of the coverage"
        );
        let reader = &mut separation::Reader {
            client: &mut client,
            repo: &repo,
            store: &mut Disk(&memo),
        };
        let terrain = bake_leaf(reader, idx).expect("bake");
        let bytes = terrain::encode(&terrain).expect("encode");
        fs::write(format!("{dir}/{tile}.db.tile"), &bytes).expect("write");
        println!("{tile}\t{}", bytes.len());
    }
}
