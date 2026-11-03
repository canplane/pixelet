// pxl-kernel/sys/io/host.spec.rs
//
// GENERATED from spec/client/kernel/sys/io/host.json
//
// The kernel's own hostcalls, numbered beside xpute's (xpute/spec/ipc/sys.json,
// pxl-kernel sys/io/host.rs, ts/client/src/driver/text.ts) and made as hostcalls, answered
// before they return; and the range of the stream xpute's calls are recorded in, which
// a hostcall's record lies past while it runs.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// The stream's range, laid out by the kernel's map.
pub mod stream {
    /// The range's size. A record names what it moves rather than carrying it, so
    /// a turn's records are words, and a hostcall's text bytes the most of what it holds.
    pub const BYTES: u32 = 65536;
}

/// The kernel's calls, each with xpute's PROGRAM bit and made as a hostcall; a record's call, then its arguments.
pub mod op {
    /// weight, px (f32 bits), max width (f32 bits, 0 for one line), at, lines, then the
    /// text's bytes: the text in the platform's own family at the weight and size, broken
    /// into lines no wider than the width where it can be; at `at`, how many lines, then
    /// a line's end (a byte offset) and its width (f32) apiece, `lines` of them at most
    pub const TEXT_LAYOUT: u32 = 2147483649;
    /// weight, px (f32 bits), color (0xRRGGBBAA), at, width, height, x (f32 bits), then
    /// the text's bytes: one line drawn in the platform's own family, starting at x with
    /// the middle of its em box on the middle row, into `width` by `height` pixels at
    /// `at`, RGBA a byte a channel, top row first, alpha not premultiplied
    pub const TEXT_RASTER: u32 = 2147483650;
}

/// TEXT_LAYOUT's arguments, a word each after its call, then the text's bytes at
/// `WORDS`: the weight, px and max width as f32 bits, where its answer goes and
/// how many lines it may hold.
pub mod LAYOUT {
    pub const weight: u32 = 0;
    pub const px: u32 = 1;
    pub const max_width: u32 = 2;
    pub const at: u32 = 3;
    pub const lines: u32 = 4;
    pub const WORDS: u32 = 5;
}

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

/// TEXT_RASTER's arguments, a word each after its call, then the text's bytes at
/// `WORDS`: the weight and px as f32 bits, the color, where its pixels go, their
/// width and height, and x as f32 bits.
pub mod RASTER {
    pub const weight: u32 = 0;
    pub const px: u32 = 1;
    pub const color: u32 = 2;
    pub const at: u32 = 3;
    pub const width: u32 = 4;
    pub const height: u32 = 5;
    pub const x: u32 = 6;
    pub const WORDS: u32 = 7;
}
