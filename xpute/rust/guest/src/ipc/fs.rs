// xpute-guest/ipc/fs.rs

//! The file device's guest end, as ipc/fs.json numbers it: a descriptor, and
//! what one posts in a turn. Its calls are `Door`'s (ipc/door.rs), its opens'
//! owners `Files`' (ipc/file.rs).

#[path = "fs.spec.rs"]
pub mod spec;

pub type Fd = u32;

/// An FS_OPENAT completed (abi/fs.json): its descriptor, the frame's tag; the
/// file's size, or -errno; and whatever number the host refused with, for a
/// reader to print.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FsOpened {
    pub fd: Fd,
    pub res: i32,
    pub detail: u32,
}

/// An FS_CLOSE completed: its descriptor, and 0, or -errno where a file
/// opened for writing could not be replaced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FsClosed {
    pub fd: Fd,
    pub res: i32,
}

/// The most frames one descriptor posts in a turn: its open, a read or a
/// write, and its close, twice where it is stopped.
pub const FD_TURN_FRAMES: u32 = 4;
