// pxl_core/module/world/object/size.rs

use pxl_shared::bake::constant::U_PER_CELL;

/// Placeable cube sizes — grid-aligned steps down from one terrain cell
/// (U_PER_CELL). Single source of truth for mode_bar's size selector, the
/// placement/erase raycasting and overlap check (camera/ray.rs,
/// object/collision.rs), and the hover preview + real cube render — every
/// size, including sub-cell, goes through the same dense voxel field. Tracks
/// U_PER_CELL itself, not a snapshot of its value, so a CELL_SHAMT change
/// rescales every tier here automatically.
///
/// Largest entry is U_PER_CELL because that's the terrain's own cell size —
/// an object at that size lines up 1:1 with a terrain cell (see
/// bake/constant.rs's OBJECT_LAYOUT, whose `max: CELL_SHAMT` is the other
/// half of that reasoning: object/store.rs's sparse tree never spans more
/// than one cell) — the other three (finer) tiers are what the object paint
/// (crates/pxl/core/src/render/layer/object/adapter.rs) writes as sub-cell
/// voxels instead of a full-cell voxel. Deliberately 4 tiers deep, not 3 — when CELL_SHAMT
/// changed U_PER_CELL 0.5u -> 1u, sliding a fixed *count* of tiers down from
/// it would have silently dropped the finest one (0.125u) that building at
/// sub-block detail already depended on, instead of just gaining a coarser
/// option to match the new cell size. One more halving step keeps every size
/// that already existed usable.
pub const OBJECT_SIZES: [f64; 4] = [U_PER_CELL, U_PER_CELL / 2.0, U_PER_CELL / 4.0, U_PER_CELL / 8.0];

pub const DEFAULT_OBJECT_SIZE: f64 = OBJECT_SIZES[0];

/// Every decor kind places at exactly one terrain cell — unlike a color
/// cube, decor has no mode_bar size flyout of its own (yet) to vary this, and
/// a scattered tree already assumes its own candidates are one full cell
/// wide. Deliberately the same value as OBJECT_SIZES[0], not a re-export of
/// it — object_size (ui_store.rs) is a live, user-changed selection for the
/// object tool specifically; this is decor's own fixed constant and must keep
/// working even if a future size flyout moves object_size elsewhere.
pub const DECOR_SIZE: f64 = OBJECT_SIZES[0];

pub const OBJECT_SCALE_EXPONENTS: [ObjectScaleExponent; 4] = [0, -1, -2, -3];
/// One of OBJECT_SCALE_EXPONENTS.
pub type ObjectScaleExponent = i32;

const DEFAULT_ACTIVE_SCALES: &[ObjectScaleExponent] = &[0, -1, -2, -3];

/// Per-cube override of which of OBJECT_SIZES' tiers mode_bar's size flyout
/// offers — empty today (every cube, flat color or terrain-styled, offers
/// every tier), reserved for a future one that shouldn't (a huge painted
/// mural block, say). Keyed by the full PlaceableCubeId since the hotbar's
/// own MRU list can surface a terrain-styled pick too.
const ACTIVE_SCALES: &[(&str, &[ObjectScaleExponent])] = &[];

pub fn object_scale_exponents(id: &str) -> &'static [ObjectScaleExponent] {
    ACTIVE_SCALES.iter().find(|(key, _)| *key == id).map_or(DEFAULT_ACTIVE_SCALES, |(_, scales)| scales)
}
