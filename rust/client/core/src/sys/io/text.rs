// pxl-core/sys/io/text.rs

//! Text laid out and drawn by the platform, called of its text device
//! (text.json TEXT_LAYOUT, TEXT_RASTER) and answered before the call returns,
//! each answer written at its `at` (runtime.json `LAYOUT_ANSWER`, `LINE`). Each
//! world frame keeps only the answers the frame before read. A native test has
//! no runtime: a text is one line of fixed width per character and is never
//! drawn.

use std::collections::hash_map::Entry;

use fnv::FnvHashMap;

use xpute_core::mem::off_of_slice;

use crate::sys::io::runtime;
use crate::sys::io::runtime_spec::{LAYOUT_ANSWER, LINE};
use crate::sys::io::text_spec::text::{text_layout, text_raster};
use crate::sys::io::text_spec::{TextLayout, TextRaster};
use xpute_core::global::Global;

/// (weight, px bits, max width bits; 0 for one line, text).
type LayoutKey = (u32, u64, u64, String);
/// (weight, px bits, color, width, height, x bits, text).
type RasterKey = (u32, u64, u32, u32, u32, u64, String);

struct Answer<T> {
    read: bool,
    at: T,
}

#[derive(Default)]
struct Text {
    layouts: FnvHashMap<LayoutKey, Answer<Box<[u32]>>>,
    rasters: FnvHashMap<RasterKey, Answer<Box<[u8]>>>,
}

static TEXT: Global<Text> = Global::new();

fn text() -> &'static mut Text {
    TEXT.get(Text::default)
}

/// The runtime that was asked is gone; an answer it never ran would read as empty forever.
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

/// One line at a width guessed from its characters, where the client does not lay it out.
fn unlaid(text_in: &str) -> Vec<(&str, f64)> {
    vec![(text_in, text_in.chars().count() as f64 * 7.0)]
}

/// `text_in` broken into lines no wider than `max_width` (0 for one line),
/// each with its width. None where the platform could not lay it out this
/// turn; a later turn asks again.
pub fn text_lines(weight: u32, px: f64, max_width: f64, text_in: &str) -> Option<Vec<(&str, f64)>> {
    let key: LayoutKey = (
        weight,
        px.to_bits(),
        max_width.to_bits(),
        text_in.to_owned(),
    );
    let answer = match text().layouts.entry(key) {
        Entry::Occupied(o) => o.into_mut(),
        Entry::Vacant(v) => {
            if runtime::door().is_none() {
                return Some(unlaid(text_in));
            }
            // No more lines than characters, and one for an empty text.
            let lines = text_in.chars().count().max(1) as u32;
            let mut at = vec![0u32; (LAYOUT_ANSWER::WORDS + LINE::WORDS * lines) as usize]
                .into_boxed_slice();
            let call = TextLayout {
                weight,
                px,
                max_width,
                at: off_of_slice(&raw mut *at) as u32,
                lines,
                text_at: off_of_slice(text_in.as_bytes()) as u32,
                text_len: text_in.len() as u32,
            };
            if text_layout(&call).is_err() {
                return None;
            }
            v.insert(Answer { read: false, at })
        }
    };
    answer.read = true;
    // The client's answer is checked here, the door it comes back through: a
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

/// One line of `text_in` drawn into `size` pixels from `x`, RGBA, top row
/// first; `color` is 0xRRGGBBAA. The pixels stay until the next frame's
/// rise at the earliest. None where the platform could not draw it this
/// turn; a later turn asks again.
pub fn text_pixels(
    weight: u32,
    px: f64,
    color: u32,
    size: [u32; 2],
    x: f64,
    text_in: &str,
) -> Option<&'static [u8]> {
    let key: RasterKey = (
        weight,
        px.to_bits(),
        color,
        size[0],
        size[1],
        x.to_bits(),
        text_in.to_owned(),
    );
    let answer = match text().rasters.entry(key) {
        Entry::Occupied(o) => o.into_mut(),
        Entry::Vacant(v) => {
            runtime::door()?;
            let mut at = vec![0u8; (size[0] * size[1] * 4) as usize].into_boxed_slice();
            let call = TextRaster {
                weight,
                px,
                color,
                at: off_of_slice(&raw mut *at) as u32,
                width: size[0],
                height: size[1],
                x,
                text_at: off_of_slice(text_in.as_bytes()) as u32,
                text_len: text_in.len() as u32,
            };
            if text_raster(&call).is_err() {
                return None;
            }
            v.insert(Answer { read: false, at })
        }
    };
    answer.read = true;
    // SAFETY: an answer's pixels are let go only at the next frame's rise or
    // at a forget, and a reader reads them within the frame they were handed
    // out in.
    Some(unsafe { core::slice::from_raw_parts(answer.at.as_ptr(), answer.at.len()) })
}
