// pxl-kernel/io/file.rs

//! xpute's file table with an owner per descriptor, so an open's answer
//! reaches its owner and one for a closed descriptor reaches no one. A
//! native test has no host and opens nothing.

use xpute_runtime::ipc::file::Files;
use xpute_runtime::ipc::sys::Fd;

use pxl_spec::bake::place_page::DICT_OBJECT;
use pxl_spec::bake::quadkey::PAGE_KEY_DEPTH;

use crate::abi::host::{root, OBJECTS_PATH, SETTINGS_PATH};
use crate::host::sys;
use crate::io::load::LOADS;
use crate::io::sched::Credit;
use crate::mem::spec::MIN_HEAP_BYTES;
use xpute_core::status::errno::Errno;
use xpute_runtime::global::Global;

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

/// The longest name the kernel opens: a page's quadkey, the dictionary's
/// name, an asset's number or a kept file's path. A longer one is a bug, so
/// the host stream's reserve can count on it.
pub const PATH_BYTES: usize = {
    let mut most = PAGE_KEY_DEPTH as usize;
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
    Objects,
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

pub fn open_kept(kept: Kept, owner: Owner) -> Option<Fd> {
    Some(files().open(sys()?, root::STORAGE, kept.path(), owner))
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
        Owner::Objects => crate::layer::object::store::objects_opened(fd, res),
        Owner::Write => {}
    }
}
