// xpute-guest/ipc/sys.rs

//! A guest's hostcalls, as ipc/sys.json numbers them: each a frame for the
//! host's port on the completion ring (ipc/door.rs `Door`), and what the
//! host's answers read as.

#[path = "sys.spec.rs"]
pub mod spec;

pub type Fd = u32;

/// A host's answer as one value: a count, or the errno it refused with. A code
/// no errno names is the host breaking the protocol, EPROTO.
pub fn answer_of(res: i32) -> Result<u32, xpute_kit::status::errno::Errno> {
    use xpute_kit::status::errno::Errno;
    u32::try_from(res).map_err(|_| Errno::of(res).unwrap_or(Errno::EPROTO))
}

/// The most frames one descriptor posts in a turn: its open, a read or a
/// write, and its close, twice where it is stopped.
pub const FD_TURN_FRAMES: u32 = 4;
