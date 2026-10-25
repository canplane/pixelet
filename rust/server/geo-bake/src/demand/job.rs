// pxl-geo-bake/demand/job.rs

use super::cache::Lease;
use super::{Dataset, Failure};
use postgres::Client;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use tokio::process::Command;
use tokio::sync::{oneshot, Semaphore};

#[derive(Clone, Copy)]
pub struct Limits {
    pub queued: usize,
    pub seconds: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            queued: 64,
            seconds: 30,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum Job {
    Tile {
        kind: super::Kind,
        subject: u64,
        cache: PathBuf,
    },
}

pub struct Runner {
    pub(super) admission: Arc<Semaphore>,
    workers: Arc<Workers>,
    pub(super) limits: Limits,
}

/// The worker processes, a free one handed to the oldest job a client waits
/// on before any a fold alone needs: a fold over a cold zoom bakes every
/// tile under it, and the tile on screen must not queue behind them.
struct Workers {
    state: Mutex<(usize, VecDeque<Waiter>)>,
}

struct Waiter {
    asked: Arc<AtomicBool>,
    wake: oneshot::Sender<()>,
}

struct Worker(Arc<Workers>);

impl Drop for Worker {
    fn drop(&mut self) {
        self.0.release();
    }
}

impl Workers {
    fn release(&self) {
        let mut state = self.state.lock().expect("workers");
        let (free, waiting) = &mut *state;
        loop {
            let next = waiting
                .iter()
                .position(|w| w.asked.load(Ordering::Relaxed))
                .or((!waiting.is_empty()).then_some(0));
            let Some(i) = next else {
                *free += 1;
                return;
            };
            // A waiter past its deadline has dropped its end and is skipped.
            if waiting.remove(i).expect("a waiter").wake.send(()).is_ok() {
                return;
            }
        }
    }

    async fn acquire(self: &Arc<Self>, lease: &Lease) -> Result<Worker, Failure> {
        let mut woken = {
            let mut state = self.state.lock().expect("workers");
            let (free, waiting) = &mut *state;
            if *free > 0 && waiting.is_empty() {
                *free -= 1;
                return Ok(Worker(self.clone()));
            }
            let (wake, woken) = oneshot::channel();
            waiting.push_back(Waiter {
                asked: lease.asked.clone(),
                wake,
            });
            woken
        };
        match tokio::time::timeout_at(lease.deadline, &mut woken).await {
            Ok(Ok(())) => Ok(Worker(self.clone())),
            _ => {
                // A worker handed over as the deadline passed is given back.
                woken.close();
                if woken.try_recv().is_ok() {
                    drop(Worker(self.clone()));
                }
                Err(Failure::Limited("demand queue deadline"))
            }
        }
    }
}

struct Staging(PathBuf);
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
impl Staging {
    /// The packet, or its absence, under `name`.
    fn publish(&self, data: &Dataset, name: &str) -> Result<(), String> {
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
}

impl Runner {
    pub fn new(workers: usize, limits: Limits) -> Result<Self, String> {
        if workers == 0
            || limits.seconds == 0
            || workers
                .checked_add(limits.queued)
                .is_none_or(|n| n > Semaphore::MAX_PERMITS)
        {
            return Err("invalid demand worker, queue or time limit".into());
        }
        Ok(Self {
            admission: Arc::new(Semaphore::new(workers + limits.queued)),
            workers: Arc::new(Workers {
                state: Mutex::new((workers, VecDeque::new())),
            }),
            limits,
        })
    }

    pub(super) async fn run(
        &self,
        job: Job,
        data: Arc<Dataset>,
        lease: Lease,
        name: String,
    ) -> Result<(), Failure> {
        let _worker = self.workers.acquire(&lease).await?;
        let deadline = lease.deadline;
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let out = Staging(
            data.cache
                .root
                .join(format!(".work-{}", SERIAL.fetch_add(1, Ordering::Relaxed))),
        );
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
        out.publish(&data, &name).map_err(Failure::from)
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
pub fn connect(
    reads: &[&[&str]],
    need: &std::collections::BTreeSet<crate::coverage::Node>,
    seconds: u64,
) -> Result<(crate::coverage::Overture, Client), String> {
    let (source, mut client) = crate::coverage::Overture::reading(reads, need)?;
    client
        .batch_execute(&format!(
            "SET statement_timeout = {}",
            seconds.saturating_mul(1000)
        ))
        .map_err(|e| e.to_string())?;
    Ok((source, client))
}

pub fn execute(job: Job, out: &Path, seconds: u64) -> Result<(), String> {
    match job {
        Job::Tile {
            kind,
            subject,
            cache,
        } => {
            let p = pxl_spec::space::layer::tile::Tile::of_subject(subject);
            match kind {
                super::Kind::Terrain => super::terrain::execute(p, &cache, out, seconds),
                super::Kind::Place => super::place::execute(p, &cache, out, seconds),
                super::Kind::Division => super::division::execute(p, &cache, out, seconds),
            }
        }
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
