// xpute-guest/ipc/frame.rs

//! A message's four words, laid out by spec/ipc/frame.json.

#[path = "frame.spec.rs"]
mod spec;

pub use spec::*;

const CMD_MASK: u32 = (1 << FLAG_SHIFT) - 1;

pub fn cmd_word(cmd: u32, flags: u32) -> u32 {
    (flags << FLAG_SHIFT) | (cmd & CMD_MASK)
}

pub fn cmd_of(word: u32) -> u32 {
    word & CMD_MASK
}

pub fn flags_of(word: u32) -> u32 {
    word >> FLAG_SHIFT
}
