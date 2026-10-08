// pxl-kernel/chunk/place/badge.rs

use crate::layer::object::ext::Ext;
use pxl_spec::bake::constant::EntryAt;
use std::collections::{HashMap, HashSet};

use pxl_spec::bake::poi_tile::{kind_color, kind_icon, Place, PoiTile, POI_LATTICE};
use pxl_spec::space::coord::pvf3::ivec;
use pxl_spec::space::layer::tile::Tile;

use crate::billboard::sheet::billboard_sheet;
use crate::io::text::text_width;
use pxl_render::gpu::pass::badges::{badge_width, BADGE_FONT};
use pxl_render::gpu::sheet::shelf::{DrawSpec, NO_SLOT};
use pxl_render::gpu::sheet::SHEET_SCALE;
use xpute_guest::global::Global;

use crate::layer::{Answer, Layer, Query};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

pub const HEIGHT: f64 = 37.0;
pub const ANCHOR: f64 = 34.0;

/// A count, or the one place a cell holds by its index in its tile's pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Label {
    /// Count, and whether it is a floor.
    Count(u32, bool),
    Place {
        tile: Tile,
        place: u32,
    },
    /// A marker someone put up, by its entry, and its words by their hash, so
    /// words changed under one entry are another badge.
    Marker { at: EntryAt, words: u64 },
}

impl Label {
    /// Nameless once that tile has gone.
    fn place(tile: Tile, place: u32) -> Place<'static> {
        // Read as the tile itself answers, at its own cells: another tile's place index names another place.
        let node = (tile.zoom, [ivec::X(tile.tile), ivec::Y(tile.tile), 0]);
        match PoiTile::residency().read(node, POI_LATTICE.grid_zoom(tile.zoom)) {
            Answer::Exact(p) if p.from == tile => p.tile.places.get(place),
            _ => Place::default(),
        }
    }

    /// A marker's words, empty once it has gone.
    fn words(at: EntryAt) -> String {
        let objects = &crate::kernel().objects;
        match objects.placed(at.zoom(), at.at()).and_then(|e| objects.ext(&e).cloned()) {
            Some(Ext::MarkerText(t)) => t,
            None => String::new(),
        }
    }

    pub fn text(self) -> String {
        let full = match self {
            Label::Count(n, partial) => return format!("{n}{}", if partial { "+" } else { "" }),
            Label::Marker { at, .. } => Label::words(at),
            Label::Place { tile, place } => Label::place(tile, place).name.to_string(),
        };
        let mut shown: String = full.chars().take(SHOWN_CHARS).collect();
        if full.chars().count() > SHOWN_CHARS {
            shown.push('…');
        }
        shown
    }
}

/// A product choice: the length a one-line badge still reads as a name at.
/// Code points, so a non-BMP character is never cut in half.
const SHOWN_CHARS: usize = 12;

struct Badge {
    width: f64,
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

/// None until its label is laid out, which a later turn has.
pub fn width(label: Label) -> Option<f64> {
    if let Some(b) = badges().get(&label) {
        return Some(b.width);
    }
    let width = badge_width(text_width(BADGE_FONT, &label.text())?.ceil(), matches!(label, Label::Place { .. }));
    badges().insert(label, Badge { width, slot: NO_SLOT });
    Some(width)
}

/// Its label was measured first.
pub fn slot(label: Label) -> i32 {
    let b = badges().get_mut(&label).or_bug(Errno::ENOTRECOVERABLE);
    if b.slot == NO_SLOT {
        b.slot = billboard_sheet().add(
            b.width * SHEET_SCALE,
            HEIGHT * SHEET_SCALE,
            match label {
                Label::Count(..) | Label::Marker { .. } => DrawSpec::Pill {
                    label: label.text(),
                    logical_w: b.width,
                },
                Label::Place { tile, place } => {
                    let kind = Label::place(tile, place).kind;
                    DrawSpec::Place {
                        name: label.text(),
                        icon: kind_icon(kind),
                        color: kind_color(kind),
                        logical_w: b.width,
                    }
                }
            },
        );
    }
    b.slot
}
