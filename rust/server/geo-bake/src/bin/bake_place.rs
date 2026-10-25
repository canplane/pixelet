// pxl-geo-bake/bin/bake_place.rs

//! The places pyramid, a shard at a time into its set (tile/shard.rs):
//! `leaf` stages the named shards' leaves, then the zooms over them are
//! folded again and committed; a version leaves out the tiles their parent
//! marks as none.
//!
//!   --stage <s>          leaf, fold, commit or publish; repeatable (default: the first three)
//!   --shard <name>       bake that shard's leaves, repeatable (default: every shard of the coverage)
//!   --out <dir>          the set (default ./out/place), staged beside it in <dir>.stage
//!   --concurrency <n>    parallel folds (default 8)

use pxl_geo_bake::coverage::{Coverage, Overture};
use pxl_geo_bake::place::{self, leaves};
use pxl_geo_bake::pyramid::Base;
use pxl_geo_bake::tile::shard::Run;
use pxl_geo_bake::util::{pack, Level};
use pxl_spec::bake::tile::place::PlaceTile;
use pxl_spec::space::coord::zoom::SHARD_ZOOM;
use pxl_spec::space::layer::tile::Tile;

fn run() -> Result<(), String> {
    pxl_geo_bake::cli::enter()?;
    let run = Run::parse("place")?;
    let coverage = Coverage::of_source()?;
    if run.leaf {
        let (_, mut client) =
            Overture::reading(&[&place::READS], &run.shards.iter().copied().collect())?;
        for &shard in &run.shards {
            let within = Tile::new(SHARD_ZOOM, shard.at);
            let tiles = leaves(&mut client, &coverage, within)?;
            run.stage.leaf::<PlaceTile>(shard, |set| {
                for (tile, place) in &tiles {
                    set.write(
                        *tile,
                        &pack(&place.encode(*tile)?, Level::Stored).map_err(|e| e.to_string())?,
                    )?;
                }
                Ok(())
            })?;
            println!("[bake/place/{}] {} leaves", shard.name(), tiles.len());
        }
    }
    run.finish::<PlaceTile, _>(|_| Ok(PlaceTile::default()), &[&place::READS], true)
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[bake/place] {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
