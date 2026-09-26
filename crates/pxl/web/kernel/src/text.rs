// pxl-kernel/text.rs

//! Text, as the platform lays it out and draws it (pxl_render::gpu::text).
//! The kernel asks through the host stream (abi/host.json): a text's lines
//! and their widths (TEXT_LAYOUT), and a line's pixels (TEXT_RASTER). What is
//! asked on one turn is answered by the next, as every host record is.
//!
//! An answer is kept while it is read: each world frame keeps what the frame
//! before it read and lets the rest go, so what is held is what is in use,
//! not what was ever asked. A line's pixels are read once — the billboard
//! sheet they are drawn into is what keeps them — and go with the frame
//! after; a sheet drawn again asks again.
//!
//! A native test has no host: a text is one line of a fixed width a
//! character, and no line is ever drawn.

use std::collections::HashMap;

use pxl_render::gpu::surface::DPR_MAX;
use pxl_render::gpu::text::{raster_box, Font, Raster};
use xpute_runtime::mem::base::off_of;

use crate::abi::host::op;
use crate::global::Global;
use crate::host::{host_record, turn};

/// A text as it was asked for: in a font, within a width (bits; 0 for one
/// line), and the text.
type LayoutKey = (u32, u32, u32, String);
/// A line as it was asked for: in a font and a color, and the text.
type RasterKey = (u32, u32, u32, String);

struct Answer<T> {
    /// The turn it was asked on: the host has written it on any turn after.
    asked: u64,
    /// Read since the last frame's rise.
    read: bool,
    /// Where the host writes it, kept until the answer goes.
    at: T,
}

struct RasterAt {
    pixels: Box<[u8]>,
    size: [u32; 2],
    origin: [f32; 2],
    advance: f32,
}

#[derive(Default)]
struct Text {
    layouts: HashMap<LayoutKey, Answer<Box<[u32]>>>,
    rasters: HashMap<RasterKey, Answer<RasterAt>>,
}

static TEXT: Global<Text> = Global::new();

fn text() -> &'static mut Text {
    TEXT.get(Text::default)
}

/// A world frame's rise: what the last frame did not read goes.
pub fn text_rise() {
    let t = text();
    t.layouts.retain(|_, a| std::mem::take(&mut a.read) || a.asked == turn());
    t.rasters.retain(|_, a| std::mem::take(&mut a.read) || a.asked == turn());
}

/// `text` in `font` broken into lines no wider than `max_width` where it can
/// be (0 for one line): each line and its width. None until the platform has
/// laid it out, and it is asked for: a later turn has it.
pub fn text_lines(font: Font, max_width: f64, text_in: &str) -> Option<Vec<(&str, f64)>> {
    let key: LayoutKey = (font.weight, font.px.to_bits(), (max_width as f32).to_bits(), text_in.to_owned());
    let t = text();
    let Some(answer) = t.layouts.get_mut(&key) else {
        // No more lines than characters, and one for an empty text.
        let lines = text_in.chars().count().max(1) as u32;
        let mut at = vec![0u32; 1 + 2 * lines as usize].into_boxed_slice();
        let args = [font.weight, font.px.to_bits(), (max_width as f32).to_bits(), off_of(at.as_mut_ptr()) as u32, lines];
        if !host_record(op::TEXT_LAYOUT, &args, text_in.as_bytes()) {
            return Some(vec![(text_in, text_in.chars().count() as f64 * 7.0)]);
        }
        t.layouts.insert(key, Answer { asked: turn(), read: true, at });
        return None;
    };
    answer.read = true;
    if answer.asked == turn() {
        return None;
    }
    let words = &answer.at;
    let mut lines = Vec::with_capacity(words[0] as usize);
    let mut start = 0;
    for pair in words[1..].as_chunks::<2>().0.iter().take(words[0] as usize) {
        let end = (pair[0] as usize).clamp(start, text_in.len());
        lines.push((&text_in[start..end], f32::from_bits(pair[1]) as f64));
        start = end;
    }
    Some(lines)
}

/// The width `text` takes on one line in `font`. None until it is laid out.
pub fn text_width(font: Font, text_in: &str) -> Option<f64> {
    text_lines(font, 0.0, text_in).map(|lines| lines.iter().map(|&(_, w)| w).sum())
}

/// One line of `text` in `font` and `color` (0xRRGGBBAA), drawn at the
/// sheet's scale (pxl_render::gpu::text::raster_box). None until the
/// platform has drawn it, and it is asked for.
pub fn text_raster(font: Font, color: u32, text_in: &str) -> Option<Raster<'static>> {
    let key: RasterKey = (font.weight, font.px.to_bits(), color, text_in.to_owned());
    if let Some(answer) = text().rasters.get_mut(&key) {
        answer.read = true;
        if answer.asked == turn() {
            return None;
        }
        let a = &answer.at;
        // SAFETY: the pixels are the answer's, which stays until the next
        // frame's rise at the earliest; what reads a raster reads it within
        // the frame it was handed out in.
        let pixels = unsafe { core::slice::from_raw_parts(a.pixels.as_ptr(), a.pixels.len()) };
        return Some(Raster {
            pixels,
            width: a.size[0],
            height: a.size[1],
            origin: a.origin,
            advance: a.advance,
        });
    }
    let advance = text_width(font, text_in)? as f32;
    let (size, origin) = raster_box(font, advance);
    let scale = DPR_MAX as f32;
    let mut pixels = vec![0u8; (size[0] * size[1] * 4) as usize].into_boxed_slice();
    let args = [
        font.weight,
        (font.px * scale).to_bits(),
        color,
        off_of(pixels.as_mut_ptr()) as u32,
        size[0],
        size[1],
        origin[0].to_bits(),
    ];
    if !host_record(op::TEXT_RASTER, &args, text_in.as_bytes()) {
        return None;
    }
    let at = RasterAt {
        pixels,
        size,
        origin,
        advance: advance * scale,
    };
    text().rasters.insert(key, Answer { asked: turn(), read: true, at });
    None
}
