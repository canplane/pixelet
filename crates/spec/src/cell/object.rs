// pxl-spec/cell/object.rs

//! A cell as the object layer stores it: a `u16` id, its class over its shape
//! (`cell/id.json`). In the cube class the byte range is the ground's palette,
//! so a cube in a look stores the look's number, and past it is what only an
//! object is: air and the placed colors (`cell/cube_type.json`); the decor
//! class is the decor kinds (`cell/decor.json`), and the marker class a label.

#![allow(non_upper_case_globals)]

use std::sync::LazyLock;

use crate::cell::color;
use crate::cell::cube_types::{object, Ground, Object};
use crate::cell::id::{Class, SHAPE_BITS};
use crate::cell::marker::MARKER_SCALE_STEPS;
use crate::cell::{decor_types, DecorId};
use crate::space::coord::zoom::Zoom;
use crate::space::layer::lattice::Lattice;
use crate::space::layer::tile::LEAF_ZOOM;

/// Explicit air: retains the leaf and erases lower layers.
pub fn object_value_is_air(value: u32) -> bool {
    value == object::air as u32
}

/// The class an id is of; none past a u16 or for a class the layout does not give out.
pub fn class_of(value: u32) -> Option<Class> {
    u16::try_from(value).ok().and_then(|v| Class::of((v >> SHAPE_BITS) as u8))
}

pub fn object_value_is_decor(value: u32) -> bool {
    class_of(value) == Some(Class::decor)
}

/// The one marker a placement puts up: a label, which its shape does not tell
/// apart from another's.
pub const MARKER: u32 = (Class::marker as u32) << SHAPE_BITS;

/// A mark's cell: 16 pixels of the web map's 256-pixel tile, four zooms
/// finer than the tile, the cell Carto keeps its 14-pixel icons apart in.
pub const MARK_STEPS: Zoom = 4;

/// The marker's cells, at its scale on the camera grid (`cell/marker.json`):
/// its zoom is the tile zoom of the grid's cell there.
pub const MARKER_LATTICE: Lattice = Lattice::new(LEAF_ZOOM + MARK_STEPS).scaled(MARKER_SCALE_STEPS);

pub fn object_value_is_marker(value: u32) -> bool {
    class_of(value) == Some(Class::marker)
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
        None => xpute_kit::bug!(ENOTRECOVERABLE, num_id),
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
    Object(Object),
}

/// The value a placement stores.
pub fn placeable_cube_num_id(cube: PlaceableCube) -> u32 {
    match cube {
        PlaceableCube::Ground(g) => g as u32,
        PlaceableCube::Object(b) => b as u32,
    }
}

/// None for VOID, UNKNOWN, AIR or a decor kind.
pub fn placeable_cube_of(num_id: u32) -> Option<PlaceableCube> {
    match u8::try_from(num_id) {
        Ok(byte) => Ground::of(byte).map(PlaceableCube::Ground),
        Err(_) => u16::try_from(num_id).ok().and_then(Object::of).map(PlaceableCube::Object),
    }
}

pub fn placeable_cube_rgb(cube: PlaceableCube) -> Option<u32> {
    match cube {
        PlaceableCube::Ground(g) => color::ground(g as u8),
        PlaceableCube::Object(b) => color::object(b as u16),
    }
    .map(|s| s.rgb)
}

/// The placed colors first, then the palette in paint order.
pub static placeable_cubes: LazyLock<Vec<PlaceableCube>> = LazyLock::new(|| Object::ALL.into_iter().map(PlaceableCube::Object).chain(Ground::ALL.into_iter().map(PlaceableCube::Ground)).collect());

#[cfg(test)]
#[path = "object.test.rs"]
mod test;
