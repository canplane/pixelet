// pxl-geo-bake/demand/cache.rs

use super::Failure;
use crate::util::{pack, write_atomic, Level};
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use tokio::sync::{Notify, Semaphore};

#[derive(Default)]
struct Flight {
    result: OnceLock<Result<(), Failure>>,
    done: Notify,
    asked: Arc<AtomicBool>,
}

/// What a job runs under: when it must be done by, and whether a client is
/// waiting on it, which turns true when one joins a job a fold started.
pub struct Lease {
    pub deadline: tokio::time::Instant,
    pub asked: Arc<AtomicBool>,
}

pub struct Cache {
    pub root: PathBuf,
    flights: Arc<Mutex<HashMap<String, Arc<Flight>>>>,
    admission: Arc<Semaphore>,
    deadline: std::time::Duration,
    /// False once purged. Writes hold it shared and the purge alone, so no write
    /// lands between a purge and the next generation.
    live: RwLock<bool>,
}

impl Cache {
    pub fn new(root: PathBuf, admission: Arc<Semaphore>, deadline: std::time::Duration) -> Result<Self, String> {
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        Ok(Self {
            root,
            flights: Arc::new(Mutex::new(HashMap::new())),
            admission,
            deadline,
            live: RwLock::new(true),
        })
    }

    /// The job, not the HTTP connection, owns its lifetime. Failures stay failed
    /// within the generation; resource refusals leave nothing. Do not time out this
    /// future: it would release the key before the work stopped. `asked` is
    /// whether a client waits on it, rather than only a fold over it.
    pub async fn once<F>(&self, key: String, asked: bool, make: impl FnOnce(Lease) -> F + Send + 'static) -> Result<(), Failure>
    where
        F: Future<Output = Result<(), Failure>> + Send + 'static,
    {
        let (flight, permit) = {
            let mut jobs = self.flights.lock().expect("flights");
            match jobs.entry(key.clone()) {
                std::collections::hash_map::Entry::Occupied(e) => (e.get().clone(), None),
                std::collections::hash_map::Entry::Vacant(e) => {
                    // A finished job leaves the map only after publishing, so a vacancy cannot miss one.
                    if self.published(&key) {
                        return Ok(());
                    }
                    let permit = self.admission.clone().try_acquire_owned().map_err(|_| Failure::Limited("demand queue full"))?;
                    (e.insert(Arc::default()).clone(), Some(permit))
                }
            }
        };
        if asked {
            flight.asked.store(true, Ordering::Relaxed);
        }
        if let Some(permit) = permit {
            let task = flight.clone();
            let asked = flight.asked.clone();
            let jobs = self.flights.clone();
            let deadline = tokio::time::Instant::now() + self.deadline;
            tokio::spawn(async move {
                let job = async move { make(Lease { deadline, asked }).await };
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
        self.commit(|| {
            let path = self.root.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
            match packed.as_deref() {
                Some(bytes) => write_atomic(&path, bytes),
                None => write_atomic(&path.with_extension("empty"), b""),
            }
            .map_err(|e| e.to_string())
        })
    }

    pub fn commit(&self, write: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
        let live = self.live.read().expect("live");
        if *live {
            write()
        } else {
            Ok(())
        }
    }

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

pub fn purged(root: &std::path::Path) -> PathBuf {
    let mut name = root.file_name().unwrap_or_default().to_owned();
    name.push(".purged");
    root.with_file_name(name)
}

#[cfg(test)]
#[path = "cache.test.rs"]
mod test;
