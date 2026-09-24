// pxl-spec/cell/object.rs

//! A cell as the object layer holds it: the value a placed cube or decor is
//! stored as in the sparse tree (pxl-kernel's layer/object/store.rs), the
//! predicates over that value, and the placeable palette — every flat cube
//! and every terrain-styled one, with the terrain channel each stands in
//! for. cube.rs and decor.rs are the data; this is the one place that reads
//! them all together.

#![allow(non_upper_case_globals)]

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::bake::format::{GroundChannelKey, GROUND_CH_KEYS};
use crate::cell::cube::{cube_colors, cube_types, CubeColorId};
use crate::cell::decor::{decor_types, DecorId};
use crate::cell::r#type::{self, Channels, Colors, Swatch};

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
    pub decor: &'static [decor_types::Decor],
}

pub const cell_types: CellTypes = CellTypes {
    cube: cube_types::CHANNELS,
    decor: &decor_types::Decor::ALL,
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
// Object-layer codec — cube.rs and decor.spec.rs are pure data (their own top doc
// comments); the actual encode/decode logic for their shared sparse-tree
// storage (pxl-kernel's layer/object/store.rs) lives here instead, the one place
// that already aggregates every cell-definition module together.
// =============================================================================

/// Zero (cube_types::common::void) means sparse absence/fallback here, not
/// explicit air — the one distinction there is, since a real value shares
/// no bit with any other real value; see cube.rs's own top comment.
pub fn object_value_is_present(value: u32) -> bool {
    value != cube_types::common::void as u32
}

/// Explicit object-layer air: retain this leaf and erase lower layers.
pub fn object_value_is_air(value: u32) -> bool {
    value == cube_types::common::air as u32
}

pub fn cube_num_id(id: &str) -> u32 {
    match cube_types::block::ENTRIES.iter().find(|(name, _)| *name == id) {
        Some((_, num)) => *num as u32,
        None => panic!("no cube color named {id}"),
    }
}

pub fn cube_hex(id: &str) -> &'static str {
    cube_swatch(id).hex
}

/// The same swatch packed `0xRRGGBB`, for the callers that light or blend with
/// it rather than write it down.
pub fn cube_rgb(id: &str) -> u32 {
    cube_swatch(id).rgb
}

fn cube_swatch(id: &str) -> Swatch {
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

/// A cube-catalog value and a decor-catalog one share the same numeric
/// range (both plain small numbers, no distinguishing bit) — this is the
/// one place that tells the two apart, same role object_value_is_air plays
/// for AIR within the cube catalog itself.
pub fn object_value_is_decor(value: u32) -> bool {
    u8::try_from(value).ok().and_then(decor_types::Decor::of).is_some()
}

/// The value a leaf holds a decor kind as.
pub fn decor_num_id(id: DecorId) -> u32 {
    id as u32
}

/// The decor kind a packed leaf holds, the reverse of decor_num_id.
pub fn decor_id_from_num(num_id: u32) -> DecorId {
    match u8::try_from(num_id).ok().and_then(decor_types::Decor::of) {
        Some(id) => id,
        None => panic!("no decor kind registered for numeric id {num_id}"),
    }
}

/// Every decor kind in encoded numeric-id order — mode-bar.tsx's decor buttons iterate this, same shape as cube_ids.
pub static decor_ids: LazyLock<Vec<DecorId>> = LazyLock::new(|| {
    let mut ids = decor_types::Decor::ALL.to_vec();
    ids.sort_by_key(|id| *id as u8);
    ids
});

// =============================================================================
// Terrain-styled cube catalog — user-placeable references into cube_types'
// own land/water/structure/transport channels, derived generically (no
// hand-typed per-type catalog to keep in sync — see
// issue/20260906-unified-block-palette.md). Its own local value
// (cube_types::land::green_deep) is whatever the bake pipeline already uses;
// the *global* sparse-tree byte a placed cube of this kind actually gets
// stored as is a separately assigned dense sequence below, since a real
// terrain cell can hold a value from every channel simultaneously but a
// placed cube's single global byte can only ever mean one of them.
// =============================================================================

/// `"<channel>.<local key>"`, e.g. `"land.green_deep"`.
pub type TerrainStyleCubeId = String;

/// Which cube_types channel+local-value a terrain-styled cube stands in for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TerrainStyleSource {
    pub channel: GroundChannelKey,
    pub terrain_num_id: u32,
}

/// A channel: its name and its types.
type Channel = (&'static str, &'static [(&'static str, u8)]);

/// Every channel but block and common, in cube_types' own order.
static TERRAIN_CHANNELS: LazyLock<Vec<Channel>> = LazyLock::new(|| cube_types::CHANNELS.iter().copied().filter(|(ch, _)| *ch != "block" && *ch != "common").collect());

/// The ground channel a terrain channel's name is.
fn ground_key(channel: &str) -> GroundChannelKey {
    *GROUND_CH_KEYS.iter().find(|k| k.as_str() == channel).unwrap()
}

/// Where a stored leaf's value stops being a number of its own and starts
/// being a terrain-styled cube's `(channel, type)`.
///
/// Everything the catalogs number directly sits below this: VOID at 0, AIR at
/// 128, block at 130..=144, decor at 228, UNKNOWN at 255 — all of one byte,
/// and the widest of them is the byte. Above it, a terrain-styled cube is the
/// channel it stands in for and the type within that channel, which is exactly
/// what it is, so the value can be read back without a table.
pub const TERRAIN_STYLE_BASE: u32 = 256;

/// Every terrain-styled cube, channel by channel, for the callers that want
/// the catalog walked. The numbering is not this walk's to give out: a
/// number handed out in walking order would change for every type after one
/// inserted into cube_type.json, in every other channel too, and a stored
/// placement would then name a different cube than the one that was placed.
static TERRAIN_STYLE: LazyLock<Vec<PlaceableCube>> = LazyLock::new(|| {
    let mut cubes = Vec::new();
    for (channel, local_types) in TERRAIN_CHANNELS.iter() {
        for (_, num) in local_types.iter() {
            let source = TerrainStyleSource {
                channel: ground_key(channel),
                terrain_num_id: *num as u32,
            };
            if let Some(cube) = cube_of_source(source) {
                cubes.push(cube);
            }
        }
    }
    cubes
});

/// The slot a ground channel is: its identity, not its place in
/// `GROUND_CH_KEYS`. A stored placement names a terrain-styled cube by this
/// number, so reordering the channels for painting must leave it alone.
fn channel_slot(channel: GroundChannelKey) -> u32 {
    channel as u32
}

const _: () = assert!(
    TERRAIN_STYLE_BASE * (GROUND_CH_KEYS.len() as u32 + 1) <= u16::MAX as u32,
    "a terrain-styled value has to fit the band tree's leaf (world/object.rs's SparseTree<u16>)"
);

/// Each channel's numbers back to the name cube_type.json gives them.
static TYPE_NAME: LazyLock<HashMap<(&'static str, u32), &'static str>> = LazyLock::new(|| {
    let mut out = HashMap::new();
    for (channel, entries) in cube_types::CHANNELS.iter() {
        for (name, num) in entries.iter() {
            out.insert((*channel, *num as u32), *name);
        }
    }
    out
});

// =============================================================================
// Placeable cube catalog — everything mode-bar.tsx's hotbar (cube_ids alone)
// and drawer-modal.tsx's own full grid (this) can offer a user. A separate
// concept from cube_ids itself: the hotbar deliberately stays scoped to just
// the original 15 flat colors (the full catalog
// lives behind its own modal, not folded into the everyday quick-pick).
// =============================================================================

/// What a cube is spelled out as: `"<channel>.<local key>"`.
pub type PlaceableCubeId = String;

/// A placeable cube: a flat colour, or a stand-in for one baked terrain
/// type. The numbers the catalog declares, never the words they are spelled
/// out with — those are what `placeable_cube_id` builds, for a label and for
/// an asset path, and nothing else reads them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceableCube {
    Block(cube_types::Block),
    Land(cube_types::Land),
    Water(cube_types::Water),
    Structure(cube_types::Structure),
    Transport(cube_types::Transport),
}

/// The channel a cube's type belongs to, and the type as its number.
fn cube_parts(cube: PlaceableCube) -> (&'static str, u32) {
    match cube {
        PlaceableCube::Block(t) => ("block", t as u32),
        PlaceableCube::Land(t) => ("land", t as u32),
        PlaceableCube::Water(t) => ("water", t as u32),
        PlaceableCube::Structure(t) => ("structure", t as u32),
        PlaceableCube::Transport(t) => ("transport", t as u32),
    }
}

/// The cube standing in for a baked (channel, type), or None where the
/// channel gives out no such type.
fn cube_of_source(source: TerrainStyleSource) -> Option<PlaceableCube> {
    let t = u8::try_from(source.terrain_num_id).ok()?;
    Some(match source.channel {
        GroundChannelKey::LAND => PlaceableCube::Land(cube_types::Land::of(t)?),
        GroundChannelKey::WATER => PlaceableCube::Water(cube_types::Water::of(t)?),
        GroundChannelKey::STRUCTURE => PlaceableCube::Structure(cube_types::Structure::of(t)?),
        GroundChannelKey::TRANSPORT => PlaceableCube::Transport(cube_types::Transport::of(t)?),
    })
}

/// The id a cube is spelled out under — `"block.red"`, `"land.bare"`.
/// Exactly two consumers want the words: the locale entry
/// (`cubes.land.bare`, i18next resolving the dot itself) and the
/// shipped texture's path. It is what a cube is called, never what it is, so
/// nothing keys by it.
///
/// Every channel alike, block included: the catalog gives block no standing
/// the other four don't have (cube_type.json lists it as one more channel),
/// so neither does the spelling.
pub fn placeable_cube_id(cube: PlaceableCube) -> PlaceableCubeId {
    let (channel, num) = cube_parts(cube);
    cube_id_of(channel, TYPE_NAME[&(channel, num)])
}

/// How a catalog id is spelled, for the callers that hold the two words
/// already rather than a cube. The shipped palette is one: it walks every
/// channel's ENTRIES, `common` included, and `common` is not a
/// `PlaceableCube`, so it cannot come through the function above — but it
/// must come out spelled the same, and it was spelling it a second time
/// instead.
pub fn cube_id_of(channel: &str, type_name: &str) -> PlaceableCubeId {
    format!("{channel}.{type_name}")
}

/// The value a leaf holds the cube as: a flat colour's own catalog number,
/// or the dense one the terrain-styled walk assigns.
pub fn placeable_cube_num_id(cube: PlaceableCube) -> u32 {
    match terrain_style_cube_source(cube) {
        None => cube_parts(cube).1,
        Some(source) => TERRAIN_STYLE_BASE * (channel_slot(source.channel) + 1) + source.terrain_num_id,
    }
}

/// The cube a stored leaf value names, or None for a value that names no
/// cube — VOID, AIR, a decor kind.
pub fn placeable_cube_of(num_id: u32) -> Option<PlaceableCube> {
    if num_id >= TERRAIN_STYLE_BASE {
        let slot = num_id / TERRAIN_STYLE_BASE - 1;
        let channel = GroundChannelKey::from_id(slot)?;
        return cube_of_source(TerrainStyleSource {
            channel,
            terrain_num_id: num_id % TERRAIN_STYLE_BASE,
        });
    }
    u8::try_from(num_id).ok().and_then(cube_types::Block::of).map(PlaceableCube::Block)
}

/// Which baked (channel, type) the cube stands in for, None for a flat
/// colour, which stands in for nothing.
pub fn terrain_style_cube_source(cube: PlaceableCube) -> Option<TerrainStyleSource> {
    let (channel, terrain_num_id) = cube_parts(cube);
    (channel != "block").then(|| TerrainStyleSource {
        channel: ground_key(channel),
        terrain_num_id,
    })
}

/// The cube standing in for a real baked (channel, type) pair — the inverse
/// of terrain_style_cube_source, for callers that hold what the terrain shows
/// and want the cube that looks like it.
pub fn terrain_style_cube_for(channel: GroundChannelKey, terrain_num_id: u32) -> Option<PlaceableCube> {
    let source = TerrainStyleSource { channel, terrain_num_id };
    cube_of_source(source)
}

/// A ground channel's types as numbers: `common`'s first, then the channel's
/// own, each in cube_type.json's order — for a caller that walks a channel
/// without naming it.
pub fn ground_channel_types(channel: GroundChannelKey) -> Vec<u8> {
    let own: Vec<u8> = match channel {
        GroundChannelKey::LAND => cube_types::Land::ALL.iter().map(|t| *t as u8).collect(),
        GroundChannelKey::WATER => cube_types::Water::ALL.iter().map(|t| *t as u8).collect(),
        GroundChannelKey::STRUCTURE => cube_types::Structure::ALL.iter().map(|t| *t as u8).collect(),
        GroundChannelKey::TRANSPORT => cube_types::Transport::ALL.iter().map(|t| *t as u8).collect(),
    };
    cube_types::Common::ALL.iter().map(|t| *t as u8).chain(own).collect()
}

/// A ground channel's colours by type, as the catalog gives them.
pub fn ground_channel_colors(channel: GroundChannelKey) -> Option<&'static r#type::ChannelColors> {
    cube_colors.get(channel.as_str())
}

/// The flat colours, in their numbers' order: the layer arrays' order for
/// placed blocks.
pub fn block_cubes() -> Vec<cube_types::Block> {
    let mut blocks = cube_types::Block::ALL.to_vec();
    blocks.sort_by_key(|b| *b as u8);
    blocks
}

/// The swatch colour the catalog gives it — cube_colors is keyed by channel
/// and number, so this is the palette's own row, not a second table.
pub fn placeable_cube_hex(cube: PlaceableCube) -> Option<&'static str> {
    placeable_cube_swatch(cube).map(|s| s.hex)
}

/// The same swatch as the one number it is, which is what everything that
/// lights or blends with a colour wants — and what nothing has to parse text
/// to find out, the catalog's JSON having been packed by the generator.
pub fn placeable_cube_rgb(cube: PlaceableCube) -> Option<u32> {
    placeable_cube_swatch(cube).map(|s| s.rgb)
}

fn placeable_cube_swatch(cube: PlaceableCube) -> Option<Swatch> {
    let (channel, num) = cube_parts(cube);
    cube_colors.get(channel).and_then(|c| c.get(&num)).copied().flatten()
}

/// Every cube the catalogs give out, flat colours first in the catalog's own
/// numeric order, then every terrain-styled entry channel by channel.
pub static placeable_cubes: LazyLock<Vec<PlaceableCube>> = LazyLock::new(|| {
    let blocks = cube_ids.iter().filter_map(|id| cube_types::Block::of(cube_num_id(id) as u8).map(PlaceableCube::Block));
    blocks.chain(TERRAIN_STYLE.iter().copied()).collect()
});

#[cfg(test)]
#[path = "object.test.rs"]
mod test;
