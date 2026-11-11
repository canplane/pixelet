// rust/client/core/src/sys/io/text.runtime.spec.rs
//
// GENERATED from spec/client/core/sys/io/text.json

//! The platform's text, a device of the app's ABI beside the UI's protocol
//! (ui.json): what the core calls of it, numbered as ui.json numbers its
//! own, each a call answered before it returns: on a page by the runtime
//! (ts/client/src/service/core/text.ts), on a device on the core's own thread
//! (pxl-core sys/io/text_device.rs). Where an answer lies in the
//! core's memory is laid out in spec/client/core/sys/io/text_answer.json.

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

pub use crate::sys::io::text_spec::{Command, Major, TextMinor};
use xpute_core::abi::number::{finite, integer, unknown};
use xpute_core::io::fs::{Path, Token};
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, Cursor, PacketWriter, StrList, TreeReader};
use xpute_runtime::abi::{Export, Outcome};

/// Command::TEXT_LAYOUT's packet.
#[derive(Clone, Copy, Debug)]
pub struct TextLayout {
    pub weight: u32,
    pub px: f64,
    /// 0 for one line.
    pub max_width: f64,
    pub at: u32,
    pub lines: u32,
    /// Where the text's UTF-8 lies in the core's memory: by address, so a text longer than a slot crosses whole, and kept until the call returns.
    pub text_at: u32,
    /// The length of that UTF-8 in bytes.
    pub text_len: u32,
}

impl TextLayout {
    pub fn read(pkt: &[u8]) -> Result<TextLayout, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(TextLayout {
            weight: integer::<u32>(f.at(0)?.get_number()?)?,
            px: finite(f.at(1)?.get_number()?)?,
            max_width: finite(f.at(2)?.get_number()?)?,
            at: integer::<u32>(f.at(3)?.get_number()?)?,
            lines: integer::<u32>(f.at(4)?.get_number()?)?,
            text_at: integer::<u32>(f.at(5)?.get_number()?)?,
            text_len: integer::<u32>(f.at(6)?.get_number()?)?,
        })
    }
}

/// Command::TEXT_RASTER's packet.
#[derive(Clone, Copy, Debug)]
pub struct TextRaster {
    pub weight: u32,
    pub px: f64,
    /// The color as 0xRRGGBBAA.
    pub color: u32,
    pub at: u32,
    pub width: u32,
    pub height: u32,
    pub x: f64,
    /// Where the text's UTF-8 lies in the core's memory: by address, so a text longer than a slot crosses whole, and kept until the call returns.
    pub text_at: u32,
    /// The length of that UTF-8 in bytes.
    pub text_len: u32,
}

impl TextRaster {
    pub fn read(pkt: &[u8]) -> Result<TextRaster, MarshalError> {
        let f = TreeReader::new(pkt)?.read_branch()?;
        Ok(TextRaster {
            weight: integer::<u32>(f.at(0)?.get_number()?)?,
            px: finite(f.at(1)?.get_number()?)?,
            color: integer::<u32>(f.at(2)?.get_number()?)?,
            at: integer::<u32>(f.at(3)?.get_number()?)?,
            width: integer::<u32>(f.at(4)?.get_number()?)?,
            height: integer::<u32>(f.at(5)?.get_number()?)?,
            x: finite(f.at(6)?.get_number()?)?,
            text_at: integer::<u32>(f.at(7)?.get_number()?)?,
            text_len: integer::<u32>(f.at(8)?.get_number()?)?,
        })
    }
}

/// The majors the runtime routes the protocol's imports by, to `serve`.
pub const MAJORS: [u32; 1] = [Major::TEXT as u32];

/// What the runtime does with each command the core posts.
pub trait Imports {
    fn text_layout(&mut self, e: TextLayout) -> Outcome;
    fn text_raster(&mut self, e: TextRaster) -> Outcome;
}

/// `cmd` read and handed to `h`; ENOSYS for one this protocol does not carry.
pub fn serve<H: Imports>(
    h: &mut H,
    _tag: u32,
    cmd: u32,
    pkt: &Option<&[u8]>,
    _named: &[u8],
) -> Outcome {
    match cmd {
        x if x == Command::TEXT_LAYOUT as u32 => h.text_layout(TextLayout::read(
            pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
        )?),
        x if x == Command::TEXT_RASTER as u32 => h.text_raster(TextRaster::read(
            pkt.ok_or_else(|| MarshalError::new(Errno::EBADMSG))?,
        )?),
        _ => Err(MarshalError::new(Errno::ENOSYS)),
    }
}
