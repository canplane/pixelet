// pxl-kernel/layer/terrain/store.rs

//! Central page cache keyed by L3 page idx.
//!
//! Owns:
//! - coarse page payload storage
//! - page replacement / eviction
//!
//! Does not own:
//! - page idx derivation
//! - fine-grained merge
//! - render / stream scheduling
//!
//! The pages live in `tile_pages` (page_table.rs): the key index, the page's
//! typed views as handles. The store's revision is the table's, for a reader
//! that wants to see a page come and go.

use pxl_spec::bake::format::Tile;
use pxl_spec::space::pvf3::ivec::IVec;

use crate::layer::poi::index::invalidate_poi_index;
use crate::layer::poi::placement::poi_place_request;
use crate::layer::terrain::ground_snap::reground_map_center;
use crate::sys::global::Global;
use crate::world::change::TERRAIN_PAGE;
use crate::world::chunk::node::{chunk_page, chunk_sources_changed};
use crate::world::page_table::PageTable;

static TILE_PAGES: Global<PageTable<Tile>> = Global::new();

/// Every resident terrain page. Walk it or look a page up here; change it
/// through the store so the change log hears.
pub fn tile_pages() -> &'static mut PageTable<Tile> {
    TILE_PAGES.get(|| PageTable::new(Some(TERRAIN_PAGE)))
}

pub struct TileStore;

impl TileStore {
    pub fn get_tile(&self, page_idx: IVec) -> Option<&'static Tile> {
        tile_pages().get(page_idx)
    }

    /// A page arrived: the scheduler mounts from it, the chunks and the POI
    /// placement read it, and a map centre that could not be grounded before
    /// may be now.
    pub fn set_tile(&self, page_idx: IVec, tile: Tile) {
        tile_pages().set(page_idx, tile);
        chunk_page(page_idx, true);
        chunk_sources_changed();
        invalidate_poi_index();
        poi_place_request();
        reground_map_center();
    }

    pub fn clear_tile(&self, page_idx: IVec) -> Option<Tile> {
        let held = tile_pages().clear(page_idx)?;
        chunk_page(page_idx, false);
        chunk_sources_changed();
        invalidate_poi_index();
        poi_place_request();
        Some(held)
    }
}

pub fn tile_store() -> TileStore {
    TileStore
}
