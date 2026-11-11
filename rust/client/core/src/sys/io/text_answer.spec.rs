// pxl-core/sys/io/text_answer.spec.rs
//
// GENERATED from spec/client/core/sys/io/text_answer.json
//
// What the platform's text writes into the core's memory as it answers the core's
// calls TEXT_LAYOUT and TEXT_RASTER (spec/client/core/sys/io/text.json, pxl-core sys/io/text.rs,
// ts/client/src/service/core/text.ts), before they return.

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
    pub const cnt: usize = 0;
    pub const WORDS: usize = 1;
}

/// A line of TEXT_LAYOUT's answer: its end, a byte offset, and its width as f32 bits.
pub mod LINE {
    pub const end: usize = 0;
    pub const width: usize = 1;
    pub const WORDS: usize = 2;
}
