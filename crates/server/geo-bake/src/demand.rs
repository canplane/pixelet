// pxl-geo-bake/demand.rs

mod cache;
mod geocode;
mod poi;
mod pyramid;
mod terrain;

use cache::Cache;
use pxl_spec::bake::format::Terrain;
use pxl_spec::bake::poi_tile::PoiTile;
use pxl_spec::space::layer::tile::Tile;

use crate::pyramid::Layer;
use std::path::PathBuf;

use crate::coverage::{holdings, Coverage};
use pxl_spec::space::coord::zoom::Zoom;
use std::sync::Arc;

pub mod job;
use job::{Limits, Runner};

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Kind {
    Terrain,
    Poi,
    Geocode,
}
impl Kind {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "terrain" => Some(Self::Terrain),
            "poi" => Some(Self::Poi),
            "geocode" => Some(Self::Geocode),
            _ => None,
        }
    }
    /// What its bake reads of the geo DB.
    fn reads(self) -> &'static [&'static [&'static str]] {
        match self {
            Self::Terrain => &crate::terrain::leaf::READS,
            Self::Poi => &[&crate::poi::READS],
            Self::Geocode => &[&crate::geocode::provider::overture::READS],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Terrain => "terrain",
            Self::Poi => "poi",
            Self::Geocode => "geocode",
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

struct Dataset {
    cache: Cache,
    source: Source,
}
/// What a kind's tiles come from: a pyramid layer's coverage and the tiles a
/// fold reads, or geocode's groups, which no coverage gates.
enum Source {
    Pyramid { coverage: crate::coverage::Coverage, sources: fn(Tile) -> Vec<Tile> },
    Geocode,
}
impl Source {
    fn holds(&self, p: Tile) -> bool {
        match self {
            Source::Pyramid { coverage, .. } => coverage.meets(p.zoom, p.tile),
            Source::Geocode => true,
        }
    }

    fn sources(&self, p: Tile) -> Vec<Tile> {
        match self {
            Source::Pyramid { sources, .. } => sources(p),
            Source::Geocode => unreachable!("geocode is no pyramid"),
        }
    }
}
impl Dataset {
    fn open(base: &std::path::Path, kind: Kind, runner: &Runner) -> Result<Self, String> {
        let data = Self {
            cache: Cache::new(base.join(kind.name()), runner.admission.clone(), std::time::Duration::from_secs(runner.limits.seconds))?,
            source: match kind {
                Kind::Terrain => Source::Pyramid {
                    coverage: Coverage::of_source()?,
                    sources: Terrain::sources,
                },
                Kind::Poi => Source::Pyramid {
                    coverage: Coverage::of_source()?,
                    sources: PoiTile::sources,
                },
                Kind::Geocode => Source::Geocode,
            },
        };
        Ok(data)
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
        let code = crate::util::code_id()?;
        let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing")?;
        let mut client = postgres::Client::connect(&url, postgres::NoTls).map_err(|e| e.to_string())?;
        let datasets = [Kind::Terrain, Kind::Poi, Kind::Geocode].map(|k| {
            let root = base.join(k.name());
            let source = root.join(crate::util::source_id(&code, &holdings(&mut client, k.reads())?));
            // Another source's cache is removed, never read.
            for entry in std::fs::read_dir(&root).into_iter().flatten().flatten() {
                if entry.path() != source {
                    std::fs::remove_dir_all(entry.path()).or_else(|_| std::fs::remove_file(entry.path())).map_err(|e| e.to_string())?;
                }
            }
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
        let path = match kind {
            Kind::Terrain | Kind::Poi => pyramid::resolve(data.clone(), runner, kind, name, self.top_z).await?,
            Kind::Geocode => Some(geocode::resolve(data.clone(), runner, name).await?),
        };
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
