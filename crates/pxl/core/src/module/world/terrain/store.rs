// pxl_core/module/world/terrain/store.rs

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

use pxl_shared::bake::format::Tile;
use pxl_shared::spec::space::pvf3::ivec::IVec;

use crate::global::Global;
use crate::module::world::change::TERRAIN_PAGE;
use crate::module::world::page_table::PageTable;

static TILE_PAGES: Global<PageTable<Tile>> = Global::new();

/// Every resident terrain page. Walk it or look a page up here; change it
/// through the store so the change log hears.
pub fn tile_pages() -> &'static mut PageTable<Tile> {
    TILE_PAGES.get(|| PageTable::new(Some(TERRAIN_PAGE)))
}

pub struct TileStore;

impl TileStore {
    /// tile_pages.revision, so a reader sees a page come and go.
    pub fn revision(&self) -> u32 {
        tile_pages().revision()
    }

    pub fn get_tile(&self, page_idx: IVec) -> Option<&'static Tile> {
        tile_pages().get(page_idx)
    }

    pub fn set_tile(&self, page_idx: IVec, tile: Tile) {
        tile_pages().set(page_idx, tile);
    }

    pub fn clear_tile(&self, page_idx: IVec) -> Option<Tile> {
        tile_pages().clear(page_idx)
    }
}

/// `useTileStore.getState()`.
pub fn use_tile_store() -> TileStore {
    TileStore
}
