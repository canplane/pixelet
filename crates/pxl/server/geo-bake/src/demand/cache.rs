// pxl-geo-bake/demand/cache.rs

use crate::util::{pack, write_atomic, Level};
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::Notify;

#[derive(Default)]
struct Flight {
    result: OnceLock<Result<(), String>>,
    done: Notify,
}

pub struct Cache {
    pub root: PathBuf,
    flights: Mutex<HashMap<String, Arc<Flight>>>,
}

impl Cache {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        Ok(Self {
            root,
            flights: Mutex::new(HashMap::new()),
        })
    }

    /// The job owns its lifetime, not the HTTP connection that first asks.
    /// Failures stay failures until the dataset's generation is replaced.
    pub async fn once(&self, key: String, make: impl Future<Output = Result<(), String>> + Send + 'static) -> Result<(), String> {
        let (flight, start) = {
            let mut jobs = self.flights.lock().expect("flights");
            match jobs.entry(key) {
                std::collections::hash_map::Entry::Occupied(e) => (e.get().clone(), false),
                std::collections::hash_map::Entry::Vacant(e) => (e.insert(Arc::default()).clone(), true),
            }
        };
        if start {
            let task = flight.clone();
            tokio::spawn(async move {
                let result = tokio::spawn(make).await.map_err(|e| e.to_string()).and_then(|r| r);
                let _ = task.result.set(result);
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
