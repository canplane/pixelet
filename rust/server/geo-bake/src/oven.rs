// pxl-geo-bake/oven.rs

mod cache;
mod division;
mod place;
mod pyramid;
mod terrain;

use cache::Cache;
use pxl_spec::bake::publication::BaseLayer;
use pxl_spec::bake::tile;
use pxl_spec::bake::tile::division::DivisionTile;
use pxl_spec::bake::tile::place::PlaceTile;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::layer::quadrant::Quadrant;

use crate::pyramid::Base;
use std::path::{Path, PathBuf};

use crate::coverage::Coverage;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::coarsest;
use pxl_spec::space::layer::quadrant::min_zoom_over;
use std::sync::Arc;

pub mod job;
use job::{Limit, Runner};

/// What the oven does for a kind, one row a kind: the kind, what its tiles
/// are, the tiles a fold reads and a job's bake.
struct Row {
    kind: BaseLayer,
    tile_row: tile::Row,
    sources: fn(Quadrant) -> Vec<Quadrant>,
    execute: fn(Quadrant, &Path, &Path, u64) -> Result<(), String>,
}

/// None for a kind the oven does not bake, which is the Earth's picture.
fn row(kind: BaseLayer) -> Option<Row> {
    let (sources, execute): (fn(Quadrant) -> Vec<Quadrant>, _) = match kind {
        BaseLayer::terrain => (TerrainTile::sources, terrain::execute as _),
        BaseLayer::place => (PlaceTile::sources, place::execute as _),
        BaseLayer::division => (DivisionTile::sources, division::execute as _),
        BaseLayer::earth => return None,
    };
    Some(Row {
        kind,
        tile_row: kind.row(),
        sources,
        execute,
    })
}

#[derive(Clone, Debug)]
pub enum Failure {
    BadName,
    Unavailable,
    Limited(&'static str),
    Bake(String),
}
impl From<String> for Failure {
    fn from(e: String) -> Self {
        Self::Bake(e)
    }
}

/// A kind's tiles: its row, their cache and the coverage that gates them.
struct Dataset {
    row: Row,
    cache: Cache,
    coverage: Coverage,
}
impl Dataset {
    fn open(base: &Path, row: Row, runner: &Runner) -> Result<Self, String> {
        Ok(Self {
            cache: Cache::new(
                base.join(row.kind.name()),
                runner.admission.clone(),
                std::time::Duration::from_secs(runner.limit.seconds),
            )?,
            coverage: Coverage::of_source()?,
            row,
        })
    }
}

pub struct Server {
    _owner: std::fs::File,
    runner: Arc<Runner>,
    datasets: Vec<Arc<Dataset>>,
    min_zoom: Zoom,
}
impl Server {
    pub fn open(
        worker_cnt: usize,
        base: PathBuf,
        min_zoom: Zoom,
        limit: Limit,
    ) -> Result<Self, String> {
        let owner = claim(&base)?;
        let runner = Arc::new(Runner::new(worker_cnt, limit)?);
        // Every kind is pruned and opened before the first failure is told.
        let datasets: Vec<Result<_, String>> = BaseLayer::ALL
            .into_iter()
            .filter_map(row)
            .map(|row| {
                let source = base.join(row.kind.name());
                prune(&source)?;
                Dataset::open(&source, row, &runner).map(Arc::new)
            })
            .collect();
        Ok(Self {
            _owner: owner,
            runner,
            datasets: datasets.into_iter().collect::<Result<_, _>>()?,
            min_zoom,
        })
    }

    pub async fn get(&self, kind: BaseLayer, name: &str) -> Result<Option<Vec<u8>>, Failure> {
        let Some(data) = self.datasets.iter().find(|d| d.row.kind == kind).cloned() else {
            return Ok(None);
        };
        let runner = self.runner.clone();
        let tile_row = &data.row.tile_row;
        let top = min_zoom_over(tile_row.leaf, self.min_zoom).max(coarsest(tile_row.lattice));
        let path = pyramid::resolve(data.clone(), runner, name, top).await?;
        let Some(path) = path else { return Ok(None) };
        data.cache.read(&path).await.map_err(Failure::Bake)
    }
}

const LOCK: &str = ".lock";

/// Clears what a killed server left that is not a tile: its jobs' staging.
fn prune(root: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        if entry.file_name().to_string_lossy().starts_with(".work-") {
            std::fs::remove_dir_all(entry.path()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// The lock keeps a second server from pruning under a running one.
fn claim(base: &Path) -> Result<std::fs::File, String> {
    std::fs::create_dir_all(base).map_err(|e| e.to_string())?;
    let owner = std::fs::File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(base.join(LOCK))
        .map_err(|e| e.to_string())?;
    owner
        .try_lock()
        .map_err(|_| format!("{}: another server owns this cache", base.display()))?;
    Ok(owner)
}

#[cfg(test)]
#[path = "oven.test.rs"]
mod test;
