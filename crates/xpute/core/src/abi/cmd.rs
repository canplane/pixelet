// xpute-core/abi/cmd.rs

//! A command's number: a major that names what it is about, and a minor that
//! names what is done to it — `major << 8 | minor` in the 16 bits a message
//! carries (ipc/frame's word 1). Numbers, never strings: a table is an index,
//! and a string was the part certain to change. What the majors and minors
//! are is the program's, not this kit's.

pub const fn cmd(major: u32, minor: u32) -> u32 {
    ((major & 0xff) << 8) | (minor & 0xff)
}

pub const fn cmd_major(c: u32) -> u32 {
    (c >> 8) & 0xff
}

pub const fn cmd_minor(c: u32) -> u32 {
    c & 0xff
}
