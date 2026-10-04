// pxl-kernel/layer/poi/badge.rs

use std::collections::{HashMap, HashSet};

use pxl_spec::bake::poi_page::{kind_color, kind_icon, Place, PoiPage};
use pxl_spec::space::pyramid::Page;

use crate::layer::billboard::sheet::billboard_sheet;
use crate::text::text_width;
use pxl_render::asset::atlas::{DrawSpec, NO_SLOT};
use pxl_render::gpu::pass::badges::{badge_width, BADGE_FONT};
use pxl_render::gpu::surface::DPR_MAX;
use xpute_runtime::global::Global;

use crate::world::pyramid::Layer;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

pub const HEIGHT: f64 = 37.0;
pub const ANCHOR: f64 = 34.0;

/// A count, or the one place a cell holds by its index in its page's pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Label {
    /// Count, and whether it is a floor.
    Count(u32, bool),
    Place {
        page: Page,
        place: u32,
    },
}

impl Label {
    /// Nameless once that page has gone.
    fn place(page: Page, place: u32) -> Place<'static> {
        PoiPage::pyramid().paging().get(page).map_or(Place::default(), |p| p.places.get(place))
    }

    pub fn text(self) -> String {
        match self {
            Label::Count(n, partial) => format!("{n}{}", if partial { "+" } else { "" }),
            Label::Place { page, place } => {
                let full = Label::place(page, place).name;
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
            b.width * DPR_MAX,
            HEIGHT * DPR_MAX,
            match label {
                Label::Count(..) => DrawSpec::Cluster {
                    label: label.text(),
                    logical_w: b.width,
                },
                Label::Place { page, place } => {
                    let kind = Label::place(page, place).kind;
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
