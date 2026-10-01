// pxl-geo-bake/demand/cache.rs

use super::Failure;
use crate::util::{pack, write_atomic, Level};
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use tokio::sync::{Notify, Semaphore};

#[derive(Default)]
struct Flight {
    result: OnceLock<Result<(), Failure>>,
    done: Notify,
}

pub struct Cache {
    pub root: PathBuf,
    flights: Arc<Mutex<HashMap<String, Arc<Flight>>>>,
    admission: Arc<Semaphore>,
    deadline: std::time::Duration,
    /// Whether this generation still owns `root`: false once purged, after
    /// which nothing it was still baking is written there. Every write holds
    /// it shared and the purge holds it alone, so no write lands between a
    /// purge and the next generation's first read.
    live: RwLock<bool>,
}

impl Cache {
    #[cfg(test)]
    pub fn new(root: PathBuf) -> Result<Self, String> {
        Self::with_admission(root, Arc::new(Semaphore::new(32)), std::time::Duration::from_secs(30))
    }

    pub fn with_admission(root: PathBuf, admission: Arc<Semaphore>, deadline: std::time::Duration) -> Result<Self, String> {
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        Ok(Self {
            root,
            flights: Arc::new(Mutex::new(HashMap::new())),
            admission,
            deadline,
            live: RwLock::new(true),
        })
    }

    /// The job owns its lifetime, not the HTTP connection that first asks.
    /// A key is the path its job publishes, so what is published answers
    /// from the disk, and a flight is held only while the work runs or after
    /// the source failed: failures stay failed within the generation, and
    /// resource refusals leave nothing and may be admitted by a later request.
    /// The owner passes one deadline through dependencies and execution. It
    /// awaits process reaping and publication before settling the flight;
    /// timing out this future would release identity before its work stopped.
    pub async fn once<F>(&self, key: String, make: impl FnOnce(tokio::time::Instant) -> F + Send + 'static) -> Result<(), Failure>
    where
        F: Future<Output = Result<(), Failure>> + Send + 'static,
    {
        let (flight, permit) = {
            let mut jobs = self.flights.lock().expect("flights");
            match jobs.entry(key.clone()) {
                std::collections::hash_map::Entry::Occupied(e) => (e.get().clone(), None),
                std::collections::hash_map::Entry::Vacant(e) => {
                    // Under the lock: a finished job leaves the map only after
                    // it has published, so a vacancy here cannot miss one.
                    if self.published(&key) {
                        return Ok(());
                    }
                    let permit = self.admission.clone().try_acquire_owned().map_err(|_| Failure::Limited("demand queue full"))?;
                    (e.insert(Arc::default()).clone(), Some(permit))
                }
            }
        };
        if let Some(permit) = permit {
            let task = flight.clone();
            let jobs = self.flights.clone();
            let deadline = tokio::time::Instant::now() + self.deadline;
            tokio::spawn(async move {
                let job = async move { make(deadline).await };
                let result = tokio::spawn(job).await.map_err(|e| Failure::Bake(e.to_string())).and_then(|r| r);
                if matches!(result, Ok(()) | Err(Failure::Limited(_))) {
                    jobs.lock().expect("flights").remove(&key);
                }
                let _ = task.result.set(result);
                drop(permit);
                task.done.notify_waiters();
            });
        }
        loop {
            let ready = flight.done.notified();
            tokio::pin!(ready);
            ready.as_mut().enable();
            if let Some(result) = flight.result.get() {
                return result.clone();
            }
            ready.await;
        }
    }

    fn published(&self, name: &str) -> bool {
        let path = self.root.join(name);
        path.exists() || path.with_extension("empty").is_file()
    }

    pub fn publish(&self, name: &str, raw: Option<&[u8]>) -> Result<(), String> {
        let packed = raw.map(|r| pack(r, Level::Live).map_err(|e| e.to_string())).transpose()?;
        self.publish_stored(name, packed.as_deref())
    }

    pub fn publish_stored(&self, name: &str, bytes: Option<&[u8]>) -> Result<(), String> {
        self.commit(|| {
            let path = self.root.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
            match bytes {
                Some(bytes) => write_atomic(&path, bytes),
                None => write_atomic(&path.with_extension("empty"), b""),
            }
            .map_err(|e| e.to_string())
        })
    }

    /// A write into `root`, made only while this generation still owns it: a
    /// purged one's late result is dropped, not published over the next's.
    pub fn commit(&self, write: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
        let live = self.live.read().expect("live");
        if *live {
            write()
        } else {
            Ok(())
        }
    }

    /// Everything this generation published, gone: `root` is moved aside and
    /// removed, and the generation can write nothing more into it.
    pub fn purge(&self) -> Result<(), String> {
        let mut live = self.live.write().expect("live");
        *live = false;
        let aside = purged(&self.root);
        match std::fs::rename(&self.root, &aside) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => {}
        }
        drop(live);
        match std::fs::remove_dir_all(&aside) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }

    pub async fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let path = self.root.join(name);
        match tokio::fs::read(&path).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && path.with_extension("empty").is_file() => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }
}

/// Where a purge moves a cache's directory before removing it, beside it so
/// the move is a rename; a start clears what a killed purge left there.
pub fn purged(root: &std::path::Path) -> PathBuf {
    let mut name = root.file_name().unwrap_or_default().to_owned();
    name.push(".purged");
    root.with_file_name(name)
}

#[cfg(test)]
#[path = "cache.test.rs"]
mod test;
