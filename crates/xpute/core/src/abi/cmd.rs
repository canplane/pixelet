// xpute-core/abi/cmd.rs

//! A command is `major << 8 | minor`, in the 16 bits a message carries.

pub const fn cmd(major: u32, minor: u32) -> u32 {
    ((major & 0xff) << 8) | (minor & 0xff)
}

pub const fn cmd_major(c: u32) -> u32 {
    (c >> 8) & 0xff
}

pub const fn cmd_minor(c: u32) -> u32 {
    c & 0xff
}
