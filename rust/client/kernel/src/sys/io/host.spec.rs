// pxl-kernel/sys/io/host.spec.rs
//
// GENERATED from spec/client/kernel/sys/io/host.json
//
// What the platform's text writes into the kernel's memory as it answers the kernel's
// calls TEXT_LAYOUT and TEXT_RASTER (domain/control/cmd.json, pxl-kernel sys/io/text.rs,
// ts/client/src/port/text.ts), before they return.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// TEXT_LAYOUT's answer at its `at`: how many lines, then `LINE` apiece.
pub mod LAYOUT_ANSWER {
    pub const cnt: u32 = 0;
    pub const WORDS: u32 = 1;
}

/// A line of TEXT_LAYOUT's answer: its end, a byte offset, and its width as f32 bits.
pub mod LINE {
    pub const end: u32 = 0;
    pub const width: u32 = 1;
    pub const WORDS: u32 = 2;
}
