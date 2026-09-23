// pxl-kernel/layer/terrain/store.rs

//! Central page cache: a table a level, keyed by the page's coordinate at its
//! level.
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
//! The pages live in page tables (page_table.rs): the key index, the page's
//! typed views as handles. The leaves' table carries the terrain topic, so its
//! revision is what a reader of the ground compares; the levels above are the
//! view's alone and are read through the chunks that draw them.

use pxl_spec::bake::constant::PAGE_TOP_LEVEL;
use pxl_spec::bake::format::Tile;
use pxl_spec::space::pvf3::ivec::IVec;

use crate::global::Global;
use crate::layer::poi::index::invalidate_poi_index;
use crate::layer::poi::placement::poi_place_request;
use crate::layer::terrain::ground_snap::reground_map_center;
use crate::layer::terrain::level::LevelPage;
use crate::world::change::TERRAIN_PAGE;
use crate::world::chunk::node::{chunk_page, chunk_sources_changed};
use crate::world::page_table::PageTable;

static LEVEL_PAGES: Global<Vec<PageTable<Tile>>> = Global::new();

fn tables() -> &'static mut Vec<PageTable<Tile>> {
    LEVEL_PAGES.get(|| (0..=PAGE_TOP_LEVEL).map(|level| PageTable::new((level == 0).then_some(TERRAIN_PAGE))).collect())
}

/// Every resident leaf. Walk it or look a page up here; change it through the
/// store so the change log hears.
pub fn tile_pages() -> &'static mut PageTable<Tile> {
    level_pages(0)
}

/// Every resident page of `level`.
pub fn level_pages(level: u32) -> &'static mut PageTable<Tile> {
    &mut tables()[level as usize]
}

pub struct TileStore;

impl TileStore {
    /// A leaf.
    pub fn get_tile(&self, page_idx: IVec) -> Option<&'static Tile> {
        tile_pages().get(page_idx)
    }

    /// None past the pyramid's top too, which is what a top page's parent is.
    pub fn get_page(&self, p: LevelPage) -> Option<&'static Tile> {
        tables().get(p.level as usize)?.get(p.page)
    }

    /// A page arrived. A leaf is what the scheduler mounts from, the chunks
    /// and the POI placement read, and what a map centre that could not be
    /// grounded before may be grounded on now.
    pub fn set_page(&self, p: LevelPage, tile: Tile) {
        level_pages(p.level).set(p.page, tile);
        chunk_page(p, true);
        if p.level == 0 {
            chunk_sources_changed();
            invalidate_poi_index();
            poi_place_request();
            reground_map_center();
        }
    }

    pub fn clear_page(&self, p: LevelPage) -> Option<Tile> {
        let held = level_pages(p.level).clear(p.page)?;
        chunk_page(p, false);
        if p.level == 0 {
            chunk_sources_changed();
            invalidate_poi_index();
            poi_place_request();
        }
        Some(held)
    }
}

pub fn tile_store() -> TileStore {
    TileStore
}
