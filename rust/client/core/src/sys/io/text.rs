// pxl-core/sys/io/text.rs

//! Text that the platform measures and draws, through its `text` (text.wit
//! `width`, `raster`), and that the core breaks into lines itself: words fill
//! each line greedily, a word too wide for a line breaks between characters,
//! no line starts with a closing mark or ends with an opening one, and a
//! line's width leaves out the spaces at its ends. The platform answers each
//! call before it returns, reading the text the call lends, and a raster
//! writes into the image it lends. Each world frame keeps only the answers that
//! the frame before it read.

use std::collections::hash_map::Entry;

use fnv::FnvHashMap;
use pxl_global::Global;

use crate::abi::text;

/// A layout's key: the weight, `px` and the max width (0 for one line) by
/// their bits, and the text's UTF-8, which a miss lends to the platform.
#[derive(PartialEq, Eq, Hash)]
struct LayoutKey {
    weight: u32,
    px: u64,
    max_width: u64,
    text: Box<[u8]>,
}

/// A raster's key: the weight, `px` by its bits, the color, the width and the
/// height, `x` by its bits, and the text's UTF-8, as a layout's.
#[derive(PartialEq, Eq, Hash)]
struct RasterKey {
    weight: u32,
    px: u64,
    color: u32,
    width: u32,
    height: u32,
    x: u64,
    text: Box<[u8]>,
}

struct Cached<T> {
    /// Whether the answer was read since the last frame's start.
    read: bool,
    /// The layout's words or the image's pixels.
    buf: T,
}

/// A layout: each line's end, a byte offset, and its width.
type Lines = Box<[(usize, f64)]>;

#[derive(Default)]
struct Cache {
    layouts: FnvHashMap<LayoutKey, Cached<Lines>>,
    rasters: FnvHashMap<RasterKey, Cached<Box<[u8]>>>,
}

static CACHE: Global<Cache> = Global::new();

fn cache() -> &'static mut Cache {
    CACHE.get(Cache::default)
}

/// `text` broken into lines no wider than `max_width`, each with its
/// width; a `max_width` of 0 gives one line. It is None where the platform
/// could not measure the text this frame, and a later frame asks again.
pub fn text_lines(weight: u32, px: f64, max_width: f64, text: &str) -> Option<Vec<(&str, f64)>> {
    let key = LayoutKey {
        weight,
        px: px.to_bits(),
        max_width: max_width.to_bits(),
        text: Box::from(text.as_bytes()),
    };
    let c = cache();
    let cached = match c.layouts.entry(key) {
        Entry::Occupied(o) => o.into_mut(),
        Entry::Vacant(v) => {
            // The key's text is lent to each measure, and becomes the key once
            // the lines are found.
            let mut key = v.into_key();
            let mut width = |at: usize, end: usize| {
                text::width(weight, px, &mut key.text[at..end])
                    .ok()
                    .filter(|w| w.is_finite())
            };
            let buf = lines(text, max_width, &mut width)?.into_boxed_slice();
            c.layouts.entry(key).or_insert(Cached { read: false, buf })
        }
    };
    cached.read = true;
    let mut start = 0;
    Some(
        cached
            .buf
            .iter()
            .map(|&(end, width)| {
                let line = (&text[start..end], width);
                start = end;
                line
            })
            .collect(),
    )
}

/// A space, as a page's `\s` and `trim` read it.
fn is_space(c: char) -> bool {
    matches!(
        c,
        '\u{9}'..='\u{d}'
            | ' '
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{feff}'
    )
}

/// The characters no line may start with: closing marks and, in Japanese, the
/// small kana and the marks that lean on the character before them.
const NO_LINE_START: &str =
    "、。，．・：；？！）］｝〕〉》」』】”’…‥ーぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮゝゞヽヾ々,.:;!?)]}";

/// The characters no line may end with: opening marks.
const NO_LINE_END: &str = "（［｛〔〈《「『【“‘([{";

/// Each line's end, a byte offset, and its width, by `width`, the width of
/// the text between two offsets; None where a measure fails.
fn lines(
    text: &str,
    max_width: f64,
    width: &mut impl FnMut(usize, usize) -> Option<f64>,
) -> Option<Vec<(usize, f64)>> {
    let mut start = 0;
    ends(text, max_width, width)?
        .into_iter()
        .map(|end| {
            let line = &text[start..end];
            let at = start + (line.len() - line.trim_start_matches(is_space).len());
            let w = width(at, at + line.trim_matches(is_space).len())?;
            start = end;
            Some((end, w))
        })
        .collect()
}

/// Where the lines end, the last at the text's end. Words fill each line
/// greedily; a word too wide for a line by itself, as Korean, Japanese or
/// Chinese typed without spaces can be, breaks between characters. A space
/// stays at the head of the next line, which its width leaves out.
fn ends(
    text: &str,
    max_width: f64,
    width: &mut impl FnMut(usize, usize) -> Option<f64>,
) -> Option<Vec<usize>> {
    let words: Vec<(usize, usize)> = text
        .split(is_space)
        .filter(|w| !w.is_empty())
        .map(|w| {
            let at = w.as_ptr() as usize - text.as_ptr() as usize;
            (at, at + w.len())
        })
        .collect();
    let Some(&(first, first_end)) = words.first() else {
        return Some(vec![text.len()]);
    };
    if max_width <= 0.0 {
        return Some(vec![text.len()]);
    }
    let mut spans = Vec::new();
    let (mut start, mut end) = (first, first_end);
    for &(ws, we) in &words[1..] {
        if width(start, we)? <= max_width {
            end = we;
        } else {
            spans.push((start, end));
            (start, end) = (ws, we);
        }
    }
    spans.push((start, end));
    let mut ends = Vec::with_capacity(spans.len());
    for (s, e) in spans {
        if width(s, e)? > max_width {
            let mut chunk = s;
            for (i, ch) in text[s..e].char_indices() {
                let (i, next) = (s + i, s + i + ch.len_utf8());
                if i > chunk && width(chunk, next)? > max_width {
                    chunk = allowed_break(text, chunk, i);
                    ends.push(chunk);
                }
            }
        }
        ends.push(e);
    }
    if let Some(last) = ends.last_mut() {
        *last = text.len();
    }
    Some(ends)
}

/// Moves the break at `at` back past any mark that NO_LINE_START or
/// NO_LINE_END keeps with its neighbor, but never past the line's first
/// character, which starts at `line`.
fn allowed_break(text: &str, line: usize, mut at: usize) -> usize {
    while let Some((before, ch)) = text[..at].char_indices().next_back() {
        let starts = text[at..].chars().next().is_some_and(|c| NO_LINE_START.contains(c));
        if before <= line || !(starts || NO_LINE_END.contains(ch)) {
            break;
        }
        at = before;
    }
    at
}

/// One line of `text` drawn from `x` into `size` pixels of RGBA, top row
/// first. `color` is 0xRRGGBBAA. The pixels stay at least until the next
/// frame's start. It is None where the platform could not draw the text this
/// frame, and a later frame asks again.
pub fn text_pixels(
    weight: u32,
    px: f64,
    color: u32,
    size: [u32; 2],
    x: f64,
    text: &str,
) -> Option<&'static [u8]> {
    let key = RasterKey {
        weight,
        px: px.to_bits(),
        color,
        width: size[0],
        height: size[1],
        x: x.to_bits(),
        text: Box::from(text.as_bytes()),
    };
    let c = cache();
    let cached = match c.rasters.entry(key) {
        Entry::Occupied(o) => o.into_mut(),
        Entry::Vacant(v) => {
            let mut image = vec![0u8; (size[0] * size[1] * 4) as usize].into_boxed_slice();
            let mut key = v.into_key();
            let drawn = text::raster(weight, px, color, &mut image[..], size[0], size[1], x, &mut key.text[..]);
            if drawn.is_err() {
                return None;
            }
            c.rasters.entry(key).or_insert(Cached { read: false, buf: image })
        }
    };
    cached.read = true;
    // SAFETY: an answer's pixels are let go only at the next frame's start or
    // at a forget, and a reader reads them within the frame they were handed
    // out in.
    Some(unsafe { core::slice::from_raw_parts(cached.buf.as_ptr(), cached.buf.len()) })
}

/// Called as a frame starts. The answers that the frame before did not read
/// are dropped.
pub fn text_drop_unread() {
    let c = cache();
    c.layouts.retain(|_, e| core::mem::take(&mut e.read));
    c.rasters.retain(|_, e| core::mem::take(&mut e.read));
}
