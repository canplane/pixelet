// pxl-geo-bake/oven/cache.rs

use super::Failure;
use crate::util::{pack, write_atomic, Level};
use fnv::FnvHashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::fs;
use tokio::sync::{Notify, Semaphore};
use tokio::time::Instant;

#[derive(Default)]
struct Flight {
    result: OnceLock<Result<(), Failure>>,
    completion: Notify,
    client_waits: Arc<AtomicBool>,
}

/// What a job runs under: when it must be done by, and whether a client is
/// waiting on it, which turns true when one joins a job a fold started.
pub(super) struct Lease {
    pub deadline: Instant,
    pub client_waits: Arc<AtomicBool>,
}

pub(super) struct Cache {
    pub root: PathBuf,
    flights: Arc<Mutex<FnvHashMap<String, Arc<Flight>>>>,
    admission: Arc<Semaphore>,
    deadline: std::time::Duration,
}

impl Cache {
    pub(super) fn new(
        root: PathBuf,
        admission: Arc<Semaphore>,
        deadline: std::time::Duration,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        Ok(Self {
            root,
            flights: Arc::new(Mutex::new(FnvHashMap::default())),
            admission,
            deadline,
        })
    }

    /// The job, not the HTTP connection, owns its lifetime. Failures stay failed
    /// while the server runs; resource refusals leave nothing. Do not time out this
    /// future: it would release the key before the work stopped. `client_waits` is
    /// whether a client waits on it, rather than only a fold over it.
    pub(super) async fn once<F>(
        &self,
        key: String,
        client_waits: bool,
        make: impl FnOnce(Lease) -> F + Send + 'static,
    ) -> Result<(), Failure>
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
                    let permit = self
                        .admission
                        .clone()
                        .try_acquire_owned()
                        .map_err(|_| Failure::Limited("oven queue full"))?;
                    (e.insert(Arc::default()).clone(), Some(permit))
                }
            }
        };
        if client_waits {
            flight.client_waits.store(true, Ordering::Relaxed);
        }
        if let Some(permit) = permit {
            let task = flight.clone();
            let client_waits = flight.client_waits.clone();
            let jobs = self.flights.clone();
            let deadline = Instant::now() + self.deadline;
            tokio::spawn(async move {
                let job = async move {
                    make(Lease {
                        deadline,
                        client_waits,
                    })
                    .await
                };
                let result = tokio::spawn(job)
                    .await
                    .map_err(|e| Failure::Bake(e.to_string()))
                    .and_then(|r| r);
                if matches!(result, Ok(()) | Err(Failure::Limited(_))) {
                    jobs.lock().expect("flights").remove(&key);
                }
                let _ = task.result.set(result);
                drop(permit);
                task.completion.notify_waiters();
            });
        }
        loop {
            let ready = flight.completion.notified();
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

    pub(super) fn publish(&self, name: &str, raw: Option<&[u8]>) -> Result<(), String> {
        let packed = raw
            .map(|r| pack(r, Level::Live).map_err(|e| e.to_string()))
            .transpose()?;
        let path = self.root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        match packed.as_deref() {
            Some(bytes) => write_atomic(&path, bytes),
            None => write_atomic(&path.with_extension("empty"), b""),
        }
        .map_err(|e| e.to_string())
    }

    pub(super) async fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let path = self.root.join(name);
        match fs::read(&path).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e)
                if e.kind() == std::io::ErrorKind::NotFound
                    && path.with_extension("empty").is_file() =>
            {
                Ok(None)
            }
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }
}

#[cfg(test)]
#[path = "cache.test.rs"]
mod test;
