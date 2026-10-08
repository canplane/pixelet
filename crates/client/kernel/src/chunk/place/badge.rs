// pxl-kernel/chunk/place/badge.rs

//! What a billboard shows, each its own design on the one sheet: a marker
//! someone put up, its words on a pill; a place, as Carto draws it at the
//! zoom of Carto's the view is at there.

use crate::layer::object::preview::words_at;
use pxl_spec::bake::constant::EntryAt;
use std::collections::{HashMap, HashSet};

use pxl_spec::bake::place_theme::{DOT_ZOOM, FILL, ICON_ZOOM, LABEL_ZOOM, TEXT};
use pxl_spec::bake::place_tile::{symbol_of, PlaceTile, Placed};
use pxl_spec::space::layer::tile::Tile;

use crate::billboard::sheet::billboard_sheet;
use crate::io::text::{text_lines, text_width};
use crate::layer::Base;
use pxl_render::gpu::pass::badges::{badge_width, point_size, BADGE_FONT, POINT_FONT, POINT_WRAP};
use pxl_render::gpu::sheet::shelf::{DrawSpec, NO_SLOT};
use pxl_render::gpu::sheet::SHEET_SCALE;
use pxl_spec::block::marker::MARKER_SHOWN_FROM;
use xpute_guest::global::Global;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

/// A marker's pill and where it stands, down from its top.
const MARKER_HEIGHT: f64 = 37.0;
const MARKER_ANCHOR: f64 = 34.0;

/// How a place shows at a zoom: its icon or a dot, and whether its name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Look {
    pub icon: bool,
    pub named: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Label {
    /// A place by its index in the tile that holds it, as it looks.
    Place { tile: Tile, place: u32, look: Look },
    /// A marker someone put up, or one waiting for its words, by its entry,
    /// and its words by their hash.
    Marker { at: EntryAt, words: u64 },
}

impl Label {
    /// Gone once that tile has: another tile's index names another place.
    fn place(tile: Tile, place: u32) -> Option<Placed<'static>> {
        let placed = PlaceTile::residency().get(tile)?;
        (place < placed.len()).then(|| placed.get(place))
    }

    /// As it shows at Carto's `zoom` where it stands, if it does: a place
    /// from the zoom it is shown from, as a dot or its icon by its category's
    /// zooms, named from its category's label zoom; a marker from its own zoom
    /// (`block/marker.json`).
    pub fn at(self, zoom: i32) -> Option<Label> {
        match self {
            Label::Marker { .. } => (zoom >= i32::from(MARKER_SHOWN_FROM)).then_some(self),
            Label::Place { tile, place, .. } => {
                let h = Label::place(tile, place)?;
                let k = usize::from(h.place.category.checked_sub(1)?);
                let from = |z: u8| z != 0 && zoom >= i32::from(z);
                let (icon, dot) = (from(ICON_ZOOM[k]), from(DOT_ZOOM[k]));
                if zoom < i32::from(h.from) || !(icon || dot) {
                    return None;
                }
                let named = from(LABEL_ZOOM[k]) && !h.place.name.is_empty();
                Some(Label::Place {
                    tile,
                    place,
                    look: Look { icon, named },
                })
            }
        }
    }

    /// A marker's words, the waiting one's as they are so far, empty once
    /// it has gone.
    fn words(at: EntryAt) -> String {
        words_at(at).unwrap_or_default().to_string()
    }

    /// What it writes: a marker's words cut to `SHOWN_CHARS`; a place's name
    /// whole, where it is named.
    pub fn text(self) -> String {
        match self {
            Label::Marker { at, .. } => {
                let full = Label::words(at);
                let mut shown: String = full.chars().take(SHOWN_CHARS).collect();
                if full.chars().count() > SHOWN_CHARS {
                    shown.push('…');
                }
                shown
            }
            Label::Place { tile, place, look } if look.named => Label::place(tile, place).map(|h| h.place.name.to_string()).unwrap_or_default(),
            Label::Place { .. } => String::new(),
        }
    }
}

/// A product choice: the length a one-line badge still reads as a name at.
/// Code points, so a non-BMP character is never cut in half.
const SHOWN_CHARS: usize = 12;

struct Badge {
    size: [f64; 3],
    /// A place's name as it was laid out when sized: the host's layouts are
    /// a cache it may let go.
    lines: Vec<String>,
    slot: i32,
}
static BADGES: Global<HashMap<Label, Badge>> = Global::new();
fn badges() -> &'static mut HashMap<Label, Badge> {
    BADGES.get(HashMap::new)
}

pub fn retain(labels: &HashSet<Label>) {
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
}

/// Its width, height and where it stands down from its top; none until its
/// words are laid out, which a later turn has.
pub fn size(label: Label) -> Option<[f64; 3]> {
    if let Some(b) = badges().get(&label) {
        return Some(b.size);
    }
    let text = label.text();
    let (size, lines) = match label {
        Label::Marker { .. } => ([badge_width(text_width(BADGE_FONT, &text)?.ceil()), MARKER_HEIGHT, MARKER_ANCHOR], Vec::new()),
        Label::Place { .. } if text.is_empty() => (point_size(None), Vec::new()),
        Label::Place { .. } => {
            let laid = text_lines(POINT_FONT, POINT_WRAP, &text)?;
            let size = point_size(Some((laid.iter().map(|&(_, w)| w).fold(0.0, f64::max), laid.len())));
            (size, laid.iter().map(|&(l, _)| l.trim().to_owned()).collect())
        }
    };
    badges().insert(label, Badge { size, lines, slot: NO_SLOT });
    Some(size)
}

/// Whether the label is laid out and painted, so drawing it shows its words.
pub fn ready(label: Label) -> bool {
    size(label).is_some() && {
        let slot = slot(label);
        slot != NO_SLOT && billboard_sheet().painted(slot as u32)
    }
}

/// Its label was measured first.
pub fn slot(label: Label) -> i32 {
    let b = badges().get_mut(&label).or_bug(Errno::ENOTRECOVERABLE);
    if b.slot == NO_SLOT {
        let [w, h, _] = b.size;
        b.slot = billboard_sheet().add(
            w * SHEET_SCALE,
            h * SHEET_SCALE,
            match label {
                Label::Marker { .. } => DrawSpec::Pill { label: label.text(), logical_w: w },
                Label::Place { tile, place, look } => {
                    let category = Label::place(tile, place).map_or(0, |h| h.place.category);
                    let k = usize::from(category.saturating_sub(1));
                    DrawSpec::Point {
                        lines: b.lines.clone(),
                        icon: look.icon,
                        symbol: symbol_of(category),
                        fill: FILL[k],
                        text: TEXT[k],
                        logical_w: w,
                    }
                }
            },
        );
    }
    b.slot
}
