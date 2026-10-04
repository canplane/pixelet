// pxl-geo-bake/demand.rs

mod cache;
mod geocode;
mod poi;
mod pyramid;
mod terrain;

use cache::Cache;
use pxl_spec::bake::format::{Terrain, OCEAN_KEY};
use pxl_spec::bake::poi_page::PoiPage;
use pxl_spec::space::pyramid::{Folds, Page};
use std::path::PathBuf;

use crate::coverage::Coverage;
use std::sync::{Arc, Mutex};

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
/// What a kind's pages come from: a pyramid layer's coverage, the pages a
/// fold reads, and the one object every absent page is, if it has one; or
/// geocode's groups, which no coverage gates.
enum Source {
    Pyramid {
        coverage: crate::coverage::Coverage,
        sources: fn(Page) -> Vec<Page>,
        shared: Option<(&'static str, Vec<u8>)>,
    },
    Geocode,
}
impl Source {
    fn shared(&self) -> Option<(&'static str, &[u8])> {
        match self {
            Source::Pyramid { shared: Some((name, bytes)), .. } => Some((name, bytes)),
            _ => None,
        }
    }

    fn holds(&self, p: Page) -> bool {
        match self {
            Source::Pyramid { coverage, .. } => coverage.meets(p.z, p.page),
            Source::Geocode => true,
        }
    }

    fn sources(&self, p: Page) -> Vec<Page> {
        match self {
            Source::Pyramid { sources, .. } => sources(p),
            Source::Geocode => unreachable!("geocode is no pyramid"),
        }
    }
}
impl Dataset {
    /// The shared object is published before the generation answers anything:
    /// written by the folds, concurrent atomic writes raced on one staging path.
    fn open(base: &std::path::Path, kind: Kind, runner: &Runner) -> Result<Self, String> {
        let data = Self {
            cache: Cache::new(base.join(kind.name()), runner.admission.clone(), std::time::Duration::from_secs(runner.limits.seconds))?,
            source: match kind {
                Kind::Terrain => Source::Pyramid {
                    coverage: Coverage::of_source()?,
                    sources: Terrain::sources,
                    shared: Some((OCEAN_KEY, terrain::ocean()?)),
                },
                Kind::Poi => Source::Pyramid {
                    coverage: Coverage::of_source()?,
                    sources: PoiPage::sources,
                    shared: None,
                },
                Kind::Geocode => Source::Geocode,
            },
        };
        if let Some((name, bytes)) = data.source.shared() {
            data.cache.publish(name, Some(bytes))?;
        }
        Ok(data)
    }
}

pub struct Server {
    _owner: std::fs::File,
    runner: Arc<Runner>,
    base: PathBuf,
    current: Mutex<[Arc<Dataset>; 3]>,
    top_z: u32,
}
impl Server {
    pub fn open(workers: usize, base: PathBuf, top_z: u32, limits: Limits) -> Result<Self, String> {
        let owner = claim(&base)?;
        let runner = Arc::new(Runner::new(workers, limits)?);
        let current = [Kind::Terrain, Kind::Poi, Kind::Geocode]
            .into_iter()
            .map(|k| {
                prune(&base.join(k.name()))?;
                Dataset::open(&base, k, &runner).map(Arc::new)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            _owner: owner,
            runner,
            base,
            current: Mutex::new(current.try_into().ok().unwrap()),
            top_z,
        })
    }

    pub async fn get(&self, kind: Kind, name: &str) -> Result<Option<Vec<u8>>, Failure> {
        let data = self.current.lock().expect("datasets")[kind as usize].clone();
        let runner = self.runner.clone();
        let path = match kind {
            Kind::Terrain | Kind::Poi => pyramid::resolve(data.clone(), runner, kind, name, self.top_z).await?,
            Kind::Geocode => Some(geocode::resolve(data.clone(), runner, name).await?),
        };
        let Some(path) = path else { return Ok(None) };
        data.cache.read(&path).await.map_err(Failure::Bake)
    }

    pub fn invalidate(&self, kind: Kind) -> Result<(), String> {
        let mut current = self.current.lock().expect("datasets");
        current[kind as usize].cache.purge()?;
        current[kind as usize] = Arc::new(Dataset::open(&self.base, kind, &self.runner)?);
        Ok(())
    }
}

/// Clears what a killed server left that is not a page: staging and a
/// half-done purge.
fn prune(root: &std::path::Path) -> Result<(), String> {
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        if entry.file_name().to_string_lossy().starts_with(".work-") {
            std::fs::remove_dir_all(entry.path()).map_err(|e| e.to_string())?;
        }
    }
    match std::fs::remove_dir_all(root.join(terrain::SOURCE)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
        _ => {}
    }
    match std::fs::remove_dir_all(cache::purged(root)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
}

/// The lock keeps a second server from pruning under a running one.
fn claim(base: &std::path::Path) -> Result<std::fs::File, String> {
    std::fs::create_dir_all(base).map_err(|e| e.to_string())?;
    let owner = std::fs::File::options().create(true).truncate(false).write(true).open(base.join(".lock")).map_err(|e| e.to_string())?;
    owner.try_lock().map_err(|_| format!("{}: another server owns this cache", base.display()))?;
    Ok(owner)
}

#[cfg(test)]
#[path = "demand.test.rs"]
mod test;
