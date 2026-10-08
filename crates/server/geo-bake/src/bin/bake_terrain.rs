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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use postgres::Client;
use pxl_spec::bake::format::{encode_terrain, TerrainTile};
use pxl_spec::space::coord::pvf3::ivec::{self, IVec};
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use pxl_geo_bake::coverage::{Coverage, Overture};
use pxl_geo_bake::elevation::Raster;
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
fn zigzag(band: &[IVec]) -> Vec<IVec> {
    let mut out = Vec::with_capacity(band.len());
    for (k, row) in band.chunk_by(|a, b| ivec::Y(*a) == ivec::Y(*b)).enumerate() {
        if k % 2 == 0 {
            out.extend(row.iter().copied());
        } else {
            out.extend(row.iter().rev().copied());
        }
    }
    out
}

/// The leaves something is drawn in.
fn land(client: &mut Client, candidates: &[IVec]) -> Result<Vec<IVec>, String> {
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
    let coverage = Coverage::of_source()?;
    if run.leaf {
        let (source, mut client) = Overture::reading(&leaf::READS, &run.shards.iter().copied().collect())?;
        let raster = Arc::new(Raster::open(&mut client)?);
        for &shard in &run.shards {
            let mut rows = land(&mut client, &shard.tiles(LEAF_ZOOM))?;
            println!("[bake/leaf/{}] {} land tiles", shard.name(), rows.len());
            run.stage.leaf::<TerrainTile>(shard, |set| {
                // Each worker walks its own band of rows, zigzag, so a tile's eight
                // around it were mostly read for the tile before.
                rows.sort_unstable_by_key(|&p| (ivec::Y(p), ivec::X(p)));
                let band = rows.len().div_ceil(run.concurrency.max(1)).max(1);
                let walks: Vec<Vec<IVec>> = rows.chunks(band).map(zigzag).collect();
                let bands: Vec<&[IVec]> = walks.iter().map(Vec::as_slice).collect();
                let flat = AtomicU64::new(0);
                let t = walk::over(
                    &format!("leaf/{}", shard.name()),
                    &bands,
                    bands.len(),
                    1000,
                    ivec::repr,
                    || Ok((connect(&source, &raster)?, separation::source::Walk::default())),
                    |(db, held), tile| {
                        held.keep_around(tile);
                        let reader = &mut separation::Reader {
                            client: &mut db.client,
                            repo: &db.repo,
                            store: held,
                        };
                        let (terrain, off_raster) = bake_leaf(reader, tile)?;
                        flat.fetch_add(u64::from(off_raster), Ordering::Relaxed);
                        let packed = pack(&encode_terrain(&terrain).map_err(|e| format!("{e:?}"))?, Level::Stored).map_err(|e| e.to_string())?;
                        set.write(Tile::new(LEAF_ZOOM, tile), &packed)?;
                        Ok(Done::Wrote { bytes: packed.len() as u64, items: 0 })
                    },
                )?;
                if t.failed > 0 {
                    return Err(format!("{}: {} tiles failed", shard.name(), t.failed));
                }
                // A degraded tile never shows up in `failed`, so report it here.
                let flat = flat.into_inner();
                if flat > 0 {
                    eprintln!(
                        "[bake/leaf/{}] WARNING: {flat} tiles are out of the elevation raster's reach and were baked FLAT (zero-filled elevation).",
                        shard.name()
                    );
                }
                Ok(())
            })?;
        }
    }
    run.finish::<TerrainTile>(&Absence::new(coverage.clone()), &leaf::READS, false)
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
