// pxl-kernel/layer/poi/badge.rs

use crate::global::Global;
use crate::host::measure_text;
use crate::layer::billboard::canvas::scale::FONT_FAMILY;
use crate::layer::billboard::sheet::billboard_sheet;
use pxl_render::asset::atlas::{DrawSpec, NO_SLOT};
use pxl_render::gpu::surface::DPR_MAX;

pub const HEIGHT: f64 = 37.0;
pub const COLLIDE: f64 = 27.0;
pub const ANCHOR: f64 = 34.0;

struct Badge {
    count: u32,
    width: f64,
    slot: i32,
}
static BADGES: Global<Vec<Badge>> = Global::new();
fn badges() -> &'static mut Vec<Badge> {
    BADGES.get(Vec::new)
}

/// Keeps the badges of `counts` and lets the rest's sheet slots go.
pub fn retain(counts: &[u32]) {
    badges().retain(|b| {
        if counts.contains(&b.count) {
            return true;
        }
        if b.slot != NO_SLOT {
            billboard_sheet().release(b.slot as u32);
        }
        false
    });
}

/// A count's badge width, measured once.
pub fn width(count: u32) -> f64 {
    if let Some(b) = badges().iter().find(|b| b.count == count) {
        return b.width;
    }
    let font = format!("400 12px {FONT_FAMILY}");
    let width = (measure_text(&font, &count.to_string()).ceil() + 20.0).max(27.0);
    badges().push(Badge { count, width, slot: NO_SLOT });
    width
}

/// A count's badge on the sheet, drawn once.
pub fn slot(count: u32) -> i32 {
    width(count);
    let b = badges().iter_mut().find(|b| b.count == count).unwrap();
    if b.slot == NO_SLOT {
        b.slot = billboard_sheet().add(
            b.width * DPR_MAX,
            HEIGHT * DPR_MAX,
            DrawSpec::Cluster {
                label: count.to_string(),
                logical_w: b.width,
            },
        );
    }
    b.slot
}
