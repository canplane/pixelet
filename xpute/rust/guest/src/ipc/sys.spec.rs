// xpute-guest/ipc/sys.spec.rs
//
// GENERATED from spec/ipc/sys.json — do not edit.
//
// A guest's system calls: its files, the report that stopped it and the program's own
// calls, as records in a stream (ipc/stream.rs) the host runs once each turn has returned
// and before the next begins. A record may name memory the guest holds: the host reads or
// writes it in that gap, and the guest keeps it that long. The directory says where the
// stream is (key SYS).
//
// Files borrow Plan 9's file namespace, in POSIX's words. The guest issues the descriptor,
// a generational handle, and names a file by a root the program numbers and a path below
// it; the program binds each root to where it keeps things, and a path that leaves it is
// refused. OPENAT is the one call that waits: the host answers it with the command
// SYS_OPENED (abi/cmd.json) once the file can be read whole. READ and WRITE are done in
// the gap; CLOSE ends the descriptor, opened or not, and nothing is said of it after.
//
// A program's own calls go in the same stream, numbered with the PROGRAM bit, and a host
// that is not the program's refuses them.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code
)]

/// Words before the first record: END, then a word kept at 0.
pub const HEAD: u32 = 2;

/// A record's call, then its arguments.
pub mod op {
    /// fd, root, flags, then the path's bytes: the file at `path` below `root` opened as `fd`,
    /// answered by SYS_OPENED with its size or -errno.
    pub const OPENAT: u32 = 1;
    /// fd, at, len: the opened file's first `len` bytes copied to `at`.
    pub const READ: u32 = 2;
    /// fd, at, len: the `len` bytes at `at` added to what the file opened for writing holds.
    pub const WRITE: u32 = 3;
    /// fd: the descriptor ended. A file opened for writing is replaced by what was written; an
    /// open not yet answered is canceled, and never answered.
    pub const CLOSE: u32 = 4;
    /// errno, line, column, a (low, high word), b (low, high word), then the file's bytes:
    /// the broken invariant the guest reports before it stops (xpute-kit status/bug.rs).
    pub const ABORT: u32 = 5;
    /// The bit every call a program numbers for itself carries.
    pub const PROGRAM: u32 = 2147483648;
    pub const KEYS: &[&str] = &["OPENAT", "READ", "WRITE", "CLOSE", "ABORT", "PROGRAM"];
}

/// How many words of arguments each call's record takes, as its `op` lists them; OPENAT's
/// and ABORT's end in bytes after them.
pub mod args {
    pub const OPENAT: u32 = 3;
    pub const READ: u32 = 3;
    pub const WRITE: u32 = 3;
    pub const CLOSE: u32 = 1;
    pub const ABORT: u32 = 7;
    pub const KEYS: &[&str] = &["OPENAT", "READ", "WRITE", "CLOSE", "ABORT"];
}

/// Where ABORT's two values lie among its arguments, each low word then high.
pub mod abort {
    /// a.
    pub const A: u32 = 3;
    /// b.
    pub const B: u32 = 5;
    pub const KEYS: &[&str] = &["A", "B"];
}

/// How OPENAT opens.
pub mod flag {
    /// To read, whole.
    pub const O_RDONLY: u32 = 0;
    /// To write: created where it is not, and replaced whole at CLOSE (O_CREAT | O_TRUNC).
    pub const O_WRONLY: u32 = 1;
    pub const KEYS: &[&str] = &["O_RDONLY", "O_WRONLY"];
}
