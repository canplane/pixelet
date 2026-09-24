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
use std::time::{SystemTime, UNIX_EPOCH};

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
    revision: u64,
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
    /// A shared object a name addresses rather than a page.
    fn shared(&self, name: &str) -> Option<&[u8]> {
        match self {
            Source::Terrain(s) if name == crate::terrain::leaf::OCEAN_KEY => Some(&s.raw_ocean),
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

    /// The object a fold reads for a page the source left as no object,
    /// where that is not simply an empty page.
    fn absent(&self) -> Option<(&'static str, &[u8])> {
        match self {
            Source::Terrain(s) => Some((crate::terrain::leaf::OCEAN_KEY, &s.raw_ocean)),
            _ => None,
        }
    }
}
impl Dataset {
    fn open(base: &std::path::Path, kind: Kind, revision: u64, runner: &Runner) -> Result<Self, String> {
        Ok(Self {
            cache: Cache::with_admission(
                base.join(kind.name()).join(revision.to_string()),
                runner.admission.clone(),
                std::time::Duration::from_secs(runner.limits.seconds),
            )?,
            revision,
            source: match kind {
                Kind::Terrain => Source::Terrain(terrain::Source::new()?),
                Kind::Poi => Source::Poi,
                Kind::Geocode => Source::Geocode,
            },
        })
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
        let revision = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_micros() as u64;
        let current = [Kind::Terrain, Kind::Poi, Kind::Geocode]
            .into_iter()
            .map(|k| Dataset::open(&base, k, revision, &runner).map(Arc::new))
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
        let revision = current[kind as usize].revision.checked_add(1).ok_or("revision exhausted")?;
        current[kind as usize] = Arc::new(Dataset::open(&self.base, kind, revision, &self.runner)?);
        Ok(())
    }
}

/// Takes the cache directory for this process. A generation is private to
/// the process that opened it, and is removed when its `Cache` drops — which
/// a signal never lets happen, so every generation a stopped server leaves
/// is an orphan. What this process did not create is cleared before its
/// first generation opens; the lock keeps that from reaching a server that
/// is still running on the same directory.
fn claim(base: &std::path::Path) -> Result<std::fs::File, String> {
    std::fs::create_dir_all(base).map_err(|e| e.to_string())?;
    let owner = std::fs::File::options().create(true).truncate(false).write(true).open(base.join(".lock")).map_err(|e| e.to_string())?;
    owner.try_lock().map_err(|_| format!("{}: another server owns this cache", base.display()))?;
    for kind in [Kind::Terrain, Kind::Poi, Kind::Geocode] {
        match std::fs::remove_dir_all(base.join(kind.name())) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => {}
        }
    }
    Ok(owner)
}

#[cfg(test)]
#[path = "demand.test.rs"]
mod test;
