// pxl-geo-bake/demand/job.rs

use super::{Dataset, Failure};
use postgres::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tokio::process::Command;
use tokio::sync::Semaphore;

#[derive(Clone, Copy)]
pub struct Limits {
    pub queued: usize,
    pub seconds: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self { queued: 64, seconds: 30 }
    }
}

#[derive(Serialize, Deserialize)]
pub enum Job {
    Page { kind: super::Kind, subject: u64, cache: PathBuf },
    Dictionary,
    Geocode { group: u64, dictionary: PathBuf },
}

pub struct Runner {
    pub(super) admission: Arc<Semaphore>,
    running: Arc<Semaphore>,
    pub(super) limits: Limits,
}

pub(super) enum Publication {
    Packet(String),
    Directory { source: String, target: String },
}

struct Staging(PathBuf);
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
impl Staging {
    fn publish(&self, data: &Dataset, publication: Publication) -> Result<(), String> {
        data.cache.commit(|| match publication {
            Publication::Packet(name) => {
                let mut target = data.cache.root.join(name);
                let source = if self.0.join("empty").is_file() {
                    target.set_extension("empty");
                    self.0.join("empty")
                } else {
                    self.0.join("packet")
                };
                std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
                std::fs::rename(source, target).map_err(|e| e.to_string())
            }
            Publication::Directory { source, target } => {
                let target = data.cache.root.join(target);
                std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
                std::fs::rename(self.0.join(source), target).map_err(|e| e.to_string())
            }
        })
    }
}

impl Runner {
    pub fn new(workers: usize, limits: Limits) -> Result<Self, String> {
        if workers == 0 || limits.seconds == 0 || workers.checked_add(limits.queued).is_none_or(|n| n > Semaphore::MAX_PERMITS) {
            return Err("invalid demand worker, queue or time limit".into());
        }
        Ok(Self {
            admission: Arc::new(Semaphore::new(workers + limits.queued)),
            running: Arc::new(Semaphore::new(workers)),
            limits,
        })
    }

    pub(super) async fn run(&self, job: Job, data: Arc<Dataset>, deadline: tokio::time::Instant, publication: Publication) -> Result<(), Failure> {
        let _permit = tokio::time::timeout_at(deadline, self.running.clone().acquire_owned())
            .await
            .map_err(|_| Failure::Limited("demand queue deadline"))?
            .expect("runner open");
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let out = Staging(data.cache.root.join(format!(".work-{}", SERIAL.fetch_add(1, Ordering::Relaxed))));
        std::fs::create_dir_all(&out.0).map_err(|e| e.to_string())?;
        let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
        command
            .arg("--demand-job")
            .arg(serde_json::to_string(&job).map_err(|e| e.to_string())?)
            .arg(&out.0)
            .arg(self.limits.seconds.to_string())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .kill_on_drop(true);
        supervise(command, deadline).await?;
        if tokio::time::Instant::now() >= deadline {
            return Err(Failure::Limited("demand generation deadline"));
        }
        out.publish(&data, publication).map_err(Failure::from)
    }
}

async fn supervise(mut command: Command, deadline: tokio::time::Instant) -> Result<(), Failure> {
    if tokio::time::Instant::now() >= deadline {
        return Err(Failure::Limited("demand generation deadline"));
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    match tokio::time::timeout_at(deadline, child.wait()).await {
        Ok(status) => {
            if status.map_err(|e| e.to_string())?.success() {
                Ok(())
            } else {
                Err(Failure::Bake("demand bake process failed".into()))
            }
        }
        Err(_) => {
            // Reap the process before releasing its slot or removing staging.
            child.kill().await.map_err(|e| e.to_string())?;
            child.wait().await.map_err(|e| e.to_string())?;
            Err(Failure::Limited("demand generation deadline"))
        }
    }
}

/// Each job opens the DB for what it reads, so a re-ingest while the server
/// runs is checked by the next job.
pub fn connect(reads: &[&[&str]], seconds: u64) -> Result<Client, String> {
    let (_, mut client) = crate::coverage::Overture::reading(reads)?;
    client.batch_execute(&format!("SET statement_timeout = {}", seconds.saturating_mul(1000))).map_err(|e| e.to_string())?;
    Ok(client)
}

pub fn execute(job: Job, out: &Path, seconds: u64) -> Result<(), String> {
    match job {
        Job::Page { kind, subject, cache } => {
            let p = pxl_spec::space::pyramid::LevelPage::of_subject(subject);
            match kind {
                super::Kind::Terrain => super::terrain::execute(p, &cache, out, seconds),
                super::Kind::Poi => super::poi::execute(p, &cache, out, seconds),
                super::Kind::Geocode => Err("geocode is not held as a pyramid".into()),
            }
        }
        Job::Dictionary => super::geocode::dictionary(out, seconds),
        Job::Geocode { group, dictionary } => super::geocode::execute(group, &dictionary, out, seconds),
    }
}

pub(super) fn write_packet(out: &Path, packet: Option<&[u8]>) -> Result<(), String> {
    let (path, bytes) = match packet {
        Some(bytes) => (out.join("packet"), bytes),
        None => (out.join("empty"), &b""[..]),
    };
    crate::util::write_atomic(&path, bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "job.test.rs"]
mod test;
