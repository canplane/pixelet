// xpute-native-host/disk.rs

//! The device's disk, as the browser's storage is a page's: the bytes at a path
//! under the root the platform gives, read whole, bytes committed to a path
//! whole (a file beside it, synced, then renamed over it), and a file set aside
//! (renamed beside itself). What a path stands
//! for is the program's, whose file binding serves wit/fs/file.wit over this.
//! The disk's work runs on a thread of its own, which hands each result back to
//! the guest's thread (xpute-native-runtime `job`).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use xpute_link::abi::types::Token;
use xpute_kit::status::errno::Errno;

use xpute_native_runtime::{done, job};

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
    SetAside {
        id: u64,
        path: PathBuf,
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
                Job::SetAside { id, path } => done(id, rename_aside(&root.join(path)).map_err(errno)),
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

/// Sets the file at `path` aside, `then` hearing it done on the guest's thread.
pub fn set_aside(path: PathBuf, then: impl FnOnce(Result<(), Errno>) + 'static) {
    send(Job::SetAside {
        id: job(then),
        path,
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
    let beside = beside(path, ".writing");
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

/// Renames the file at `path` beside itself, to a name that says when it was
/// set aside (`<name>.aside.<ms>`, or `<name>.aside.<ms>-<n>` past the names
/// taken), as a page's storage keeps `<key>:aside:<ms>`. The disk's thread
/// alone names files here, so a name found free stays free until the rename.
fn rename_aside(path: &Path) -> io::Result<()> {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let mut aside = beside(path, &format!(".aside.{ms}"));
    let mut n = 0;
    while aside.try_exists()? {
        n += 1;
        aside = beside(path, &format!(".aside.{ms}-{n}"));
    }
    fs::rename(path, aside)
}

/// The path beside `path`, its name with `suffix` after it.
fn beside(path: &Path, suffix: &str) -> PathBuf {
    let mut beside = path.as_os_str().to_owned();
    beside.push(suffix);
    PathBuf::from(beside)
}

fn errno(e: io::Error) -> Errno {
    match e.kind() {
        io::ErrorKind::NotFound => Errno::ENOENT,
        io::ErrorKind::PermissionDenied => Errno::EACCES,
        _ => Errno::EIO,
    }
}
