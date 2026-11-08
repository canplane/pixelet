// rust/client/core/src/sys/io/text.spec.rs
//
// GENERATED from spec/client/core/sys/io/text.json

//! The platform's text, a device of the app's ABI beside the UI's protocol
//! (ui.json): what the core calls of it, numbered as ui.json numbers its
//! own, each a call the runtime answers beside the core before it returns
//! (ts/client/src/runtime/start.ts, port/text.ts). Where an answer lies in the
//! core's memory is laid out in spec/client/core/sys/io/text_answer.json.

// A list a record carries is a function handing out its items, whose
// type is long to spell and is spelled once, here, for each. A table
// with no import or no enum read off the wire leaves a writer or a
// number's check unused.
#![allow(
    non_camel_case_types,
    clippy::upper_case_acronyms,
    clippy::type_complexity,
    clippy::absolute_paths,
    unused_imports,
    dead_code,
    reason = "generated from the spec"
)]

use core::fmt::Display;

use xpute_core::abi::number::{finite, integer, unknown};
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{PacketWriter, TreeReader};

/// The module a command is for: the text device's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Major {
    TEXT = 0x1c,
}

impl Major {
    pub const ALL: [Major; 1] = [Major::TEXT];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Major> {
        Major::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// What the core asks the platform's text, answered before the call returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum TextMinor {
    LAYOUT = 0x01,
    RASTER = 0x02,
}

impl TextMinor {
    pub const ALL: [TextMinor; 2] = [TextMinor::LAYOUT, TextMinor::RASTER];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<TextMinor> {
        TextMinor::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// Every command, by the side that sends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Command {
    // ---- text, import ----
    /// The text in the platform's own family at the weight and size, broken into lines no
    /// wider than the width where it can be; at `at` in the core's memory, how many lines,
    /// then a line's end (a byte offset) and its width (f32) apiece, `lines` of them at most
    /// (sys/io/text_answer.json LAYOUT_ANSWER, LINE). A call: answered before it returns.
    TEXT_LAYOUT = 0x1c01,
    /// One line drawn in the platform's own family, starting at x with the middle of its em
    /// box on the middle row, into `width` by `height` pixels at `at` in the core's memory,
    /// RGBA a byte a channel, top row first, alpha not premultiplied. A call: answered before
    /// it returns.
    TEXT_RASTER = 0x1c02,
}

impl Command {
    pub const ALL: [Command; 2] = [Command::TEXT_LAYOUT, Command::TEXT_RASTER];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Command> {
        Command::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// Command::TEXT_LAYOUT's packet.
pub struct TextLayout {
    pub weight: u32,
    pub px: f64,
    /// 0 for one line.
    pub max_width: f64,
    pub at: u32,
    pub lines: u32,
    /// Where the text's UTF-8 lies in the core's memory: by address, so a text longer than a slot crosses whole, and kept until the call returns.
    pub text_at: u32,
    /// Its bytes.
    pub text_len: u32,
}

impl TextLayout {
    /// The children it is written as.
    pub const FIELDS: u32 = 7;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.weight));
        w.f64(Some(self.px));
        w.f64(Some(self.max_width));
        w.u32(Some(self.at));
        w.u32(Some(self.lines));
        w.u32(Some(self.text_at));
        w.u32(Some(self.text_len));
    }
}

impl xpute_core::ipc::door::Import for TextLayout {
    const NUMBER: u32 = 7169;
    const FIELDS: u32 = 7;
    fn write(&self, w: &mut PacketWriter) {
        TextLayout::write(self, w);
    }
}

/// Command::TEXT_RASTER's packet.
pub struct TextRaster {
    pub weight: u32,
    pub px: f64,
    /// 0xRRGGBBAA.
    pub color: u32,
    pub at: u32,
    pub width: u32,
    pub height: u32,
    pub x: f64,
    /// Where the text's UTF-8 lies in the core's memory: by address, so a text longer than a slot crosses whole, and kept until the call returns.
    pub text_at: u32,
    /// Its bytes.
    pub text_len: u32,
}

impl TextRaster {
    /// The children it is written as.
    pub const FIELDS: u32 = 9;

    pub fn write(&self, w: &mut PacketWriter) {
        w.u32(Some(self.weight));
        w.f64(Some(self.px));
        w.u32(Some(self.color));
        w.u32(Some(self.at));
        w.u32(Some(self.width));
        w.u32(Some(self.height));
        w.f64(Some(self.x));
        w.u32(Some(self.text_at));
        w.u32(Some(self.text_len));
    }
}

impl xpute_core::ipc::door::Import for TextRaster {
    const NUMBER: u32 = 7170;
    const FIELDS: u32 = 9;
    fn write(&self, w: &mut PacketWriter) {
        TextRaster::write(self, w);
    }
}

/// The text protocol's core end: what it posts to the runtime and what it hears from it.
pub mod text {
    use super::*;
    use xpute_core::ipc::door::Outcome;
    use xpute_core::machine;
    use xpute_kit::status::errno::Errno;

    /// Command::TEXT_LAYOUT, answered by the runtime before this returns.
    pub fn text_layout(e: &TextLayout) -> Result<u32, Errno> {
        machine::door().call(e)
    }

    /// Command::TEXT_RASTER, answered by the runtime before this returns.
    pub fn text_raster(e: &TextRaster) -> Result<u32, Errno> {
        machine::door().call(e)
    }
}
