// pxl-kernel/billboard/badge.rs

//! What a billboard paints, each its own design on the one sheet: words on a
//! pill, a marker's someone put up, or in its colors turned over the count
//! of a cell's markers where more than one stands in it; a place, as Carto draws it; a line said in the
//! chat, in a bubble. A label is what is painted and nothing of who shows
//! it, so two that paint alike are one: measured, given its slot and painted
//! once, and shown only once painted (`ready`).

use std::rc::Rc;

use fnv::{FnvHashMap, FnvHashSet};

use crate::billboard::sheet::billboard_sheet;
use crate::io::text::{text_lines, text_width};
use pxl_render::gpu::sheet::shelf::{DrawSpec, NO_SLOT};
use pxl_render::gpu::sheet::SHEET_SCALE;
use pxl_render::look::badge::{bubble_size, marker_size, point_size, BADGE_FONT, BUBBLE_FONT, BUBBLE_TEXT_W, COUNT_FILL, COUNT_TEXT, PILL_FILL, PILL_TEXT, POINT_FONT};
use xpute_guest::global::Global;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Label {
    /// Words on a pill in its `fill`, in its `ink`: a marker's (`pill`) or
    /// a count of them (`count`).
    Pill { text: Rc<str>, fill: u32, ink: u32 },
    /// A place as Carto draws it: its symbol (`icon`), or a dot, in its
    /// category's `fill`, and under it its name, where it is named, in its
    /// category's `text` color, wrapped at `wrap` pixels (`spaced`).
    Point {
        name: Option<Rc<str>>,
        wrap: u32,
        icon: bool,
        symbol: Option<u16>,
        fill: u32,
        text: u32,
    },
    /// A line said in the chat.
    Bubble { text: Rc<str> },
}

impl Label {
    /// A marker's words, cut to `SHOWN_CHARS`.
    pub fn pill(words: &str) -> Label {
        let mut shown: String = words.chars().take(SHOWN_CHARS).collect();
        if words.chars().count() > SHOWN_CHARS {
            shown.push('…');
        }
        Label::Pill {
            text: shown.into(),
            fill: PILL_FILL,
            ink: PILL_TEXT,
        }
    }

    /// The count of `n` markers in one marker's cell (`marker_cell`), on a
    /// marker's pill in the count's colors.
    pub fn count(n: usize) -> Label {
        Label::Pill {
            text: n.to_string().into(),
            fill: COUNT_FILL,
            ink: COUNT_TEXT,
        }
    }

    /// As it is laid out where a place keeps `spacing` pixels to itself
    /// (scene/place.rs `spacing_px`), and the width of the region it keeps
    /// (`field.rs`): a place's name wrapped to it, and its region it; any
    /// other's region its own width (0, which the batch widens to it).
    pub fn spaced(mut self, spacing: f64) -> (Label, f64) {
        match &mut self {
            Label::Point { name, wrap, .. } => {
                if name.is_some() {
                    *wrap = spacing as u32;
                }
                (self, spacing)
            }
            _ => (self, 0.0),
        }
    }
}

/// A product choice: the length a one-line badge still reads as a name at.
/// Code points, so a non-BMP character is never cut in half.
const SHOWN_CHARS: usize = 12;

struct Badge {
    size: [f64; 3],
    /// Its words as they were laid out when sized: the host's layouts are a
    /// cache it may let go.
    lines: Vec<String>,
    slot: i32,
}
static BADGES: Global<FnvHashMap<Label, Badge>> = Global::new();
fn badges() -> &'static mut FnvHashMap<Label, Badge> {
    BADGES.get(FnvHashMap::default)
}

/// Lets go of every label but `labels`, those no billboard shown or waiting
/// uses, their slots freed; whether any slot was, so a label the sheet had
/// no room for may have it now.
pub fn retain(labels: &FnvHashSet<Label>) -> bool {
    let mut dropped = false;
    badges().retain(|label, b| {
        if labels.contains(label) {
            return true;
        }
        if b.slot != NO_SLOT {
            billboard_sheet().release(b.slot as u32);
            dropped = true;
        }
        false
    });
    // The moment the sheet's live set shrinks, so the moment it may stop being
    // as large as the densest view made it.
    if dropped {
        billboard_sheet().compact();
    }
    dropped
}

/// Whether it becomes ready (`ready`) with nothing else moving: its words
/// asked of the host, answered by a later turn, or its slot given and
/// painted at render. One the sheet has no room for becomes ready only once
/// a slot is let go (`retain`), so it asks no turn of its own.
pub fn coming(label: &Label) -> bool {
    badges().get(label).is_none_or(|b| b.slot != NO_SLOT && !billboard_sheet().painted(b.slot as u32))
}

/// Its width, height and where it stands down from its top; none until its
/// words are laid out, which a later turn has.
pub fn size(label: &Label) -> Option<[f64; 3]> {
    if let Some(b) = badges().get(label) {
        return Some(b.size);
    }
    let laid = |font, wrap, text: &str| -> Option<(f64, Vec<String>)> {
        let laid = text_lines(font, wrap, text)?;
        Some((laid.iter().map(|&(_, w)| w).fold(0.0, f64::max), laid.iter().map(|&(l, _)| l.trim().to_owned()).collect()))
    };
    let (size, lines) = match label {
        Label::Pill { text, .. } => (marker_size(text_width(BADGE_FONT, text)?.ceil()), Vec::new()),
        Label::Point { name: None, icon, .. } => (point_size(*icon, None), Vec::new()),
        Label::Point { name: Some(name), wrap, icon, .. } => {
            let (w, lines) = laid(POINT_FONT, f64::from(*wrap), name)?;
            (point_size(*icon, Some((w, lines.len()))), lines)
        }
        Label::Bubble { text } => {
            let (w, lines) = laid(BUBBLE_FONT, BUBBLE_TEXT_W, text)?;
            (bubble_size(w, lines.len()), lines)
        }
    };
    badges().insert(label.clone(), Badge { size, lines, slot: NO_SLOT });
    Some(size)
}

/// Whether the label is laid out and painted, so drawing it shows its words.
pub fn ready(label: &Label) -> bool {
    size(label).is_some() && {
        let slot = slot(label);
        slot != NO_SLOT && billboard_sheet().painted(slot as u32)
    }
}

/// Its label was measured first.
pub fn slot(label: &Label) -> i32 {
    let b = badges().get_mut(label).or_bug(Errno::ENOTRECOVERABLE);
    if b.slot == NO_SLOT {
        let [w, h, _] = b.size;
        b.slot = billboard_sheet().add(
            w * SHEET_SCALE,
            h * SHEET_SCALE,
            match label {
                Label::Pill { text, fill, ink } => DrawSpec::Pill {
                    label: text.to_string(),
                    fill: *fill,
                    text: *ink,
                    logical_w: w,
                },
                Label::Bubble { .. } => DrawSpec::Bubble { lines: b.lines.clone(), box_w: w },
                &Label::Point { icon, symbol, fill, text, .. } => DrawSpec::Point {
                    lines: b.lines.clone(),
                    icon,
                    symbol,
                    fill,
                    text,
                    logical_w: w,
                },
            },
        );
    }
    b.slot
}
