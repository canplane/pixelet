// pxl_core/module/world/terrain/resident.rs

//! Which page each of the slab's records carries.
//!
//! A page's record is its own coordinate reduced modulo the slab's axis
//! (slab.rs), so a page arriving takes the record of whichever page shares its
//! residue. Inside the load radius no two pages do, so the page it replaces is
//! one the view has already left; a jump moves further than the axis and takes
//! the whole window with it, so the page it replaces is gone with the rest.
//! Either way the record is overwritten where it lies.
//!
//! That leaves one thing to keep: which page a record carries, so the store can
//! let go of a page whose bytes have been taken from under it. It replaces the
//! quarantine the pool needed — a released block could be handed to a different
//! page and written while an old reader still held it, and an address that is a
//! page's own coordinate cannot be handed to anyone else.

use pxl_shared::spec::space::pvf3::ivec::IVec;
use pxl_shared::spec::space::xy::TO_XY;

use super::slab::{slot_of, SLAB_SLOTS};
use crate::global::Global;

static SLOT_PAGE: Global<[Option<IVec>; SLAB_SLOTS as usize]> = Global::new();

fn slot_page() -> &'static mut [Option<IVec>; SLAB_SLOTS as usize] {
    SLOT_PAGE.get(|| [None; SLAB_SLOTS as usize])
}

/// Records that `page_idx` now holds its record, and answers with the page the
/// record carried before, whose bytes it has just taken.
pub fn take_slot(page_idx: IVec) -> Option<IVec> {
    let at = slot_of(page_idx) as usize;
    let held = slot_page()[at].replace(page_idx);
    held.filter(|p| TO_XY(*p) != TO_XY(page_idx))
}

/// Forgets `page_idx` where it still holds its record.
pub fn free_slot(page_idx: IVec) {
    let at = slot_of(page_idx) as usize;
    if slot_page()[at].is_some_and(|p| TO_XY(p) == TO_XY(page_idx)) {
        slot_page()[at] = None;
    }
}

/// Whether `page_idx` still holds the record its coordinate names.
pub fn holds_slot(page_idx: IVec) -> bool {
    slot_page()[slot_of(page_idx) as usize].is_some_and(|p| TO_XY(p) == TO_XY(page_idx))
}

#[cfg(test)]
#[path = "resident.test.rs"]
mod test;
