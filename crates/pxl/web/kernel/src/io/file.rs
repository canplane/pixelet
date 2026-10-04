// pxl-kernel/io/file.rs

//! The kernel's files: xpute's file table (xpute-runtime ipc/file.rs) with
//! who opened each, so the host's answer to an open (SYS_OPENED) reaches its
//! owner, and an answer to a descriptor already closed reaches no one.
//!
//! The roots are host.json's: a bake by its kind, the platform's storage,
//! the shipped assets. A native test has no host and opens nothing.

use xpute_runtime::ipc::file::Files;
use xpute_runtime::ipc::sys::Fd;

use crate::global::Global;
use crate::host::sys;

/// Who a descriptor is for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Owner {
    /// A page or packet of a bake, for the scheduler's attempt `key`, opened
    /// at `at_ms`.
    Fetch { key: u64, at_ms: f64 },
    /// A model's file, for the scheduler's attempt `key`.
    Model { key: u64 },
    /// The placed objects storage kept.
    Objects,
    /// A file written whole and closed in one turn.
    Write,
}

static FILES: Global<Files<Owner>> = Global::new();

fn files() -> &'static mut Files<Owner> {
    FILES.get(Files::new)
}

/// Opens `path` below `root` to read, for `owner`; None where there is no
/// host.
pub fn open(root: u32, path: &str, owner: Owner) -> Option<Fd> {
    Some(files().open(sys()?, root, path, owner))
}

/// The file `fd`'s first `dst.len()` bytes, in `dst` from the next turn on.
pub fn read(fd: Fd, dst: &mut [u8]) {
    if let Some(sys) = sys() {
        sys.read(fd, dst);
    }
}

/// Ends `fd`, answered or not.
pub fn close(fd: Fd) {
    if let Some(sys) = sys() {
        files().close(sys, fd);
    }
}

/// The descriptor held for `owner`, if one is.
pub fn held_for(owner: impl Fn(&Owner) -> bool) -> Option<Fd> {
    files().held_for(owner)
}

/// `bytes` replace the file at `path` below `root`.
pub fn write_whole(root: u32, path: &str, bytes: Vec<u8>) {
    if let Some(sys) = sys() {
        files().write_whole(sys, root, path, bytes, Owner::Write);
    }
}

/// The host's answer to an open: the file's size, or an errno (negative).
pub fn opened(fd: Fd, res: i32, detail: u32) {
    let Some(&owner) = files().owner(fd) else { return };
    match owner {
        Owner::Fetch { key, at_ms } => crate::io::fetch::opened(fd, key, at_ms, res, detail),
        Owner::Model { key } => crate::io::load::model_opened(fd, key, res),
        Owner::Objects => crate::layer::object::store::objects_opened(fd, res),
        Owner::Write => {}
    }
}
