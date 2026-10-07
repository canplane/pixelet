// pxl-kernel/chunk/place/badge.rs

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

    pub fn text(self) -> String {
        match self {
            Label::Count(n, partial) => format!("{n}{}", if partial { "+" } else { "" }),
            Label::Place { tile, place } => {
                let full = Label::place(tile, place).name;
                let mut shown: String = full.chars().take(SHOWN_CHARS).collect();
                if full.chars().count() > SHOWN_CHARS {
                    shown.push('…');
                }
                shown
            }
        }
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
                Label::Count(..) => DrawSpec::Cluster {
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
