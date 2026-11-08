// xpute-core/ipc/fs.spec.rs
//
// GENERATED from spec/ipc/fs.json
//
// The numbers of the file device, whose protocol is abi/fs.json on major FS. The core posts
// its file calls as frames on the completion ring, and the runtime takes them after the
// turn returns and before the next one begins. A frame may name memory the core holds. The
// runtime reads or writes that memory in the gap between the turns, or at FS_CLOSE for an
// FS_WRITE, and the core keeps the memory unchanged until then.
//
// Files follow Plan 9's file namespace, described in POSIX's words. The core issues each
// descriptor, a generational handle. It names a file by a root, which the program numbers,
// and a path below that root. The program binds each root to where it keeps things, and a
// path that leads out of its root is refused. FS_OPENAT and FS_CLOSE are requests in
// io_uring's manner: each is completed once, under its tag, which is the descriptor. An
// open is completed once the file can be read whole, and a close once a written file has
// been replaced. FS_READ and FS_WRITE are done in the gap between turns, and nothing
// completes them.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// The flags FS_OPENAT takes, which say how it opens the file.
pub mod flag {
    /// Opens the file to be read whole.
    pub const O_RDONLY: u32 = 0;
    /// Opens the file to be written. The file is created if it does not exist, and replaced
    /// whole at FS_CLOSE, as O_CREAT | O_TRUNC would do.
    pub const O_WRONLY: u32 = 1;
    pub const KEYS: &[&str] = &["O_RDONLY", "O_WRONLY"];
}
