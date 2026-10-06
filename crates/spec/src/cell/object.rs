// pxl-spec/cell/object.rs

//! A cell as the object layer stores it: a `u16` whose byte range is the
//! ground's palette, so a cube in a look stores the look's number, and whose
//! range past a byte is what only an object is: air, a placed color, a decor
//! kind (`cell/cube_type.json`, `cell/decor.json`).

#![allow(non_upper_case_globals)]

use std::sync::LazyLock;

use crate::cell::color;
use crate::cell::cube_types::{object, Block, Ground};
use crate::cell::{decor_types, DecorId};

/// Explicit air: retains the leaf and erases lower layers.
pub fn object_value_is_air(value: u32) -> bool {
    value == object::air as u32
}

pub fn object_value_is_decor(value: u32) -> bool {
    u16::try_from(value).ok().and_then(decor_types::Decor::of).is_some()
}

/// Whether a leaf is drawn as a voxel (a placeable cube or air) rather than
/// an instance or nothing.
pub fn object_value_is_voxel(value: u32) -> bool {
    object_value_is_air(value) || placeable_cube_of(value).is_some()
}

pub fn decor_num_id(id: DecorId) -> u32 {
    id as u32
}

pub fn decor_id_from_num(num_id: u32) -> DecorId {
    match u16::try_from(num_id).ok().and_then(decor_types::Decor::of) {
        Some(id) => id,
        None => xpute_core::bug!(ENOTRECOVERABLE, num_id),
    }
}

/// In numeric order.
pub static decor_ids: LazyLock<Vec<DecorId>> = LazyLock::new(|| {
    let mut ids = decor_types::Decor::ALL.to_vec();
    ids.sort_by_key(|id| *id as u16);
    ids
});

/// A placeable cube: a look of the ground's palette, or a placed color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceableCube {
    Ground(Ground),
    Block(Block),
}

/// The value a placement stores.
pub fn placeable_cube_num_id(cube: PlaceableCube) -> u32 {
    match cube {
        PlaceableCube::Ground(g) => g as u32,
        PlaceableCube::Block(b) => b as u32,
    }
}

/// None for VOID, UNKNOWN, AIR or a decor kind.
pub fn placeable_cube_of(num_id: u32) -> Option<PlaceableCube> {
    match u8::try_from(num_id) {
        Ok(byte) => Ground::of(byte).map(PlaceableCube::Ground),
        Err(_) => u16::try_from(num_id).ok().and_then(Block::of).map(PlaceableCube::Block),
    }
}

pub fn placeable_cube_rgb(cube: PlaceableCube) -> Option<u32> {
    match cube {
        PlaceableCube::Ground(g) => color::ground(g as u8),
        PlaceableCube::Block(b) => color::object(b as u16),
    }
    .map(|s| s.rgb)
}

/// The placed colors first, then the palette in paint order.
pub static placeable_cubes: LazyLock<Vec<PlaceableCube>> = LazyLock::new(|| Block::ALL.into_iter().map(PlaceableCube::Block).chain(Ground::ALL.into_iter().map(PlaceableCube::Ground)).collect());

#[cfg(test)]
#[path = "object.test.rs"]
mod test;
