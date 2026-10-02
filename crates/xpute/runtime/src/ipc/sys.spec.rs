// xpute-runtime/ipc/sys.spec.rs
//
// GENERATED from spec/xpute/ipc/sys.json — do not edit.
//
// A guest's system calls: what it asks of its host that no command answers, as records
// in a stream (ipc/stream.rs) the host runs once each turn has returned and before the
// next begins. A record may name memory the guest holds: the host reads or writes it
// in that gap, and the guest keeps it that long. The directory says where the stream
// is (key SYS).
//
// Files are reached as Plan 9 reaches them, in POSIX's words. The guest issues the
// descriptor, a generational handle, and names a file by a root the program numbers and
// a path below it; the host binds each root to where it keeps things and refuses a path
// that leaves it. OPENAT is the one call that waits: the host answers it with the
// command SYS_OPENED (abi/cmd.json) once the file can be read whole. READ and WRITE are
// done in the gap; CLOSE ends the descriptor, opened or not, and nothing is said of it
// after.
//
// A program's own calls go in the same stream, numbered with the PROGRAM bit, and a host
// that is not the program's refuses them.

// Casing here is the spec's and not Rust's: a name is spelled as the JSON
// spells it, so that one record is indexed by one name on both sides. To
// case them the way Rust would is to rename the protocol.
//
// `dead_code` because a table is generated whole: an entry earns its
// place in the JSON, not by having a caller in this crate.
#![allow(non_upper_case_globals, non_snake_case, dead_code)]

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
    /// open not yet answered is cancelled, and never answered.
    pub const CLOSE: u32 = 4;
    /// errno, line, column, a (low, high word), b (low, high word), then the file's bytes:
    /// the broken invariant the guest reports before it stops (xpute-core status/bug.rs).
    pub const ABORT: u32 = 5;
    /// The bit every call a program numbers for itself carries.
    pub const PROGRAM: u32 = 2147483648;
    pub const KEYS: &[&str] = &["OPENAT", "READ", "WRITE", "CLOSE", "ABORT", "PROGRAM"];
}

/// How OPENAT opens.
pub mod flag {
    /// To read, whole.
    pub const O_RDONLY: u32 = 0;
    /// To write: created where it is not, and replaced whole at CLOSE (O_CREAT | O_TRUNC).
    pub const O_WRONLY: u32 = 1;
    pub const KEYS: &[&str] = &["O_RDONLY", "O_WRONLY"];
}
