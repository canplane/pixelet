// pxl-geo-bake/demand.rs

mod cache;
mod division;
mod place;
mod pyramid;
mod terrain;

use cache::Cache;
use pxl_spec::bake::tile::division::DivisionTile;
use pxl_spec::bake::tile::place::PlaceTile;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::layer::tile::Tile;

use crate::pyramid::Base;
use std::path::PathBuf;

use crate::coverage::Coverage;
use pxl_spec::space::coord::zoom::Zoom;
use std::sync::Arc;

pub mod job;
use job::{Limits, Runner};

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Kind {
    Terrain,
    Place,
    Division,
}
impl Kind {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "terrain" => Some(Self::Terrain),
            "place" => Some(Self::Place),
            "division" => Some(Self::Division),
            _ => None,
        }
    }
    /// The zoom its tiles are made at.
    fn leaf(self) -> Zoom {
        match self {
            Self::Terrain => TerrainTile::LEAF,
            Self::Place => PlaceTile::LEAF,
            Self::Division => DivisionTile::LEAF,
        }
    }

    /// Its coarsest tile served, which the client reads from too.
    fn top(self, top: Zoom) -> Zoom {
        pxl_spec::space::layer::tile::top_over(self.leaf(), top)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Terrain => "terrain",
            Self::Place => "place",
            Self::Division => "division",
        }
    }
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

/// A kind's tiles: their cache, the coverage that gates them and the tiles a fold reads.
struct Dataset {
    cache: Cache,
    coverage: Coverage,
    sources: fn(Tile) -> Vec<Tile>,
}
impl Dataset {
    fn open(base: &std::path::Path, kind: Kind, runner: &Runner) -> Result<Self, String> {
        Ok(Self {
            cache: Cache::new(base.join(kind.name()), runner.admission.clone(), std::time::Duration::from_secs(runner.limits.seconds))?,
            coverage: Coverage::of_source()?,
            sources: match kind {
                Kind::Terrain => TerrainTile::sources,
                Kind::Place => PlaceTile::sources,
                Kind::Division => DivisionTile::sources,
            },
        })
    }
}

pub struct Server {
    _owner: std::fs::File,
    runner: Arc<Runner>,
    datasets: [Arc<Dataset>; 3],
    top_z: Zoom,
}
impl Server {
    pub fn open(workers: usize, base: PathBuf, top_z: Zoom, limits: Limits) -> Result<Self, String> {
        let owner = claim(&base)?;
        let runner = Arc::new(Runner::new(workers, limits)?);
        let datasets = [Kind::Terrain, Kind::Place, Kind::Division].map(|k| {
            let source = base.join(k.name());
            prune(&source)?;
            Dataset::open(&source, k, &runner).map(Arc::new)
        });
        let [t, p, g] = datasets;
        Ok(Self {
            _owner: owner,
            runner,
            datasets: [t?, p?, g?],
            top_z,
        })
    }

    pub async fn get(&self, kind: Kind, name: &str) -> Result<Option<Vec<u8>>, Failure> {
        let data = self.datasets[kind as usize].clone();
        let runner = self.runner.clone();
        let path = pyramid::resolve(data.clone(), runner, kind, name, kind.top(self.top_z)).await?;
        let Some(path) = path else { return Ok(None) };
        data.cache.read(&path).await.map_err(Failure::Bake)
    }
}

const LOCK: &str = ".lock";

/// Clears what a killed server left that is not a tile: its jobs' staging.
fn prune(root: &std::path::Path) -> Result<(), String> {
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        if entry.file_name().to_string_lossy().starts_with(".work-") {
            std::fs::remove_dir_all(entry.path()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// The lock keeps a second server from pruning under a running one.
fn claim(base: &std::path::Path) -> Result<std::fs::File, String> {
    std::fs::create_dir_all(base).map_err(|e| e.to_string())?;
    let owner = std::fs::File::options().create(true).truncate(false).write(true).open(base.join(LOCK)).map_err(|e| e.to_string())?;
    owner.try_lock().map_err(|_| format!("{}: another server owns this cache", base.display()))?;
    Ok(owner)
}

#[cfg(test)]
#[path = "demand.test.rs"]
mod test;
