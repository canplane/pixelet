// xpute-core/ipc/fs.rs

//! This module is the core's end of the file device, as ipc/fs.json numbers
//! it: a descriptor, and what one descriptor posts in a turn. The file calls
//! are `Door`'s (ipc/door.rs), and `FileTable` keeps the owner of each open
//! (ipc/file.rs).

#[path = "fs.spec.rs"]
pub mod spec;

pub type Fd = u32;

/// A completed FS_OPENAT (abi/fs.json): its descriptor, which is the frame's
/// tag; the file's size, or a negative errno; and any number the runtime
/// refused with, kept only for a reader to print.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FsOpened {
    pub fd: Fd,
    pub res: i32,
    pub detail: u32,
}

/// A completed FS_CLOSE: its descriptor, and 0, or a negative errno when a
/// file opened for writing could not be replaced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FsClosed {
    pub fd: Fd,
    pub res: i32,
}

/// The most frames one descriptor posts in a turn: its open, a read or a
/// write, and its close, which a stopped descriptor may post twice.
pub const FD_TURN_FRAMES: u32 = 4;
