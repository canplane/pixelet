// pxl-spec/block/object.rs

//! A block as the object layer stores it: a `u16` id, its kind over its shape
//! (`asset/id.json`). In the cube kind the byte range is the ground's palette,
//! so a cube in a look stores the look's number, and past it is what only an
//! object is: air and the placed colors (`asset/cube.json`); the decor kind
//! is the decors (`asset/decor.json`), and the marker kind a label.

#![allow(non_upper_case_globals)]

use std::sync::LazyLock;

use crate::bake::constant::EntryAt;
use crate::block::color;
use crate::block::cube::{object, Ground, Object};
use crate::block::id::{Kind, SHAPE_BITS};
use crate::block::marker::{MARKER_SCALE_STEPS, MARKER_SHOWN_FROM};
use crate::block::DecorId;
use crate::space::camera::grid::CHUNK_STEPS;
use crate::space::coord::zoom::Zoom;
use crate::space::hierarchy::tree::ancestor_of;

/// Explicit air: retains the leaf and erases lower layers.
pub fn object_value_is_air(value: u32) -> bool {
    value == object::air as u32
}

/// The class an id is of; none past a u16 or for a class the layout does not give out.
pub fn kind_of(value: u32) -> Option<Kind> {
    u16::try_from(value).ok().and_then(|v| Kind::of((v >> SHAPE_BITS) as u8))
}

pub fn object_value_is_decor(value: u32) -> bool {
    kind_of(value) == Some(Kind::decor)
}

/// The one marker a placement puts up: a label, which its shape does not tell
/// apart from another's.
pub const MARKER: u32 = (Kind::marker as u32) << SHAPE_BITS;

/// A mark's cell: 16 pixels of the web map's 256-pixel tile, four zooms
/// finer than the tile, the cell Carto keeps its 14-pixel icons apart in.
pub const MARK_STEPS: Zoom = 4;

/// A marker's cell where the camera grid's cell is at `cell`: its scale on
/// the grid (`asset/marker.json`).
pub const fn marker_cell(cell: Zoom) -> i32 {
    cell as i32 + MARKER_SCALE_STEPS as i32
}

/// The marker's cell holding a marker at `at` where the camera grid's cell
/// is at `cell`, which counts the markers in it as one; the marker's own
/// where it is finer.
pub fn marker_cell_of(cell: Zoom, at: EntryAt) -> (Zoom, [i32; 3]) {
    let z = (marker_cell(cell).max(0) as Zoom).min(at.zoom());
    (z, ancestor_of(at.at(), at.zoom() - z))
}

/// Whether a marker shows where the camera grid's cell is at `cell`: its
/// chunk no coarser than `MARKER_SHOWN_FROM`.
pub const fn marker_shows(cell: Zoom) -> bool {
    cell as i32 - CHUNK_STEPS as i32 >= MARKER_SHOWN_FROM as i32
}

pub fn object_value_is_marker(value: u32) -> bool {
    kind_of(value) == Some(Kind::marker)
}

pub fn decor_num_id(id: DecorId) -> u32 {
    id as u32
}

pub fn decor_id_from_num(num_id: u32) -> DecorId {
    match u16::try_from(num_id).ok().and_then(DecorId::of) {
        Some(id) => id,
        None => xpute_kit::bug!(ENOTRECOVERABLE, num_id),
    }
}

/// In numeric order.
pub static decor_ids: LazyLock<Vec<DecorId>> = LazyLock::new(|| {
    let mut ids = DecorId::ALL.to_vec();
    ids.sort_by_key(|id| *id as u16);
    ids
});

/// A cube of the catalog (`asset/cube.json`): a look of the ground's palette,
/// or a color only an object takes; a player may place either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cube {
    Ground(Ground),
    Object(Object),
}

/// The value a placement stores.
pub fn cube_id(cube: Cube) -> u32 {
    match cube {
        Cube::Ground(g) => g as u32,
        Cube::Object(b) => b as u32,
    }
}

/// None for VOID, UNKNOWN, AIR or a decor kind.
pub fn cube_of(num_id: u32) -> Option<Cube> {
    match u8::try_from(num_id) {
        Ok(byte) => Ground::of(byte).map(Cube::Ground),
        Err(_) => u16::try_from(num_id).ok().and_then(Object::of).map(Cube::Object),
    }
}

pub fn cube_rgb(cube: Cube) -> Option<u32> {
    match cube {
        Cube::Ground(g) => color::ground(g as u8),
        Cube::Object(b) => color::object(b as u16),
    }
    .map(|s| s.rgb)
}

/// The placed colors first, then the palette in paint order.
pub static ALL_CUBES: LazyLock<Vec<Cube>> = LazyLock::new(|| Object::ALL.into_iter().map(Cube::Object).chain(Ground::ALL.into_iter().map(Cube::Ground)).collect());

#[cfg(test)]
#[path = "object.test.rs"]
mod test;
