// xpute-guest/ipc/fs.spec.rs
//
// GENERATED from spec/ipc/fs.json
//
// The file device's numbers (abi/fs.json, its protocol, Major FS): a guest's files,
// posted as frames on the completion ring for the device's port, which the host takes
// once the turn has returned and before the next begins. A frame may name memory the
// guest holds: the host reads or writes it in that gap, and the guest keeps it that long.
//
// Files borrow Plan 9's file namespace, in POSIX's words. The guest issues the descriptor,
// a generational handle, and names a file by a root the program numbers and a path below
// it; the program binds each root to where it keeps things, and a path that leaves it is
// refused. FS_OPENAT is the one call that waits: the host answers it with FS_OPENED once
// the file can be read whole. FS_READ and FS_WRITE are done in the gap; FS_CLOSE ends
// the descriptor, opened or not, and nothing is said of it after.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// How FS_OPENAT opens.
pub mod flag {
    /// To read, whole.
    pub const O_RDONLY: u32 = 0;
    /// To write: created where it is not, and replaced whole at FS_CLOSE (O_CREAT | O_TRUNC).
    pub const O_WRONLY: u32 = 1;
    pub const KEYS: &[&str] = &["O_RDONLY", "O_WRONLY"];
}
