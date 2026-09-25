// pxl-kernel/layer/poi/badge.rs

use std::collections::{HashMap, HashSet};

use pxl_spec::bake::poi_page::{theme, Place, PoiPage};
use pxl_spec::space::pyramid::LevelPage;

use crate::global::Global;
use crate::host::measure_text;
use crate::layer::billboard::sheet::billboard_sheet;
use pxl_render::asset::atlas::{DrawSpec, NO_SLOT};
use pxl_render::gpu::pass::badges::{badge_width, BADGE_FONT};
use pxl_render::gpu::surface::DPR_MAX;

use crate::world::pyramid::Layer;

pub const HEIGHT: f64 = 37.0;
pub const ANCHOR: f64 = 34.0;

/// What a badge says, by what it is rather than by its text: a count, or the
/// one place a cell holds, by its index in the pool of the page it was read
/// from. The place is read only to measure the badge and to draw it (`text`,
/// `slot`), and a badge is found again by these numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Label {
    Count(u32),
    Place { page: LevelPage, place: u32 },
}

impl Label {
    /// The place a badge names, from the page it was read from; nameless and
    /// of no kind once that page has gone.
    fn place(page: LevelPage, place: u32) -> Place<'static> {
        PoiPage::pyramid().get(page).map_or(Place { name: "", kind: 0 }, |p| p.places.get(place))
    }

    /// What the badge shows: the count, or the name cut where a badge would
    /// run past the rest.
    pub fn text(self) -> String {
        match self {
            Label::Count(n) => n.to_string(),
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

/// How much of a name a badge shows, the rest behind an ellipsis. A product
/// choice and not a derivation: the length a one-line badge still reads as a
/// name at. Counted in code points, so a character outside the BMP is one
/// and is never cut in half, as a UTF-16 count would; a cluster of several,
/// a flag or a joined emoji, can still be cut, and is rare in a place's name.
const SHOWN_CHARS: usize = 12;

struct Badge {
    width: f64,
    slot: i32,
}
static BADGES: Global<HashMap<Label, Badge>> = Global::new();
fn badges() -> &'static mut HashMap<Label, Badge> {
    BADGES.get(HashMap::new)
}

/// Keeps the badges of `labels` and lets the rest's sheet slots go.
pub fn retain(labels: &HashSet<Label>) {
    badges().retain(|label, b| {
        if labels.contains(label) {
            return true;
        }
        if b.slot != NO_SLOT {
            billboard_sheet().release(b.slot as u32);
        }
        false
    });
}

/// A badge's width, measured once.
pub fn width(label: Label) -> f64 {
    badges()
        .entry(label)
        .or_insert_with(|| {
            let width = badge_width(measure_text(BADGE_FONT, &label.text()).ceil(), matches!(label, Label::Place { .. }));
            Badge { width, slot: NO_SLOT }
        })
        .width
}

/// A badge on the sheet, drawn once.
pub fn slot(label: Label) -> i32 {
    width(label);
    let b = badges().get_mut(&label).expect("measured above");
    if b.slot == NO_SLOT {
        b.slot = billboard_sheet().add(
            b.width * DPR_MAX,
            HEIGHT * DPR_MAX,
            match label {
                Label::Count(_) => DrawSpec::Cluster {
                    label: label.text(),
                    logical_w: b.width,
                },
                Label::Place { page, place } => {
                    let t = theme(Label::place(page, place).kind);
                    DrawSpec::Place {
                        name: label.text(),
                        icon: t.icon,
                        color: t.color,
                        logical_w: b.width,
                    }
                }
            },
        );
    }
    b.slot
}
