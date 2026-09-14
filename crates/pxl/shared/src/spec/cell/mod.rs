// pxl_shared/spec/cell/mod.rs
// Entry point for the cell model. Assembles and re-exports the modular definitions.

#![allow(non_upper_case_globals)]

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use crate::bake::format::{GroundChannelKey, GROUND_CH_KEYS};

pub mod color;
pub mod cube;
pub mod cube_type;
pub mod decor;
pub mod r#type;

pub use cube::{cube_colors, cube_types, CubeColorId};
pub use decor::{decor_types, DecorId};

use r#type::{Channels, Colors};

// =============================================================================
// Aggregated Types
// =============================================================================

pub struct CellTypes {
    /// Terrain raster + user-placed block colors (cube_types::common holds the
    /// shared VOID/AIR/UNKNOWN sentinel registry every other channel/consumer
    /// references as needed — no separate top-level slot for it here, see
    /// cube.rs's own top comment).
    pub cube: &'static Channels,

    /// User-placed decor kinds
    pub decor: &'static [(&'static str, u8)],
}

pub const cell_types: CellTypes = CellTypes {
    cube: cube_types::CHANNELS,
    decor: decor_types::ENTRIES,
};

// =============================================================================
// Aggregated Colors
// =============================================================================

pub struct CellColors {
    /// Terrain raster + user-placed block colors
    pub cube: &'static LazyLock<Colors>,
}

pub static cell_colors: CellColors = CellColors { cube: &cube_colors };

// =============================================================================
// Object-layer codec — cube.rs/decor.rs are pure data (their own top doc
// comments); the actual encode/decode logic for their shared sparse-tree
// storage (object/store.ts, apps/web) lives here instead, the one place
// that already aggregates every cell-definition module together.
// =============================================================================

/// Zero (cube_types::common::VOID) means sparse absence/fallback here, not
/// explicit air — the only real distinction left (2026-09-06) now that a
/// real value doesn't need to share a bit with every other real value; see
/// cube.rs's own top comment.
pub fn object_value_is_present(value: u32) -> bool {
    value != cube_types::common::VOID as u32
}

/// Explicit object-layer air: retain this leaf and erase lower layers.
pub fn object_value_is_air(value: u32) -> bool {
    value == cube_types::common::AIR as u32
}

pub fn cube_num_id(id: &str) -> u32 {
    match cube_types::block::ENTRIES.iter().find(|(name, _)| *name == id) {
        Some((_, num)) => *num as u32,
        None => panic!("no cube color named {id}"),
    }
}

pub fn cube_hex(id: &str) -> &'static str {
    // block carries no sentinels of its own any more — every key is a real color.
    cube_colors.get("block").and_then(|c| c.get(&cube_num_id(id))).copied().flatten().unwrap()
}

/// Numeric id -> name, in numeric order: the reverse table `NUM_ID_TO_CUBE`
/// is there, compacted.
fn by_num(entries: &'static [(&'static str, u8)]) -> Vec<(&'static str, u32)> {
    let mut out: Vec<(&'static str, u32)> = entries.iter().map(|(name, num)| (*name, *num as u32)).collect();
    // A later key of the same number overwrites an earlier one in the table.
    out.reverse();
    out.sort_by_key(|(_, num)| *num);
    out.dedup_by_key(|(_, num)| *num);
    out
}

/// Numeric id -> cube color name — the reverse of cube_num_id, used to decode a packed cube channel back into a PlacedObject's own CubeColorId.
pub fn cube_id_from_num(num_id: u32) -> CubeColorId {
    match by_num(cube_types::block::ENTRIES).iter().find(|(_, num)| *num == num_id) {
        Some((name, _)) => name,
        None => panic!("no cube color registered for numeric id {num_id}"),
    }
}

/// Every real cube color id in encoded numeric-id order.
pub static cube_ids: LazyLock<Vec<CubeColorId>> = LazyLock::new(|| by_num(cube_types::block::ENTRIES).into_iter().map(|(name, _)| name).collect());

static DECOR_NUM_IDS: LazyLock<HashSet<u32>> = LazyLock::new(|| decor_types::ENTRIES.iter().map(|(_, num)| *num as u32).collect());

/// A cube-catalog value and a decor-catalog one share the same numeric
/// range (both plain small numbers, no distinguishing bit) — this is the
/// one place that tells the two apart, same role object_value_is_air plays
/// for AIR within the cube catalog itself.
pub fn object_value_is_decor(value: u32) -> bool {
    DECOR_NUM_IDS.contains(&value)
}

pub fn decor_num_id(id: &str) -> u32 {
    match decor_types::ENTRIES.iter().find(|(name, _)| *name == id) {
        Some((_, num)) => *num as u32,
        None => panic!("no decor kind named {id}"),
    }
}

/// Numeric id -> decor name — the reverse of decor_num_id, used to decode a packed decor leaf back into a PlacedDecor's own DecorId.
pub fn decor_id_from_num(num_id: u32) -> DecorId {
    match by_num(decor_types::ENTRIES).iter().find(|(_, num)| *num == num_id) {
        Some((name, _)) => name,
        None => panic!("no decor kind registered for numeric id {num_id}"),
    }
}

/// Every real decor id in encoded numeric-id order — mode_bar.tsx's decor buttons iterate this, same shape as cube_ids.
pub static decor_ids: LazyLock<Vec<DecorId>> = LazyLock::new(|| by_num(decor_types::ENTRIES).into_iter().map(|(name, _)| name).collect());

// =============================================================================
// Terrain-styled cube catalog — user-placeable references into cube_types'
// own land/water/structure/transport channels, derived generically (no
// hand-typed per-type catalog to keep in sync — 2026-09-06, see
// issue/20260906-unified-block-palette.md). A terrain-styled id is
// just `"<channel>.<local key>"`, e.g. "land.GREEN_DEEP" — its own local
// value (cube_types::land::GREEN_DEEP) is whatever the bake pipeline already
// uses; the *global* sparse-tree byte a placed cube of this kind actually
// gets stored as is a separately assigned dense sequence below, since a
// real terrain cell can hold a value from every channel simultaneously but
// a placed cube's single global byte can only ever mean one of them.
// =============================================================================

/// `"<channel>.<local key>"`.
pub type TerrainStyleCubeId = String;

/// Which cube_types channel+local-value a terrain-styled id stands in for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainStyleSource {
    pub channel: GroundChannelKey,
    pub terrain_num_id: u32,
}

struct TerrainStyle {
    /// In assignment order, which is the order `terrain_style_cube_for` walks.
    source: Vec<(TerrainStyleCubeId, TerrainStyleSource)>,
    to_num: HashMap<TerrainStyleCubeId, u32>,
    num_to: HashMap<u32, TerrainStyleCubeId>,
    hex: HashMap<TerrainStyleCubeId, Option<&'static str>>,
}

/// A channel: its name and its types.
type Channel = (&'static str, &'static [(&'static str, u8)]);

/// Every channel but block and common, in cube_types' own order.
static TERRAIN_CHANNELS: LazyLock<Vec<Channel>> = LazyLock::new(|| cube_types::CHANNELS.iter().copied().filter(|(ch, _)| *ch != "block" && *ch != "common").collect());

/// The ground channel a terrain channel's name is.
fn ground_key(channel: &str) -> GroundChannelKey {
    *GROUND_CH_KEYS.iter().find(|k| k.as_str() == channel).unwrap()
}

static TERRAIN_STYLE: LazyLock<TerrainStyle> = LazyLock::new(|| {
    let mut t = TerrainStyle {
        source: Vec::new(),
        to_num: HashMap::new(),
        num_to: HashMap::new(),
        hex: HashMap::new(),
    };
    let mut next_num = 1;
    for (channel, local_types) in TERRAIN_CHANNELS.iter() {
        for (local_key, num) in local_types.iter() {
            let id = format!("{channel}.{local_key}");
            t.source.push((
                id.clone(),
                TerrainStyleSource {
                    channel: ground_key(channel),
                    terrain_num_id: *num as u32,
                },
            ));
            t.to_num.insert(id.clone(), next_num);
            t.num_to.insert(next_num, id);
            next_num += 1;
        }
    }
    // Real swatch colors for every entry above, read straight from cube_colors'
    // own already-computed palette (no second hand-authored copy to keep in sync).
    for (id, source) in &t.source {
        let hex = cube_colors.get(source.channel.as_str()).and_then(|c| c.get(&source.terrain_num_id)).copied().flatten();
        t.hex.insert(id.clone(), hex);
    }
    t
});

// =============================================================================
// Placeable cube catalog — everything mode_bar.tsx's hotbar (cube_ids alone)
// and palette_modal.tsx's own full grid (this) can offer a user. A separate
// concept from cube_ids itself: the hotbar deliberately stays scoped to just
// the original 15 flat colors (2026-09-06 UX decision — the full catalog
// lives behind its own modal, not folded into the everyday quick-pick).
// =============================================================================

/// A CubeColorId or a TerrainStyleCubeId.
pub type PlaceableCubeId = String;

pub fn is_terrain_style_cube(id: &str) -> bool {
    TERRAIN_STYLE.to_num.contains_key(id)
}

/// channel/terrain_num_id a terrain-styled placeable id resolves to — render/layer/object/adapter.ts's own material resolver.
pub fn terrain_style_cube_source(id: &str) -> TerrainStyleSource {
    match TERRAIN_STYLE.source.iter().find(|(have, _)| have == id) {
        Some((_, source)) => *source,
        None => panic!("no terrain-style source registered for \"{id}\""),
    }
}

/// The terrain-styled id standing for a real baked (channel, type) pair —
/// the inverse of terrain_style_cube_source, for callers that hold what the
/// terrain shows and want the cube that looks like it.
pub fn terrain_style_cube_for(channel: GroundChannelKey, terrain_num_id: u32) -> Option<TerrainStyleCubeId> {
    TERRAIN_STYLE
        .source
        .iter()
        .find(|(_, s)| s.channel == channel && s.terrain_num_id == terrain_num_id)
        .map(|(id, _)| id.clone())
}

pub fn placeable_cube_hex(id: &str) -> Option<&'static str> {
    if is_terrain_style_cube(id) {
        TERRAIN_STYLE.hex.get(id).copied().flatten()
    } else {
        Some(cube_hex(id))
    }
}

pub fn placeable_cube_num_id(id: &str) -> u32 {
    if is_terrain_style_cube(id) {
        TERRAIN_STYLE.to_num[id]
    } else {
        cube_num_id(id)
    }
}

pub fn placeable_cube_id_from_num(num_id: u32) -> PlaceableCubeId {
    match TERRAIN_STYLE.num_to.get(&num_id) {
        Some(id) => id.clone(),
        None => cube_id_from_num(num_id).to_string(),
    }
}

/// Every placeable id, flat colors first (cube_ids' own order) then every
/// terrain-styled entry, channel by channel (GROUND_CH_KEYS' own order) —
/// mode_bar.tsx's hotbar keeps using cube_ids alone; this is
/// palette_modal.tsx's own full-catalog source.
pub static placeable_cube_ids: LazyLock<Vec<PlaceableCubeId>> = LazyLock::new(|| {
    let mut out: Vec<PlaceableCubeId> = cube_ids.iter().map(|id| id.to_string()).collect();
    for channel in GROUND_CH_KEYS {
        let entries = cube_types::CHANNELS.iter().find(|(ch, _)| *ch == channel.as_str()).unwrap().1;
        out.extend(entries.iter().map(|(local_key, _)| format!("{}.{local_key}", channel.as_str())));
    }
    out
});

#[cfg(test)]
#[path = "mod.test.rs"]
mod test;
