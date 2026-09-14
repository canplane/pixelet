// pxl_core/module/world/poi/store.rs

//! Page cache for POI, keyed by L3 page idx — POI's own counterpart to
//! terrain/store.rs, deliberately simpler: a single reader (the POI layer), so
//! no interests registry.

use pxl_shared::spec::space::pvf3::ivec::IVec;

use super::repository::{Block, ResidentPoiPage};
use crate::global::Global;
use crate::module::world::change::POI_PAGE;
use crate::module::world::page_table::PageTable;

struct PoiStore {
    pages: PageTable<ResidentPoiPage>,
    /// Blocks of pages that left, for the shell to give back to the page pool.
    released: Vec<Block>,
}

static STORE: Global<PoiStore> = Global::new();

fn store() -> &'static mut PoiStore {
    STORE.get(|| PoiStore {
        pages: PageTable::new(Some(POI_PAGE)),
        released: Vec::new(),
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

fn release(held: ResidentPoiPage) {
    if let Some(block) = held.block {
        store().released.push(block);
    }
}

/// The blocks released since the last take: the shell's to give back.
pub fn take_released_blocks() -> Vec<Block> {
    std::mem::take(&mut store().released)
}
