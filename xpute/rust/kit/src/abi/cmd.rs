// xpute-kit/abi/cmd.rs

//! A command number is `major << 8 | minor`, which fits the 16 bits a frame
//! carries for it.

pub const fn cmd(major: u32, minor: u32) -> u32 {
    ((major & 0xff) << 8) | (minor & 0xff)
}

pub const fn cmd_major(c: u32) -> u32 {
    (c >> 8) & 0xff
}

pub const fn cmd_minor(c: u32) -> u32 {
    c & 0xff
}
