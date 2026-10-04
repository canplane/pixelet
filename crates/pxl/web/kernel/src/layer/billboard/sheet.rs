// pxl-kernel/layer/billboard/sheet.rs

//! The one sheet every billboard draws into, mirrored by the renderer, so
//! every set shares one pipeline and binding; and the sets themselves.

use pxl_render::asset::atlas::BillboardAtlas;
use pxl_spec::space::v3::V3;

use crate::global::Global;
use crate::host::handheld;
use crate::kernel::kernel;
pub use crate::layer::billboard::order::BillboardOrder;

static SHEET: Global<BillboardAtlas> = Global::new();

pub fn billboard_sheet() -> &'static mut BillboardAtlas {
    SHEET.get(|| BillboardAtlas::new(handheld()))
}

pub fn new_billboard_set(order: BillboardOrder) -> u32 {
    kernel().sched.billboards.new_set(order as u32)
}

pub fn place_billboard_set(set: u32, origin: V3) {
    kernel().sched.billboards.place(set, origin);
}

pub fn follow_billboard_set(set: u32, actor: u32, lift: f64) {
    kernel().sched.billboards.follow(set, actor, lift);
}

pub fn write_billboard_set(set: u32, lane: &[f32]) {
    let Some((_, sched)) = kernel().world() else { return };
    sched.billboards.write(&mut sched.draw, set, lane);
}

pub fn remove_billboard_set(set: u32) {
    let k = kernel();
    let sched = &mut k.sched;
    sched.billboards.remove(&mut sched.draw, set);
}
