// pxl-kernel/io/file.rs

//! xpute's file table with an owner per descriptor, so an open's answer
//! reaches its owner and one for a closed descriptor reaches no one. A
//! native test has no host and opens nothing.

use xpute_guest::ipc::file::Files;
use xpute_guest::ipc::sys::Fd;

use pxl_spec::bake::place_tile::DICT_OBJECT;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use crate::abi::host::{root, OBJECTS_PATH, SETTINGS_PATH};
use crate::door::host::{heap_stat, sys, turn};
use crate::io::load::LOADS;
use crate::io::request::Credit;
use crate::mem::spec::MIN_HEAP_BYTES;
use crate::sched::edge::request_world_tick;
use xpute_core::status::errno::Errno;
use xpute_guest::global::Global;

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

/// The longest name the kernel opens: a leaf's name, a digit a zoom, the dictionary's
/// name, an asset's number or a kept file's path. A longer one is a bug, so
/// the host stream's reserve can count on it.
pub const PATH_BYTES: usize = {
    let mut most = LEAF_ZOOM as usize;
    let names = [DICT_OBJECT.len(), LOADS.ilog10() as usize + 1];
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

static FILES: Global<Files<Owner>> = Global::new();

fn files() -> &'static mut Files<Owner> {
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
    read(fd, &mut held);
    close(fd);
    kept_read(kept, KeptRead::Reading { bytes: held, asked: turn() });
}

fn kept_opened(kept: Kept, fd: Fd, res: i32) {
    if res > 0 {
        read_kept(kept, fd, res as usize);
        return;
    }
    close(fd);
    // Only its absence says nothing was kept.
    let file = if res == 0 || res == -(Errno::ENOENT as i32) { KeptFile::None } else { KeptFile::Unreadable };
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
pub fn read(fd: Fd, dst: &mut [u8]) {
    if let Some(sys) = sys() {
        sys.read(fd, dst);
    }
}

pub fn close(fd: Fd) {
    if let Some(sys) = sys() {
        files().close(sys, fd);
    }
}

fn named(path: &str) -> &str {
    xpute_core::ensure!(path.len() <= PATH_BYTES, EMSGSIZE, path.len());
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

/// `res` is the file's size, or a negative errno. A file larger than the heap
/// every device is given cannot be held anywhere: it is the host's answer
/// refused, and the owner hears EFBIG.
pub fn opened(fd: Fd, res: i32, detail: u32) {
    let Some(&owner) = files().owner(fd) else { return };
    let res = if res > 0 && res as u32 > MIN_HEAP_BYTES { -(Errno::EFBIG as i32) } else { res };
    match owner {
        Owner::Fetch { key, at_ms } => crate::io::fetch::opened(fd, key, at_ms, res, detail),
        Owner::Model { key } => crate::io::load::model_opened(fd, key, res),
        Owner::Image { key } => crate::io::load::image_opened(fd, key, res),
        Owner::Kept(kept) => kept_opened(kept, fd, res),
        Owner::Write => {}
    }
}
