// pxl-geo-bake/bin/bake_terrain.rs

//! The terrain bake, a shard at a time into its set (tile/shard.rs): `leaf`
//! stages the named shards' leaves (terrain/leaf.rs `bake_leaf`, each where
//! `has_ground` finds something drawn), then the zooms over them are folded
//! again and committed.
//!
//!   cargo run --profile server -p pxl-geo-bake --bin bake_terrain -- [flags]
//!
//!   --stage <s>          leaf, fold, commit or publish; repeatable (default: the first three)
//!   --shard <name>       bake that shard's leaves, repeatable (default: every shard of the coverage)
//!   --out <dir>          the set (default ./out/terrain), staged beside it in <dir>.stage
//!   --concurrency <n>    parallel tiles, one connection each (default 8)

use std::process::ExitCode;
use std::sync::Arc;

use postgres::Client;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use pxl_geo_bake::coverage::Overture;
use pxl_geo_bake::elevation::Raster;
use pxl_geo_bake::pyramid::Base as _;
use pxl_geo_bake::separation;
use pxl_geo_bake::terrain::absent::Absence;
use pxl_geo_bake::terrain::leaf::{self, bake_leaf, has_ground};
use pxl_geo_bake::terrain::repository::TerrainRepository;
use pxl_geo_bake::tile::shard::Run;
use pxl_geo_bake::tile::walk::{self, Done};
use pxl_geo_bake::util::{pack, Level};

const CLASSIFY_BATCH: usize = 2000;

struct Db {
    client: Client,
    repo: TerrainRepository,
}

fn connect(source: &Overture, raster: &Arc<Raster>) -> Result<Db, String> {
    let mut client = source.connect()?;
    let repo = TerrainRepository::prepare(&mut client, raster.clone()).map_err(|e| e.to_string())?;
    Ok(Db { client, repo })
}

/// A band's tiles row by row, each row the way the last one ended.
fn zigzag(band: &[[i32; 2]]) -> Vec<[i32; 2]> {
    let mut out = Vec::with_capacity(band.len());
    for (k, row) in band.chunk_by(|a, b| a[1] == b[1]).enumerate() {
        if k % 2 == 0 {
            out.extend(row.iter().copied());
        } else {
            out.extend(row.iter().rev().copied());
        }
    }
    out
}

/// The leaves something is drawn in.
fn land(client: &mut Client, candidates: &[[i32; 2]]) -> Result<Vec<[i32; 2]>, String> {
    let mut out = Vec::new();
    for batch in candidates.chunks(CLASSIFY_BATCH) {
        for (&tile, drawn) in batch.iter().zip(has_ground(client, batch).map_err(|e| e.to_string())?) {
            if drawn {
                out.push(tile);
            }
        }
    }
    Ok(out)
}

fn run() -> Result<(), String> {
    let run = Run::parse("terrain")?;
    if run.leaf {
        let (source, mut client) = Overture::reading(&leaf::READS, &run.shards.iter().copied().collect())?;
        let raster = Arc::new(Raster::open(&mut client)?);
        for &shard in &run.shards {
            let mut rows = land(&mut client, &shard.tiles(LEAF_ZOOM))?;
            println!("[bake/leaf/{}] {} land tiles", shard.name(), rows.len());
            run.stage.leaf::<TerrainTile>(shard, |set| {
                // Each worker walks its own band of rows, zigzag, so a tile's eight
                // around it were mostly read for the tile before.
                rows.sort_unstable_by_key(|&p| (p[1], p[0]));
                let band = rows.len().div_ceil(run.concurrency.max(1)).max(1);
                let walks: Vec<Vec<[i32; 2]>> = rows.chunks(band).map(zigzag).collect();
                let bands: Vec<&[[i32; 2]]> = walks.iter().map(Vec::as_slice).collect();
                let t = walk::over(
                    &format!("leaf/{}", shard.name()),
                    &bands,
                    bands.len(),
                    1000,
                    |[x, y]| format!("{x}:{y}"),
                    || Ok((connect(&source, &raster)?, separation::source::Walk::default())),
                    |(db, held), tile| {
                        held.keep_around(tile);
                        let reader = &mut separation::Reader {
                            client: &mut db.client,
                            repo: &db.repo,
                            store: held,
                        };
                        let terrain = bake_leaf(reader, tile)?;
                        let at = Tile::new(LEAF_ZOOM, tile);
                        let packed = pack(&terrain.encode(at).map_err(|e| format!("{}: {e}", at.name()))?, Level::Stored).map_err(|e| e.to_string())?;
                        set.write(at, &packed)?;
                        Ok(Done::Wrote { bytes: packed.len() as u64, items: 0 })
                    },
                )?;
                if t.failed > 0 {
                    return Err(format!("{}: {} tiles failed", shard.name(), t.failed));
                }
                Ok(())
            })?;
        }
    }
    run.finish::<TerrainTile, _>(|run| Ok(Absence::new(run.baked()?)), &leaf::READS, false)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("[bake] {err}");
            ExitCode::FAILURE
        }
    }
}
