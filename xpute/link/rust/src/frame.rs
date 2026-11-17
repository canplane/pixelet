// xpute-link/frame.rs

//! A frame's words are laid out by `link/spec/frame.json5`: the request's tag,
//! its command, the result, and the packet's length. The command is the `RES`
//! bit, the handle the callee issued and the method's place among its
//! object's. The packet, the method's args or its answer's value in the
//! Canonical ABI's layout (`cabi.rs`), lies right past the words. The words
//! are little-endian, as both forms' memory is.

use xpute_kit::status::errno::Errno;

use crate::mem::{self, set_word, word};

#[path = "frame.spec.rs"]
pub(crate) mod spec;

use spec::{CMD, METHOD_BITS, NAMED, PKT, RES, RESULT, TAG, WORDS};

/// The size of a frame's words, its header before its packet.
pub const HDR_SZ: u32 = WORDS * 4;

// `NAMED` is a handle's top bit.
const _: () = assert!(NAMED == RES >> METHOD_BITS >> 1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub tag: u32,
    /// An answer, to the receiver's request `tag`.
    pub res: bool,
    pub handle: u32,
    pub method: u32,
    pub result: i32,
    pub pkt_len: u32,
}

impl Frame {
    /// The frame its words `w` hold.
    pub fn of(w: [u32; WORDS as usize]) -> Frame {
        let cmd = w[CMD as usize];
        Frame {
            tag: w[TAG as usize],
            res: cmd & RES != 0,
            handle: (cmd & !RES) >> METHOD_BITS,
            method: cmd & ((1 << METHOD_BITS) - 1),
            result: w[RESULT as usize] as i32,
            pkt_len: w[PKT as usize],
        }
    }

    /// The frame's words.
    pub fn words(&self) -> [u32; WORDS as usize] {
        let res = if self.res { RES } else { 0 };
        let mut w = [0; WORDS as usize];
        w[TAG as usize] = self.tag;
        w[CMD as usize] = res | self.handle << METHOD_BITS | self.method;
        w[RESULT as usize] = self.result as u32;
        w[PKT as usize] = self.pkt_len;
        w
    }

    /// The frame laid at `at`.
    pub fn at(at: u64) -> Frame {
        // SAFETY: a frame's words in a link's box, which only the side that
        // holds the box touches (link.rs).
        Frame::of(core::array::from_fn(|k| unsafe { word(at + 4 * k as u64) }))
    }

    /// Lays the frame's words at `at`.
    pub fn lay(&self, at: u64) {
        for (k, w) in self.words().into_iter().enumerate() {
            // SAFETY: as in `at`.
            unsafe { set_word(at + 4 * k as u64, w) };
        }
    }

    /// The packet of the frame of `cap` bytes laid at `at`; a packet past
    /// the cap is EBADMSG.
    pub fn pkt(&self, at: u64, cap: u32) -> Result<Option<&'static [u8]>, Errno> {
        if self.pkt_len == 0 {
            return Ok(None);
        }
        if u64::from(HDR_SZ) + u64::from(self.pkt_len) > u64::from(cap) {
            return Err(Errno::EBADMSG);
        }
        // SAFETY: the frame's bytes in the guest's memory.
        Ok(Some(unsafe {
            mem::bytes(at + u64::from(HDR_SZ), self.pkt_len)
        }))
    }
}

#[cfg(test)]
#[path = "frame.test.rs"]
mod test;
