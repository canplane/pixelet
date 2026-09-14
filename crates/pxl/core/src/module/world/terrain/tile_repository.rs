// pxl_core/module/world/terrain/tile_repository.rs

//! Terrain transport's kernel half: where a page is fetched from, and the
//! page the shell decoded and copied into the core memory opened as views.
//! The fetch and the decode are the shell's.
//!
//! The pre-bake pass skips pure open-ocean pages entirely — about 65% of the
//! coverage bbox — so the static bucket has no object for them at all. A 404
//! there isn't a failure, it's the signal for "nothing but water here", and
//! every such page resolves to one shared tile: baking an empty source at
//! different pages was verified to produce identical bytes, so a single decode
//! can be aliased by all of them. Only reachable when a static tile origin is
//! configured; against the live API a 404 is a genuine error.

use pxl_shared::bake::constant::{ELE_PADDED_SPAN, PAGE_CELL_SPAN};
use pxl_shared::bake::format::{Ground, Terrain};
use xpute_core::kit::array::{ArrayBuffer, I16Array, U8Array};

use crate::lib::env::config::TILE_BASE_URL;

/// Object key of the single shared open-ocean tile in the static bucket.
pub const OCEAN_KEY: &str = "_ocean";

pub fn tile_url(key: &str) -> String {
    format!("{}/{key}", TILE_BASE_URL())
}

/// A page's lanes where the shell copied them: land, water, structure and
/// transport, each PAGE_CELL_SPAN² bytes, and the padded elevation grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainLanes {
    pub ground: [usize; 4],
    pub ele: usize,
}

/// Opens a page's lanes as the terrain the kernel reads.
///
/// # Safety
/// Every lane is a block of the kernel's memory the shell holds for this page
/// for as long as the page is resident, at the page's fixed lane sizes.
pub unsafe fn open_terrain(lanes: TerrainLanes) -> Terrain {
    let cells = (PAGE_CELL_SPAN * PAGE_CELL_SPAN) as usize;
    let samples = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
    let u8_lane = |at: usize| {
        // SAFETY: the caller's contract.
        let buffer = unsafe { ArrayBuffer::from_raw(at as *mut u8, cells) };
        U8Array::over(buffer, 0, cells)
    };
    // SAFETY: the caller's contract; an i16 lane is 2 bytes a sample.
    let ele_buffer = unsafe { ArrayBuffer::from_raw(lanes.ele as *mut u8, samples * 2) };
    Terrain {
        ground: Ground {
            land: u8_lane(lanes.ground[0]),
            water: u8_lane(lanes.ground[1]),
            structure: u8_lane(lanes.ground[2]),
            transport: u8_lane(lanes.ground[3]),
        },
        ele: I16Array::over(ele_buffer, 0, samples),
    }
}
