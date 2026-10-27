// pxl-spec/object.rs

//! What the object layer stores in a cell: the asset placed there, by its
//! id, the kind over the shape (`asset/id.json`). In the cube kind the byte
//! range is the ground's palette, so a cube in a look stores the look's
//! number, and past it is what only an object is: air and the placed colors
//! (`asset/cube.json`); the decor kind is the decors (`asset/decor.json`),
//! and the marker kind the pin. `ext` is what a placed asset carries beyond
//! its id, `volume` the objects' tree folded as it is written.

pub mod ext;
pub mod volume;

use crate::asset::cube::object;
use crate::asset::id::{kind_of, AssetId, Kind};

/// Explicit air: retains the leaf and erases lower layers.
pub fn is_air(value: AssetId) -> bool {
    value == object::air as u32
}

pub fn is_decor(value: AssetId) -> bool {
    kind_of(value) == Some(Kind::decor)
}

pub fn is_marker(value: AssetId) -> bool {
    kind_of(value) == Some(Kind::marker)
}
