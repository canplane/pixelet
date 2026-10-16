// pxl-kernel/io/file.rs

//! xpute's file table with an owner per descriptor, so an open's answer
//! reaches its owner and one for a closed descriptor reaches no one. A
//! native test has no host and opens nothing.

use xpute_guest::ipc::file::Files;
use xpute_guest::ipc::sys::Fd;

use pxl_spec::space::layer::tile::LEAF_ZOOM;

use crate::abi::host::{root, OBJECTS_PATH, SETTINGS_PATH};
use crate::asset::LOADS;
use crate::io::failure::Failure;
use crate::io::host::{answered, heap_stat, sys, turn};
use crate::io::request::Credit;
use crate::mem::spec::MIN_HEAP_BYTES;
use crate::param::MAX_CREDITS;
use crate::sched::edge::request_world_tick;
use pxl_spec::bake::source::VERSION_BYTES;
use xpute_guest::global::Global;
use xpute_kit::status::errno::Errno;

/// A file the kernel keeps in storage. Storage is opened and written only by
/// one of these, so what a turn can record there follows from how many.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kept {
    Settings,
    Objects,
}

impl Kept {
    pub const ALL: [Kept; 2] = [Kept::Settings, Kept::Objects];

    pub const fn path(self) -> &'static str {
        match self {
            Kept::Settings => SETTINGS_PATH,
            Kept::Objects => OBJECTS_PATH,
        }
    }
}

/// The longest name the kernel opens: a leaf's name, a digit a zoom, under
/// its version, an asset's number or a kept file's path. A longer one is a bug, so the host
/// stream's reserve can count on it.
pub const PATH_BYTES: usize = {
    let mut most = VERSION_BYTES + 1 + LEAF_ZOOM as usize;
    let names = [LOADS.ilog10() as usize + 1];
    let mut i = 0;
    while i < names.len() + Kept::ALL.len() {
        let len = if i < names.len() { names[i] } else { Kept::ALL[i - names.len()].path().len() };
        most = if len > most { len } else { most };
        i += 1;
    }
    most
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Owner {
    Fetch { key: u64, at_ms: f64 },
    Model { key: u64 },
    Image { key: u64 },
    Kept(Kept),
    Write,
}

/// The most files open at once, and slot 0: a credit's each (`open` takes
/// one), each kept file's read, and a write, opened and closed in one call.
const OPEN: usize = MAX_CREDITS as usize + Kept::ALL.len() + 2;

static FILES: Global<Files<Owner, OPEN>> = Global::new();

fn files() -> &'static mut Files<Owner, OPEN> {
    FILES.get(Files::new)
}

/// None where there is no host.
pub fn open(_: Credit, root: u32, path: &str, owner: Owner) -> Option<Fd> {
    Some(files().open(sys()?, root, named(path), owner))
}

/// Opens a kept file to be read whole; what it held is taken with `take_kept`.
pub fn open_kept(kept: Kept) -> Option<Fd> {
    Some(files().open(sys()?, root::STORAGE, kept.path(), Owner::Kept(kept)))
}

/// What a kept file held.
pub enum KeptFile {
    /// None was kept, or it was empty.
    None,
    Bytes(Vec<u8>),
    /// There, and not readable here: larger than any heap a device is given,
    /// or an error other than its absence. It must not be written over.
    Unreadable,
}

enum KeptRead {
    /// The heap refused its room: opened again after the heap gives some
    /// back, never taken for a file that held nothing. No descriptor is kept.
    Refused {
        releases: u32,
    },
    /// Filled from the turn after `asked`.
    Reading {
        bytes: Vec<u8>,
        asked: u64,
    },
    Read(KeptFile),
}

static KEPT: Global<Vec<(Kept, KeptRead)>> = Global::new();

fn kept_read(kept: Kept, read: KeptRead) {
    let all = KEPT.get(Vec::new);
    all.retain(|(k, _)| *k != kept);
    all.push((kept, read));
    request_world_tick();
}

fn read_kept(kept: Kept, fd: Fd, bytes: usize) {
    let mut held = Vec::new();
    if held.try_reserve_exact(bytes).is_err() {
        close(fd);
        kept_read(kept, KeptRead::Refused { releases: heap_stat().releases });
        return;
    }
    held.resize(bytes, 0);
    // SAFETY: `held` moves into KEPT, its heap buffer with it, and is taken
    // no sooner than the turn after this one.
    unsafe { read(fd, &mut held) };
    close(fd);
    kept_read(kept, KeptRead::Reading { bytes: held, asked: turn() });
}

fn kept_opened(kept: Kept, fd: Fd, opened: Opened) {
    let file = match opened {
        Opened::Bytes(0) => {
            close(fd);
            KeptFile::None
        }
        Opened::Bytes(n) => return read_kept(kept, fd, n as usize),
        // Only its absence says nothing was kept.
        Opened::Absent => KeptFile::None,
        Opened::Failed { .. } => KeptFile::Unreadable,
    };
    kept_read(kept, KeptRead::Read(file));
}

/// What a kept file held, once it is read whole; None until then.
pub fn take_kept(kept: Kept) -> Option<KeptFile> {
    let all = KEPT.get(Vec::new);
    let at = all.iter().position(|(k, _)| *k == kept)?;
    match &all[at].1 {
        &KeptRead::Refused { releases } => {
            if heap_stat().releases != releases {
                all.remove(at);
                open_kept(kept);
            }
            None
        }
        KeptRead::Reading { asked, .. } if turn() <= *asked => None,
        _ => match all.remove(at).1 {
            KeptRead::Reading { bytes, .. } => Some(KeptFile::Bytes(bytes)),
            KeptRead::Read(file) => Some(file),
            KeptRead::Refused { .. } => None,
        },
    }
}

/// `dst` is filled from the next turn on.
///
/// # Safety
/// As `Sys::read`: `dst` outlives this turn, unmoved and unread.
pub unsafe fn read(fd: Fd, dst: &mut [u8]) {
    if let Some(sys) = sys() {
        // SAFETY: the caller's.
        unsafe { sys.read(fd, dst) };
    }
}

pub fn close(fd: Fd) {
    if let Some(sys) = sys() {
        files().close(sys, fd);
    }
}

/// A path as `open` takes it, written where it is wanted: at most
/// `PATH_BYTES`, so no path is allocated.
pub struct Path {
    bytes: [u8; PATH_BYTES],
    len: usize,
}

impl Path {
    /// The path `write` writes; past `PATH_BYTES` is a bug, as `named` says.
    pub fn of(write: impl FnOnce(&mut Path) -> core::fmt::Result) -> Path {
        let mut p = Path { bytes: [0; PATH_BYTES], len: 0 };
        xpute_kit::ensure!(write(&mut p).is_ok(), EMSGSIZE, PATH_BYTES);
        p
    }

    pub fn as_str(&self) -> &str {
        // SAFETY: only `write_str`'s whole strs are in it.
        unsafe { core::str::from_utf8_unchecked(&self.bytes[..self.len]) }
    }
}

impl core::fmt::Write for Path {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len + s.len();
        let to = self.bytes.get_mut(self.len..end).ok_or(core::fmt::Error)?;
        to.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

fn named(path: &str) -> &str {
    xpute_kit::ensure!(path.len() <= PATH_BYTES, EMSGSIZE, path.len());
    path
}

pub fn held_for(owner: impl Fn(&Owner) -> bool) -> Option<Fd> {
    files().held_for(owner)
}

pub fn write_kept(kept: Kept, bytes: Vec<u8>) {
    if let Some(sys) = sys() {
        files().write_whole(sys, root::STORAGE, kept.path(), bytes, Owner::Write);
    }
}

/// What an open came to, read once from the host's answer, so no owner
/// compares a raw result.
#[derive(Clone, Copy, Debug)]
pub enum Opened {
    /// The file's size; the descriptor is the owner's to read and close.
    Bytes(u32),
    /// No such file; the descriptor is closed.
    Absent,
    /// Every other failure; the descriptor is closed.
    Failed { errno: Errno, detail: u32 },
}

impl Opened {
    /// An open that came to no bytes, as its run's failure.
    #[track_caller]
    pub fn failure(self) -> Failure {
        match self {
            Opened::Failed { errno, detail } => Failure {
                errno,
                detail: detail as u16,
                at: None,
            },
            Opened::Absent => Failure::from(Errno::ENOENT),
            Opened::Bytes(_) => Failure::from(Errno::ENOTRECOVERABLE),
        }
    }
}

/// `res` is the file's size, or a negative errno. A file larger than the heap
/// every device is given cannot be held anywhere: it is the host's answer
/// refused, and the owner hears EFBIG.
pub fn opened(fd: Fd, res: i32, detail: u32) {
    let Some(&owner) = files().owner(fd) else { return };
    let opened = match answered(res) {
        Ok(n) if n > MIN_HEAP_BYTES => Opened::Failed { errno: Errno::EFBIG, detail },
        Ok(n) => Opened::Bytes(n),
        Err(Errno::ENOENT) => Opened::Absent,
        Err(errno) => Opened::Failed { errno, detail },
    };
    if !matches!(opened, Opened::Bytes(_)) {
        close(fd);
    }
    match owner {
        Owner::Fetch { key, at_ms } => crate::io::fetch::opened(fd, key, at_ms, opened),
        Owner::Model { key } => crate::io::load::model_opened(fd, key, opened),
        Owner::Image { key } => crate::io::load::image_opened(fd, key, opened),
        Owner::Kept(kept) => kept_opened(kept, fd, opened),
        Owner::Write => {}
    }
}
