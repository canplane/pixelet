// pxl-kernel/billboard/sheet.rs

//! The one sheet every billboard draws into, mirrored by the renderer, so
//! every set shares one pipeline and binding; and the sets themselves.

use crate::laid;
use pxl_render::asset::atlas::BillboardAtlas;
use pxl_spec::space::coord::v3::V3;

use crate::kernel;
use xpute_guest::global::Global;

static SHEET: Global<BillboardAtlas> = Global::new();

pub fn billboard_sheet() -> &'static mut BillboardAtlas {
    SHEET.get(BillboardAtlas::new)
}

pub fn new_billboard_set(order: BillboardOrder) -> u32 {
    kernel().billboards.new_set(order as u32)
}

pub fn place_billboard_set(set: u32, origin: V3) {
    kernel().billboards.place(set, origin);
}

pub fn follow_billboard_set(set: u32, actor: u32, lift: f64) {
    kernel().billboards.follow(set, actor, lift);
}

pub fn write_billboard_set(set: u32, lane: &[f32]) {
    let Some(k) = laid() else { return };
    k.billboards.write(&mut k.draw, set, lane);
}

pub fn remove_billboard_set(set: u32) {
    let k = kernel();
    k.billboards.remove(&mut k.draw, set);
}

/// Draw order among billboard sets, only for ties on the same pixels at the
/// same depth: the transient bubble wins over the badge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum BillboardOrder {
    Poi = 0,
    Chat = 1,
}

/// One set per order, each owner letting its set go before making another.
pub const SETS_MAX: usize = 2;
