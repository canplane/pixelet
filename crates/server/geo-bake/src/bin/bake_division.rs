// pxl-geo-bake/bin/bake_division.rs

//! The divisions pyramid, a shard at a time into its set (tile/shard.rs):
//! `leaf` stages the named shards' leaves (division.rs `leaf`), then the
//! zooms over them are folded again and committed; a version leaves out the
//! tiles that say no more than their parent.
//!
//!   cargo run --profile server -p pxl-geo-bake --bin bake_division -- [flags]
//!
//!   --stage <s>          leaf, fold, commit or publish; repeatable (default: the first three)
//!   --shard <name>       bake that shard's leaves, repeatable (default: every shard of the coverage)
//!   --out <dir>          the set (default ./out/division), staged beside it in <dir>.stage
//!   --concurrency <n>    parallel tiles, one connection each (default 8)

use std::process::ExitCode;

use pxl_geo_bake::coverage::Overture;
use pxl_geo_bake::division::{leaf, READS};
use pxl_geo_bake::pyramid::Base;
use pxl_geo_bake::tile::shard::Run;
use pxl_geo_bake::tile::walk::{over, Done};
use pxl_geo_bake::util::{pack, Level};
use pxl_spec::bake::tile::division::{DivisionTile, DIVISION_LEAF};
use pxl_spec::space::coord::pvf3::ivec::{self, IVec};
use pxl_spec::space::layer::tile::Tile;

fn run() -> Result<(), String> {
    let run = Run::parse("division")?;
    if run.leaf {
        let (source, _) = Overture::reading(&[&READS], &run.shards.iter().copied().collect())?;
        for &shard in &run.shards {
            let tiles = shard.tiles(DIVISION_LEAF);
            run.stage.leaf::<DivisionTile>(shard, |set| {
                let bands: Vec<&[IVec]> = tiles.chunks(1).collect();
                let t = over(
                    &format!("division/{}", shard.name()),
                    &bands,
                    run.concurrency,
                    200,
                    ivec::repr,
                    || source.connect(),
                    |client, tile| match leaf(client, tile)? {
                        None => Ok(Done::Empty),
                        Some(divisions) => {
                            let packed = pack(&divisions.encode()?, Level::Stored).map_err(|e| e.to_string())?;
                            set.write(Tile::new(DIVISION_LEAF, tile), &packed)?;
                            Ok(Done::Wrote { bytes: packed.len() as u64, items: 0 })
                        }
                    },
                )?;
                if t.failed > 0 {
                    return Err(format!("{}: {} tiles failed", shard.name(), t.failed));
                }
                Ok(())
            })?;
        }
    }
    run.finish::<DivisionTile>(&DivisionTile::default(), &[&READS], true)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("[bake/division] {err}");
            ExitCode::FAILURE
        }
    }
}
