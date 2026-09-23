// pxl-kernel/layer/terrain/resident.rs

//! Which page each of `.fixed.terrain`'s records carries.
//!
//! A page's record is its level's group and its coordinate reduced modulo the
//! ring's axis (fixed.rs), so a page arriving takes the record of whichever
//! page shares its residue. Inside the window no two held pages do, so the
//! page it replaces is one the view has already left; a jump moves further
//! than the axis and takes the whole ring with it, so the page it replaces is
//! gone with the rest. Either way the record is overwritten where it lies.
//!
//! That leaves one thing to keep: which page a record carries, so the store can
//! let go of a page whose bytes have been taken from under it. No quarantine is
//! needed: an allocated block, once released, could be handed to a different
//! page and written while an old reader still held it, but an address that is a
//! page's own coordinate cannot be handed to anyone else.

use crate::global::Global;
use crate::layer::terrain::fixed::{slot_of, TERRAIN_SLOTS};
use crate::layer::terrain::level::LevelPage;

static SLOT_PAGE: Global<[Option<LevelPage>; TERRAIN_SLOTS as usize]> = Global::new();

fn slot_page() -> &'static mut [Option<LevelPage>; TERRAIN_SLOTS as usize] {
    SLOT_PAGE.get(|| [None; TERRAIN_SLOTS as usize])
}

/// Records that `p` now holds its record, and answers with the page the record
/// carried before, whose bytes it has just taken.
pub fn take_slot(p: LevelPage) -> Option<LevelPage> {
    let at = slot_of(p) as usize;
    slot_page()[at].replace(p).filter(|held| *held != p)
}

/// Forgets `p` where it still holds its record.
pub fn free_slot(p: LevelPage) {
    let at = slot_of(p) as usize;
    if slot_page()[at] == Some(p) {
        slot_page()[at] = None;
    }
}

/// The page holding the record `p`'s coordinate names, if any.
pub fn holder(p: LevelPage) -> Option<LevelPage> {
    slot_page()[slot_of(p) as usize]
}

/// Whether `p` still holds the record its coordinate names.
pub fn holds_slot(p: LevelPage) -> bool {
    slot_page()[slot_of(p) as usize] == Some(p)
}

#[cfg(test)]
#[path = "resident.test.rs"]
mod test;
