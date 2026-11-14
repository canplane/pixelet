// pxl-geo-bake/bin/division.rs

//! The divisions pyramid, a shard at a time into its set (`tile/shard.rs`):
//! `leaf` stages the named shards' leaves (`division::leaf`), then the
//! zooms over them are folded again and committed; a version leaves out the
//! tiles that say no more than their parent.
//!
//!   cargo run --profile server -p pxl-geo-bake --bin division -- [flags]
//!
//!   --step <s>           leaf, fold, commit or publish; repeatable (default: the first three)
//!   --shard <name>       bake that shard's leaves, repeatable (default: every shard of the coverage)
//!   --out <dir>          the set (default ./out/division), staged beside it in <dir>.stage
//!   --concurrency <n>    parallel tiles, one connection each (default 8)

use std::process::ExitCode;

use pxl_geo_bake::cli;
use pxl_geo_bake::coverage::Overture;
use pxl_geo_bake::division::{leaf, READS};
use pxl_geo_bake::pyramid::Base;
use pxl_geo_bake::tile::shard::Run;
use pxl_geo_bake::tile::walk::{over, Outcome};
use pxl_geo_bake::util::{pack, Level};
use pxl_spec::bake::tile::division::{DivisionTile, DIVISION_LEAF};
use pxl_spec::space::layer::quadrant::Quadrant;

fn run() -> Result<(), String> {
    cli::enter()?;
    let run = Run::parse("division")?;
    if run.leaf {
        let (src, _) = Overture::reading(&[&READS], &run.shards.iter().copied().collect())?;
        for &shard in &run.shards {
            let tiles = shard.tiles(DIVISION_LEAF);
            run.stage.leaf::<DivisionTile>(shard, |set| {
                let bands: Vec<&[[i32; 2]]> = tiles.chunks(1).collect();
                let t = over(
                    &format!("division/{}", shard.name()),
                    &bands,
                    run.concurrency,
                    200,
                    |[x, y]| format!("{x}:{y}"),
                    || src.connect(),
                    |client, tile| match leaf(client, tile)? {
                        None => Ok(Outcome::Empty),
                        Some(divisions) => {
                            let at = Quadrant::new(DIVISION_LEAF, tile);
                            let packed = pack(&divisions.encode(at)?, Level::Stored)
                                .map_err(|e| e.to_string())?;
                            set.write(at, &packed)?;
                            Ok(Outcome::Tile {
                                bytes: packed.len() as u64,
                            })
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
    run.finish::<DivisionTile, _>(|_| Ok(DivisionTile::default()), &[&READS])
}

fn main() -> ExitCode {
    cli::main("bake/division", run)
}
