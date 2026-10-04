// pxl-kernel/text.rs

//! Text laid out and drawn by the platform, asked through the host stream
//! (TEXT_LAYOUT, TEXT_RASTER) and answered next turn. Each world frame keeps
//! only the answers the frame before read. A native test has no host: a text
//! is one line of fixed width per character and is never drawn.

use std::collections::HashMap;

use pxl_render::gpu::surface::DPR_MAX;
use pxl_render::gpu::text::{raster_box, Font, Raster};
use xpute_runtime::mem::base::off_of_slice;

use crate::abi::host::op;
use crate::host::{host_record, turn};
use xpute_runtime::global::Global;

/// (weight, px bits, max width bits; 0 for one line, text).
type LayoutKey = (u32, u32, u32, String);
/// (weight, px bits, color, text).
type RasterKey = (u32, u32, u32, String);

struct Answer<T> {
    /// The host has written it on any turn after this.
    asked: u64,
    read: bool,
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

/// The host that was asked is gone; an answer it never ran would read as empty forever.
pub fn text_forget() {
    let t = text();
    t.layouts.clear();
    t.rasters.clear();
}

pub fn text_rise() {
    let t = text();
    t.layouts.retain(|_, a| std::mem::take(&mut a.read) || a.asked == turn());
    t.rasters.retain(|_, a| std::mem::take(&mut a.read) || a.asked == turn());
}

/// None until the platform has laid it out; a later turn has it.
pub fn text_lines(font: Font, max_width: f64, text_in: &str) -> Option<Vec<(&str, f64)>> {
    let key: LayoutKey = (font.weight, font.px.to_bits(), (max_width as f32).to_bits(), text_in.to_owned());
    let t = text();
    let Some(answer) = t.layouts.get_mut(&key) else {
        // No more lines than characters, and one for an empty text.
        let lines = text_in.chars().count().max(1) as u32;
        let mut at = vec![0u32; 1 + 2 * lines as usize].into_boxed_slice();
        let args = [font.weight, font.px.to_bits(), (max_width as f32).to_bits(), off_of_slice(&raw mut *at) as u32, lines];
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

pub fn text_width(font: Font, text_in: &str) -> Option<f64> {
    text_lines(font, 0.0, text_in).map(|lines| lines.iter().map(|&(_, w)| w).sum())
}

/// `color` is 0xRRGGBBAA. None until the platform has drawn it.
pub fn text_raster(font: Font, color: u32, text_in: &str) -> Option<Raster<'static>> {
    let key: RasterKey = (font.weight, font.px.to_bits(), color, text_in.to_owned());
    if let Some(answer) = text().rasters.get_mut(&key) {
        answer.read = true;
        if answer.asked == turn() {
            return None;
        }
        let a = &answer.at;
        // SAFETY: the answer's pixels stay until the next frame's rise at the
        // earliest, and a raster is read within the frame it was handed out in.
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
        off_of_slice(&raw mut *pixels) as u32,
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
