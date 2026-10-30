// xpute-guest/ipc/frame.rs

//! A frame's four words, laid out by spec/ipc/frame.json.

#[path = "frame.spec.rs"]
mod spec;

pub use spec::*;

const CMD_MASK: u32 = (1 << FLAG_SHIFT) - 1;

/// A frame's four words as read, its command word split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub tag: u32,
    pub cmd: u32,
    pub flags: u32,
    pub result: i32,
    /// 0 for none.
    pub pkt_at: u32,
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

/// The far port a frame's tag names.
pub fn port_of(tag: u32) -> u32 {
    tag >> PORT_SHIFT
}

/// The tag of a frame for `port`, its id or subject `id`.
pub fn tagged(port: u32, id: u32) -> u32 {
    (port << PORT_SHIFT) | id
}
