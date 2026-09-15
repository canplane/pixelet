// pxl_core/module/world/terrain/config.rs

//! Tile streaming, in world units (config.json says why each knob is where it
//! is).
//!
//! The page stays the unit of transfer and storage: one PostGIS query and bake
//! on the API side, one ~4MB decode, one table key, one eviction. What is
//! derived rather than chosen is *which* pages: the nearest RESIDENT_PAGES to
//! the view anchor (module/world/frame/range.rs's pages_to_hold), which is at
//! once what gets fetched and what stays decoded.

use pxl_shared::bake::constant::{ELE_PADDED_SPAN, PAGE_CELL_SPAN};
use pxl_shared::bake::format::GROUND_CH_KEYS;

use super::config_spec;

/// What one resident page takes of the page section: its ground lanes and its
/// elevation block, each rounded up to the buddy's power of two.
fn pow2_ceil(bytes: u32) -> u32 {
    bytes.max(1).next_power_of_two()
}

fn page_block_bytes() -> u32 {
    GROUND_CH_KEYS.len() as u32 * pow2_ceil(PAGE_CELL_SPAN * PAGE_CELL_SPAN) + pow2_ceil(ELE_PADDED_SPAN * ELE_PADDED_SPAN * 2)
}

/// How many pages stay decoded: the nearest this many, and the rest are
/// dropped. Never more than the page section holds (crate::memory), less
/// the margin: a page the section refuses is a page whose chunks are never
/// built — so the count is bounded here by construction.
#[allow(non_snake_case)]
pub fn RESIDENT_PAGES() -> u32 {
    let section = crate::memory::map().map_or(u32::MAX, |m| m.page.bytes);
    config_spec::RESIDENT_PAGES_CAP.min((section / page_block_bytes()).saturating_sub(config_spec::PAGE_MARGIN))
}

/// How far to look when picking those pages.
pub const KEEP_DIST: f64 = config_spec::KEEP_DIST;
