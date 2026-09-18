// pxl-kernel/layer/poi/store.rs

//! Page cache for POI, keyed by L3 page idx — POI's own counterpart to
//! terrain/store.rs, deliberately simpler: a single reader (the POI layer), so
//! no interests registry.

use pxl_spec::space::pvf3::ivec::IVec;
use xpute_runtime::sys::mem::free;

use crate::layer::poi::placement::poi_place_request;
use crate::layer::poi::repository::ResidentPoiPage;
use crate::sys::global::Global;
use crate::world::change::POI_PAGE;
use crate::world::page_table::PageTable;

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

pub fn get_pois(page_idx: IVec) -> Option<&'static ResidentPoiPage> {
    poi_pages().get(page_idx)
}

pub fn set_pois(page_idx: IVec, page: ResidentPoiPage) {
    if let Some(held) = poi_pages().clear(page_idx) {
        release(held);
    }
    poi_pages().set(page_idx, page);
    poi_place_request();
}

pub fn clear_pois(page_idx: IVec) {
    if let Some(held) = poi_pages().clear(page_idx) {
        release(held);
        poi_place_request();
    }
}

/// Gives a page's heap block back, with the page. POI is disconnected until
/// its per-page index is rewritten (issue/BACKLOG.md).
fn release(held: ResidentPoiPage) {
    if let Some(block) = held.block {
        // SAFETY: the page's own block, taken when it arrived.
        unsafe { free(block) };
    }
}
