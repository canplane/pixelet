// pxl-spec/cell/object.rs

//! A cell as the object layer holds it: the value a placed cube or decor is
//! stored as in the sparse tree (pxl-kernel's layer/object/store.rs), the
//! predicates over that value, and the placeable palette — every flat cube
//! and every terrain-styled one, with the terrain channel each stands in
//! for. cube.rs and decor.rs are the data; this is the one place that reads
//! them all together.

#![allow(non_upper_case_globals)]

use std::sync::LazyLock;

use crate::bake::format::GroundChannelKey;
use crate::cell::color;
use crate::cell::cube::cube_types;
use crate::cell::decor::{decor_types, DecorId};

// =============================================================================
// Object-layer codec — cube.rs and decor.spec.rs are pure data (their own top doc
// comments); the actual encode/decode logic for their shared sparse-tree
// storage (pxl-kernel's layer/object/store.rs) lives here instead, the one place
// that already aggregates every cell-definition module together.
// =============================================================================

/// Explicit object-layer air: retain this leaf and erase lower layers.
pub fn object_value_is_air(value: u32) -> bool {
    value == cube_types::common::air as u32
}

/// A cube-catalog value and a decor-catalog one share the same numeric
/// range (both plain small numbers, no distinguishing bit) — this is the
/// one place that tells the two apart, same role object_value_is_air plays
/// for AIR within the cube catalog itself.
pub fn object_value_is_decor(value: u32) -> bool {
    u8::try_from(value).ok().and_then(decor_types::Decor::of).is_some()
}

/// Whether a leaf's value is drawn as a voxel — a placeable cube, or the
/// eraser's air — rather than as an instance or nothing: what the objects'
/// fold counts (pxl-kernel layer/object/store.rs `fold_cell`). A number
/// both catalogs have is the decor's, as
/// `object_value_is_decor` tells them apart.
pub fn object_value_is_voxel(value: u32) -> bool {
    !object_value_is_decor(value) && (object_value_is_air(value) || placeable_cube_of(value).is_some())
}

/// The value a leaf holds a decor kind as.
pub fn decor_num_id(id: DecorId) -> u32 {
    id as u32
}

/// The decor kind a packed leaf holds, the reverse of decor_num_id.
pub fn decor_id_from_num(num_id: u32) -> DecorId {
    match u8::try_from(num_id).ok().and_then(decor_types::Decor::of) {
        Some(id) => id,
        None => xpute_core::bug!(ENOTRECOVERABLE, num_id),
    }
}

/// Every decor kind in encoded numeric-id order.
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

/// Which cube_types channel+local-value a terrain-styled cube stands in for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TerrainStyleSource {
    pub channel: GroundChannelKey,
    pub terrain_num_id: u32,
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
    for channel in GroundChannelKey::ALL {
        for terrain_num_id in own_types(channel) {
            let source = TerrainStyleSource {
                channel,
                terrain_num_id: terrain_num_id as u32,
            };
            if let Some(cube) = cube_of_source(source) {
                cubes.push(cube);
            }
        }
    }
    cubes
});

/// A ground channel's own types as numbers, in cube_type.json's order.
fn own_types(channel: GroundChannelKey) -> Vec<u8> {
    match channel {
        GroundChannelKey::LAND => cube_types::Land::ALL.iter().map(|t| *t as u8).collect(),
        GroundChannelKey::WATER => cube_types::Water::ALL.iter().map(|t| *t as u8).collect(),
        GroundChannelKey::STRUCTURE => cube_types::Structure::ALL.iter().map(|t| *t as u8).collect(),
        GroundChannelKey::TRANSPORT => cube_types::Transport::ALL.iter().map(|t| *t as u8).collect(),
    }
}

/// The slot a ground channel is: its identity, not its place in
/// `GROUND_CH_KEYS`. A stored placement names a terrain-styled cube by this
/// number, so reordering the channels for painting must leave it alone.
fn channel_slot(channel: GroundChannelKey) -> u32 {
    channel as u32
}

const _: () = assert!(
    TERRAIN_STYLE_BASE * (GroundChannelKey::ALL.len() as u32 + 1) <= u16::MAX as u32,
    "a terrain-styled value has to fit the band tree's leaf (world/object.rs's SparseTree<u16>)"
);

// =============================================================================
// Placeable cube catalog — everything the shell's hotbar and drawer can offer:
// every flat color, then every terrain-styled cube.
// =============================================================================

/// A placeable cube: a flat color, or a stand-in for one baked terrain
/// type. The numbers the catalog declares, never the words they are spelled
/// out with — those are the shell's, for a label and for an asset path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceableCube {
    Block(cube_types::Block),
    Land(cube_types::Land),
    Water(cube_types::Water),
    Structure(cube_types::Structure),
    Transport(cube_types::Transport),
}

/// The ground channel a cube stands in for, none for a flat color, and its
/// type as its number: what the shell names a cube by (INPUT_POINTER).
pub fn placeable_cube_parts(cube: PlaceableCube) -> (Option<GroundChannelKey>, u8) {
    match cube {
        PlaceableCube::Block(t) => (None, t as u8),
        PlaceableCube::Land(t) => (Some(GroundChannelKey::LAND), t as u8),
        PlaceableCube::Water(t) => (Some(GroundChannelKey::WATER), t as u8),
        PlaceableCube::Structure(t) => (Some(GroundChannelKey::STRUCTURE), t as u8),
        PlaceableCube::Transport(t) => (Some(GroundChannelKey::TRANSPORT), t as u8),
    }
}

/// The cube the shell names by a ground channel, or none for a flat color,
/// and a type's number; none for a pair the catalog gives out no cube for.
pub fn placeable_cube(channel: Option<GroundChannelKey>, type_num: u32) -> Option<PlaceableCube> {
    match channel {
        None => u8::try_from(type_num).ok().and_then(cube_types::Block::of).map(PlaceableCube::Block),
        Some(channel) => terrain_style_cube_for(channel, type_num),
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

/// The value a leaf holds the cube as: a flat color's own catalog number,
/// or the dense one the terrain-styled walk assigns.
pub fn placeable_cube_num_id(cube: PlaceableCube) -> u32 {
    match placeable_cube_parts(cube) {
        (None, t) => t as u32,
        (Some(channel), t) => TERRAIN_STYLE_BASE * (channel_slot(channel) + 1) + t as u32,
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
/// color, which stands in for nothing.
pub fn terrain_style_cube_source(cube: PlaceableCube) -> Option<TerrainStyleSource> {
    let (channel, t) = placeable_cube_parts(cube);
    channel.map(|channel| TerrainStyleSource { channel, terrain_num_id: t as u32 })
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
    cube_types::Common::ALL.iter().map(|t| *t as u8).chain(own_types(channel)).collect()
}

/// The flat colors, in their numbers' order: the layer arrays' order for
/// placed blocks.
pub fn block_cubes() -> Vec<cube_types::Block> {
    let mut blocks = cube_types::Block::ALL.to_vec();
    blocks.sort_by_key(|b| *b as u8);
    blocks
}

/// The color the catalog gives a cube, as the one number it is — what
/// everything that lights or blends with a color wants.
pub fn placeable_cube_rgb(cube: PlaceableCube) -> Option<u32> {
    placeable_cube_swatch(cube).map(|s| s.rgb)
}

fn placeable_cube_swatch(cube: PlaceableCube) -> Option<crate::cell::r#type::Swatch> {
    match placeable_cube_parts(cube) {
        (None, t) => color::block(t),
        (Some(channel), t) => color::ground(channel, t),
    }
}

/// Every cube the catalogs give out, flat colors first in the catalog's own
/// numeric order, then every terrain-styled entry channel by channel.
pub static placeable_cubes: LazyLock<Vec<PlaceableCube>> = LazyLock::new(|| block_cubes().into_iter().map(PlaceableCube::Block).chain(TERRAIN_STYLE.iter().copied()).collect());

#[cfg(test)]
#[path = "object.test.rs"]
mod test;
