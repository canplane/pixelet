// xpute-guest/ipc/fs.rs

//! The file device's guest end, as ipc/fs.json numbers it: a descriptor, and
//! what one posts in a turn. Its calls are `Door`'s (ipc/door.rs), its opens'
//! owners `Files`' (ipc/file.rs).

#[path = "fs.spec.rs"]
pub mod spec;

pub type Fd = u32;

/// The most frames one descriptor posts in a turn: its open, a read or a
/// write, and its close, twice where it is stopped.
pub const FD_TURN_FRAMES: u32 = 4;
