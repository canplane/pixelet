// pxl-geo-bake/demand.rs

//! Local demand bakes share jobs, disk publication and generation ownership.
//! Each producer supplies only its dependency and bake rules.

mod cache;
mod geocode;
mod poi;
mod pyramid;
mod terrain;

use cache::Cache;
use std::path::PathBuf;
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
    fn name(self) -> &'static str {
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
enum Source {
    Terrain(terrain::Source),
    Poi,
    Geocode,
}
/// What a layer's source says about its pyramid that the pyramid cannot:
/// where it has nothing, and the one page terrain's open water reads as.
impl Source {
    /// The object a name addresses rather than a page, which is also what a
    /// fold reads for a page the source left as no object where that is not
    /// simply an empty page: terrain's open water.
    fn shared(&self) -> Option<(&'static str, &[u8])> {
        match self {
            Source::Terrain(s) => Some((crate::terrain::leaf::OCEAN_KEY, &s.raw_ocean)),
            _ => None,
        }
    }

    /// Whether the source can have anything under a page.
    fn holds(&self, p: pxl_spec::space::pyramid::LevelPage) -> bool {
        match self {
            Source::Terrain(s) => s.coverage.meets(p.level, p.page),
            _ => true,
        }
    }

    /// The pages of the level below a page is folded from, as its layer
    /// reads them (pxl-spec space/pyramid.rs `Folds::sources`).
    fn sources(&self, p: pxl_spec::space::pyramid::LevelPage) -> Vec<pxl_spec::space::pyramid::LevelPage> {
        use pxl_spec::space::pyramid::Folds;
        match self {
            Source::Terrain(_) => pxl_spec::bake::format::Terrain::sources(p),
            _ => pxl_spec::bake::poi_page::PoiPage::sources(p),
        }
    }
}
impl Dataset {
    /// The shared object is the same for the generation's whole life, so it
    /// is published here, before the generation can be asked for anything,
    /// and every fold only reads it. Written by the folds that read it, it
    /// would have a writer for every fold in flight, and those race on the one
    /// staging path an atomic write goes through: the loser's rename finds
    /// nothing, and its page fails for the rest of the generation.
    fn open(base: &std::path::Path, kind: Kind, runner: &Runner) -> Result<Self, String> {
        let data = Self {
            cache: Cache::with_admission(base.join(kind.name()), runner.admission.clone(), std::time::Duration::from_secs(runner.limits.seconds))?,
            source: match kind {
                Kind::Terrain => Source::Terrain(terrain::Source::new()?),
                Kind::Poi => Source::Poi,
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
    // Held for the process's life: the operating system releases it however
    // the process ends, which no cleanup of our own can promise.
    _owner: std::fs::File,
    runner: Arc<Runner>,
    base: PathBuf,
    current: Mutex<[Arc<Dataset>; 3]>,
    top_level: u32,
}
impl Server {
    pub fn open(url: &str, workers: usize, base: PathBuf, top_level: u32, limits: Limits) -> Result<Self, String> {
        let owner = claim(&base)?;
        let runner = Arc::new(Runner::new(url.to_owned(), workers, limits)?);
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
            top_level,
        })
    }

    pub async fn get(&self, kind: Kind, name: &str) -> Result<Option<Vec<u8>>, Failure> {
        let data = self.current.lock().expect("datasets")[kind as usize].clone();
        let runner = self.runner.clone();
        let path = match kind {
            Kind::Terrain | Kind::Poi => pyramid::resolve(data.clone(), runner, kind, name, self.top_level).await?,
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

/// What a stopped server left in a kind's cache that is not a page: the
/// staging its jobs were killed in (job.rs `Staging`), and a purge it was
/// killed in the middle of. The pages themselves stay — a page is cached
/// while its file is there, and a change to what a page would be is a purge
/// (`Server::invalidate`), not a restart.
fn prune(root: &std::path::Path) -> Result<(), String> {
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        if entry.file_name().to_string_lossy().starts_with(".work-") {
            std::fs::remove_dir_all(entry.path()).map_err(|e| e.to_string())?;
        }
    }
    match std::fs::remove_dir_all(cache::purged(root)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
}

/// Takes the cache directory for this process: one server writes it at a
/// time, and the lock keeps a second from pruning under a running one.
fn claim(base: &std::path::Path) -> Result<std::fs::File, String> {
    std::fs::create_dir_all(base).map_err(|e| e.to_string())?;
    let owner = std::fs::File::options().create(true).truncate(false).write(true).open(base.join(".lock")).map_err(|e| e.to_string())?;
    owner.try_lock().map_err(|_| format!("{}: another server owns this cache", base.display()))?;
    Ok(owner)
}

#[cfg(test)]
#[path = "demand.test.rs"]
mod test;
