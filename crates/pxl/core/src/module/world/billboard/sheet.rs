// pxl_core/module/world/billboard/sheet.rs

//! The one sheet every billboard draws into — POI badges and the chat bubble
//! alike — and the one the renderer mirrors. One sheet is what lets every
//! billboard set share a pipeline and a binding: a second sheet would be a
//! second pass's worth of state for a bubble or two.
//!
//! And the billboard sets themselves (crates/pxl/core/src/module/world/billboard/field.rs),
//! reached here: what a set shows is written as instances of
//! BILLBOARD_FLOATS floats.

use super::atlas::BillboardAtlas;
pub use super::order::BILLBOARD_ORDER;
use crate::module::world::chunk_node::World;
use crate::sys::global::Global;
use crate::sys::kernel::kernel;

static SHEET: Global<BillboardAtlas> = Global::new();

/// `billboard_sheet`.
pub fn billboard_sheet() -> &'static mut BillboardAtlas {
    SHEET.get(BillboardAtlas::new)
}

/// A billboard set, empty and standing at the origin, drawn in `order`.
pub fn new_billboard_set(order: u32) -> u32 {
    kernel().sched.billboards.new_set(order)
}

/// Stands a set at `origin`, in units.
pub fn place_billboard_set(set: u32, origin: [f64; 3]) {
    kernel().sched.billboards.place(set, origin);
}

/// Stands a set `lift` units above wherever `actor` stands.
pub fn follow_billboard_set(set: u32, actor: u32, lift: f64) {
    kernel().sched.billboards.follow(set, actor, lift);
}

/// Replaces a set's instances with `lane`, BILLBOARD_FLOATS floats each.
pub fn write_billboard_set(set: u32, lane: &[f32]) {
    let k = kernel();
    let Some(volumes) = k.volumes.as_mut() else { return };
    let mut world = World {
        volumes,
        materials: &k.terrain,
        store: &k.objects,
        painter: &mut k.painter,
    };
    let sched = &mut k.sched;
    let count = (lane.len() / crate::module::world::billboard::field::BILLBOARD_FLOATS) as u32;
    // SAFETY: `lane` is `count` instances of f32s in the kernel's memory, read
    // before this returns.
    unsafe { sched.billboards.write(&mut world, &mut sched.draw, set, count, lane.as_ptr() as usize) };
}

pub fn remove_billboard_set(set: u32) {
    let k = kernel();
    let sched = &mut k.sched;
    sched.billboards.remove(&mut sched.draw, set);
}
