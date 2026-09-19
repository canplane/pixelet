// pxl-kernel/layer/terrain/config.rs

//! Tile streaming, in world units (config.json says why each knob is where it
//! is).
//!
//! The page stays the unit of transfer and storage: one PostGIS query and bake
//! on the API side, one ~4MB decode, one table key, one eviction. What is
//! derived rather than chosen is *which* pages: the nearest RESIDENT_PAGES to
//! the view anchor (world/chunk/range.rs's pages_to_hold), which is at
//! once what gets fetched and what stays decoded.

use crate::layer::terrain::config_spec;

/// How many pages stay decoded: every record `.fixed.terrain` has, which is every
/// page the load radius reaches. Not a count to be chosen and not a division
/// to be measured — the range is the set (fixed.rs).
pub const RESIDENT_PAGES: u32 = crate::layer::terrain::fixed::TERRAIN_SLOTS;

/// How far to look when picking those pages.
pub const KEEP_DIST: f64 = config_spec::KEEP_DIST;
