// xpute-link/frame.rs

//! A frame's words are laid out by link/spec/frame.json: the request's tag, its
//! command, the result, and the packet's length. The command is the RES bit,
//! the handle the callee issued and the method's place among its object's.
//! The packet, the method's args in XTP, lies right past the words. The words
//! are little-endian, as both forms' memory is.

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

use crate::mem::{self, set_word, word};

#[path = "frame.spec.rs"]
pub(crate) mod spec;

use spec::{CMD, METHOD_BITS, NAMED, PKT, RES, RESULT, TAG, WORDS};

/// The bytes a frame's words take, before its packet.
pub const WORD_BYTES: u32 = WORDS * 4;

// NAMED is a handle's top bit.
const _: () = assert!(NAMED == RES >> METHOD_BITS >> 1);

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

impl Frame {
    /// The frame laid at `at`.
    pub fn at(at: u32) -> Frame {
        // SAFETY: a frame's words in a link's box, which only the side that
        // holds the box touches (link.rs).
        let w = |k: u32| unsafe { word(at + 4 * k) };
        let cmd = w(CMD);
        Frame {
            tag: w(TAG),
            res: cmd & RES != 0,
            handle: (cmd & !RES) >> METHOD_BITS,
            method: cmd & ((1 << METHOD_BITS) - 1),
            result: w(RESULT) as i32,
            pkt_len: w(PKT),
        }
    }

    /// Lays the frame's words at `at`.
    pub fn lay(&self, at: u32) {
        // SAFETY: as in `at`.
        let set = |k: u32, v: u32| unsafe { set_word(at + 4 * k, v) };
        let res = if self.res { RES } else { 0 };
        set(TAG, self.tag);
        set(CMD, res | self.handle << METHOD_BITS | self.method);
        set(RESULT, self.result as u32);
        set(PKT, self.pkt_len);
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
        // SAFETY: the frame's room in the guest's memory.
        Ok(Some(unsafe { mem::bytes(at + WORD_BYTES, self.pkt_len) }))
    }
}
