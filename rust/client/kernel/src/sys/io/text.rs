// pxl-kernel/sys/io/text.rs

//! Text laid out and drawn by the platform, asked by hostcall (TEXT_LAYOUT,
//! TEXT_RASTER) and answered before the call returns. Each world frame keeps
//! only the answers the frame before read. A native test has no host: a text
//! is one line of fixed width per character and is never drawn.

use std::collections::hash_map::Entry;

use fnv::FnvHashMap;

use pxl_render::gpu::sheet::SHEET_SCALE;
use pxl_render::gpu::text::{raster_box, Font, Raster};
use xpute_guest::mem::off_of_slice;

use crate::sys::io::host::call_now;
use crate::sys::io::host_spec::{op, LAYOUT, LAYOUT_ANSWER, LINE, RASTER};
use xpute_guest::global::Global;

/// (weight, px bits, max width bits; 0 for one line, text).
type LayoutKey = (u32, u32, u32, String);
/// (weight, px bits, color, text).
type RasterKey = (u32, u32, u32, String);

struct Answer<T> {
    read: bool,
    at: T,
}

struct RasterAt {
    px: Box<[u8]>,
    size: [u32; 2],
    origin: [f32; 2],
    advance: f32,
}

#[derive(Default)]
struct Text {
    layouts: FnvHashMap<LayoutKey, Answer<Box<[u32]>>>,
    rasters: FnvHashMap<RasterKey, Answer<RasterAt>>,
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
    t.layouts.retain(|_, a| std::mem::take(&mut a.read));
    t.rasters.retain(|_, a| std::mem::take(&mut a.read));
}

/// One line at a width guessed from its characters, where no host lays it out.
fn unlaid(text_in: &str) -> Vec<(&str, f64)> {
    vec![(text_in, text_in.chars().count() as f64 * 7.0)]
}

/// None where the platform could not lay it out this turn; a later turn asks again.
pub fn text_lines(font: Font, max_width: f64, text_in: &str) -> Option<Vec<(&str, f64)>> {
    let key: LayoutKey = (
        font.weight,
        font.px.to_bits(),
        (max_width as f32).to_bits(),
        text_in.to_owned(),
    );
    let answer = match text().layouts.entry(key) {
        Entry::Occupied(o) => o.into_mut(),
        Entry::Vacant(v) => {
            // No more lines than characters, and one for an empty text.
            let lines = text_in.chars().count().max(1) as u32;
            let mut at = vec![0u32; (LAYOUT_ANSWER::WORDS + LINE::WORDS * lines) as usize]
                .into_boxed_slice();
            let mut args = [0u32; LAYOUT::WORDS as usize];
            args[LAYOUT::weight as usize] = font.weight;
            args[LAYOUT::px as usize] = font.px.to_bits();
            args[LAYOUT::max_width as usize] = (max_width as f32).to_bits();
            args[LAYOUT::at as usize] = off_of_slice(&raw mut *at) as u32;
            args[LAYOUT::lines as usize] = lines;
            match call_now(op::TEXT_LAYOUT, &args, text_in.as_bytes()) {
                None => return Some(unlaid(text_in)),
                Some(Err(_)) => return None,
                Some(Ok(_)) => {}
            }
            v.insert(Answer { read: false, at })
        }
    };
    answer.read = true;
    // The host's answer is checked here, the door it comes back through: a
    // line ending inside a character or out of order is no layout.
    let pairs = answer.at[LAYOUT_ANSWER::WORDS as usize..]
        .as_chunks::<{ LINE::WORDS as usize }>()
        .0;
    let cnt = answer.at[LAYOUT_ANSWER::cnt as usize] as usize;
    let mut lines = Vec::with_capacity(cnt.min(pairs.len()));
    let mut start = 0;
    for pair in pairs.iter().take(cnt) {
        let (end, width) = (
            pair[LINE::end as usize] as usize,
            f32::from_bits(pair[LINE::width as usize]) as f64,
        );
        if cnt > pairs.len() || end < start || !text_in.is_char_boundary(end) || !width.is_finite()
        {
            return Some(unlaid(text_in));
        }
        lines.push((&text_in[start..end], width));
        start = end;
    }
    Some(lines)
}

pub fn text_width(font: Font, text_in: &str) -> Option<f64> {
    text_lines(font, 0.0, text_in).map(|lines| lines.iter().map(|&(_, w)| w).sum())
}

/// The answer's pixels as a raster.
fn raster_of(a: &RasterAt) -> Raster<'static> {
    // SAFETY: the answer's pixels stay until the next frame's rise at the
    // earliest, and a raster is read within the frame it was handed out in.
    let px = unsafe { core::slice::from_raw_parts(a.px.as_ptr(), a.px.len()) };
    Raster {
        px,
        width: a.size[0],
        height: a.size[1],
        origin: a.origin,
        advance: a.advance,
    }
}

/// `color` is 0xRRGGBBAA. None where the platform could not draw it this
/// turn; a later turn asks again.
pub fn text_raster(font: Font, color: u32, text_in: &str) -> Option<Raster<'static>> {
    let key: RasterKey = (font.weight, font.px.to_bits(), color, text_in.to_owned());
    if let Some(answer) = text().rasters.get_mut(&key) {
        answer.read = true;
        return Some(raster_of(&answer.at));
    }
    let advance = text_width(font, text_in)? as f32;
    let (size, origin) = raster_box(font, advance);
    let scale = SHEET_SCALE as f32;
    let mut px = vec![0u8; (size[0] * size[1] * 4) as usize].into_boxed_slice();
    let mut args = [0u32; RASTER::WORDS as usize];
    args[RASTER::weight as usize] = font.weight;
    args[RASTER::px as usize] = (font.px * scale).to_bits();
    args[RASTER::color as usize] = color;
    args[RASTER::at as usize] = off_of_slice(&raw mut *px) as u32;
    args[RASTER::width as usize] = size[0];
    args[RASTER::height as usize] = size[1];
    args[RASTER::x as usize] = origin[0].to_bits();
    if !matches!(
        call_now(op::TEXT_RASTER, &args, text_in.as_bytes()),
        Some(Ok(_))
    ) {
        return None;
    }
    let at = RasterAt {
        px,
        size,
        origin,
        advance: advance * scale,
    };
    let raster = raster_of(&at);
    text().rasters.insert(key, Answer { read: true, at });
    Some(raster)
}
