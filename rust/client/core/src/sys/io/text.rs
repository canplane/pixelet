// pxl-core/sys/io/text.rs

//! Text that the platform lays out and draws, through calls to its text
//! device (text.json TEXT_LAYOUT, TEXT_RASTER). The platform answers each
//! call before the call returns, and writes the answer at the call's `at`
//! (text_answer.json `LAYOUT_ANSWER`, `LINE`). Each world frame keeps only the
//! answers that the frame before it read.

use std::collections::hash_map::Entry;

use fnv::FnvHashMap;
use pxl_global::Global;
use xpute_core::mem;

use crate::sys::io::text_answer_spec::{LAYOUT_ANSWER, LINE};
use crate::abi::text;

/// A layout's key: the weight, the bits of `px`, the bits of the max width
/// (0 for one line), and the text.
type LayoutKey = (u32, u64, u64, String);
/// A raster's key: the weight, the bits of `px`, the color, the width, the
/// height, the bits of `x`, and the text.
type RasterKey = (u32, u64, u32, u32, u32, u64, String);

struct Answer<T> {
    /// Whether the answer was read since the last frame's start.
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

/// `text_in` broken into lines no wider than `max_width`, each with its
/// width; a `max_width` of 0 gives one line. It is None where the platform
/// could not lay the text out this frame, and a later frame asks again.
pub fn text_lines(weight: u32, px: f64, max_width: f64, text_in: &str) -> Option<Vec<(&str, f64)>> {
    let key = (
        weight,
        px.to_bits(),
        max_width.to_bits(),
        text_in.to_owned(),
    );
    let answer = match text().layouts.entry(key) {
        Entry::Occupied(o) => o.into_mut(),
        Entry::Vacant(v) => {
            // A text breaks into no more lines than it has characters, and an
            // empty text into one.
            let lines = text_in.chars().count().max(1);
            let mut at = vec![0u32; LAYOUT_ANSWER::WORDS + LINE::WORDS * lines].into_boxed_slice();
            let laid = text::layout(
                weight,
                px,
                max_width,
                mem::off(at.as_mut_ptr().cast()),
                lines as u32,
                mem::off(text_in.as_ptr()),
                text_in.len() as u32,
            );
            if laid.is_err() {
                return None;
            }
            v.insert(Answer { read: false, at })
        }
    };
    answer.read = true;
    // The client's answer is checked here, where it comes back in. A line
    // that ends inside a character or out of order makes the answer no
    // layout.
    let pairs = answer.at[LAYOUT_ANSWER::WORDS..]
        .as_chunks::<{ LINE::WORDS }>()
        .0;
    let cnt = answer.at[LAYOUT_ANSWER::cnt] as usize;
    if cnt > pairs.len() {
        return Some(unlaid(text_in));
    }
    let mut lines = Vec::with_capacity(cnt);
    let mut start = 0;
    for pair in &pairs[..cnt] {
        let end = pair[LINE::end] as usize;
        let width = f64::from(f32::from_bits(pair[LINE::width]));
        if end < start || !text_in.is_char_boundary(end) || !width.is_finite() {
            return Some(unlaid(text_in));
        }
        lines.push((&text_in[start..end], width));
        start = end;
    }
    Some(lines)
}

/// One line at a width guessed from its characters, for where the client
/// does not lay the text out.
fn unlaid(text_in: &str) -> Vec<(&str, f64)> {
    vec![(text_in, text_in.chars().count() as f64 * 7.0)]
}

/// One line of `text_in` drawn from `x` into `size` pixels of RGBA, top row
/// first. `color` is 0xRRGGBBAA. The pixels stay at least until the next
/// frame's start. It is None where the platform could not draw the text this
/// frame, and a later frame asks again.
pub fn text_pixels(
    weight: u32,
    px: f64,
    color: u32,
    size: [u32; 2],
    x: f64,
    text_in: &str,
) -> Option<&'static [u8]> {
    let key = (
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
            let mut at = vec![0u8; (size[0] * size[1] * 4) as usize].into_boxed_slice();
            let drawn = text::raster(
                weight,
                px,
                color,
                mem::off(at.as_mut_ptr().cast()),
                size[0],
                size[1],
                x,
                mem::off(text_in.as_ptr()),
                text_in.len() as u32,
            );
            if drawn.is_err() {
                return None;
            }
            v.insert(Answer { read: false, at })
        }
    };
    answer.read = true;
    // SAFETY: an answer's pixels are let go only at the next frame's start or
    // at a forget, and a reader reads them within the frame they were handed
    // out in.
    Some(unsafe { core::slice::from_raw_parts(answer.at.as_ptr(), answer.at.len()) })
}

/// Called as a frame starts. The answers that the frame before did not read
/// are dropped.
pub fn text_drop_unread() {
    let t = text();
    t.layouts.retain(|_, a| core::mem::take(&mut a.read));
    t.rasters.retain(|_, a| core::mem::take(&mut a.read));
}

