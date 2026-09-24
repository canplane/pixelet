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
    label: String,
    width: f64,
    slot: i32,
}
static BADGES: Global<Vec<Badge>> = Global::new();
fn badges() -> &'static mut Vec<Badge> {
    BADGES.get(Vec::new)
}

pub fn retain(labels: &[String]) {
    badges().retain(|b| {
        if labels.contains(&b.label) {
            return true;
        }
        if b.slot != NO_SLOT {
            billboard_sheet().release(b.slot as u32);
        }
        false
    });
}

pub fn width(label: &str) -> f64 {
    if let Some(b) = badges().iter().find(|b| b.label == label) {
        return b.width;
    }
    let font = format!("400 12px {FONT_FAMILY}");
    let width = (measure_text(&font, label).ceil() + 20.0).max(27.0);
    badges().push(Badge {
        label: label.to_owned(),
        width,
        slot: NO_SLOT,
    });
    width
}

pub fn slot(label: &str) -> i32 {
    width(label);
    let b = badges().iter_mut().find(|b| b.label == label).unwrap();
    if b.slot == NO_SLOT {
        b.slot = billboard_sheet().add(
            b.width * DPR_MAX,
            HEIGHT * DPR_MAX,
            DrawSpec::Cluster {
                label: label.to_owned(),
                logical_w: b.width,
            },
        );
    }
    b.slot
}
