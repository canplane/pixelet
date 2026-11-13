// xpute-native-host/disk.rs

//! The device's disk, as the browser's storage is a page's: the bytes at a path
//! under the root the platform gives, read whole, and bytes committed to a path
//! whole (a file beside it, synced, then renamed over it). What a path stands
//! for is the program's, whose file binding serves wit/fs/file.wit over this.
//! The disk's work runs on a thread of its own, which hands each result back to
//! the guest's thread (`event_loop::job`).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::OnceLock;

use xpute_link::abi::types::Token;
use xpute_kit::status::errno::Errno;

use crate::event_loop::{done, job};

/// The disk's work: its job's id and its path under the root.
enum Job {
    Read {
        id: u64,
        path: PathBuf,
    },
    Commit {
        id: u64,
        path: PathBuf,
        bytes: Vec<u8>,
    },
}

/// The I/O thread's queue of work, in the order it was asked for.
static WORK: OnceLock<Sender<Job>> = OnceLock::new();

fn send(job: Job) {
    // A send fails only once the I/O thread is gone, with the guest.
    if let Some(work) = WORK.get() {
        let _ = work.send(job);
    }
}

/// Spawns the I/O thread over `root`.
pub fn serve(root: PathBuf) {
    let (work, jobs) = mpsc::channel::<Job>();
    let _ = WORK.set(work);
    std::thread::spawn(move || {
        for job in jobs {
            match job {
                Job::Read { id, path } => done(id, fs::read(root.join(path)).map_err(errno)),
                Job::Commit { id, path, bytes } => {
                    done(
                        id,
                        replace(&root.join(path), &bytes).map_err(|_| Errno::EIO),
                    );
                }
            }
        }
    });
}

/// Reads the bytes at `path`, `then` hearing them on the guest's thread.
pub fn read(path: PathBuf, then: impl FnOnce(Result<Vec<u8>, Errno>) + 'static) {
    send(Job::Read {
        id: job(then),
        path,
    });
}

/// Commits `bytes` to `path`, `then` hearing it done on the guest's thread.
pub fn commit(path: PathBuf, bytes: Vec<u8>, then: impl FnOnce(Result<(), Errno>) + 'static) {
    send(Job::Commit {
        id: job(then),
        path,
        bytes,
    });
}

/// The path a URI's tokens name under the root, each a component. A token
/// that is not a plain name (empty, `.`, `..`, or holding a separator) would
/// leave the root, and is refused.
pub fn path(uri: &[Token]) -> Result<PathBuf, Errno> {
    uri.iter()
        .map(|t| match t {
            Token::Str(s)
                if s.is_empty() || s == "." || s == ".." || s.contains(['/', '\\', '\0']) =>
            {
                Err(Errno::EINVAL)
            }
            Token::Str(s) => Ok(s.clone()),
            Token::Int(n) => Ok(n.to_string()),
        })
        .collect()
}

/// Replaces the file at `path` with `bytes`: a file beside it, synced, then
/// renamed over it, its directories made first. A failed commit leaves no
/// file beside it.
fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut beside = path.as_os_str().to_owned();
    beside.push(".writing");
    let beside = PathBuf::from(beside);
    let written = fs::File::create(&beside).and_then(|mut f| {
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&beside, path)
    });
    if written.is_err() {
        let _ = fs::remove_file(&beside);
    }
    written
}

fn errno(e: io::Error) -> Errno {
    match e.kind() {
        io::ErrorKind::NotFound => Errno::ENOENT,
        io::ErrorKind::PermissionDenied => Errno::EACCES,
        _ => Errno::EIO,
    }
}
