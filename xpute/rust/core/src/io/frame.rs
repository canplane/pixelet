// xpute-core/io/frame.rs

//! A frame's words are laid out by spec/io/frame.json: the request's tag, its
//! command, the result, and the packet's length. The command is the RES bit,
//! the handle the callee issued and the method's place among its object's.
//! The packet, the method's args in XTP, lies right past the words. The words
//! are little-endian, as both forms' memory is.

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

use crate::mem;

#[path = "frame.spec.rs"]
mod spec;

use spec::{CMD, METHOD_BITS, PKT, RES, RESULT, TAG, WORDS};

/// The bytes a frame's words take, before its packet.
pub const WORD_BYTES: u32 = WORDS * 4;

/// The largest handle a command holds.
pub const HANDLE_MAX: u32 = (RES >> METHOD_BITS) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub tag: u32,
    /// A later answer, to the receiver's request `tag`.
    pub res: bool,
    pub handle: u32,
    pub method: u32,
    pub result: i32,
    pub pkt_len: u32,
}

fn word(at: u32) -> u32 {
    // SAFETY: a frame's word in the core's memory.
    let b = unsafe { mem::bytes(at, 4) };
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn set_word(at: u32, w: u32) {
    // SAFETY: as `word`.
    unsafe { mem::bytes(at, 4) }.copy_from_slice(&w.to_le_bytes());
}

impl Frame {
    /// The frame laid at `at`.
    pub fn at(at: u32) -> Frame {
        let cmd = word(at + 4 * CMD);
        Frame {
            tag: word(at + 4 * TAG),
            res: cmd & RES != 0,
            handle: (cmd & !RES) >> METHOD_BITS,
            method: cmd & ((1 << METHOD_BITS) - 1),
            result: word(at + 4 * RESULT) as i32,
            pkt_len: word(at + 4 * PKT),
        }
    }

    /// Lays the frame's words at `at`.
    pub fn lay(&self, at: u32) {
        set_word(at + 4 * TAG, self.tag);
        let res = if self.res { RES } else { 0 };
        set_word(at + 4 * CMD, res | self.handle << METHOD_BITS | self.method);
        set_word(at + 4 * RESULT, self.result as u32);
        set_word(at + 4 * PKT, self.pkt_len);
    }

    /// The packet of the frame laid at `at` in a room of `room` bytes; a
    /// packet past the room is EBADMSG.
    pub fn pkt(&self, at: u32, room: u32) -> Result<Option<&'static [u8]>, MarshalError> {
        if self.pkt_len == 0 {
            return Ok(None);
        }
        if u64::from(WORD_BYTES) + u64::from(self.pkt_len) > u64::from(room) {
            return Err(MarshalError::new(Errno::EBADMSG));
        }
        // SAFETY: the frame's room in the core's memory.
        Ok(Some(unsafe { mem::bytes(at + WORD_BYTES, self.pkt_len) }))
    }
}
