// pxl-spec/block/object.rs

//! A block as the object layer stores it: the id of the asset standing in
//! the cell, its kind over its shape (`asset/id.json`). In the cube kind the
//! byte range is the ground's palette, so a cube in a look stores the look's
//! number, and past it is what only an object is: air and the placed colors
//! (`asset/cube.json`); the decor kind is the decors (`asset/decor.json`),
//! and the marker kind the pin.

use crate::asset::cube::object;
use crate::asset::id::{asset_id, kind_of, AssetId, Kind};

/// Explicit air: retains the leaf and erases lower layers.
pub fn object_value_is_air(value: AssetId) -> bool {
    value == object::air as u32
}

pub fn object_value_is_decor(value: AssetId) -> bool {
    kind_of(value) == Some(Kind::decor)
}

/// The one marker a placement puts up: the pin, which its shape does not
/// tell apart from another's.
pub const MARKER: AssetId = asset_id(Kind::marker, 0);

pub fn object_value_is_marker(value: AssetId) -> bool {
    kind_of(value) == Some(Kind::marker)
}
