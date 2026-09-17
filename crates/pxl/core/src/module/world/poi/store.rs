// pxl_core/module/world/poi/store.rs

//! Page cache for POI, keyed by L3 page idx — POI's own counterpart to
//! terrain/store.rs, deliberately simpler: a single reader (the POI layer), so
//! no interests registry.

use pxl_shared::spec::space::pvf3::ivec::IVec;
use xpute_core::sys::mem::free;

use super::repository::ResidentPoiPage;
use crate::module::world::change::POI_PAGE;
use crate::module::world::page_table::PageTable;
use crate::sys::global::Global;

struct PoiStore {
    pages: PageTable<ResidentPoiPage>,
}

static STORE: Global<PoiStore> = Global::new();

fn store() -> &'static mut PoiStore {
    STORE.get(|| PoiStore {
        pages: PageTable::new(Some(POI_PAGE)),
    })
}

/// Every resident POI page.
pub fn poi_pages() -> &'static mut PageTable<ResidentPoiPage> {
    &mut store().pages
}

/// poi_pages' revision, so a reader sees a page come and go.
pub fn revision() -> u32 {
    poi_pages().revision()
}

pub fn get_pois(page_idx: IVec) -> Option<&'static ResidentPoiPage> {
    poi_pages().get(page_idx)
}

pub fn set_pois(page_idx: IVec, page: ResidentPoiPage) {
    if let Some(held) = poi_pages().clear(page_idx) {
        release(held);
    }
    poi_pages().set(page_idx, page);
}

pub fn clear_pois(page_idx: IVec) {
    if let Some(held) = poi_pages().clear(page_idx) {
        release(held);
    }
}

/// Gives a page's block back, with the page. The mesh section holds it: POI
/// is disconnected until its per-page index is rewritten (issue/BACKLOG.md),
/// and the page section is `.fixed.terrain` now.
fn release(held: ResidentPoiPage) {
    if let Some(block) = held.block {
        // SAFETY: the page's own block, taken when it arrived.
        unsafe { free(block) };
    }
}
