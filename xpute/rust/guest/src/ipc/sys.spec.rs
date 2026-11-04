// xpute-guest/ipc/sys.spec.rs
//
// GENERATED from spec/ipc/sys.json
//
// A guest's hostcalls, its calls into its host: its files and the report that stopped it.
// Each is a frame for the host's port, 0, on the completion ring (abi/cmd.json, Major SYS),
// which the host takes once the turn has returned and before the next begins. A frame may
// name memory the guest holds: the host reads or writes it in that gap, and the guest
// keeps it that long.
//
// Files borrow Plan 9's file namespace, in POSIX's words. The guest issues the descriptor,
// a generational handle, and names a file by a root the program numbers and a path below
// it; the program binds each root to where it keeps things, and a path that leaves it is
// refused. SYS_OPENAT is the one call that waits: the host answers it with SYS_OPENED once
// the file can be read whole. SYS_READ and SYS_WRITE are done in the gap; SYS_CLOSE ends
// the descriptor, opened or not, and nothing is said of it after. SYS_ABORT is the report
// a guest makes as it stops, the one frame of its last turn.
//
// A call answered before it returns is a frame too, to any port: laid in the completion
// ring's next slot, not published, its position handed to the host's `hostcall` entry,
// which hands it to the port it names and returns that port's answer, a value or -errno.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// How SYS_OPENAT opens.
pub mod flag {
    /// To read, whole.
    pub const O_RDONLY: u32 = 0;
    /// To write: created where it is not, and replaced whole at SYS_CLOSE (O_CREAT | O_TRUNC).
    pub const O_WRONLY: u32 = 1;
    pub const KEYS: &[&str] = &["O_RDONLY", "O_WRONLY"];
}
