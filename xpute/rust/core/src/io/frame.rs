// xpute-core/io/frame.rs

//! A frame's four words are laid out by spec/io/frame.json, and its packet,
//! when it has one, lies right past them. The door keeps two frames, the one
//! the core receives into and the one it sends from (door.rs).

#[path = "frame.spec.rs"]
mod spec;

pub use spec::*;

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

use crate::mem::Memory;

const CMD_MASK: u32 = (1 << FLAG_SHIFT) - 1;

/// The bytes a frame's words take, before its packet.
pub const WORD_BYTES: u32 = WORDS * 4;

/// A frame's four words as read, with its command word split into the command
/// and its flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub tag: u32,
    pub cmd: u32,
    pub flags: u32,
    pub result: i32,
    /// The length in bytes of the frame's packet, 0 for none.
    pub pkt_len: u32,
}

/// A command's number is `major << 8 | minor`, which fits the 16 bits a frame
/// carries for it; its major says what answers it (spec/io/frame.json).
pub const fn cmd(major: u32, minor: u32) -> u32 {
    ((major & 0xff) << 8) | (minor & 0xff)
}

pub const fn cmd_major(cmd: u32) -> u32 {
    (cmd >> 8) & 0xff
}

pub const fn cmd_minor(cmd: u32) -> u32 {
    cmd & 0xff
}

pub fn cmd_word(cmd: u32, flags: u32) -> u32 {
    (flags << FLAG_SHIFT) | (cmd & CMD_MASK)
}

pub fn cmd_of(word: u32) -> u32 {
    word & CMD_MASK
}

pub fn flags_of(word: u32) -> u32 {
    word >> FLAG_SHIFT
}

impl Frame {
    /// The frame's words at `at`.
    pub fn at(mem: Memory, at: u32) -> Frame {
        let word = |k: u32| mem.word((at + 4 * k) as usize);
        let cmd = word(CMD);
        Frame {
            tag: word(TAG),
            cmd: cmd_of(cmd),
            flags: flags_of(cmd),
            result: word(RESULT) as i32,
            pkt_len: word(PKT),
        }
    }

    /// Writes the frame's words at `at`.
    pub fn lay(&self, mem: Memory, at: u32) {
        let set = |k: u32, w: u32| mem.set_word((at + 4 * k) as usize, w);
        set(TAG, self.tag);
        set(CMD, cmd_word(self.cmd, self.flags));
        set(RESULT, self.result as u32);
        set(PKT, self.pkt_len);
    }

    /// The packet of the frame laid at `at`, which with the frame's words
    /// takes no more than `room` bytes. A packet longer than the room is
    /// EBADMSG.
    pub fn pkt(
        &self,
        mem: Memory,
        at: u32,
        room: u32,
    ) -> Result<Option<&'static [u8]>, MarshalError> {
        if self.pkt_len == 0 {
            return Ok(None);
        }
        if u64::from(WORD_BYTES) + u64::from(self.pkt_len) > u64::from(room) {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        // SAFETY: the frame's room in the memory, which the caller names.
        Ok(Some(unsafe {
            core::slice::from_raw_parts(
                mem.at::<u8>((at + WORD_BYTES) as usize),
                self.pkt_len as usize,
            )
        }))
    }
}
