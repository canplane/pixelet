// pxl-geo-bake/demand/cache.rs

use super::Failure;
use crate::util::{pack, write_atomic, Level};
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
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
        let path = self.root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        match bytes {
            Some(bytes) => write_atomic(&path, bytes),
            None => write_atomic(&path.with_extension("empty"), b""),
        }
        .map_err(|e| e.to_string())
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

impl Drop for Cache {
    fn drop(&mut self) {
        // A generation is private to this process. In-flight jobs keep it
        // alive, so obsolete writes finish before the directory disappears.
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[cfg(test)]
#[path = "cache.test.rs"]
mod test;
