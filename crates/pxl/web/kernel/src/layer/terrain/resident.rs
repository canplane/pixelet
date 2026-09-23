// pxl-kernel/layer/terrain/resident.rs

//! Which page each of `.fixed.terrain`'s records carries, and which records
//! carry nothing.
//!
//! A record is a page's room, not a page's address. Any page can sit in any
//! record, so what the range holds is decided by a policy
//! (tile_repository.rs's `victim`) rather than by an arriving page's
//! coordinate. That is the whole of the change here, and the reason for it:
//! with the coordinate deciding, the set of pages the range *could* hold was
//! the set the camera wanted, and a page the view had left had nowhere to be
//! — a ring page sharing its residue took its bytes the moment it arrived.
//! Pages the camera has left are exactly what a zoom back, a pan back and a
//! fold are made of.
//!
//! The range is the size it always was. What changed is that its occupancy
//! follows the camera's *demand* rather than the camera's *arithmetic*: a
//! view holding two levels leaves three levels' worth of records to whatever
//! was resident before, where the residue left none. A view holding all
//! WINDOW_LEVELS leaves none either way — the range has a record per page of
//! the window's rings and not one more — so what survives a camera move is
//! what the window has room for, and no promise is made past that.
//!
//! No quarantine is needed, and the reason is not the addressing. A record's
//! bytes are read through the `Terrain` slices the store hands out, and none
//! of those crosses a turn: a reader takes one, reads it and drops it inside
//! one synchronous step, while a page is written in `fetch_task`, which runs
//! only where another task has yielded. What protects a reader of the *page*
//! is the policy — the record taken carries a page the view has left and
//! something else already draws.

use crate::global::Global;
use crate::layer::terrain::fixed::TERRAIN_SLOTS;
use crate::layer::terrain::level::LevelPage;

/// Records in the range. Every one can carry any page.
pub const RECORDS: u32 = TERRAIN_SLOTS;

static RECORD_PAGE: Global<[Option<LevelPage>; RECORDS as usize]> = Global::new();

fn record_page() -> &'static mut [Option<LevelPage>; RECORDS as usize] {
    RECORD_PAGE.get(|| [None; RECORDS as usize])
}

/// The record `p` is resident in, if it is.
pub fn record_of(p: LevelPage) -> Option<u32> {
    record_page().iter().position(|held| *held == Some(p)).map(|at| at as u32)
}

/// The page a record carries, if any.
pub fn page_in(record: u32) -> Option<LevelPage> {
    record_page()[record as usize]
}

/// A record carrying nothing, if there is one.
pub fn free_record() -> Option<u32> {
    record_page().iter().position(|held| held.is_none()).map(|at| at as u32)
}

/// Gives `record` to `p`, and answers with the page it is taken from. The
/// caller writes `p`'s bytes there, so a displaced page must be one the
/// caller has already decided can go.
///
/// There is no matching free, and that is the point rather than an omission:
/// a record is only ever handed from one page to the next, so nothing can
/// hold a record it is not in, and the range cannot be emptied by anything —
/// a camera move, a zoom, a reset — that is not another page arriving. Every
/// record is free once, at boot.
pub fn put(p: LevelPage, record: u32) -> Option<LevelPage> {
    record_page()[record as usize].replace(p).filter(|held| *held != p)
}

/// Whether `p` is resident in the range.
pub fn holds_record(p: LevelPage) -> bool {
    record_of(p).is_some()
}

/// Test-only: empties the range, so each test starts against records that
/// carry nothing. Nothing in the kernel frees a record.
#[cfg(test)]
pub fn forget_all() {
    *record_page() = [None; RECORDS as usize];
}

#[cfg(test)]
#[path = "resident.test.rs"]
mod test;
