// pxl-spec/cell/object.rs

//! A cell as the object layer stores it: the leaf value of a placed cube or
//! decor, predicates over it, and the placeable palette.

#![allow(non_upper_case_globals)]

use std::sync::LazyLock;

use crate::bake::format::GroundChannelKey;
use crate::cell::color;
use crate::cell::cube::cube_types;
use crate::cell::decor::{decor_types, DecorId};

/// Explicit air: retains the leaf and erases lower layers.
pub fn object_value_is_air(value: u32) -> bool {
    value == cube_types::common::air as u32
}

/// Cube and decor values share one numeric range; this tells them apart.
pub fn object_value_is_decor(value: u32) -> bool {
    u8::try_from(value).ok().and_then(decor_types::Decor::of).is_some()
}

/// Whether a leaf is drawn as a voxel (a placeable cube or air) rather than
/// an instance or nothing. A number both catalogs have is the decor's.
pub fn object_value_is_voxel(value: u32) -> bool {
    !object_value_is_decor(value) && (object_value_is_air(value) || placeable_cube_of(value).is_some())
}

pub fn decor_num_id(id: DecorId) -> u32 {
    id as u32
}

pub fn decor_id_from_num(num_id: u32) -> DecorId {
    match u8::try_from(num_id).ok().and_then(decor_types::Decor::of) {
        Some(id) => id,
        None => xpute_core::bug!(ENOTRECOVERABLE, num_id),
    }
}

/// In numeric order.
pub static decor_ids: LazyLock<Vec<DecorId>> = LazyLock::new(|| {
    let mut ids = decor_types::Decor::ALL.to_vec();
    ids.sort_by_key(|id| *id as u8);
    ids
});

// A terrain-styled cube's stored value is its (channel, type), not its
// position in the catalog walk.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TerrainStyleSource {
    pub channel: GroundChannelKey,
    pub terrain_num_id: u32,
}

/// Values at and above this are terrain-styled cubes, `(channel slot + 1) ·
/// base + type`; everything the catalogs number directly is one byte below it.
pub const TERRAIN_STYLE_BASE: u32 = 256;

/// Every terrain-styled cube, channel by channel. Numbering is not by this
/// walk: a type inserted into cube_type.json would renumber every stored placement.
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

fn own_types(channel: GroundChannelKey) -> Vec<u8> {
    match channel {
        GroundChannelKey::LAND => cube_types::Land::ALL.iter().map(|t| *t as u8).collect(),
        GroundChannelKey::WATER => cube_types::Water::ALL.iter().map(|t| *t as u8).collect(),
        GroundChannelKey::STRUCTURE => cube_types::Structure::ALL.iter().map(|t| *t as u8).collect(),
        GroundChannelKey::TRANSPORT => cube_types::Transport::ALL.iter().map(|t| *t as u8).collect(),
    }
}

/// A channel's identity, not its paint order: stored placements name a
/// terrain-styled cube by it.
fn channel_slot(channel: GroundChannelKey) -> u32 {
    channel as u32
}

const _: () = assert!(
    TERRAIN_STYLE_BASE * (GroundChannelKey::ALL.len() as u32 + 1) <= u16::MAX as u32,
    "a terrain-styled value has to fit the band tree's leaf (world/object.rs's SparseTree<u16>)"
);

/// A placeable cube: a flat color, or a stand-in for one baked terrain type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceableCube {
    Block(cube_types::Block),
    Land(cube_types::Land),
    Water(cube_types::Water),
    Structure(cube_types::Structure),
    Transport(cube_types::Transport),
}

/// The ground channel a cube stands in for (none for a flat color) and its type number.
pub fn placeable_cube_parts(cube: PlaceableCube) -> (Option<GroundChannelKey>, u8) {
    match cube {
        PlaceableCube::Block(t) => (None, t as u8),
        PlaceableCube::Land(t) => (Some(GroundChannelKey::LAND), t as u8),
        PlaceableCube::Water(t) => (Some(GroundChannelKey::WATER), t as u8),
        PlaceableCube::Structure(t) => (Some(GroundChannelKey::STRUCTURE), t as u8),
        PlaceableCube::Transport(t) => (Some(GroundChannelKey::TRANSPORT), t as u8),
    }
}

/// `placeable_cube_parts`' inverse.
pub fn placeable_cube(channel: Option<GroundChannelKey>, type_num: u32) -> Option<PlaceableCube> {
    match channel {
        None => u8::try_from(type_num).ok().and_then(cube_types::Block::of).map(PlaceableCube::Block),
        Some(channel) => terrain_style_cube_for(channel, type_num),
    }
}

fn cube_of_source(source: TerrainStyleSource) -> Option<PlaceableCube> {
    let t = u8::try_from(source.terrain_num_id).ok()?;
    Some(match source.channel {
        GroundChannelKey::LAND => PlaceableCube::Land(cube_types::Land::of(t)?),
        GroundChannelKey::WATER => PlaceableCube::Water(cube_types::Water::of(t)?),
        GroundChannelKey::STRUCTURE => PlaceableCube::Structure(cube_types::Structure::of(t)?),
        GroundChannelKey::TRANSPORT => PlaceableCube::Transport(cube_types::Transport::of(t)?),
    })
}

pub fn placeable_cube_num_id(cube: PlaceableCube) -> u32 {
    match placeable_cube_parts(cube) {
        (None, t) => t as u32,
        (Some(channel), t) => TERRAIN_STYLE_BASE * (channel_slot(channel) + 1) + t as u32,
    }
}

/// None for VOID, AIR or a decor kind.
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

pub fn terrain_style_cube_source(cube: PlaceableCube) -> Option<TerrainStyleSource> {
    let (channel, t) = placeable_cube_parts(cube);
    channel.map(|channel| TerrainStyleSource { channel, terrain_num_id: t as u32 })
}

pub fn terrain_style_cube_for(channel: GroundChannelKey, terrain_num_id: u32) -> Option<PlaceableCube> {
    let source = TerrainStyleSource { channel, terrain_num_id };
    cube_of_source(source)
}

/// `common`'s types first, then the channel's own, in cube_type.json's order.
pub fn ground_channel_types(channel: GroundChannelKey) -> Vec<u8> {
    cube_types::Common::ALL.iter().map(|t| *t as u8).chain(own_types(channel)).collect()
}

/// In numeric order.
pub fn block_cubes() -> Vec<cube_types::Block> {
    let mut blocks = cube_types::Block::ALL.to_vec();
    blocks.sort_by_key(|b| *b as u8);
    blocks
}

pub fn placeable_cube_rgb(cube: PlaceableCube) -> Option<u32> {
    placeable_cube_swatch(cube).map(|s| s.rgb)
}

fn placeable_cube_swatch(cube: PlaceableCube) -> Option<crate::cell::r#type::Swatch> {
    match placeable_cube_parts(cube) {
        (None, t) => color::block(t),
        (Some(channel), t) => color::ground(channel, t),
    }
}

/// Flat colors first, then terrain-styled cubes channel by channel.
pub static placeable_cubes: LazyLock<Vec<PlaceableCube>> = LazyLock::new(|| block_cubes().into_iter().map(PlaceableCube::Block).chain(TERRAIN_STYLE.iter().copied()).collect());

#[cfg(test)]
#[path = "object.test.rs"]
mod test;
