// pxl-kernel/domain/scene/marker/design.rs

//! What a marker paints, each its own design on the one sheet: words on a
//! pill, a place as Carto draws it, a line said in a bubble. A design is what
//! is painted and nothing of who shows it, so two that paint alike are one:
//! measured, given its slot and painted once, and shown only once painted
//! (`ready`). Which words and colors a source paints are its own
//! (`place.rs`, `pin.rs` and `line.rs`).

use std::rc::Rc;

use fnv::{FnvHashMap, FnvHashSet};

use crate::domain::scene::marker::sheet::sheet;
use crate::sys::io::text::{text_lines, text_width};
use pxl_render::gpu::sheet::shelf::{Paint, NO_SLOT};
use pxl_render::gpu::sheet::SHEET_SCALE;
use pxl_render::look::marker::{bubble_size, pill_size, point_size, BUBBLE_FONT, BUBBLE_TEXT_W, PILL_FONT, POINT_FONT};
use xpute_guest::global::Global;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Design {
    /// Words on a pill in its `fill`, in its `ink`.
    Pill { words: Rc<str>, fill: u32, ink: u32 },
    /// A place as Carto draws it: its symbol (`icon`), or a dot, in its
    /// category's `fill`, and under it its name, where it is named, in its
    /// category's text color, its `ink`, wrapped at `wrap` pixels
    /// (`spaced`).
    Point {
        name: Option<Rc<str>>,
        wrap: u32,
        icon: bool,
        symbol: Option<u16>,
        fill: u32,
        ink: u32,
    },
    /// A line said in the chat.
    Bubble { words: Rc<str> },
}

impl Design {
    /// As it is laid out where a place keeps `spacing` pixels to itself
    /// (`layout.rs` `spacing_px`), and the width of the region it keeps
    /// (`set.rs`): a place's name wrapped to it, and its region it; any
    /// other's region its own width (0, which the batch widens to it).
    pub fn spaced(mut self, spacing: f64) -> (Design, f64) {
        match &mut self {
            Design::Point { name, wrap, .. } => {
                if name.is_some() {
                    *wrap = spacing as u32;
                }
                (self, spacing)
            }
            _ => (self, 0.0),
        }
    }
}

/// A design as the sheet holds it: its size, laid out once, and its slot,
/// none until it is asked for (`slot`).
struct Sprite {
    size: [f64; 3],
    /// Its words as they were laid out when sized: the host's layouts are a
    /// cache it may let go.
    lines: Vec<String>,
    slot: i32,
}
static SPRITES: Global<FnvHashMap<Design, Sprite>> = Global::new();
fn sprites() -> &'static mut FnvHashMap<Design, Sprite> {
    SPRITES.get(FnvHashMap::default)
}

/// Lets go of every design but `designs`, those no marker shown or waiting
/// uses, their slots freed; whether any slot was, so a design the sheet had
/// no room for may have it now.
pub fn retain(designs: &FnvHashSet<Design>) -> bool {
    let mut dropped = false;
    sprites().retain(|design, b| {
        if designs.contains(design) {
            return true;
        }
        if b.slot != NO_SLOT {
            sheet().release(b.slot as u32);
            dropped = true;
        }
        false
    });
    // The moment the sheet's live set shrinks, so the moment it may stop being
    // as large as the densest view made it.
    if dropped {
        sheet().compact();
    }
    dropped
}

/// Whether it becomes ready (`ready`) with nothing else moving: its words
/// asked of the host, answered by a later turn, or its slot given and
/// painted at render. One the sheet has no room for becomes ready only once
/// a slot is let go (`retain`), so it asks no turn of its own.
pub fn coming(design: &Design) -> bool {
    sprites().get(design).is_none_or(|b| b.slot != NO_SLOT && !sheet().painted(b.slot as u32))
}

/// Its width, height and where it stands down from its top; none until its
/// words are laid out, which a later turn has.
pub fn size(design: &Design) -> Option<[f64; 3]> {
    if let Some(b) = sprites().get(design) {
        return Some(b.size);
    }
    let laid = |font, wrap, text: &str| -> Option<(f64, Vec<String>)> {
        let laid = text_lines(font, wrap, text)?;
        Some((laid.iter().map(|&(_, w)| w).fold(0.0, f64::max), laid.iter().map(|&(l, _)| l.trim().to_owned()).collect()))
    };
    let (size, lines) = match design {
        Design::Pill { words, .. } => (pill_size(text_width(PILL_FONT, words)?.ceil()), Vec::new()),
        Design::Point { name: None, icon, .. } => (point_size(*icon, None), Vec::new()),
        Design::Point { name: Some(name), wrap, icon, .. } => {
            let (w, lines) = laid(POINT_FONT, f64::from(*wrap), name)?;
            (point_size(*icon, Some((w, lines.len()))), lines)
        }
        Design::Bubble { words } => {
            let (w, lines) = laid(BUBBLE_FONT, BUBBLE_TEXT_W, words)?;
            (bubble_size(w, lines.len()), lines)
        }
    };
    sprites().insert(design.clone(), Sprite { size, lines, slot: NO_SLOT });
    Some(size)
}

/// Whether the design is laid out and painted, so drawing it shows its
/// words. It asks the sheet for no slot, which `slot` does.
pub fn ready(design: &Design) -> bool {
    size(design).is_some() && sprites().get(design).is_some_and(|b| b.slot != NO_SLOT && sheet().painted(b.slot as u32))
}

/// Its design was measured first.
pub fn slot(design: &Design) -> i32 {
    let b = sprites().get_mut(design).or_bug(Errno::ENOTRECOVERABLE);
    if b.slot == NO_SLOT {
        let [w, h, _] = b.size;
        b.slot = sheet().add(
            w * SHEET_SCALE,
            h * SHEET_SCALE,
            match design {
                Design::Pill { words, fill, ink } => Paint::Pill {
                    words: words.to_string(),
                    fill: *fill,
                    ink: *ink,
                    logical_w: w,
                },
                Design::Bubble { .. } => Paint::Bubble { lines: b.lines.clone(), box_w: w },
                &Design::Point { icon, symbol, fill, ink, .. } => Paint::Point {
                    lines: b.lines.clone(),
                    icon,
                    symbol,
                    fill,
                    ink,
                    logical_w: w,
                },
            },
        );
    }
    b.slot
}
