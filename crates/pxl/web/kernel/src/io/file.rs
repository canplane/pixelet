// pxl-kernel/io/file.rs

//! xpute's file table with an owner per descriptor, so an open's answer
//! reaches its owner and one for a closed descriptor reaches no one. A
//! native test has no host and opens nothing.

use xpute_runtime::ipc::file::Files;
use xpute_runtime::ipc::sys::Fd;

use crate::host::sys;
use xpute_runtime::global::Global;

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
pub fn open(root: u32, path: &str, owner: Owner) -> Option<Fd> {
    Some(files().open(sys()?, root, path, owner))
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

pub fn held_for(owner: impl Fn(&Owner) -> bool) -> Option<Fd> {
    files().held_for(owner)
}

pub fn write_whole(root: u32, path: &str, bytes: Vec<u8>) {
    if let Some(sys) = sys() {
        files().write_whole(sys, root, path, bytes, Owner::Write);
    }
}

/// `res` is the file's size, or a negative errno.
pub fn opened(fd: Fd, res: i32, detail: u32) {
    let Some(&owner) = files().owner(fd) else { return };
    match owner {
        Owner::Fetch { key, at_ms } => crate::io::fetch::opened(fd, key, at_ms, res, detail),
        Owner::Model { key } => crate::io::load::model_opened(fd, key, res),
        Owner::Image { key } => crate::io::load::image_opened(fd, key, res),
        Owner::Objects => crate::layer::object::store::objects_opened(fd, res),
        Owner::Write => {}
    }
}
