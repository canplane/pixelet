// pxl-kernel/layer/billboard/sheet.rs

//! The one sheet every billboard draws into — POI badges and the chat bubble
//! alike — and the one the renderer mirrors. One sheet is what lets every
//! billboard set share a pipeline and a binding: a second sheet would be a
//! second pass's worth of state for a bubble or two.
//!
//! And the billboard sets themselves (crates/pxl/web/kernel/src/layer/billboard/field.rs),
//! reached here: what a set shows is written as instances of
//! BILLBOARD_FLOATS floats.

use pxl_render::asset::atlas::BillboardAtlas;
use pxl_spec::space::v3::V3;

use crate::global::Global;
use crate::host::handheld;
use crate::kernel::kernel;
pub use crate::layer::billboard::order::BillboardOrder;

static SHEET: Global<BillboardAtlas> = Global::new();

/// `billboard_sheet`.
pub fn billboard_sheet() -> &'static mut BillboardAtlas {
    SHEET.get(|| BillboardAtlas::new(handheld()))
}

/// A billboard set, empty and standing at the origin, drawn in `order`.
pub fn new_billboard_set(order: BillboardOrder) -> u32 {
    kernel().sched.billboards.new_set(order as u32)
}

/// Stands a set at `origin`, in units.
pub fn place_billboard_set(set: u32, origin: V3) {
    kernel().sched.billboards.place(set, origin);
}

/// Stands a set `lift` units above wherever `actor` stands.
pub fn follow_billboard_set(set: u32, actor: u32, lift: f64) {
    kernel().sched.billboards.follow(set, actor, lift);
}

/// Replaces a set's instances with `lane`, BILLBOARD_FLOATS floats each.
pub fn write_billboard_set(set: u32, lane: &[f32]) {
    let Some((mut world, sched, _)) = kernel().world() else { return };
    let count = (lane.len() / crate::layer::billboard::field::BILLBOARD_FLOATS) as u32;
    // SAFETY: `lane` is `count` instances of f32s in the kernel's memory, read
    // before this returns.
    unsafe { sched.billboards.write(&mut world, &mut sched.draw, set, count, lane.as_ptr() as usize) };
}

pub fn remove_billboard_set(set: u32) {
    let k = kernel();
    let sched = &mut k.sched;
    sched.billboards.remove(&mut sched.draw, set);
}
