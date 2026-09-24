// pxl-kernel/layer/poi/badge.rs

use std::collections::{HashMap, HashSet};

use pxl_spec::bake::poi_page::PoiPage;
use pxl_spec::space::pyramid::LevelPage;

use crate::global::Global;
use crate::host::measure_text;
use crate::layer::billboard::canvas::scale::FONT_FAMILY;
use crate::layer::billboard::sheet::billboard_sheet;
use pxl_render::asset::atlas::{DrawSpec, NO_SLOT};
use pxl_render::gpu::surface::DPR_MAX;

use crate::world::pyramid::Layer;

pub const HEIGHT: f64 = 37.0;
pub const ANCHOR: f64 = 34.0;

/// What a badge says, by what it is rather than by its text: a count, or the
/// one place a cell holds, by its name's index in the pool of the page it was
/// read from. The text is read only to measure the badge and to draw it
/// (`text`), and a badge is found again by these numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Label {
    Count(u32),
    Place { page: LevelPage, name: u32 },
}

impl Label {
    /// What the badge shows: the count, or the name cut where a badge would
    /// run past the rest.
    pub fn text(self) -> String {
        match self {
            Label::Count(n) => n.to_string(),
            Label::Place { page, name } => {
                let full = PoiPage::pyramid().get(page).map_or("", |p| p.names.get(name));
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
/// choice and not a derivation: long enough for nearly every place's name
/// whole, and short enough that one badge cannot cross the screen.
const SHOWN_CHARS: usize = 48;

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
            let font = format!("400 12px {FONT_FAMILY}");
            let width = (measure_text(&font, &label.text()).ceil() + 20.0).max(27.0);
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
            DrawSpec::Cluster {
                label: label.text(),
                logical_w: b.width,
            },
        );
    }
    b.slot
}
