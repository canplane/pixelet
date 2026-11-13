// pxl-spec/object.rs

//! What the object layer stores in a cell: the asset placed there, by its id, which is the
//! kind over the index (`asset/id.json`). In the cube kind the byte range is the ground's
//! palette, so a cube in a palette entry stores the palette entry's number, and past a byte
//! lies what only an object is: air and the placed colors (`asset/cube.json`). The decor kind
//! is the decors (`asset/decor.json`), and the marker kind the pin. `ext` is what a placed
//! asset carries beyond its id.

pub mod ext;

use crate::asset::cube::Sentinel;
use crate::asset::id::AssetId;

/// Whether `val` is explicit air, which keeps its leaf and erases the layers under it.
pub fn is_air(val: AssetId) -> bool {
    val == Sentinel::air as u32
}
