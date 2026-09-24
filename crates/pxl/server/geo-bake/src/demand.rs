// pxl-geo-bake/demand.rs

//! Local demand bakes share jobs, disk publication and generation ownership.
//! Each producer supplies only its dependency and bake rules.

mod cache;
mod geocode;
mod poi;
mod terrain;

use crate::geocode::provider::overture::GeocodeProvider;
use crate::terrain::repository::TerrainRepository;
use crate::terrain::source::SourceStats;
use cache::Cache;
use postgres::{Client, NoTls};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy)]
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

pub enum Failure {
    BadName,
    Unavailable,
    Bake(String),
}
impl From<String> for Failure {
    fn from(e: String) -> Self {
        Self::Bake(e)
    }
}

struct Worker {
    client: Client,
    terrain: TerrainRepository,
    geocode: GeocodeProvider,
    stats: SourceStats,
}
struct Pool {
    workers: Mutex<Vec<Worker>>,
    available: Condvar,
}
impl Pool {
    fn with<T>(&self, f: impl FnOnce(&mut Worker) -> T) -> T {
        let mut workers = self.workers.lock().expect("workers");
        while workers.is_empty() {
            workers = self.available.wait(workers).expect("workers");
        }
        let mut worker = workers.pop().unwrap();
        drop(workers);
        let value = f(&mut worker);
        self.workers.lock().expect("workers").push(worker);
        self.available.notify_one();
        value
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
    Geocode(geocode::Source),
}
impl Dataset {
    fn open(base: &std::path::Path, kind: Kind, revision: u64) -> Result<Self, String> {
        Ok(Self {
            cache: Cache::new(base.join(kind.name()).join(revision.to_string()))?,
            revision,
            source: match kind {
                Kind::Terrain => Source::Terrain(terrain::Source::new()?),
                Kind::Poi => Source::Poi,
                Kind::Geocode => Source::Geocode(geocode::Source::default()),
            },
        })
    }
}

pub struct Server {
    pool: Arc<Pool>,
    base: PathBuf,
    current: Mutex<[Arc<Dataset>; 3]>,
    top_level: u32,
}
impl Server {
    pub fn open(url: &str, workers: usize, base: PathBuf, top_level: u32) -> Result<Self, String> {
        let mut pool = Vec::new();
        for _ in 0..workers.max(1) {
            let mut client = Client::connect(url, NoTls).map_err(|e| e.to_string())?;
            pool.push(Worker {
                terrain: TerrainRepository::prepare(&mut client).map_err(|e| e.to_string())?,
                geocode: GeocodeProvider::prepare(&mut client).map_err(|e| e.to_string())?,
                client,
                stats: SourceStats::default(),
            });
        }
        let revision = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_micros() as u64;
        let current = [Kind::Terrain, Kind::Poi, Kind::Geocode]
            .into_iter()
            .map(|k| Dataset::open(&base, k, revision).map(Arc::new))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            pool: Arc::new(Pool {
                workers: Mutex::new(pool),
                available: Condvar::new(),
            }),
            base,
            current: Mutex::new(current.try_into().ok().unwrap()),
            top_level,
        })
    }

    pub async fn get(&self, kind: Kind, name: &str) -> Result<Option<Vec<u8>>, Failure> {
        let data = self.current.lock().expect("datasets")[kind as usize].clone();
        let pool = self.pool.clone();
        let path = match kind {
            Kind::Terrain => terrain::resolve(data.clone(), pool, name, self.top_level).await?,
            Kind::Poi => poi::resolve(data.clone(), pool, name).await?,
            Kind::Geocode => geocode::resolve(data.clone(), pool, name).await?,
        };
        data.cache.read(&path).await.map_err(Failure::Bake)
    }

    pub fn invalidate(&self, kind: Kind) -> Result<(), String> {
        let mut current = self.current.lock().expect("datasets");
        let revision = current[kind as usize].revision.checked_add(1).ok_or("revision exhausted")?;
        current[kind as usize] = Arc::new(Dataset::open(&self.base, kind, revision)?);
        Ok(())
    }
}
